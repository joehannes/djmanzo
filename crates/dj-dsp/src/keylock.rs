//! Keylock: changing tempo without changing pitch.
//!
//! # What it does, and where it sits
//!
//! Pulling a pitch fader speeds a track up and takes it sharp at the same time,
//! because both come from the same act of reading the samples faster. Keylock
//! separates them: the tempo change stays, the key does not move.
//!
//! This implementation is deliberately a **pitch shifter placed after the
//! resampler**, not a time-stretcher placed instead of it. The deck reads the
//! source exactly as it always did -- same playhead arithmetic, same step, same
//! sample-rate conversion -- and then the resulting audio is transposed back
//! down by the amount the speed change pushed it up. So keylock is an insert,
//! not a change to the transport, and the position, the beat grid, the
//! waveform and every seek behave identically whether it is on or off. That
//! property is worth more than the small theoretical quality edge a true
//! time-stretch would have.
//!
//! # Latency is the hard part
//!
//! Any phase-vocoder pitch shifter has group delay: a sample handed in comes
//! back out some milliseconds later. If keylock simply inserted that delay, a
//! DJ who beatmatched a track and then pressed keylock would watch it slide
//! backwards out of sync -- at 128 BPM, 60 ms is most of a semiquaver.
//!
//! So the deck compensates by reading ahead: it feeds the shifter audio from
//! [`Keylock::latency_frames`] ahead of the playhead, so what emerges lines up
//! with where the playhead actually is. [`Keylock::prime_with`] fills the
//! shifter's history from before that point, so the first block out is already
//! correct rather than a fade-in from silence.
//!
//! The alignment this produces is asserted in `dj-engine`'s deck tests rather
//! than assumed, because being 40 ms out is exactly the kind of error that
//! sounds like "the DJ can't beatmatch" instead of like a bug.
//!
//! # Realtime rules
//!
//! Everything is allocated in [`Keylock::new`]. `process`, `set_tempo` and
//! `prime_with` only do arithmetic and writes into buffers that already exist.
//! The upstream library is written for realtime use and sizes its internals in
//! `configure`; that it genuinely does not allocate afterwards is proven, not
//! trusted, by `crates/dj-engine/tests/rt_safety.rs`.

use crate::CHANNELS;
use signalsmith_stretch::Stretch;
use std::fmt;

/// Frames handed to the shifter per call.
///
/// The audio callback's block size varies between devices and, on some drivers,
/// between callbacks. Chunking to a fixed size means the scratch buffer is
/// sized once and can never be outgrown, whatever the device does.
const CHUNK_FRAMES: usize = 256;

/// Analysis window, in seconds.
///
/// The trade is latency against low-frequency resolution. 40 ms resolves down
/// to about 25 Hz -- below the bottom of a kick drum -- while keeping the round
/// trip short enough that a pitch-fader move is felt as immediate. Longer
/// windows sound marginally smoother on sustained material and noticeably worse
/// on transients, which for dance music is the wrong way round.
const BLOCK_SECONDS: f32 = 0.04;

/// Hop between analysis windows, in seconds. A quarter of the block is 75%
/// overlap, the usual quality point for a phase vocoder.
const INTERVAL_SECONDS: f32 = 0.01;

/// Speed range over which keylock is applied.
///
/// Outside this, the transposition needed is more than two octaves and the
/// result is a special effect rather than a corrected key. Clamping keeps a
/// stray `rate` -- from a jog wheel spun hard, say -- from asking for something
/// absurd.
const MIN_TEMPO: f64 = 0.25;
const MAX_TEMPO: f64 = 4.0;

/// A pitch shifter sized for one deck.
pub struct Keylock {
    stretch: Stretch,
    /// Working buffer, big enough for a chunk of audio *or* a full pre-roll.
    /// One buffer rather than two: they are never in use at the same time.
    scratch: Vec<f32>,
    /// Frames of history the shifter wants when priming.
    preroll_frames: usize,
    /// Total group delay, input to output.
    latency_frames: usize,
    tempo: f64,
}

// The upstream binding is a raw pointer with no `Debug`, and the workspace
// requires one on every public type.
impl fmt::Debug for Keylock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keylock")
            .field("tempo", &self.tempo)
            .field("latency_frames", &self.latency_frames)
            .field("preroll_frames", &self.preroll_frames)
            .finish()
    }
}

impl Keylock {
    /// Build a shifter for one stereo deck at `sample_rate`.
    ///
    /// This is the only place that allocates.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        // Guard against a nonsense rate rather than trusting it: a zero or NaN
        // here would reach the FFT setup as a zero-length window.
        let sr = if sample_rate.is_finite() && sample_rate > 0.0 {
            sample_rate
        } else {
            48_000.0
        };
        let block = ((sr * BLOCK_SECONDS) as usize).max(64);
        let interval = ((sr * INTERVAL_SECONDS) as usize).max(16);

