//! Recording the set to disk.
//!
//! # The shape
//!
//! The engine pushes the master bus into a ring buffer every block and never
//! waits; a thread here drains it, converts to 16-bit and appends to a WAV.
//! Nothing on the audio thread opens a file, allocates, or blocks on a disk.
//!
//! # What happens when the disk cannot keep up
//!
//! The ring fills and the engine drops samples. It counts them and publishes
//! the count, which is the whole reason it is counted: a recording with a gap
//! in it has to *say* it has a gap. Blocking the audio thread until the disk
//! caught up would trade a flawed recording for a dropout in the room, which
//! is not a trade anyone would take.
//!
//! The ring holds several seconds, so only a genuinely stalled disk loses
//! anything.

use crate::wav::{Wav, WavError};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Seconds of audio the ring can hold before the engine starts dropping.
///
/// Four seconds is far longer than any scheduling hiccup and about 1.5 MB —
/// bought once, at the start of a recording, on the host thread.
const RING_SECONDS: f64 = 4.0;

/// How often the WAV's size fields are rewritten.
///
/// A recording lost to a crash is then short by at most this much rather than
/// being a header that claims the file is empty. See [`crate::wav`].
const FLUSH_EVERY: std::time::Duration = std::time::Duration::from_secs(5);

/// What the interface needs to draw the record button.
///
/// Owned by the application for its whole life and handed to each recording,
/// rather than created per recording: the snapshot pump reads it sixty times a
/// second and would otherwise need to reach through a lock into an `Option` to
/// find out there was nothing there.
#[derive(Debug, Default)]
pub struct RecordingState {
    /// A recording is running.
    pub active: AtomicBool,
    /// Frames written to disk.
    pub frames: AtomicU64,
    /// Samples the engine could not hand over because the ring was full.
    pub dropped: AtomicU64,
    /// Set when the writer thread stops on its own — a disk that filled up.
    pub failed: AtomicBool,
    /// The device rate, so elapsed time can be worked out from `frames`.
    pub sample_rate: AtomicU32,
}

impl RecordingState {
    #[must_use]
    pub fn seconds(&self) -> f64 {
        let rate = self.sample_rate.load(Ordering::Relaxed);
        if rate == 0 {
            return 0.0;
        }
        self.frames.load(Ordering::Relaxed) as f64 / f64::from(rate)
    }

    /// Forget the last recording, so a stopped one does not report its length
    /// for ever.
    pub fn clear(&self) {
        self.active.store(false, Ordering::Relaxed);
        self.frames.store(0, Ordering::Relaxed);
        self.dropped.store(0, Ordering::Relaxed);
        self.failed.store(false, Ordering::Relaxed);
    }
}

/// A recording in progress.
#[derive(Debug)]
pub struct Recording {
    path: PathBuf,
    sample_rate: u32,
    state: Arc<RecordingState>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// Why the writer gave up, once it has been joined.
    error: Option<String>,
}

impl Recording {
    /// Open `path` and start a thread draining `samples` into it.
    ///
    /// The ring's other half has already gone to the engine; this owns the
    /// receiving end.
    pub fn start(
        path: impl AsRef<Path>,
        sample_rate: u32,
        mut samples: rtrb::Consumer<f32>,
        state: Arc<RecordingState>,
    ) -> Result<Self, WavError> {
        let path = path.as_ref().to_path_buf();
        let mut wav = Wav::create(&path, sample_rate)?;

        state.clear();
        state.sample_rate.store(sample_rate, Ordering::Relaxed);
        state.active.store(true, Ordering::Relaxed);
        let stop = Arc::new(AtomicBool::new(false));

        let thread = {
            let state = Arc::clone(&state);
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("dj-set-recorder".to_owned())
                .spawn(move || {
                    let mut dither = Dither::new();
                    let mut block = Vec::with_capacity(8_192);
                    let mut flushed = std::time::Instant::now();

                    loop {
                        let ending = stop.load(Ordering::Relaxed);
                        block.clear();
                        while block.len() < 8_192 {
                            match samples.pop() {
                                Ok(sample) => block.push(dither.quantise(sample)),
                                Err(_) => break,
                            }
                        }

                        if !block.is_empty() {
                            if wav.write(&block).is_err() {
                                state.failed.store(true, Ordering::Relaxed);
                                state.active.store(false, Ordering::Relaxed);
                                return;
                            }
                            state.frames.store(wav.frames(), Ordering::Relaxed);
                        } else if ending {
                            // Asked to stop and the ring is empty: everything
                            // the engine handed over is on disk. Checked in this
                            // order so a stop never truncates the tail.
                            let _ = wav.close();
                            state.active.store(false, Ordering::Relaxed);
                            return;
                        } else {
                            // Nothing to do. Sleeping beats spinning a core for
                            // the length of a set.
                            std::thread::sleep(std::time::Duration::from_millis(20));
                        }

                        if flushed.elapsed() >= FLUSH_EVERY {
                            flushed = std::time::Instant::now();
                            if wav.flush_sizes().is_err() {
                                state.failed.store(true, Ordering::Relaxed);
                                state.active.store(false, Ordering::Relaxed);
                                return;
                            }
                        }
                    }
                })
                .map_err(|source| WavError::Io {
                    path: path.clone(),
                    source,
                })?
        };

        Ok(Self {
            path,
            sample_rate,
            state,
            stop,
            thread: Some(thread),
            error: None,
        })
    }

