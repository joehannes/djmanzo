//! Headless backend and offline rendering.
//!
//! This container has no `/dev/snd`, and GitHub's runners have no sound card, so
//! without something here the engine could not be exercised in CI at all. These
//! two types are what make the realtime path testable:
//!
//! - [`NullBackend`] drives the callback from an ordinary thread at wall-clock
//!   pace, so the whole application can run headless.
//! - [`OfflineRenderer`] calls the callback synchronously and hands back the
//!   samples, so engine behaviour can be *asserted* rather than listened to.

use crate::device::{ActiveConfig, DeviceId, DeviceInfo, StreamConfig};
use crate::{AudioBackend, AudioCallback, AudioError, AudioStream, RenderContext};
use dj_core::SampleRate;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// How many null capture streams are open right now.
///
/// Test-visible state in shipping code, which needs justifying: the bug this
/// exists to catch is a *stream that is never closed*, and nothing else about
/// an unclosed stream is observable from outside. It went unnoticed on the
/// microphone path for exactly that reason. The counter is confined to the
/// backend that has no hardware behind it and whose stated purpose is making
/// the realtime path testable, and it costs one relaxed increment per device
/// open.
static LIVE_INPUTS: AtomicUsize = AtomicUsize::new(0);

/// Null capture streams currently open. See [`LIVE_INPUTS`].
#[must_use]
pub fn live_input_streams() -> usize {
    LIVE_INPUTS.load(Ordering::Relaxed)
}

/// A backend with no hardware behind it.
#[derive(Debug, Default)]
pub struct NullBackend;

impl NullBackend {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn device_info() -> DeviceInfo {
        DeviceInfo {
            id: DeviceId::new("null"),
            name: "Null output (no hardware)".to_owned(),
            max_output_channels: 4,
            default_sample_rate: SampleRate::DEFAULT,
            is_default: true,
        }
    }

    /// A second virtual device, so the two-card path is testable.
    ///
    /// Not padding. Split output puts the headphone cue on a *different* card
    /// from the master, and a backend that can only ever offer one device makes
    /// that path impossible to exercise — which is the one path where the
    /// interesting failure lives, because the two cards run on independent
    /// clocks. Two channels, like the cheap USB interface it stands in for.
    fn cue_device_info() -> DeviceInfo {
        DeviceInfo {
            id: DeviceId::new("null-cue"),
            name: "Null headphone output (no hardware)".to_owned(),
            max_output_channels: 2,
            default_sample_rate: SampleRate::DEFAULT,
            is_default: false,
        }
    }

    /// A third virtual device, wide enough for the arrangements that need
    /// real sockets.
    ///
    /// Also not padding. Sending a deck out in parts needs eight outputs, and
    /// the booth and cue buses only exist above four — so on a backend that
    /// tops out at four, none of those paths can be reached at all. That is
    /// the shape of bug this project keeps finding: code that compiles, passes
    /// its own tests and cannot be exercised. Eight channels, like the
    /// multi-out interface a DJ running an external mixer actually owns.
    fn wide_device_info() -> DeviceInfo {
        DeviceInfo {
            id: DeviceId::new("null-wide"),
            name: "Null 8-channel output (no hardware)".to_owned(),
            max_output_channels: 8,
            default_sample_rate: SampleRate::DEFAULT,
            is_default: false,
        }
    }

    /// A virtual capture device.
    ///
    /// Same argument as the wide output above, one layer along: until this
    /// existed, *every* input path -- the microphone, and a deck following a
    /// control record -- could only be reached on a machine with a sound card,
    /// which is to say never in CI and never here. The consequence was not
    /// hypothetical: opening a different output device left the microphone's
    /// stream running into a ring belonging to an engine that had been dropped,
    /// silently, and no test could see it.
    ///
    /// What it captures is silence, and that is the honest null: a card with
    /// nothing plugged into it. Silence is also a state the interface has to
    /// tell apart from having no input at all, so it is worth being able to
    /// produce on purpose.
    fn input_device_info() -> DeviceInfo {
        DeviceInfo {
            id: DeviceId::new("null-input"),
            name: "Null input (no hardware)".to_owned(),
            // Named for outputs, meaning "channels" on either side of the
            // trait. Two, because a control record needs a stereo pair and a
            // mono virtual microphone could not carry one.
            max_output_channels: 2,
            default_sample_rate: SampleRate::DEFAULT,
            is_default: true,
        }
    }

