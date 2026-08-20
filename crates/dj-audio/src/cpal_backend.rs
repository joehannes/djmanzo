//! The real-hardware backend, over `cpal`.
//!
//! `cpal` gives us CoreAudio on macOS and PipeWire / PulseAudio / ALSA / JACK on
//! Linux, and raises the callback thread to realtime priority. It is wrapped
//! rather than used directly because a DJ application needs things `cpal` does
//! not model -- aggregate devices, two devices on independent clocks -- and that
//! work lands behind this same trait in M1.

use crate::device::{ActiveConfig, DeviceId, DeviceInfo, StreamConfig};
use crate::{AudioBackend, AudioCallback, AudioError, AudioStream, RenderContext};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dj_core::SampleRate;

pub struct CpalBackend {
    host: cpal::Host,
}

// cpal::Host holds platform handles that do not implement Debug.
impl std::fmt::Debug for CpalBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpalBackend").finish_non_exhaustive()
    }
}

impl CpalBackend {
    #[must_use]
    pub fn new() -> Self {
        Self {
            host: cpal::default_host(),
        }
    }

    fn find_device(&self, id: Option<&DeviceId>) -> Result<cpal::Device, AudioError> {
        match id {
            None => self
                .host
                .default_output_device()
                .ok_or(AudioError::NoDefaultDevice),
            Some(wanted) => {
                let mut devices = self
                    .host
                    .output_devices()
                    .map_err(|e| AudioError::Enumerate(e.to_string()))?;
                devices
                    .find(|d| d.name().is_ok_and(|n| n == wanted.as_str()))
                    // A device named in saved settings may simply be unplugged.
                    // Falling back to the default beats refusing to start.
                    .or_else(|| self.host.default_output_device())
                    .ok_or(AudioError::NoDefaultDevice)
            }
        }
    }

    /// The same lookup for the capture side.
    ///
    /// Deliberately *not* folded into `find_device` with a direction flag. The
    /// fallbacks differ in kind: an unplugged output falls back to the default
    /// because a DJ application with no output is useless, while an unplugged
    /// input must fail — silently opening the built-in microphone instead of
    /// the DJ's chosen one puts laptop fan noise into a PA.
    fn find_input(&self, id: Option<&DeviceId>) -> Result<cpal::Device, AudioError> {
        match id {
            None => self
                .host
                .default_input_device()
                .ok_or(AudioError::NoInputDevice),
            Some(wanted) => self
                .host
                .input_devices()
                .map_err(|e| AudioError::Enumerate(e.to_string()))?
                .find(|d| d.name().is_ok_and(|n| n == wanted.as_str()))
                .ok_or(AudioError::NoInputDevice),
        }
    }
}

