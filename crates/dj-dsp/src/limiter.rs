//! The last thing before the speakers.
//!
//! A DJ mixer clips constantly. Two tracks overlap, both faders are up, the
//! trim was set for a quieter record, someone pushes the bass — and the master
//! goes past full scale. Without a limiter that arrives as hard digital
//! clipping, which is the ugliest sound a PA can make and the fastest way to
//! blow a tweeter.
//!
//! # Look-ahead is what makes it a limiter rather than a compressor
//!
//! A compressor reacts *after* a peak, so the first millisecond of every
//! transient escapes. A limiter delays the signal by a few milliseconds and
//! looks at what is coming, so the gain is already down when the peak arrives
//! and nothing gets through. The cost is exactly that delay — 5 ms here, which
//! is below the threshold where a DJ feels the difference between pressing play
//! and hearing it, and small enough that headphone cue and master stay in step.
//!
//! # Why the release is slow and the attack is instant
//!
//! Gain reduction that engages instantly and recovers slowly is inaudible on
//! programme material. The reverse — a slow attack — lets peaks through, and a
//! fast release turns sustained bass into a pumping mess, because the limiter
//! recovers between every cycle of a 50 Hz note and modulates it.
//!
//! # Realtime rules
//!
//! Allocated once in [`Limiter::new`]. `process` is arithmetic and a ring
//! buffer index; it never allocates, locks or branches on anything unbounded.

use crate::CHANNELS;

/// Look-ahead, in seconds.
///
/// Long enough to catch a transient, short enough that nobody feels it. Also
/// the amount the master is delayed relative to the headphone cue, which is why
/// it is not longer.
const LOOKAHEAD_SECONDS: f32 = 0.005;

/// How fast gain recovers, in seconds, for a 1/e step.
///
/// Slow on purpose: fast release modulates sustained bass, because the limiter
/// recovers between every cycle of a low note.
const RELEASE_SECONDS: f32 = 0.100;

/// Ceiling, as a linear amplitude.
///
/// A shade under full scale. Inter-sample peaks can exceed the sample values
/// they sit between, and a converter reconstructing the waveform can overshoot
/// a signal that never touched 1.0 in the samples themselves. Leaving a little
/// headroom costs nothing audible and avoids clipping on the way out of the
/// DAC.
const CEILING: f32 = 0.977; // about -0.2 dBFS

/// How close to unity counts as arrived.
///
/// A one-pole release approaches its target geometrically and never reaches it,
/// so without a snap the limiter never comes fully to rest: the gain sits a
/// hair under 1.0 forever and the reduction meter shows a permanent fraction of
/// a decibel, which reads as a stuck limiter.
///
/// 0.001 in linear gain is 0.0087 dB. That is two orders of magnitude below the
/// smallest level change anyone can hear, and below the resolution of any meter
/// that would display it. Choosing it also decides how long recovery takes to
/// *finish*: from heavy reduction the limiter reaches true rest in about
/// three quarters of a second, rather than never.
const GAIN_EPSILON: f32 = 1e-3;

/// A look-ahead peak limiter for one stereo bus.
#[derive(Debug)]
pub struct Limiter {
    /// Delayed audio, interleaved. Sized at construction, never resized.
    delay: Vec<f32>,
    /// Where the next sample goes. The oldest sample is at the same index.
    write: usize,
    /// Frames of look-ahead.
    lookahead: usize,
    /// Current gain reduction, 0..=1. Moves to `target` at the release rate.
    gain: f32,
    /// One-pole coefficient for the release.
    release: f32,
    /// Peak seen in the look-ahead window, tracked so the whole window does not
    /// have to be rescanned per sample.
    window_peak: f32,
    /// Samples since the window peak was last recomputed from scratch.
    since_rescan: usize,
    ceiling: f32,
    /// When set, the delay still runs but no gain is applied.
    bypass: bool,
}

impl Limiter {
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let rate = if sample_rate.is_finite() && sample_rate > 0.0 {
            sample_rate
        } else {
            48_000.0
        };
        let lookahead = ((rate * LOOKAHEAD_SECONDS) as usize).max(1);