    fn describe(id: Option<&DeviceId>) -> DeviceInfo {
        match id {
            Some(wanted) if wanted == &Self::cue_device_info().id => Self::cue_device_info(),
            Some(wanted) if wanted == &Self::wide_device_info().id => Self::wide_device_info(),
            _ => Self::device_info(),
        }
    }
}

impl AudioBackend for NullBackend {
    fn name(&self) -> &'static str {
        "null"
    }

    fn output_devices(&self) -> Result<Vec<DeviceInfo>, AudioError> {
        Ok(vec![
            Self::device_info(),
            Self::cue_device_info(),
            Self::wide_device_info(),
        ])
    }

    fn open_output(
        &self,
        config: &StreamConfig,
        mut callback: Box<dyn AudioCallback>,
    ) -> Result<Box<dyn AudioStream>, AudioError> {
        let active = ActiveConfig {
            device_name: Self::describe(config.device.as_ref()).name,
            sample_rate: config.sample_rate,
            buffer_frames: config.buffer_frames,
            channels: config.channels,
        };

        let running = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(true));

        let thread = {
            let running = Arc::clone(&running);
            let alive = Arc::clone(&alive);
            let active = active.clone();
            std::thread::Builder::new()
                .name("dj-null-audio".to_owned())
                .spawn(move || {
                    let frames = active.buffer_frames as usize;
                    let channels = active.channels as usize;
                    let mut buffer = vec![0.0f32; frames * channels];
                    let period = std::time::Duration::from_secs_f64(
                        frames as f64 / active.sample_rate.as_f64(),
                    );
                    let context = RenderContext {
                        frames,
                        channels,
                        sample_rate: active.sample_rate,
                    };

                    let mut next = std::time::Instant::now();
                    while alive.load(Ordering::Relaxed) {
                        if running.load(Ordering::Relaxed) {
                            buffer.fill(0.0);
                            callback.render(&mut buffer, &context);
                        }
                        next += period;
                        // If we fell behind, resync rather than spiralling: a
                        // headless run has no listener to care about the gap.
                        let now = std::time::Instant::now();
                        if next > now {
                            std::thread::sleep(next - now);
                        } else {
                            next = now;
                        }
                    }
                })
                .map_err(|e| AudioError::OpenStream(e.to_string()))?
        };

        Ok(Box::new(NullStream {
            active,
            running,
            alive,
            thread: Some(thread),
            counted: false,
        }))
    }

    fn input_devices(&self) -> Result<Vec<DeviceInfo>, AudioError> {
        Ok(vec![Self::input_device_info()])
    }

    fn open_input(
        &self,
        config: &StreamConfig,
        mut sink: rtrb::Producer<f32>,
    ) -> Result<Box<dyn AudioStream>, AudioError> {
        let active = ActiveConfig {
            device_name: Self::input_device_info().name,
            sample_rate: config.sample_rate,
            buffer_frames: config.buffer_frames,
            channels: config.channels,
        };

        let running = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(true));

        let thread = {
            let running = Arc::clone(&running);
            let alive = Arc::clone(&alive);
            let active = active.clone();
            std::thread::Builder::new()
                .name("dj-null-input".to_owned())
                .spawn(move || {
                    let frames = active.buffer_frames as usize;
                    let channels = active.channels as usize;
                    let period = std::time::Duration::from_secs_f64(
                        frames as f64 / active.sample_rate.as_f64(),
                    );
                    let mut next = std::time::Instant::now();
                    while alive.load(Ordering::Relaxed) {
                        if running.load(Ordering::Relaxed) {
                            // Whatever will fit and no more. A real capture
                            // callback drops what the ring will not take rather
                            // than blocking, and so does this one -- a reader
                            // that has stopped draining must not stall the
                            // device thread.
                            for _ in 0..frames * channels {
                                if sink.push(0.0).is_err() {
                                    break;
                                }
                            }
                        }
                        next += period;
                        let now = std::time::Instant::now();
                        if next > now {
                            std::thread::sleep(next - now);
                        } else {
                            next = now;
                        }
                    }
                })
                .map_err(|e| AudioError::OpenStream(e.to_string()))?
        };

        LIVE_INPUTS.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(NullStream {
            active,
            running,
            alive,
            thread: Some(thread),
            counted: true,
        }))
    }
}