impl Default for CpalBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for CpalBackend {
    fn name(&self) -> &'static str {
        "cpal"
    }

    fn output_devices(&self) -> Result<Vec<DeviceInfo>, AudioError> {
        let default_name = self
            .host
            .default_output_device()
            .and_then(|d| d.name().ok());

        let devices = self
            .host
            .output_devices()
            .map_err(|e| AudioError::Enumerate(e.to_string()))?;

        let mut out = Vec::new();
        for device in devices {
            // A device that refuses to describe itself is not one we can open;
            // skip it rather than failing the whole enumeration.
            let Ok(name) = device.name() else { continue };
            let Ok(config) = device.default_output_config() else {
                continue;
            };
            let Some(sample_rate) = SampleRate::new(config.sample_rate().0) else {
                continue;
            };

            let max_channels = device
                .supported_output_configs()
                .map(|configs| configs.map(|c| c.channels()).max().unwrap_or(2))
                .unwrap_or_else(|_| config.channels());

            out.push(DeviceInfo {
                id: DeviceId::new(name.clone()),
                is_default: default_name.as_deref() == Some(name.as_str()),
                name,
                max_output_channels: max_channels,
                default_sample_rate: sample_rate,
            });
        }
        Ok(out)
    }

    fn input_devices(&self) -> Result<Vec<DeviceInfo>, AudioError> {
        let default_name = self.host.default_input_device().and_then(|d| d.name().ok());
        let devices = self
            .host
            .input_devices()
            .map_err(|e| AudioError::Enumerate(e.to_string()))?;

        let mut out = Vec::new();
        for device in devices {
            let Ok(name) = device.name() else { continue };
            let Ok(config) = device.default_input_config() else {
                continue;
            };
            let Some(sample_rate) = SampleRate::new(config.sample_rate().0) else {
                continue;
            };
            out.push(DeviceInfo {
                id: DeviceId::new(name.clone()),
                is_default: default_name.as_deref() == Some(name.as_str()),
                name,
                // An input device's channel count, in the field the type has
                // for a channel count. The name says "output" because outputs
                // came first; splitting it in two would mean two structs that
                // differ in one word.
                max_output_channels: config.channels(),
                default_sample_rate: sample_rate,
            });
        }
        Ok(out)
    }

    fn open_input(
        &self,
        config: &StreamConfig,
        mut sink: rtrb::Producer<f32>,
    ) -> Result<Box<dyn AudioStream>, AudioError> {
        let device = self.find_input(config.device.as_ref())?;
        let device_name = device.name().unwrap_or_else(|_| "unknown".to_owned());

        // Ask the device for what it natively does rather than insisting on
        // stereo. A great many microphones are mono, and a stereo request to a
        // mono device fails outright on some backends — so the doubling happens
        // here, in software, where it always works.
        let supported = device
            .default_input_config()
            .map_err(|e| AudioError::OpenStream(e.to_string()))?;
        let device_channels = supported.channels().max(1);

        let stream_config = cpal::StreamConfig {
            channels: device_channels,
            sample_rate: cpal::SampleRate(config.sample_rate.get()),
            buffer_size: cpal::BufferSize::Fixed(config.buffer_frames),
        };

        let stream = device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                    let stride = device_channels as usize;
                    for frame in data.chunks_exact(stride) {
                        // Mono is doubled, stereo passes through, and anything
                        // wider has its first two channels taken. The engine
                        // sees interleaved stereo either way.
                        let left = frame[0];
                        let right = if stride > 1 { frame[1] } else { left };
                        // A full ring means the engine is not draining, which
                        // it always is while it renders. Dropping is the only
                        // option that keeps this callback bounded, and the
                        // engine's starvation counter is the visible half of
                        // the same fault.
                        if sink.push(left).is_err() || sink.push(right).is_err() {
                            break;
                        }
                    }
                },
                move |err| {
                    tracing::error!(error = %err, "audio input stream error");
                },
                None,
            )
            .map_err(|e| AudioError::OpenStream(e.to_string()))?;

        Ok(Box::new(CpalStream {
            active: ActiveConfig {
                device_name,
                sample_rate: config.sample_rate,
                buffer_frames: config.buffer_frames,
                channels: device_channels,
            },
            stream,
        }))
    }

    fn open_output(
        &self,
        config: &StreamConfig,
        mut callback: Box<dyn AudioCallback>,
    ) -> Result<Box<dyn AudioStream>, AudioError> {
        let device = self.find_device(config.device.as_ref())?;
        let device_name = device.name().unwrap_or_else(|_| "unknown".to_owned());

        let stream_config = cpal::StreamConfig {
            channels: config.channels,
            sample_rate: cpal::SampleRate(config.sample_rate.get()),
            buffer_size: cpal::BufferSize::Fixed(config.buffer_frames),
        };

        let channels = config.channels as usize;
        let sample_rate = config.sample_rate;

        let stream = device
            .build_output_stream(
                &stream_config,
                move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    // cpal hands over whatever the device asked for, which is not
                    // always the size we requested; derive frames from the slice
                    // rather than trusting the configuration.
                    let context = RenderContext {
                        frames: data.len() / channels.max(1),
                        channels,
                        sample_rate,
                    };
                    data.fill(0.0);
                    callback.render(data, &context);
                },
                move |err| {
                    // The audio thread cannot report upward safely, so log and
                    // let the xrun counter in the registry tell the user.
                    tracing::error!(error = %err, "audio stream error");
                },
                None,
            )
            .map_err(|e| AudioError::OpenStream(e.to_string()))?;

        Ok(Box::new(CpalStream {
            active: ActiveConfig {
                device_name,
                sample_rate: config.sample_rate,
                buffer_frames: config.buffer_frames,
                channels: config.channels,
            },
            stream,
        }))
    }
}

struct CpalStream {
    active: ActiveConfig,
    stream: cpal::Stream,
}

// cpal::Stream is deliberately !Send on some platforms because the underlying
// handle is tied to its creating thread. We only ever call play/pause from the
// thread that owns the app state, and the stream is dropped there too.
impl std::fmt::Debug for CpalStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpalStream")
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl AudioStream for CpalStream {
    fn play(&self) -> Result<(), AudioError> {
        self.stream
            .play()
            .map_err(|e| AudioError::Control(e.to_string()))
    }

    fn pause(&self) -> Result<(), AudioError> {
        self.stream
            .pause()
            .map_err(|e| AudioError::Control(e.to_string()))
    }

    fn config(&self) -> &ActiveConfig {
        &self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Enumeration must not panic or hang even with no sound card present --
    /// which is exactly the situation in CI and in this container.
    #[test]
    fn enumeration_is_safe_without_hardware() {
        let backend = CpalBackend::new();
        match backend.output_devices() {
            Ok(devices) => {
                for device in devices {
                    assert!(!device.name.is_empty());
                    assert!(device.max_output_channels > 0);
                }
            }
            // No audio subsystem at all is a legitimate outcome here.
            Err(AudioError::Enumerate(_)) => {}
            Err(e) => panic!("unexpected enumeration failure: {e}"),
        }
    }

    #[test]
    fn backend_reports_its_name() {
        assert_eq!(CpalBackend::new().name(), "cpal");
    }
}