        Self {
            delay: vec![0.0; lookahead * CHANNELS],
            write: 0,
            lookahead,
            gain: 1.0,
            release: (-1.0 / (RELEASE_SECONDS * rate)).exp(),
            window_peak: 0.0,
            since_rescan: 0,
            ceiling: CEILING,
            bypass: false,
        }
    }

    /// Bypass the gain reduction, leaving the delay in place.
    ///
    /// The delay deliberately keeps running. Dropping it would change the
    /// output latency the instant someone toggled the switch, which shifts the
    /// master against the headphone cue mid-set — the one place a 5 ms jump is
    /// guaranteed to be noticed, because beatmatching *is* the act of comparing
    /// those two. Constant latency in both states costs 5 ms of delay nobody
    /// hears and makes the control safe to touch during a set.
    ///
    /// The transition itself is free of clicks for the same reason gain
    /// recovery is: bypass simply asks for a target of 1.0, and the existing
    /// release slew walks there.
    pub fn set_bypass(&mut self, bypass: bool) {
        self.bypass = bypass;
    }

    #[must_use]
    pub fn is_bypassed(&self) -> bool {
        self.bypass
    }

    /// Frames of delay the limiter adds.
    ///
    /// Reported so the rest of the application can compensate rather than
    /// discover it. Anything that has to line up with the master — the
    /// headphone cue, a recording, a video output — needs this number.
    #[must_use]
    pub fn latency_frames(&self) -> usize {
        self.lookahead
    }

    /// Current gain reduction in decibels, as a positive number.
    ///
    /// What a gain-reduction meter shows. Zero means the limiter is doing
    /// nothing, which is where it should sit most of the night.
    #[must_use]
    pub fn reduction_db(&self) -> f32 {
        if self.gain >= 1.0 {
            0.0
        } else {
            -20.0 * self.gain.max(1e-6).log10()
        }
    }

    pub fn reset(&mut self) {
        self.delay.fill(0.0);
        self.write = 0;
        self.gain = 1.0;
        self.window_peak = 0.0;
        self.since_rescan = 0;
    }

    /// Limit one interleaved stereo buffer in place.
    ///
    /// Realtime-safe: no allocation, no locking, no I/O.
    pub fn process(&mut self, buffer: &mut [f32]) {
        // `as_chunks_mut` rather than `chunks_exact_mut`: a frame comes back
        // as `&mut [f32; CHANNELS]`, so reading and writing the pair is four
        // array accesses the compiler already knows are in range instead of
        // four slice accesses it has to bounds-check. The remainder is any
        // trailing partial frame, dropped exactly as `chunks_exact_mut`
        // dropped it.
        let (frames, _) = buffer.as_chunks_mut::<CHANNELS>();
        for frame in frames {
            let (left, right) = self.process_frame(frame[0], frame[1]);
            frame[0] = left;
            frame[1] = right;
        }
    }

    /// Limit a single stereo frame.
    ///
    /// The engine's master bus is a stereo *pair inside* a wider interleaved
    /// device buffer — four or six channels, with the booth and cue on the
    /// others — so it cannot hand the limiter a contiguous stereo slice. It
    /// already walks the buffer frame by frame, so this is the shape that fits
    /// without a scratch buffer or a strided iterator.
    ///
    /// Returns the delayed, limited frame: the output lags the input by
    /// [`Self::latency_frames`].
    pub fn process_frame(&mut self, left: f32, right: f32) -> (f32, f32) {
        {
            let incoming = left.abs().max(right.abs());

            // Push the new frame in, take the delayed one out. Same index: the
            // slot being overwritten holds the oldest sample.
            let slot = self.write * CHANNELS;
            let delayed = [self.delay[slot], self.delay[slot + 1]];
            self.delay[slot] = left;
            self.delay[slot + 1] = right;
            self.write = (self.write + 1) % self.lookahead;

            // Track the loudest thing in the look-ahead window. Rescanning is
            // O(lookahead) so it is done only when the sample leaving the
            // window *was* the peak -- otherwise the running maximum is exact
            // and costs one comparison.
            if incoming >= self.window_peak {
                self.window_peak = incoming;
                self.since_rescan = 0;
            } else {
                self.since_rescan += 1;
                if self.since_rescan >= self.lookahead {
                    self.window_peak = self.scan_window();
                    self.since_rescan = 0;
                }
            }

            // The sample about to leave has already been overwritten in the
            // ring, so it is *not* in `window_peak` any more. Folding it back in
            // is not a tidiness fix: without it the ceiling is only as good as
            // the running-peak bookkeeping, and a rescan that lands one frame
            // early lets the loudest sample of a transient out at full gain.
            //
            // With it the guarantee is unconditional and provable. If the
            // outgoing sample is over the ceiling then `target <= ceiling/|out|`,
            // and `gain` either snaps to `target` or is already below it, so the
            // product never exceeds the ceiling — whatever the look-ahead
            // thought was coming. Look-ahead is what makes the reduction smooth;
            // this line is what makes it safe.
            let outgoing = delayed[0].abs().max(delayed[1].abs());
            let ahead = self.window_peak.max(outgoing);

            // Gain that would bring the loudest upcoming sample to the ceiling.
            // Bypass asks for unity and lets the release slew get there, so the
            // switch does not click.
            let target = if ahead > self.ceiling && !self.bypass {
                self.ceiling / ahead
            } else {
                1.0
            };

            // Attack is instantaneous, release is a one-pole slew. Anything
            // slower on the attack lets the transient through, which is the one
            // thing a limiter must not do.
            self.gain = if target < self.gain {
                target
            } else {
                let released = target + (self.gain - target) * self.release;
                // Snap once the remainder is inaudible, so the limiter actually
                // comes to rest. See `GAIN_EPSILON`.
                if target - released < GAIN_EPSILON {
                    target
                } else {
                    released
                }
            };

            (delayed[0] * self.gain, delayed[1] * self.gain)
        }
    }

    fn scan_window(&self) -> f32 {
        self.delay
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: f32 = 48_000.0;

    fn sine(amplitude: f32, hz: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|n| {
                let v = (TAU * hz * n as f32 / SR).sin() * amplitude;
                [v, v]
            })
            .collect()
    }

    fn peak(buffer: &[f32]) -> f32 {
        buffer.iter().fold(0.0f32, |a, s| a.max(s.abs()))
    }

    /// **The whole job.** Nothing may leave above the ceiling, ever.
    #[test]
    fn nothing_gets_past_the_ceiling() {
        let mut limiter = Limiter::new(SR);
        // Wildly over: four times full scale.
        let mut audio = sine(4.0, 220.0, 48_000);
        limiter.process(&mut audio);
        assert!(
            peak(&audio) <= CEILING + 1e-4,
            "peak {} exceeded the ceiling {CEILING}",
            peak(&audio)
        );
    }

    /// A transient must not escape through the attack. This is exactly what
    /// look-ahead buys and what a compressor would fail.
    #[test]
    fn a_sudden_spike_does_not_escape() {
        let mut limiter = Limiter::new(SR);
        let mut audio = vec![0.0f32; 4_800 * CHANNELS];
        // Silence, then an instant full-scale-times-eight spike.
        for frame in 1_000..1_010 {
            audio[frame * CHANNELS] = 8.0;
            audio[frame * CHANNELS + 1] = 8.0;
        }
        limiter.process(&mut audio);
        assert!(
            peak(&audio) <= CEILING + 1e-4,
            "a transient escaped at {}",
            peak(&audio)
        );
    }

    /// The spike test above found a real escape, but only because its spike
    /// happened to land on the alignment that exposed it. The bug was in
    /// bookkeeping that is a function of position in the look-ahead ring, so a
    /// single placement proves very little: slide the spike across every offset
    /// in the ring and the whole class is covered.
    #[test]
    fn a_spike_escapes_at_no_alignment_in_the_ring() {
        let lookahead = Limiter::new(SR).latency_frames();

        for offset in 0..lookahead {
            let mut limiter = Limiter::new(SR);
            let mut audio = vec![0.0f32; 4_800 * CHANNELS];
            let at = 1_000 + offset;
            audio[at * CHANNELS] = 8.0;
            audio[at * CHANNELS + 1] = -8.0;
            limiter.process(&mut audio);
            assert!(
                peak(&audio) <= CEILING + 1e-4,
                "a spike at offset {offset} escaped at {}",
                peak(&audio)
            );
        }
    }

    /// The engine hands the limiter whatever block size the device asks for,
    /// and that size is not a multiple of the look-ahead. State that survives
    /// across calls has to be correct at every split point.
    #[test]
    fn the_ceiling_holds_across_arbitrary_block_sizes() {
        for block in [1_usize, 7, 64, 240, 241, 511, 1_024] {
            let mut limiter = Limiter::new(SR);
            let mut audio = sine(3.0, 220.0, 24_000);
            for chunk in audio.chunks_mut(block * CHANNELS) {
                limiter.process(chunk);
            }
            assert!(
                peak(&audio) <= CEILING + 1e-4,
                "block size {block} let {} through",
                peak(&audio)
            );
        }
    }

    /// Material already below the ceiling must come out untouched apart from
    /// the delay. A limiter that colours quiet material is a broken limiter.
    #[test]
    fn quiet_material_passes_through_unchanged() {
        let mut limiter = Limiter::new(SR);
        let original = sine(0.3, 440.0, 48_000);
        let mut audio = original.clone();
        limiter.process(&mut audio);

        assert_eq!(limiter.reduction_db(), 0.0, "reduced a quiet signal");

        // Compare past the look-ahead delay: identical, sample for sample.
        let delay = limiter.latency_frames() * CHANNELS;
        for i in delay..original.len() {
            assert!(
                (audio[i] - original[i - delay]).abs() < 1e-6,
                "sample {i} changed: {} vs {}",
                audio[i],
                original[i - delay]
            );
        }
    }

    /// The delay is the price of look-ahead, and the rest of the application
    /// needs the number to compensate.
    #[test]
    fn the_latency_is_reported_and_is_the_lookahead() {
        let limiter = Limiter::new(SR);
        let expected = (SR * LOOKAHEAD_SECONDS) as usize;
        assert_eq!(limiter.latency_frames(), expected);
        // 5 ms at 48 kHz.
        assert_eq!(limiter.latency_frames(), 240);
    }

    /// A gain-reduction meter has to read something when the limiter works, and
    /// zero when it does not.
    #[test]
    fn reduction_is_reported_for_the_meter() {
        let mut limiter = Limiter::new(SR);
        assert_eq!(limiter.reduction_db(), 0.0);

        let mut audio = sine(2.0, 220.0, 9_600);
        limiter.process(&mut audio);
        let reduction = limiter.reduction_db();
        assert!(
            reduction > 3.0,
            "6 dB over should show real reduction, showed {reduction}"
        );
    }

    /// Gain must recover after the loud part, or one peak would duck the rest
    /// of the night.
    #[test]
    fn gain_recovers_after_a_loud_passage() {
        let mut limiter = Limiter::new(SR);

        let mut loud = sine(4.0, 220.0, 4_800);
        limiter.process(&mut loud);
        let during = limiter.reduction_db();
        assert!(during > 6.0, "expected reduction, got {during}");

        // A full second of quiet.
        let mut quiet = sine(0.2, 220.0, 48_000);
        limiter.process(&mut quiet);
        // Exactly zero, not merely close. A one-pole release never arrives on
        // its own; without the snap in `process_frame` the meter would sit at a
        // permanent fraction of a decibel and read as a stuck limiter.
        assert_eq!(
            limiter.reduction_db(),
            0.0,
            "gain never came fully back to rest"
        );
    }

    /// The running peak is an optimisation, so it must agree with a full scan.
    /// If it drifts, the limiter would either clip or duck for no reason.
    #[test]
    fn the_running_peak_tracks_a_falling_signal() {
        let mut limiter = Limiter::new(SR);
        // Loud burst then silence: the window peak must fall back to zero once
        // the burst has left the look-ahead window entirely.
        let mut audio = sine(3.0, 440.0, 2_400);
        limiter.process(&mut audio);

        let mut silence = vec![0.0f32; 48_000 * CHANNELS];
        limiter.process(&mut silence);
        assert!(
            limiter.window_peak < 1e-6,
            "window peak stuck at {}",
            limiter.window_peak
        );
        assert!(limiter.reduction_db() < 0.5);
    }

    /// Sustained bass is where a badly-tuned release audibly pumps.
    #[test]
    fn sustained_bass_is_not_modulated() {
        let mut limiter = Limiter::new(SR);
        let mut audio = sine(1.5, 50.0, 48_000);
        limiter.process(&mut audio);

        // Measure the envelope over the second half, once settled. A pumping
        // limiter shows large swings between cycles of the note.
        let tail = &audio[audio.len() / 2..];
        let windows: Vec<f32> = tail
            .chunks(2_400 * CHANNELS)
            .map(peak)
            .filter(|p| *p > 0.0)
            .collect();
        let smallest = windows.iter().cloned().fold(f32::INFINITY, f32::min);
        let largest = windows.iter().cloned().fold(0.0f32, f32::max);
        assert!(
            largest - smallest < 0.1,
            "envelope swings from {smallest} to {largest} -- the release is pumping"
        );
    }

    /// Bypass means bypass: the signal comes out as it went in, over the
    /// ceiling and all. Anything else would make the control a lie.
    #[test]
    fn bypass_lets_the_signal_through_untouched() {
        let mut limiter = Limiter::new(SR);
        limiter.set_bypass(true);

        let original = sine(2.0, 220.0, 24_000);
        let mut audio = original.clone();
        limiter.process(&mut audio);

        assert!(peak(&audio) > 1.9, "bypass reduced the signal");
        assert_eq!(limiter.reduction_db(), 0.0);

        let delay = limiter.latency_frames() * CHANNELS;
        for i in delay..original.len() {
            assert!(
                (audio[i] - original[i - delay]).abs() < 1e-6,
                "bypass changed sample {i}"
            );
        }
    }

    /// **The reason bypass keeps the delay.** If the latency changed with the
    /// switch, toggling it would shift the master against the headphone cue by
    /// 5 ms — during the one activity, beatmatching, that consists entirely of
    /// comparing those two.
    #[test]
    fn bypass_does_not_change_the_latency() {
        let mut limiter = Limiter::new(SR);
        let engaged = limiter.latency_frames();
        limiter.set_bypass(true);
        assert_eq!(limiter.latency_frames(), engaged);
    }

    /// Toggling mid-signal must not step the gain, or the switch clicks through
    /// a PA. The release slew is what makes it smooth, so check the output is
    /// continuous across the transition.
    #[test]
    fn leaving_bypass_and_returning_does_not_click() {
        let mut limiter = Limiter::new(SR);

        // Settle into real reduction first.
        let mut audio = sine(4.0, 220.0, 9_600);
        limiter.process(&mut audio);
        assert!(limiter.reduction_db() > 6.0);

        // Now bypass, and watch the output for a step.
        limiter.set_bypass(true);
        let mut after = sine(4.0, 220.0, 480);
        limiter.process(&mut after);

        let biggest_step = after
            .as_chunks::<CHANNELS>()
            .0
            .iter()
            .map(|f| f[0])
            .collect::<Vec<_>>()
            .windows(2)
            .fold(0.0f32, |worst, w| worst.max((w[1] - w[0]).abs()));

        // A 220 Hz sine at amplitude 4 moves at most ~0.115 per sample; a
        // gain step from 0.24 to 1.0 would show up as a jump many times that.
        assert!(
            biggest_step < 0.2,
            "output stepped by {biggest_step} -- the bypass switch clicks"
        );
    }

    #[test]
    fn reset_clears_everything() {
        let mut limiter = Limiter::new(SR);
        let mut audio = sine(4.0, 220.0, 4_800);
        limiter.process(&mut audio);
        assert!(limiter.reduction_db() > 0.0);

        limiter.reset();
        assert_eq!(limiter.reduction_db(), 0.0);
        assert!(limiter.delay.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn a_nonsense_sample_rate_still_produces_a_working_limiter() {
        for rate in [0.0, -48_000.0, f32::NAN] {
            let mut limiter = Limiter::new(rate);
            assert!(limiter.latency_frames() > 0);
            let mut audio = sine(4.0, 220.0, 4_800);
            limiter.process(&mut audio);
            assert!(peak(&audio) <= CEILING + 1e-4);
        }
    }

    #[test]
    fn an_empty_buffer_is_not_a_problem() {
        Limiter::new(SR).process(&mut []);
    }
}
