//! Audio device abstraction.
//!
//! Everything the application does with hardware goes through [`AudioBackend`].
//! Two implementations ship: [`CpalBackend`] for real devices and
//! [`NullBackend`] for headless runs. [`OfflineRenderer`] renders a callback
//! synchronously, which is how the realtime engine is tested where no sound card
//! exists.
//!
//! The trait exists rather than using `cpal` directly because a DJ application
//! needs behaviour `cpal` does not model: four-channel split output, two devices
//! on independent clocks with drift correction, macOS aggregate devices. Those
//! land behind this same interface in M1 without disturbing the engine.

pub mod cpal_backend;
pub mod device;
pub mod null;

pub use cpal_backend::CpalBackend;
pub use device::{ActiveConfig, DeviceId, DeviceInfo, StreamConfig};
pub use null::{NullBackend, OfflineRenderer};

use dj_core::SampleRate;

/// What the backend tells the callback about the block it wants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderContext {
    /// Frames requested. Derived from the actual buffer handed over, not from
    /// the configuration -- devices do not always honour a requested size.
    pub frames: usize,
    pub channels: usize,
    pub sample_rate: SampleRate,
}

impl RenderContext {
    #[must_use]
    pub fn samples(&self) -> usize {
        self.frames * self.channels
    }
}

/// Produces audio.
///
/// `render` runs on the realtime thread. It must not allocate, lock, perform
/// I/O, log, or panic. The buffer arrives zeroed, interleaved, and exactly
/// [`RenderContext::samples`] long.
pub trait AudioCallback: Send + std::fmt::Debug {
    fn render(&mut self, out: &mut [f32], ctx: &RenderContext);
}

/// A running output stream. Dropping it closes the device.
pub trait AudioStream: std::fmt::Debug {
    fn play(&self) -> Result<(), AudioError>;
    fn pause(&self) -> Result<(), AudioError>;
    /// What the device actually gave us, which may differ from what was asked.
    fn config(&self) -> &ActiveConfig;
}

/// A source of audio devices.
pub trait AudioBackend: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;

    fn output_devices(&self) -> Result<Vec<DeviceInfo>, AudioError>;

    /// Open an output stream. The returned stream starts paused; call
    /// [`AudioStream::play`] to begin.
    fn open_output(
        &self,
        config: &StreamConfig,
        callback: Box<dyn AudioCallback>,
    ) -> Result<Box<dyn AudioStream>, AudioError>;

    /// The system default output, if there is one.
    fn default_output(&self) -> Result<Option<DeviceInfo>, AudioError> {
        Ok(self.output_devices()?.into_iter().find(|d| d.is_default))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AudioError {
    #[error("cannot list audio devices: {0}")]
    Enumerate(String),
    #[error("no default output device")]
    NoDefaultDevice,
    #[error("cannot open output stream: {0}")]
    OpenStream(String),
    #[error("stream control failed: {0}")]
    Control(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_context_sample_count() {
        let ctx = RenderContext {
            frames: 256,
            channels: 2,
            sample_rate: SampleRate::DEFAULT,
        };
        assert_eq!(ctx.samples(), 512);
    }

    #[test]
    fn default_output_finds_the_flagged_device() {
        let backend = NullBackend::new();
        let default = backend.default_output().unwrap();
        assert!(default.is_some());
        assert!(default.unwrap().is_default);
    }
}
