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
use dj_engine::{Capture, Command, Engine, Retired};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::time::Duration;

/// What the host does with a finished recording.
///
/// A callback rather than a handle on the interface's sample-name store: the
/// capture arrives here, but naming the new sample is the interface's business,
/// and this module has no reason to know where names are kept.
pub type OnCapture = Box<dyn Fn(u8, u8, String) + Send>;

/// What to do with a plugin processor the engine has finished with.
///
/// Its own callback rather than a drop, because dropping it is exactly wrong:
/// only the plugin instance that made it may deactivate it, and that instance
/// lives on the other side of this module. See `crate::plugins::Insert`.
pub type OnPlugin = Box<dyn Fn(Box<dj_clap::Processor>) + Send>;

/// Decks the engine is built with.
///
/// Re-exported from `state` rather than declared again: the two used to be
/// separate constants kept in step by a comment, which is a thing that stays
/// true right up until it does not.
use crate::state::DECK_COUNT;

/// Commands sent from the application to the audio host thread.
enum HostCommand {
    ListDevices(SyncSender<Result<Vec<DeviceInfo>, HostError>>),
    Open {
        device: Option<DeviceId>,
        cue_device: Option<DeviceId>,
        buffer_frames: u32,
        reply: SyncSender<Result<OpenOutcome, HostError>>,
    },
    ListInputs(SyncSender<Result<Vec<DeviceInfo>, HostError>>),
    OpenMic {
        device: Option<DeviceId>,
        reply: SyncSender<Result<ActiveConfig, HostError>>,
    },
    CloseMic(SyncSender<Result<(), HostError>>),
    Play(SyncSender<Result<(), HostError>>),
    Pause(SyncSender<Result<(), HostError>>),
    Shutdown,
}

/// How much microphone the ring between the two callbacks can hold.
///
/// Half a second. The ring's *steady* fill is set by the two buffer sizes, not
/// by its capacity — this is headroom for a scheduling hiccup, and making it
/// large would not add latency, only delay the point at which a genuinely
/// stalled input starts dropping instead of piling up.
const MIC_RING_SECONDS: f64 = 0.5;

