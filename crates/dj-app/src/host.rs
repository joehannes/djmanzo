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
use dj_audio::{
    AudioBackend, AudioStream, BridgeStats, CpalBackend, NullBackend, SplitPrimary, SplitSecondary,
    StreamConfig, cue_bridge,
};
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
        cue_device: Option<DeviceId>,
        buffer_frames: u32,
        reply: SyncSender<Result<OpenOutcome, HostError>>,
    },
    Play(SyncSender<Result<(), HostError>>),
    Pause(SyncSender<Result<(), HostError>>),
    Shutdown,
}

/// What opening produced.
///
/// Two configs rather than one, because in split mode the headphone device is a
/// separate card with its own rate and buffer size — and the whole reason the
/// bridge exists is that those numbers are *not* the master's.
#[derive(Debug, Clone)]
pub struct OpenOutcome {
    pub master: ActiveConfig,
    /// Present only when a second device carries the cue.
    pub cue: Option<ActiveConfig>,
    /// Why the requested headphone device was not used, if one was requested
    /// and would not open. Not an error: the master still runs.
    pub cue_error: Option<String>,
    /// Live figures from the drift-correcting bridge, when there is one.
    pub bridge: Option<Arc<BridgeStats>>,
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

    /// Open the output.
    ///
    /// `cue_device` selects a *second* device for the headphone cue. Passing it
    /// is what puts the application into split mode, where the two cards run on
    /// independent clocks and a drift-corrected bridge sits between them; see
    /// `dj_audio::bridge`. Passing `None`, or the same device as the master,
    /// keeps everything on one clock, which is always better when the hardware
    /// allows it.
    pub fn open(
        &self,
        device: Option<DeviceId>,
        cue_device: Option<DeviceId>,
        buffer_frames: u32,
    ) -> Result<OpenOutcome, HostError> {
        self.request(|reply| HostCommand::Open {
            device,
            cue_device,
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
    // Held for its lifetime, not for its interface: dropping it closes the
    // headphone device. Kept beside the master so both are torn down together.
    let mut cue_stream: Option<Box<dyn AudioStream>> = None;
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
                cue_device,
                buffer_frames,
                reply,
            }) => {
                let result = open_device(
                    backend.as_ref(),
                    &bus,
                    &registry,
                    device,
                    cue_device,
                    buffer_frames,
                    &mut stream,
                    &mut cue_stream,
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
#[allow(clippy::too_many_arguments)]
fn open_device(
    backend: &dyn AudioBackend,
    bus: &Arc<ActionBus<Command>>,
    registry: &Arc<ParameterRegistry>,
    device: Option<DeviceId>,
    cue_device: Option<DeviceId>,
    buffer_frames: u32,
    stream_slot: &mut Option<Box<dyn AudioStream>>,
    cue_slot: &mut Option<Box<dyn AudioStream>>,
    retired_slot: &mut Option<rtrb::Consumer<Retired>>,
) -> Result<OpenOutcome, HostError> {
    // Close the old streams first: two devices open at once means two callbacks
    // racing on the same registry, and on some backends it simply fails. The
    // cue goes first, so it cannot be left reading a bridge whose other end has
    // already gone.
    if let Some(previous) = cue_slot.take() {
        let _ = previous.pause();
        drop(previous);
    }
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

    // A second device is only a second device if it is actually a different
    // one. Asking for a split onto the card already carrying the master would
    // build a bridge between a clock and itself -- more latency, more
    // resampling, nothing gained.
    //
    // Compared against the *resolved* device rather than the request, because
    // the common way to hit this is to leave the master on "default" and then
    // pick that same card by name for the cue. Those are two different
    // requests and one device.
    let master_id = chosen.as_ref().map(|d| d.id.clone());
    let split_to = cue_device.filter(|wanted| Some(wanted) != master_id.as_ref());

    // A fresh graph gets fresh queues; the bus is re-aimed at the new one.
    let (command_tx, command_rx) = rtrb::RingBuffer::new(4096);
    let (retired_tx, retired_rx) = rtrb::RingBuffer::new(256);

    // Held in an Option because the engine goes into one of two different
    // callbacks depending on whether the second device opens.
    let mut engine = Some(Engine::new(
        DECK_COUNT,
        sample_rate,
        command_rx,
        retired_tx,
        Arc::clone(registry),
    ));

    let mut cue_active: Option<ActiveConfig> = None;
    let mut cue_error: Option<String> = None;
    let mut bridge: Option<Arc<BridgeStats>> = None;
    let mut primary: Option<Box<dyn dj_audio::AudioCallback>> = None;

    // The headphone device is opened *first*, deliberately.
    //
    // Opening the master first and the cue second leaves a failure with a
    // master stream already running and pushing into a bridge nobody drains --
    // audible on the PA, but reporting a permanent fault and impossible to
    // back out of cleanly. Doing it this way, a headphone device that will not
    // open costs nothing: the master is then built as an ordinary single-device
    // graph, and on a four-channel card it gets its on-device cue back.
    if let Some(cue_id) = split_to {
        let (producer, consumer, stats) = cue_bridge(sample_rate, buffer_frames);
        let cue_config = StreamConfig {
            device: Some(cue_id),
            sample_rate,
            buffer_frames,
            channels: 2,
        };

        match backend.open_output(&cue_config, Box::new(SplitSecondary::new(consumer))) {
            Ok(stream) => {
                cue_active = Some(stream.config().clone());
                // Started now; it will output silence until the bridge primes.
                stream.play().map_err(|e| HostError::Audio(e.to_string()))?;
                *cue_slot = Some(stream);
                bridge = Some(stats);
                primary = Some(Box::new(SplitPrimary::new(
                    Box::new(engine.take().expect("engine taken twice")),
                    producer,
                )));
            }
            Err(error) => {
                // Reported rather than fatal. Losing the PA because the
                // headphones would not open is the wrong trade.
                cue_error = Some(error.to_string());
            }
        }
    }

    // Open four channels when the device has them, so master and headphone cue
    // can share one interface -- the layout every controller with a built-in
    // card provides. Opening only two would make cueing impossible on hardware
    // that supports it perfectly well. In split mode the cue leaves by another
    // card entirely, so this device only needs its stereo pair.
    let channels = match chosen.as_ref().map(|d| d.max_output_channels) {
        _ if cue_active.is_some() => 2,
        Some(available) if available >= 4 => 4,
        _ => 2,
    };

    let config = StreamConfig {
        device,
        sample_rate,
        buffer_frames,
        channels,
    };

    let callback = primary
        .unwrap_or_else(|| Box::new(engine.take().expect("engine consumed without a split")));

    let stream = match backend.open_output(&config, callback) {
        Ok(stream) => stream,
        Err(error) => {
            // The master is the one that must not be left half-built: close the
            // headphone device rather than leaving it playing into nothing.
            if let Some(orphan) = cue_slot.take() {
                let _ = orphan.pause();
            }
            return Err(HostError::Audio(error.to_string()));
        }
    };

    bus.reconnect(command_tx);
    let master = stream.config().clone();
    stream.play().map_err(|e| HostError::Audio(e.to_string()))?;

    if let Some(reason) = &cue_error {
        tracing::warn!(%reason, "headphone device would not open; cueing stays on the main device");
    }

    *stream_slot = Some(stream);
    *retired_slot = Some(retired_rx);
    Ok(OpenOutcome {
        master,
        cue: cue_active,
        cue_error,
        bridge,
    })
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
    fn lists_the_null_devices() {
        let (host, _bus, _reg) = host();
        let devices = host.list_devices().unwrap();
        // A master and a stand-in headphone device, so the split path has
        // somewhere to split to.
        assert_eq!(devices.len(), 2);
        assert!(devices[0].is_default);
    }

    /// **The two-device path.** Master on one card, headphones on another, with
    /// the drift-correcting bridge between them.
    #[test]
    fn a_second_device_carries_the_cue_and_gets_a_bridge() {
        let (host, _bus, _reg) = host();
        let outcome = host
            .open(
                Some(DeviceId::new("null")),
                Some(DeviceId::new("null-cue")),
                128,
            )
            .unwrap();

        let cue = outcome.cue.expect("no headphone device was opened");
        assert!(cue.device_name.contains("headphone"));
        assert!(outcome.cue_error.is_none());
        assert!(
            outcome.bridge.is_some(),
            "two clocks with no bridge between them is the bug this feature exists to fix"
        );

        // The master only needs its own pair now: the cue leaves by the other
        // card, so asking this one for four channels would reserve two that
        // nothing writes to.
        assert_eq!(outcome.master.channels, 2);
    }

    /// The bridge has to actually run, not merely exist. A few callbacks in,
    /// audio should have moved across it.
    #[test]
    fn the_bridge_carries_audio_between_the_two_devices() {
        let (host, _bus, _reg) = host();
        let outcome = host
            .open(
                Some(DeviceId::new("null")),
                Some(DeviceId::new("null-cue")),
                128,
            )
            .unwrap();
        let bridge = outcome.bridge.expect("no bridge");

        std::thread::sleep(Duration::from_millis(250));

        assert!(
            bridge.queued_frames() > 0,
            "nothing ever crossed the bridge"
        );
        assert_eq!(
            bridge.target_frames(),
            128 * 3,
            "the queue should be sized from the buffer"
        );
        assert!(
            bridge.is_healthy(),
            "the bridge lost audio: {} starved, {} dropped",
            bridge.starved_frames(),
            bridge.dropped_samples()
        );
    }

    /// Naming the master as the headphone device is not a split. Building a
    /// bridge from a clock to itself would add latency and resampling for
    /// nothing, and would take away the four-channel cue that card already has.
    #[test]
    fn splitting_onto_the_same_device_is_not_a_split() {
        let (host, _bus, _reg) = host();
        let outcome = host
            .open(
                Some(DeviceId::new("null")),
                Some(DeviceId::new("null")),
                128,
            )
            .unwrap();
        assert!(outcome.cue.is_none());
        assert!(outcome.bridge.is_none());
        assert_eq!(outcome.master.channels, 4, "the on-device cue was lost");
    }

    /// The same thing by another route: master left on "default" and the cue
    /// pointed at that same card by name. Two different requests, one device.
    #[test]
    fn the_default_device_is_recognised_by_name_too() {
        let (host, _bus, _reg) = host();
        let outcome = host.open(None, Some(DeviceId::new("null")), 128).unwrap();
        assert!(
            outcome.bridge.is_none(),
            "the default device was not recognised as the one already in use"
        );
        assert_eq!(outcome.master.channels, 4);
    }

    /// Switching back to a single device must close the headphone stream.
    /// Leaving it open would be a device the user cannot release and a bridge
    /// filling with audio nobody hears.
    #[test]
    fn reopening_without_a_cue_device_tears_the_bridge_down() {
        let (host, _bus, _reg) = host();
        let split = host
            .open(
                Some(DeviceId::new("null")),
                Some(DeviceId::new("null-cue")),
                128,
            )
            .unwrap();
        assert!(split.bridge.is_some());

        let plain = host.open(Some(DeviceId::new("null")), None, 128).unwrap();
        assert!(plain.cue.is_none());
        assert!(plain.bridge.is_none());
        assert_eq!(
            plain.master.channels, 4,
            "the on-device cue did not come back"
        );
    }

    #[test]
    fn opening_a_device_reports_the_active_config() {
        let (host, _bus, _reg) = host();
        let active = host.open(None, None, 128).unwrap().master;
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
        let active = host.open(None, None, 128).unwrap().master;
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
        host.open(None, None, 128).unwrap();
        assert!(host.play().is_ok());
        assert!(host.pause().is_ok());
    }

    /// Reopening must re-aim the bus, or actions would vanish into the old
    /// queue and the application would go silently unresponsive.
    #[test]
    fn reopening_keeps_the_bus_connected() {
        let (host, bus, _reg) = host();
        host.open(None, None, 128).unwrap();
        host.open(None, None, 256).unwrap();

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
        host.open(None, None, 128).unwrap();
        drop(host); // Must join without hanging.
    }
}