#[derive(Debug)]
struct NullStream {
    active: ActiveConfig,
    running: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// Whether this stream is one of the ones [`LIVE_INPUTS`] counts.
    counted: bool,
}

impl AudioStream for NullStream {
    fn play(&self) -> Result<(), AudioError> {
        self.running.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn pause(&self) -> Result<(), AudioError> {
        self.running.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn config(&self) -> &ActiveConfig {
        &self.active
    }
}

impl Drop for NullStream {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        if self.counted {
            LIVE_INPUTS.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

/// Runs a callback synchronously and collects what it produced.
///
/// The whole point is determinism: no threads, no timing, no device. A test can
/// render exactly N blocks and assert on the exact samples, which is how engine
/// behaviour gets verified in an environment with no audio hardware.
#[derive(Debug)]
pub struct OfflineRenderer {
    callback: Box<dyn AudioCallback>,
    context: RenderContext,
    buffer: Vec<f32>,
}

impl OfflineRenderer {
    #[must_use]
    pub fn new(callback: Box<dyn AudioCallback>, config: &StreamConfig) -> Self {
        let frames = config.buffer_frames as usize;
        let channels = config.channels as usize;
        Self {
            callback,
            context: RenderContext {
                frames,
                channels,
                sample_rate: config.sample_rate,
            },
            buffer: vec![0.0; frames * channels],
        }
    }

    /// Render one block, returning the samples produced.
    ///
    /// The buffer is cleared first, exactly as a real backend hands over a fresh
    /// (or stale) buffer -- so a callback that writes nothing yields silence
    /// rather than whatever was there last time.
    pub fn render_block(&mut self) -> &[f32] {
        self.buffer.fill(0.0);
        self.callback.render(&mut self.buffer, &self.context);
        &self.buffer
    }

    /// Render `blocks` blocks and concatenate them.
    pub fn render(&mut self, blocks: usize) -> Vec<f32> {
        let mut out = Vec::with_capacity(blocks * self.buffer.len());
        for _ in 0..blocks {
            out.extend_from_slice(self.render_block());
        }
        out
    }

    /// Render `blocks` blocks, discarding the audio.
    ///
    /// For the realtime-safety test, which cares about what the callback *does*,
    /// not what it produces -- and must not allocate a result buffer that would
    /// pollute the allocation count.
    pub fn render_discarding(&mut self, blocks: usize) {
        for _ in 0..blocks {
            self.render_block();
        }
    }

    #[must_use]
    pub fn context(&self) -> &RenderContext {
        &self.context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes a constant so tests can tell "ran" from "did not run".
    #[derive(Debug)]
    struct ConstantCallback {
        value: f32,
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl AudioCallback for ConstantCallback {
        fn render(&mut self, out: &mut [f32], _ctx: &RenderContext) {
            self.calls.fetch_add(1, Ordering::Relaxed);
            out.fill(self.value);
        }
    }

    fn callback(value: f32) -> (Box<dyn AudioCallback>, Arc<std::sync::atomic::AtomicUsize>) {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        (
            Box::new(ConstantCallback {
                value,
                calls: Arc::clone(&calls),
            }),
            calls,
        )
    }

    #[test]
    fn null_backend_reports_a_default_and_two_more_devices() {
        let devices = NullBackend::new().output_devices().unwrap();
        // Three: one to split *to*, and one wide enough for the arrangements
        // four channels cannot hold.
        assert_eq!(devices.len(), 3);
        assert!(devices[0].is_default);
        assert!(devices[0].supports_split_output());
        assert!(!devices[1].is_default);
        assert!(
            !devices[1].supports_split_output(),
            "the stand-in headphone device should be stereo, like the hardware it represents"
        );
        assert!(!devices[2].is_default);
        assert_eq!(
            devices[2].max_output_channels, 8,
            "the wide device exists to make the eight-channel paths reachable"
        );
    }

    /// Opening the wide device must give eight channels, not the default
    /// device's four -- otherwise it is a name in a list and nothing more.
    #[test]
    fn the_wide_device_opens_at_its_own_width() {
        let backend = NullBackend::new();
        let info = backend
            .output_devices()
            .unwrap()
            .into_iter()
            .find(|device| device.max_output_channels == 8)
            .expect("the wide device is listed");
        let (callback, _calls) = callback(0.0);
        let stream = backend
            .open_output(
                &StreamConfig {
                    buffer_frames: 64,
                    sample_rate: SampleRate::DEFAULT,
                    channels: 8,
                    device: Some(info.id.clone()),
                },
                callback,
            )
            .unwrap();
        assert_eq!(stream.config().channels, 8);
        assert_eq!(stream.config().device_name, info.name);
    }

    /// Opening the second device must actually open the second device, or a
    /// test of the split path would be testing one device twice.
    #[test]
    fn the_named_device_is_the_one_opened() {
        let backend = NullBackend::new();
        let config = StreamConfig {
            device: Some(DeviceId::new("null-cue")),
            channels: 2,
            ..Default::default()
        };
        let stream = backend
            .open_output(
                &config,
                Box::new(ConstantCallback {
                    value: 0.0,
                    calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                }),
            )
            .unwrap();
        assert!(stream.config().device_name.contains("headphone"));
    }

    #[test]
    fn opened_stream_reports_what_was_asked_for() {
        let (cb, _) = callback(0.0);
        let config = StreamConfig {
            buffer_frames: 128,
            ..Default::default()
        };
        let stream = NullBackend::new().open_output(&config, cb).unwrap();
        assert_eq!(stream.config().buffer_frames, 128);
        assert_eq!(stream.config().channels, 2);
    }

    #[test]
    fn stream_only_calls_back_while_playing() {
        let (cb, calls) = callback(1.0);
        let config = StreamConfig {
            buffer_frames: 64,
            ..Default::default()
        };
        let stream = NullBackend::new().open_output(&config, cb).unwrap();

        // Paused by default: nothing should happen.
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(
            calls.load(Ordering::Relaxed),
            0,
            "callback ran while paused"
        );

        stream.play().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        stream.pause().unwrap();
        let after_play = calls.load(Ordering::Relaxed);
        assert!(after_play > 0, "callback never ran while playing");

        std::thread::sleep(std::time::Duration::from_millis(30));
        let after_pause = calls.load(Ordering::Relaxed);
        // Allow one in-flight block that started before the pause landed.
        assert!(
            after_pause <= after_play + 1,
            "callback kept running after pause: {after_play} -> {after_pause}"
        );
    }

    #[test]
    fn dropping_the_stream_stops_the_thread() {
        let (cb, calls) = callback(1.0);
        let stream = NullBackend::new()
            .open_output(&StreamConfig::default(), cb)
            .unwrap();
        stream.play().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        drop(stream);

        let at_drop = calls.load(Ordering::Relaxed);
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert_eq!(
            calls.load(Ordering::Relaxed),
            at_drop,
            "thread outlived the stream"
        );
    }

    #[test]
    fn offline_renderer_produces_exactly_what_the_callback_wrote() {
        let (cb, calls) = callback(0.5);
        let config = StreamConfig {
            buffer_frames: 4,
            channels: 2,
            ..Default::default()
        };
        let mut renderer = OfflineRenderer::new(cb, &config);

        let block = renderer.render_block();
        assert_eq!(block.len(), 8, "4 frames x 2 channels");
        assert!(block.iter().all(|&s| s == 0.5));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn offline_renderer_concatenates_blocks() {
        let (cb, calls) = callback(1.0);
        let config = StreamConfig {
            buffer_frames: 4,
            channels: 2,
            ..Default::default()
        };
        let mut renderer = OfflineRenderer::new(cb, &config);
        let out = renderer.render(3);
        assert_eq!(out.len(), 24);
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }

    /// A callback that writes nothing must yield silence, not stale audio. If
    /// the renderer skipped this, a deck that failed to render would replay the
    /// previous block forever and the bug would sound like a stuck loop.
    #[test]
    fn buffer_is_cleared_between_blocks() {
        #[derive(Debug)]
        struct WriteOnce(bool);
        impl AudioCallback for WriteOnce {
            fn render(&mut self, out: &mut [f32], _ctx: &RenderContext) {
                if !self.0 {
                    out.fill(1.0);
                    self.0 = true;
                }
            }
        }

        let config = StreamConfig {
            buffer_frames: 4,
            channels: 2,
            ..Default::default()
        };
        let mut renderer = OfflineRenderer::new(Box::new(WriteOnce(false)), &config);
        assert!(renderer.render_block().iter().all(|&s| s == 1.0));
        assert!(
            renderer.render_block().iter().all(|&s| s == 0.0),
            "stale audio survived into the next block"
        );
    }
}