/// The widest output djmanzo opens, however many sockets an interface has.
///
/// Eight, because that is the widest arrangement anything asks for: four
/// stereo stems, which is [`dj_engine::STEM_OUT_CHANNELS`]. Beyond it the
/// extra channels would be buffer allocated to carry silence.
const MAX_OUTPUT_CHANNELS: u16 = dj_engine::STEM_OUT_CHANNELS as u16;

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
        on_capture: OnCapture,
        on_plugin: OnPlugin,
    ) -> Self {
        let (tx, rx) = channel();
        let thread = std::thread::Builder::new()
            .name("dj-audio-host".to_owned())
            .spawn(move || run_host(rx, bus, registry, use_null_backend, on_capture, on_plugin))
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

    /// Devices that can capture. Empty is a normal answer — many laptops in a
    /// booth have nothing plugged in.
    pub fn list_inputs(&self) -> Result<Vec<DeviceInfo>, HostError> {
        self.request(HostCommand::ListInputs)
    }

    /// Attach an input device to the microphone strip.
    ///
    /// Independent of whether the *channel* is open: this is the cable, and
    /// `mic on` is the switch. Opening a sound card takes long enough to miss a
    /// cue, so a DJ plugs in once at the start of the night and toggles the
    /// channel all evening.
    pub fn open_mic(&self, device: Option<DeviceId>) -> Result<ActiveConfig, HostError> {
        self.request(|reply| HostCommand::OpenMic { device, reply })
    }

    pub fn close_mic(&self) -> Result<(), HostError> {
        self.request(HostCommand::CloseMic)
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
    on_capture: OnCapture,
    on_plugin: OnPlugin,
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
    // Held for its lifetime like the cue stream: dropping it closes the input
    // device and stops the callback that fills the engine's ring.
    let mut mic_stream: Option<Box<dyn AudioStream>> = None;

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
            Ok(HostCommand::ListInputs(reply)) => {
                let result = backend
                    .input_devices()
                    .map_err(|e| HostError::Audio(e.to_string()));
                let _ = reply.send(result);
            }
            Ok(HostCommand::OpenMic { device, reply }) => {
                // The output has to be running first: the engine only exists
                // once a device is open, and a microphone attached to nothing
                // is a ring that fills and never drains.
                let result = match stream.as_ref().map(|s| s.config().clone()) {
                    Some(master) => {
                        open_mic(backend.as_ref(), &bus, device, &master, &mut mic_stream)
                    }
                    None => Err(HostError::NoDevice),
                };
                let _ = reply.send(result);
            }
            Ok(HostCommand::CloseMic(reply)) => {
                close_mic(&bus, &mut mic_stream);
                let _ = reply.send(Ok(()));
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

        // Take back what the engine handed over. Freeing a track buffer is the
        // blocking deallocation the audio callback deliberately refused to do;
        // a capture is the same handover with something to do at the end of it.
        if let Some(queue) = retired.as_mut() {
            while let Ok(item) = queue.pop() {
                match item {
                    Retired::Capture(capture) => land_capture(&bus, &on_capture, capture),
                    // Not dropped: deactivating a plugin's processor is the
                    // instance's job, and the instance is not here.
                    Retired::Clap(processor) => on_plugin(processor),
                    other => drop(other),
                }
            }
        }
    }
}

/// Attach an input device to the microphone strip.
///
/// The ring is created here and split: the consumer goes to the engine through
/// the command queue, the producer into the input callback. Neither end is ever
/// dropped on an audio thread — the engine hands its half back through the
/// retirement queue, and this one is dropped here when the stream closes.
fn open_mic(
    backend: &dyn AudioBackend,
    bus: &Arc<ActionBus<Command>>,
    device: Option<DeviceId>,
    master: &ActiveConfig,
    slot: &mut Option<Box<dyn AudioStream>>,
) -> Result<ActiveConfig, HostError> {
    // Whatever was there goes first, or two callbacks write to two rings and
    // the engine reads one of them.
    close_mic(bus, slot);

    // The input runs at the *master's* rate, not at its own preference. A
    // 44.1 kHz microphone read by a 48 kHz engine is a voice pitched up by a
    // semitone and drifting further out all night — and unlike the headphone
    // bridge, there is no drift correction on this path to hide it. If the
    // device will not do that rate, that is worth failing over rather than
    // papering over.
    let sample_rate = master.sample_rate;
    let buffer_frames = master.buffer_frames;

    let capacity = (sample_rate.as_f64() * MIC_RING_SECONDS) as usize * dj_engine::mic::CHANNELS;
    let (producer, consumer) = rtrb::RingBuffer::new(capacity);

    let config = dj_audio::StreamConfig {
        device,
        sample_rate,
        buffer_frames,
        channels: dj_engine::mic::CHANNELS as u16,
    };
    let stream = backend
        .open_input(&config, producer)
        .map_err(|e| HostError::Audio(e.to_string()))?;

    // Only once the device is actually open, so a failed open leaves the engine
    // with no input rather than with a ring nobody fills.
    if bus
        .send_command(Command::MicInput {
            source: Some(consumer),
        })
        .is_err()
    {
        return Err(HostError::Audio(
            "command queue full; the microphone could not be attached".to_owned(),
        ));
    }

    stream.play().map_err(|e| HostError::Audio(e.to_string()))?;
    let active = stream.config().clone();
    *slot = Some(stream);
    Ok(active)
}

/// Detach the input device and tell the engine to let go of its end.
fn close_mic(bus: &Arc<ActionBus<Command>>, slot: &mut Option<Box<dyn AudioStream>>) {
    if let Some(previous) = slot.take() {
        // The producer goes first, so the callback has stopped writing before
        // the engine is told to stop reading.
        let _ = previous.pause();
        drop(previous);
    }
    if bus
        .send_command(Command::MicInput { source: None })
        .is_err()
    {
        tracing::warn!("command queue full; the microphone stays attached in the engine");
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

    // Open everything the device has, in stereo pairs, up to the widest
    // arrangement anything asks for.
    //
    // Four channels is the controller-with-a-built-in-card layout and the
    // common case: master and headphone cue on one interface. Opening only two
    // would make cueing impossible on hardware that supports it perfectly
    // well. But stopping at four is the same mistake one step further out --
    // `BusLayout::for_channels` puts the booth on a six-channel device, and
    // sending a deck out in parts needs eight, and neither could ever happen
    // on an interface whose extra sockets were never opened.
    //
    // Rounded down to a pair because every bus is stereo; an odd channel has
    // nothing to carry. Capped because opening sockets nothing routes to costs
    // buffer for silence on the interfaces that have thirty-two of them.
    //
    // In split mode the cue leaves by another card entirely, so this device
    // only needs its stereo pair.
    let channels = match chosen.as_ref().map(|d| d.max_output_channels) {
        _ if cue_active.is_some() => 2,
        Some(available) if available >= 4 => (available.min(MAX_OUTPUT_CHANNELS) / 2) * 2,
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

    // Somewhere for the sampler to record into. Sent on every open because the
    // engine is new each time -- and it has to come from here, since the audio
    // thread may not allocate one for itself. See `dj_engine::record`.
    if bus
        .send_command(Command::RecordSpace {
            samples: fresh_record_space(sample_rate),
        })
        .is_err()
    {
        tracing::warn!("command queue full at open; the sampler cannot record until it drains");
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

/// A silent buffer long enough for one capture.
///
/// Sized from [`dj_core::MAX_RECORD_SECONDS`] at the device's rate, so a
/// 96 kHz card gets the same thirty seconds a 44.1 kHz one does rather than
/// half of them.
fn fresh_record_space(sample_rate: SampleRate) -> Vec<f32> {
    let frames = (sample_rate.as_f64() * dj_core::MAX_RECORD_SECONDS).ceil() as usize;
    vec![0.0; frames * 2]
}

/// Turn a finished recording into a sample, and give the engine somewhere to
/// record next.
///
/// Both halves matter. Without the second the recorder has nowhere to write and
/// the record button stays greyed for the rest of the set -- which is why the
/// engine publishes `RecordReady` rather than leaving the interface to guess.
fn land_capture(bus: &Arc<ActionBus<Command>>, on_capture: &OnCapture, capture: Capture) {
    let Capture {
        bank,
        slot,
        source,
        mut samples,
        frames,
        sample_rate,
        bpm,
    } = capture;

    // The tail past `frames` is the silence the buffer was handed over with.
    // Truncating rather than copying keeps the one allocation this whole path
    // makes on the side of the thread that is allowed to make it.
    samples.truncate(frames * 2);
    let name = format!("rec {source}");
    let buffer: Arc<dyn dj_decode::TrackSource> = Arc::new(
        dj_decode::AudioBuffer::from_interleaved(samples, sample_rate),
    );

    if bus
        .send_command(Command::LoadSample {
            bank,
            slot,
            source: buffer,
            bpm,
        })
        .is_err()
    {
        // The recording is lost, and saying so beats a slot that silently
        // stays empty after the DJ watched the timer run.
        tracing::warn!(bank, slot, "command queue full; a recording was dropped");
        return;
    }
    on_capture(bank, slot, name);

    if bus
        .send_command(Command::RecordSpace {
            samples: fresh_record_space(sample_rate),
        })
        .is_err()
    {
        tracing::warn!("command queue full; the recorder has nowhere to write until the next open");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::{Action, DeckAction, DeckId};

    fn host() -> (AudioHost, Arc<ActionBus<Command>>, Arc<ParameterRegistry>) {
        let (bus, _unused) = ActionBus::<Command>::new(16);
        let bus = Arc::new(bus);
        let registry = Arc::new(ParameterRegistry::new());
        let host = AudioHost::start(
            Arc::clone(&bus),
            Arc::clone(&registry),
            true,
            Box::new(|_, _, _| {}),
            Box::new(|_| {}),
        );
        (host, bus, registry)
    }

    #[test]
    fn lists_the_null_devices() {
        let (host, _bus, _reg) = host();
        let devices = host.list_devices().unwrap();
        // A master, a stand-in headphone device so the split path has somewhere
        // to split to, and a wide one so the eight-channel paths are reachable.
        assert_eq!(devices.len(), 3);
        assert!(devices[0].is_default);
    }

    /// An interface with eight sockets has to get eight channels opened.
    ///
    /// The bug this covers is the one that was here: the width was capped at
    /// four, so `BusLayout::for_channels` never saw a six-channel device to
    /// put a booth output on and never saw an eight-channel one to send a deck
    /// out in parts through. Both features were reachable in the engine's own
    /// tests and unreachable from the application, on any hardware.
    #[test]
    fn a_wide_device_opens_all_of_its_channels() {
        let (host, _bus, _reg) = host();
        let outcome = host
            .open(Some(DeviceId::new("null-wide")), None, 128)
            .unwrap();
        assert_eq!(
            outcome.master.channels, 8,
            "an eight-output interface was opened at {} channels",
            outcome.master.channels
        );
    }

    /// The common case is unchanged: master and cue sharing one card.
    #[test]
    fn a_four_channel_device_still_opens_four() {
        let (host, _bus, _reg) = host();
        let outcome = host.open(Some(DeviceId::new("null")), None, 128).unwrap();
        assert_eq!(outcome.master.channels, 4);
    }

    /// A stereo interface gets a stereo stream, not two silent channels of
    /// padding.
    #[test]
    fn a_stereo_device_still_opens_stereo() {
        let (host, _bus, _reg) = host();
        let outcome = host
            .open(Some(DeviceId::new("null-cue")), None, 128)
            .unwrap();
        assert_eq!(outcome.master.channels, 2);
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