    /// How many frames of ring to ask for at this rate.
    #[must_use]
    pub fn ring_capacity(sample_rate: u32) -> usize {
        ((f64::from(sample_rate) * RING_SECONDS) as usize * 2).max(1_024)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn seconds(&self) -> f64 {
        let frames = self.state.frames.load(Ordering::Relaxed);
        if self.sample_rate == 0 {
            return 0.0;
        }
        frames as f64 / f64::from(self.sample_rate)
    }

    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.state.dropped.load(Ordering::Relaxed)
    }

    /// Tell the interface how many samples the engine could not hand over.
    pub fn set_dropped(&self, dropped: u64) {
        self.state.dropped.store(dropped, Ordering::Relaxed);
    }

    #[must_use]
    pub fn failed(&self) -> bool {
        self.state.failed.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Stop, wait for the tail to reach disk, and hand back the finished file.
    ///
    /// Joins rather than detaching: the point of stopping is that the file is
    /// complete afterwards, and a caller that got the path back before the last
    /// block was written would be handed a file still being appended to.
    pub fn finish(mut self) -> PathBuf {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        if self.failed() {
            // The header may never have been finalised. The bytes are still
            // there, so say how many rather than leaving an unplayable file.
            let _ = Wav::repair(&self.path);
        }
        self.path.clone()
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        // A recording dropped without `finish` — the application closing
        // mid-set — still gets its thread stopped and its file closed.
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Triangular dither, applied once on the way to 16 bits.
///
/// Truncating instead would fold quantisation error into the signal as
/// distortion that tracks the music; dither turns it into a steady hiss at
/// -93 dBFS, which is inaudible and, unlike distortion, does not get worse on
/// quiet passages. Two uniform values summed give the triangular distribution
/// that is the standard choice for this.
///
/// This runs on the writer thread, so it costs the audio path nothing.
#[derive(Debug)]
struct Dither {
    seed: u64,
}

impl Dither {
    fn new() -> Self {
        Self {
            seed: 0x2545_F491_4F6C_DD1D,
        }
    }

    fn uniform(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        // Top bits, which is where an LCG's randomness actually is.
        ((self.seed >> 40) as f32 / 16_777_216.0) - 0.5
    }

    /// Dither one sample and quantise it to 16 bits.
    fn quantise(&mut self, sample: f32) -> i16 {
        // One least-significant bit, spread triangularly.
        let noise = (self.uniform() + self.uniform()) / f32::from(i16::MAX);
        let dithered = (sample + noise).clamp(-1.0, 1.0);
        // `i16::MAX` rather than 32768: scaling by the larger number and
        // clamping would put every full-scale sample at exactly the clip point.
        (dithered * f32::from(i16::MAX)).round() as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("djmanzo-set-{name}-{}.wav", std::process::id()));
        path
    }

    /// Read the samples back out of a finished file.
    fn samples_of(path: &Path) -> Vec<i16> {
        let raw = std::fs::read(path).expect("the recording should exist");
        raw[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect()
    }

    /// Feed a recording and wait for the writer to catch up.
    fn record(name: &str, audio: &[f32]) -> PathBuf {
        let path = temp(name);
        let (mut tx, rx) = rtrb::RingBuffer::<f32>::new(Recording::ring_capacity(48_000));
        let recording =
            Recording::start(&path, 48_000, rx, Arc::new(RecordingState::default())).unwrap();
        for sample in audio {
            // The test is not real time, so it may outrun the writer; spin
            // rather than drop, since here we are testing the file and not the
            // overflow behaviour.
            let mut value = *sample;
            while let Err(rtrb::PushError::Full(returned)) = tx.push(value) {
                value = returned;
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        recording.finish()
    }

    #[test]
    fn what_the_master_played_is_what_lands_on_disk() {
        // Full scale, silence, and full scale the other way: the three values
        // a conversion is most likely to get wrong.
        let path = record("roundtrip", &[1.0, -1.0, 0.0, 0.0, 0.5, -0.5]);
        let out = samples_of(&path);

        assert_eq!(out.len(), 6);
        assert_eq!(out[0], i16::MAX, "full scale should reach full scale");
        assert_eq!(out[1], -i16::MAX);
        // Dither means silence is not exactly zero, but it must be inaudible:
        // a couple of counts out of 32,767.
        assert!(out[2].abs() <= 2, "silence read as {}", out[2]);
        // Half scale, within the dither's reach.
        assert!((i32::from(out[4]) - 16_384).abs() < 8, "got {}", out[4]);
        let _ = std::fs::remove_file(&path);
    }

    /// Channel order is the one thing a listener notices and a size check does
    /// not: interleaved in, interleaved out, left first.
    #[test]
    fn the_two_channels_stay_in_the_order_they_arrived() {
        let path = record("channels", &[1.0, -1.0, 1.0, -1.0]);
        let out = samples_of(&path);
        assert_eq!(out[0], i16::MAX);
        assert_eq!(out[1], -i16::MAX);
        assert_eq!(out[2], i16::MAX);
        assert_eq!(out[3], -i16::MAX);
        let _ = std::fs::remove_file(&path);
    }

    /// **The property `finish` exists for.** Stopping has to mean the file is
    /// complete, not that it is about to be — a caller handed the path back
    /// early would find a file still being appended to.
    #[test]
    fn stopping_waits_for_the_tail_to_reach_disk() {
        let audio: Vec<f32> = (0..40_000).map(|n| (n % 7) as f32 / 16.0).collect();
        let path = record("tail", &audio);
        assert_eq!(
            samples_of(&path).len(),
            audio.len(),
            "the recording was cut short by its own stop"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// Anything past full scale is clamped rather than wrapped. Wrapping turns
    /// a loud passage into a burst of noise at the opposite polarity, which is
    /// the worst possible failure for a recording of a set.
    #[test]
    fn overs_clamp_rather_than_wrap() {
        let path = record("overs", &[4.0, -4.0, f32::INFINITY, f32::NEG_INFINITY]);
        let out = samples_of(&path);
        assert_eq!(out[0], i16::MAX);
        assert_eq!(out[1], -i16::MAX);
        assert_eq!(out[2], i16::MAX);
        assert_eq!(out[3], -i16::MAX);
        let _ = std::fs::remove_file(&path);
    }

    /// Dither has to be *noise*, not a constant offset — a fixed offset would
    /// be DC on the recording rather than a masked quantisation error.
    #[test]
    fn dither_averages_out_rather_than_biasing_the_signal() {
        let mut dither = Dither::new();
        let mut total = 0i64;
        for _ in 0..100_000 {
            total += i64::from(dither.quantise(0.0));
        }
        let mean = total as f64 / 100_000.0;
        assert!(mean.abs() < 0.05, "dither pulled silence to {mean}");
    }

    /// And it has to actually vary, or it is not dither.
    #[test]
    fn dither_is_not_the_same_number_every_time() {
        let mut dither = Dither::new();
        let seen: std::collections::HashSet<i16> =
            (0..1_000).map(|_| dither.quantise(0.25)).collect();
        assert!(seen.len() > 1, "the dither produced one value");
    }

    #[test]
    fn a_recording_that_never_started_playing_is_still_a_valid_file() {
        let path = record("empty", &[]);
        let raw = std::fs::read(&path).unwrap();
        assert_eq!(raw.len(), 44, "header only");
        assert_eq!(&raw[..4], b"RIFF");
        let _ = std::fs::remove_file(&path);
    }

    /// The ring is sized in seconds so the answer scales with the device rate
    /// rather than being a frame count that means four seconds at 48 kHz and
    /// two at 96.
    #[test]
    fn the_ring_holds_the_same_time_at_any_rate() {
        for rate in [44_100u32, 48_000, 96_000] {
            let capacity = Recording::ring_capacity(rate);
            let seconds = capacity as f64 / (f64::from(rate) * 2.0);
            assert!(
                (seconds - RING_SECONDS).abs() < 0.01,
                "{rate} Hz holds {seconds} s"
            );
        }
    }
}