        let stretch = Stretch::new(CHANNELS as u32, block, interval);
        let latency_frames = stretch.input_latency() + stretch.output_latency();
        let preroll_frames = block + interval;

        Self {
            scratch: vec![0.0; preroll_frames.max(CHUNK_FRAMES) * CHANNELS],
            preroll_frames,
            latency_frames,
            stretch,
            tempo: 1.0,
        }
    }

    /// Frames of history [`Self::prime_with`] wants.
    #[must_use]
    pub fn preroll_frames(&self) -> usize {
        self.preroll_frames
    }

    /// Group delay from input to output, in frames.
    ///
    /// The deck reads this far ahead of the playhead so the delay cancels.
    #[must_use]
    pub fn latency_frames(&self) -> usize {
        self.latency_frames
    }

    /// Tempo currently being corrected for.
    #[must_use]
    pub fn tempo(&self) -> f64 {
        self.tempo
    }

    /// Tell the shifter how fast the deck is running.
    ///
    /// `tempo` is the musical speed multiplier -- 1.08 for +8% -- and does
    /// *not* include sample-rate conversion, which changes no pitch and must
    /// not be undone. The transposition is its reciprocal: play 8% fast, come
    /// down 8%, land back on the original key.
    ///
    /// Cheap enough to call every block; it stores a float and returns.
    pub fn set_tempo(&mut self, tempo: f64) {
        let tempo = if tempo.is_finite() {
            tempo.clamp(MIN_TEMPO, MAX_TEMPO)
        } else {
            1.0
        };
        if tempo != self.tempo {
            self.tempo = tempo;
            self.stretch
                .set_transpose_factor((1.0 / tempo) as f32, None);
        }
    }

    /// Clear the shifter and fill its history from `read`.
    ///
    /// `read(i)` supplies frame `i` of the [`Self::preroll_frames`] frames
    /// immediately *preceding* the point playback resumes from. Without this a
    /// deck that just loaded, seeked or engaged keylock would emit a
    /// latency-long fade-in from silence -- audible as a swallowed downbeat.
    pub fn prime_with(&mut self, mut read: impl FnMut(usize) -> [f32; CHANNELS]) {
        let frames = self.preroll_frames;
        for frame in 0..frames {
            let [left, right] = read(frame);
            self.scratch[frame * CHANNELS] = left;
            self.scratch[frame * CHANNELS + 1] = right;
        }
        self.stretch.reset();
        // `set_transpose_factor` survives `reset`, but re-stating it costs
        // nothing and means priming can never resurrect a stale ratio.
        self.stretch
            .set_transpose_factor((1.0 / self.tempo) as f32, None);
        self.stretch.seek(&self.scratch[..frames * CHANNELS], 1.0);
    }

    /// Transpose `buf` in place. Interleaved stereo, any length.
    ///
    /// Realtime-safe: no allocation, no locking, no I/O.
    pub fn process(&mut self, buf: &mut [f32]) {
        for chunk in buf.chunks_mut(CHUNK_FRAMES * CHANNELS) {
            let n = chunk.len();
            // The shifter cannot read and write the same memory, so the input
            // is staged in the scratch and the caller's buffer receives output.
            self.scratch[..n].copy_from_slice(chunk);
            self.stretch.process(&self.scratch[..n], chunk);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: f32 = 48_000.0;

    fn sine(frequency: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|n| {
                let v = (TAU * frequency * n as f32 / SR).sin() * 0.7;
                [v, v]
            })
            .collect()
    }

    /// Estimate the frequency of a near-pure tone by counting zero crossings.
    ///
    /// Enough for these tests and it pulls in no FFT: the shifter's output for
    /// a steady sine is a steady sine, so crossings land where the maths says.
    /// Measured on the left channel only.
    fn frequency_of(buf: &[f32]) -> f32 {
        let left: Vec<f32> = buf.iter().step_by(CHANNELS).copied().collect();
        let mut crossings = 0usize;
        let mut first = None;
        let mut last = 0usize;
        for i in 1..left.len() {
            if left[i - 1] <= 0.0 && left[i] > 0.0 {
                if first.is_none() {
                    first = Some(i);
                } else {
                    last = i;
                }
                crossings += 1;
            }
        }
        let Some(first) = first else { return 0.0 };
        if crossings < 3 {
            return 0.0;
        }
        // Whole periods between the first and last upward crossing.
        (crossings - 1) as f32 * SR / (last - first) as f32
    }

    /// Run `input` through a shifter set to `tempo`, discarding the settling
    /// region so the measurement sees steady state.
    fn shifted(tempo: f64, input: &[f32]) -> Vec<f32> {
        let mut keylock = Keylock::new(SR);
        keylock.set_tempo(tempo);
        keylock.prime_with(|_| [0.0, 0.0]);
        let mut out = input.to_vec();
        keylock.process(&mut out);
        let skip = (keylock.latency_frames() + keylock.preroll_frames()) * CHANNELS;
        out[skip.min(out.len())..].to_vec()
    }

    #[test]
    fn a_deck_running_fast_comes_back_to_the_original_key() {
        // Playing at 1.5x makes a 440 Hz note read as 660 Hz. Keylock's job is
        // to put it back.
        let out = shifted(1.5, &sine(660.0, 240_000));
        let measured = frequency_of(&out);
        assert!(
            (measured - 440.0).abs() < 20.0,
            "expected ~440 Hz back from a 1.5x deck, measured {measured}"
        );
    }

    #[test]
    fn a_deck_running_slow_comes_back_to_the_original_key() {
        // 0.75x drags 440 Hz down to 330 Hz.
        let out = shifted(0.75, &sine(330.0, 240_000));
        let measured = frequency_of(&out);
        assert!(
            (measured - 440.0).abs() < 20.0,
            "expected ~440 Hz back from a 0.75x deck, measured {measured}"
        );
    }

    /// The case that actually happens: a beatmatch nudge of a few percent.
    #[test]
    fn a_realistic_eight_percent_nudge_is_corrected() {
        let out = shifted(1.08, &sine(440.0 * 1.08, 240_000));
        let measured = frequency_of(&out);
        assert!(
            (measured - 440.0).abs() < 8.0,
            "expected ~440 Hz back from a +8% deck, measured {measured}"
        );
    }

    #[test]
    fn at_unity_the_pitch_is_left_alone() {
        let out = shifted(1.0, &sine(440.0, 240_000));
        let measured = frequency_of(&out);
        assert!(
            (measured - 440.0).abs() < 5.0,
            "unity keylock moved the pitch to {measured}"
        );
    }

    /// Audio must come out at roughly the level that went in. A shifter that
    /// halves the level would be "correct" in pitch and useless in a mix.
    #[test]
    fn level_survives_the_shift() {
        let input = sine(440.0, 240_000);
        let out = shifted(1.08, &input);
        let rms = |b: &[f32]| (b.iter().map(|s| s * s).sum::<f32>() / b.len() as f32).sqrt();
        let (before, after) = (rms(&input), rms(&out));
        assert!(
            after > before * 0.5 && after < before * 1.5,
            "level changed too much: {before} in, {after} out"
        );
    }

    #[test]
    fn silence_in_is_silence_out() {
        let mut out = vec![0.0f32; 48_000 * CHANNELS];
        let mut keylock = Keylock::new(SR);
        keylock.set_tempo(1.08);
        keylock.prime_with(|_| [0.0, 0.0]);
        keylock.process(&mut out);
        assert!(
            out.iter().all(|s| s.abs() < 1e-6),
            "the shifter invented audio from silence"
        );
    }

    /// A block bigger than the internal chunk must still come out whole --
    /// drivers do hand over 1024 and 2048 frames.
    #[test]
    fn a_block_larger_than_the_chunk_is_processed_entirely() {
        let big = CHUNK_FRAMES * 5;
        let mut keylock = Keylock::new(SR);
        keylock.set_tempo(1.08);
        keylock.prime_with(|i| {
            let v = (TAU * 440.0 * i as f32 / SR).sin() * 0.7;
            [v, v]
        });
        let mut out = sine(440.0, big);
        keylock.process(&mut out);
        // Primed with matching audio, so output is live from the first frame.
        let energy: f32 = out.iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "a large block produced nothing");
    }

    #[test]
    fn an_absurd_tempo_is_clamped_rather_than_obeyed() {
        let mut keylock = Keylock::new(SR);
        keylock.set_tempo(1000.0);
        assert_eq!(keylock.tempo(), MAX_TEMPO);
        keylock.set_tempo(0.0);
        assert_eq!(keylock.tempo(), MIN_TEMPO);
        keylock.set_tempo(f64::NAN);
        assert_eq!(
            keylock.tempo(),
            1.0,
            "NaN should fall back to no correction"
        );
    }

    #[test]
    fn a_nonsense_sample_rate_does_not_produce_a_zero_length_window() {
        for rate in [0.0, -48_000.0, f32::NAN] {
            let keylock = Keylock::new(rate);
            assert!(keylock.preroll_frames() > 0);
            assert!(keylock.latency_frames() > 0);
        }
    }
}
