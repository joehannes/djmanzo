//! The audio host thread.
//!
//! # Why a thread
//!
//! `cpal::Stream` is not `Send` on every platform -- the handle is tied to the
//! thread that created it. Tauri's managed state must be `Send + Sync`. So the
//! stream lives on a thread of its own that owns it outright, and the rest of
//! the application talks to that thread over a channel.
//!
//! This thread is *not* the audio callback. It opens and closes devices, and it
//! drains the retirement queue -- freeing the track buffers the engine handed
//! back, which is precisely the blocking work the callback must never do.

use dj_audio::{ActiveConfig, DeviceId, DeviceInfo};
use dj_audio::{AudioBackend, AudioStream, CpalBackend, NullBackend, StreamConfig};
use dj_control::{ActionBus, ParameterRegistry};
use dj_core::SampleRate;
use dj_engine::{Command, Engine, Retired};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::time::Duration;

/// Decks the engine is built with. Four covers the common layouts; six is the
/// ceiling and arrives with the UI to drive it in M5.
const DECK_COUNT: usize = 4;

/// Commands sent from the application to the audio host thread.
enum HostCommand {
    ListDevices(SyncSender<Result<Vec<DeviceInfo>, HostError>>),
    Open {
        device: Option<DeviceId>,
        buffer_frames: u32,
        reply: SyncSender<Result<ActiveConfig, HostError>>,
    },
    Play(SyncSender<Result<(), HostError>>),
    Pause(SyncSender<Result<(), HostError>>),
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HostError {
    #[error("audio: {0}")]
    Audio(String),
    #[error("no audio device is open")]
    NoDevice,
    #[error("audio host is not responding")]
    Unreachable,
}

/// Handle to the audio host thread.
#[derive(Debug)]
pub struct AudioHost {
    commands: Sender<HostCommand>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl AudioHost {
    /// Start the host thread.
    ///
    /// `use_null_backend` selects the headless backend -- what CI and any
    /// machine without a sound card needs.
    pub fn start(
        bus: Arc<ActionBus<Command>>,
        registry: Arc<ParameterRegistry>,
        use_null_backend: bool,
    ) -> Self {
        let (tx, rx) = channel();
        let thread = std::thread::Builder::new()
            .name("dj-audio-host".to_owned())
            .spawn(move || run_host(rx, bus, registry, use_null_backend))
            .expect("failed to spawn audio host thread");

        Self {
            commands: tx,
            thread: Some(thread),
        }
    }

    fn request<T>(
        &self,
        make: impl FnOnce(SyncSender<Result<T, HostError>>) -> HostCommand,
    ) -> Result<T, HostError> {
        let (tx, rx) = sync_channel(1);
        self.commands
            .send(make(tx))
            .map_err(|_| HostError::Unreachable)?;
        // The host may be mid-open on a slow device; give it room but never
        // block the UI thread indefinitely.
        rx.recv_timeout(Duration::from_secs(10))
            .map_err(|_| HostError::Unreachable)?
    }

    pub fn list_devices(&self) -> Result<Vec<DeviceInfo>, HostError> {
        self.request(HostCommand::ListDevices)
    }

    pub fn open(
        &self,
        device: Option<DeviceId>,
        buffer_frames: u32,
    ) -> Result<ActiveConfig, HostError> {
        self.request(|reply| HostCommand::Open {
            device,
            buffer_frames,
            reply,
        })
    }

    pub fn play(&self) -> Result<(), HostError> {
        self.request(HostCommand::Play)
    }

    pub fn pause(&self) -> Result<(), HostError> {
        self.request(HostCommand::Pause)
    }
}

impl Drop for AudioHost {
    fn drop(&mut self) {
        let _ = self.commands.send(HostCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_host(
    commands: Receiver<HostCommand>,
    bus: Arc<ActionBus<Command>>,
    registry: Arc<ParameterRegistry>,
    use_null_backend: bool,
) {
    let backend: Box<dyn AudioBackend> = if use_null_backend {
        Box::new(NullBackend::new())
    } else {
        Box::new(CpalBackend::new())
    };

    let mut stream: Option<Box<dyn AudioStream>> = None;
    let mut retired: Option<rtrb::Consumer<Retired>> = None;

    loop {
        // Wake regularly even with no commands, so retired buffers are freed
        // promptly rather than piling up until the next user action.
        match commands.recv_timeout(Duration::from_millis(50)) {
            Ok(HostCommand::Shutdown) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
            Ok(HostCommand::ListDevices(reply)) => {
                let result = backend
                    .output_devices()
                    .map_err(|e| HostError::Audio(e.to_string()));
                let _ = reply.send(result);
            }
            Ok(HostCommand::Open {
                device,
                buffer_frames,
                reply,
            }) => {
                let result = open_device(
                    backend.as_ref(),
                    &bus,
                    &registry,
                    device,
                    buffer_frames,
                    &mut stream,
                    &mut retired,
                );
                let _ = reply.send(result);
            }
            Ok(HostCommand::Play(reply)) => {
                let result = match &stream {
                    Some(s) => s.play().map_err(|e| HostError::Audio(e.to_string())),
                    None => Err(HostError::NoDevice),
                };
                let _ = reply.send(result);
            }
            Ok(HostCommand::Pause(reply)) => {
                let result = match &stream {
                    Some(s) => s.pause().map_err(|e| HostError::Audio(e.to_string())),
                    None => Err(HostError::NoDevice),
                };
                let _ = reply.send(result);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }

        // Free what the engine handed back. This is the blocking deallocation
        // the audio callback deliberately refused to do.
        if let Some(queue) = retired.as_mut() {
            while let Ok(item) = queue.pop() {
                drop(item);
            }
        }
    }
}

/// Tear down the current realtime graph and build a new one on `device`.
fn open_device(
    backend: &dyn AudioBackend,
    bus: &Arc<ActionBus<Command>>,
    registry: &Arc<ParameterRegistry>,
    device: Option<DeviceId>,
    buffer_frames: u32,
    stream_slot: &mut Option<Box<dyn AudioStream>>,
    retired_slot: &mut Option<rtrb::Consumer<Retired>>,
) -> Result<ActiveConfig, HostError> {
    // Close the old stream first: two devices open at once means two callbacks
    // racing on the same registry, and on some backends it simply fails.
    if let Some(previous) = stream_slot.take() {
        let _ = previous.pause();
        drop(previous);
    }
    // Drain whatever the outgoing engine left behind before dropping its queue.
    if let Some(mut queue) = retired_slot.take() {
        while let Ok(item) = queue.pop() {
            drop(item);
        }
    }

    let chosen = backend.output_devices().ok().and_then(|devices| {
        devices
            .into_iter()
            .find(|d| device.as_ref().is_none_or(|wanted| &d.id == wanted))
    });
    let sample_rate = chosen
        .as_ref()
        .map(|d| d.default_sample_rate)
        .unwrap_or(SampleRate::DEFAULT);

    // Open four channels when the device has them, so master and headphone cue
    // can share one interface -- the layout every controller with a built-in
    // card provides. Opening only two would make cueing impossible on hardware
    // that supports it perfectly well.
    let channels = match chosen.as_ref().map(|d| d.max_output_channels) {
        Some(available) if available >= 4 => 4,
        _ => 2,
    };

    let config = StreamConfig {
        device,
        sample_rate,
        buffer_frames,
        channels,
    };

    // A fresh graph gets fresh queues; the bus is re-aimed at the new one.
    let (command_tx, command_rx) = rtrb::RingBuffer::new(4096);
    let (retired_tx, retired_rx) = rtrb::RingBuffer::new(256);

    let engine = Engine::new(
        DECK_COUNT,
        sample_rate,
        command_rx,
        retired_tx,
        Arc::clone(registry),
    );

    let stream = backend
        .open_output(&config, Box::new(engine))
        .map_err(|e| HostError::Audio(e.to_string()))?;

    bus.reconnect(command_tx);
    let active = stream.config().clone();
    stream.play().map_err(|e| HostError::Audio(e.to_string()))?;

    *stream_slot = Some(stream);
    *retired_slot = Some(retired_rx);
    Ok(active)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::{Action, DeckAction, DeckId};

    fn host() -> (AudioHost, Arc<ActionBus<Command>>, Arc<ParameterRegistry>) {
        let (bus, _unused) = ActionBus::<Command>::new(16);
        let bus = Arc::new(bus);
        let registry = Arc::new(ParameterRegistry::new());
        let host = AudioHost::start(Arc::clone(&bus), Arc::clone(&registry), true);
        (host, bus, registry)
    }

    #[test]
    fn lists_the_null_device() {
        let (host, _bus, _reg) = host();
        let devices = host.list_devices().unwrap();
        assert_eq!(devices.len(), 1);
        assert!(devices[0].is_default);
    }

    #[test]
    fn opening_a_device_reports_the_active_config() {
        let (host, _bus, _reg) = host();
        let active = host.open(None, 128).unwrap();
        assert_eq!(active.buffer_frames, 128);
        // The null backend advertises four channels, so the host should take
        // them: that is what makes the headphone cue reachable.
        assert_eq!(active.channels, 4);
    }

    /// A four-channel open is what puts master and cue on one interface. If the
    /// host quietly opened two, cueing would be impossible on hardware that
    /// supports it, and nothing would say why.
    #[test]
    fn a_four_channel_device_gets_a_cue_bus() {
        let (host, _bus, reg) = host();
        let active = host.open(None, 128).unwrap();
        assert!(active.channels >= 4);

        // Let a few callbacks run so the engine publishes availability.
        std::thread::sleep(Duration::from_millis(80));
        assert!(
            reg.get_bool(dj_core::ParamId::Global(
                dj_core::param::GlobalParam::CueAvailable
            )),
            "a four-channel device should report the cue as available"
        );
    }

    #[test]
    fn transport_control_requires_an_open_device() {
        let (host, _bus, _reg) = host();
        assert_eq!(host.play(), Err(HostError::NoDevice));
        host.open(None, 128).unwrap();
        assert!(host.play().is_ok());
        assert!(host.pause().is_ok());
    }

    /// Reopening must re-aim the bus, or actions would vanish into the old
    /// queue and the application would go silently unresponsive.
    #[test]
    fn reopening_keeps_the_bus_connected() {
        let (host, bus, _reg) = host();
        host.open(None, 128).unwrap();
        host.open(None, 256).unwrap();

        let action = Action::Deck {
            deck: DeckId::from_human(1).unwrap(),
            action: DeckAction::Play,
        };
        assert!(
            bus.dispatch(action).is_ok(),
            "bus should be connected to the new engine"
        );
    }

    #[test]
    fn shutting_down_is_clean() {
        let (host, _bus, _reg) = host();
        host.open(None, 128).unwrap();
        drop(host); // Must join without hanging.
    }
}
