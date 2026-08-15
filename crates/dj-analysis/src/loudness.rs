//! Integrated loudness, to ITU-R BS.1770-4 / EBU R 128.
//!
//! This is what auto-gain is built on. Peak level is nearly useless for
//! matching two tracks — a heavily limited modern master and a dynamic 1970s
//! recording can share a peak and differ by 12 dB in how loud they sound — and
//! guessing at a gain per track is exactly the kind of chore a DJ should not be
//! doing during a set.
//!
//! # Why implement it rather than take a crate
//!
//! `libebur128` and its bindings are MPL/LGPL-adjacent territory and the
//! algorithm is fully specified in a public standard, so writing it is cheap
//! and keeps the licence story simple (see `docs/RESEARCH.md`).
//!
//! # Why this one can be *verified* rather than merely tested
//!
//! Most analysis has no ground truth. This does: EBU Tech 3341 specifies that a
//! 1 kHz sine at −23 dBFS in both channels must read **−23.0 LUFS ±0.1**. That
//! is a real conformance target, and [`tests`] asserts it. The famous −0.691 dB
//! offset in the formula exists precisely so that this case comes out round —
//! the K-weighting contributes about +0.691 dB at 1 kHz and the two cancel.

use dj_dsp::Biquad;

/// Block length for the gating measurement, in seconds.
const BLOCK_SECONDS: f64 = 0.400;
/// Blocks overlap by 75%, so a new one starts every 100 ms.
const BLOCK_STEP: f64 = 0.100;

/// Absolute gate. Silence and near-silence must not drag the average down.
const ABSOLUTE_GATE_LUFS: f64 = -70.0;
/// Relative gate, in LU below the ungated mean.
const RELATIVE_GATE_LU: f64 = -10.0;

/// The constant from BS.1770. Calibrates the K-weighting so a 1 kHz sine at
/// −23 dBFS reads −23 LUFS.
const OFFSET_DB: f64 = -0.691;

/// K-weighting: the analog prototype parameters BS.1770 specifies.
///
/// Given as filter parameters rather than as the standard's 48 kHz coefficient
/// table, so the same design works at 44.1 kHz and 96 kHz. Re-deriving from
/// these reproduces the published 48 kHz numbers.
const SHELF_HZ: f32 = 1_681.974_5;
const SHELF_GAIN_DB: f32 = 3.999_844;
const HIGHPASS_HZ: f32 = 38.135_47;
const HIGHPASS_Q: f32 = 0.500_327;

/// Loudness, in LUFS.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Lufs(f64);

impl Lufs {
    /// What a completely silent track reports.
    pub const SILENCE: Lufs = Lufs(f64::NEG_INFINITY);

    /// The broadcast reference, and a sensible target for a DJ library: every
    /// track trimmed to the same perceived loudness.
    pub const REFERENCE: f64 = -14.0;

    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }

    #[must_use]
    pub fn is_silent(self) -> bool {
        !self.0.is_finite()
    }

    /// Gain in decibels that would bring this track to `target`.
    ///
    /// Returns 0 for silence: there is no gain that makes silence louder, and
    /// returning infinity would blow up whatever used it.
    #[must_use]
    pub fn gain_to(self, target: f64) -> f64 {
        if self.is_silent() {
            0.0
        } else {
            target - self.0
        }
    }
}

/// Measure integrated loudness over interleaved stereo.
///
/// Returns [`Lufs::SILENCE`] for anything with no block above the absolute
/// gate, which includes silence and a track that is only a few milliseconds
/// long.
#[must_use]
pub fn integrated(samples: &[f32], sample_rate: u32) -> Lufs {
    if samples.len() < 2 || sample_rate == 0 {
        return Lufs::SILENCE;
    }
    let rate = sample_rate as f32;

    // K-weighting: a high shelf that lifts the treble, then a high-pass that
    // discards the rumble the ear does not weigh. Per channel, because filters
    // carry state.
    let mut filters: [[Biquad; 2]; 2] = [
        [
            Biquad::high_shelf(rate, SHELF_HZ, SHELF_GAIN_DB),
            Biquad::high_pass(rate, HIGHPASS_HZ, HIGHPASS_Q),
        ],
        [
            Biquad::high_shelf(rate, SHELF_HZ, SHELF_GAIN_DB),
            Biquad::high_pass(rate, HIGHPASS_HZ, HIGHPASS_Q),
        ],
    ];

    let frames = samples.len() / 2;
    let block_frames = (BLOCK_SECONDS * f64::from(sample_rate)).round() as usize;
    let step_frames = (BLOCK_STEP * f64::from(sample_rate)).round() as usize;
    if block_frames == 0 || step_frames == 0 || frames < block_frames {
        return Lufs::SILENCE;
    }

    // Weight every sample once, keeping the squares. The gating pass then only
    // has to sum ranges of this rather than re-filtering per block, which
    // matters because blocks overlap by 75%.
    let mut squares = vec![0.0f64; frames * 2];
    for frame in 0..frames {
        for channel in 0..2 {
            let raw = samples[frame * 2 + channel];
            let shelved = filters[channel][0].process(raw);
            let weighted = filters[channel][1].process(shelved);
            squares[frame * 2 + channel] = f64::from(weighted) * f64::from(weighted);
        }
    }

    // Loudness of each overlapping block.
    let mut blocks = Vec::with_capacity(frames / step_frames + 1);
    let mut start = 0;
    while start + block_frames <= frames {
        let mut sum = 0.0;
        for frame in start..start + block_frames {
            sum += squares[frame * 2] + squares[frame * 2 + 1];
        }
        // Mean square per channel, summed with G = 1.0 for left and right.
        let mean_square = sum / block_frames as f64;
        blocks.push(loudness_of(mean_square));
        start += step_frames;
    }

    gated_mean(&blocks, &squares, block_frames, step_frames)
}

fn loudness_of(mean_square: f64) -> f64 {
    if mean_square <= 0.0 {
        f64::NEG_INFINITY
    } else {
        OFFSET_DB + 10.0 * mean_square.log10()
    }
}

/// The two-stage gate: drop near-silence, then drop anything well below the
/// average of what is left.
///
/// The second stage is what stops a quiet intro from dragging the measurement
/// down and making the whole track come back too loud.
fn gated_mean(blocks: &[f64], squares: &[f64], block_frames: usize, step_frames: usize) -> Lufs {
    let mean_square_of = |index: usize| -> f64 {
        let start = index * step_frames;
        let mut sum = 0.0;
        for frame in start..start + block_frames {
            sum += squares[frame * 2] + squares[frame * 2 + 1];
        }
        sum / block_frames as f64
    };

    // First gate: absolute.
    let above_absolute: Vec<usize> = blocks
        .iter()
        .enumerate()
        .filter(|(_, l)| **l > ABSOLUTE_GATE_LUFS)
        .map(|(i, _)| i)
        .collect();
    if above_absolute.is_empty() {
        return Lufs::SILENCE;
    }

    let ungated_mean: f64 = above_absolute
        .iter()
        .map(|i| mean_square_of(*i))
        .sum::<f64>()
        / above_absolute.len() as f64;
    let relative_gate = loudness_of(ungated_mean) + RELATIVE_GATE_LU;

    // Second gate: relative to that mean.
    let kept: Vec<usize> = above_absolute
        .into_iter()
        .filter(|i| blocks[*i] > relative_gate)
        .collect();
    if kept.is_empty() {
        return Lufs::SILENCE;
    }

    let mean: f64 = kept.iter().map(|i| mean_square_of(*i)).sum::<f64>() / kept.len() as f64;
    Lufs::new(loudness_of(mean))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: u32 = 48_000;

    /// Stereo sine, both channels identical, at a given dBFS.
    fn sine(dbfs: f64, seconds: f64) -> Vec<f32> {
        let amplitude = 10.0_f64.powf(dbfs / 20.0) as f32;
        let frames = (seconds * f64::from(SR)) as usize;
        (0..frames)
            .flat_map(|n| {
                let v = (TAU * 1000.0 * n as f32 / SR as f32).sin() * amplitude;
                [v, v]
            })
            .collect()
    }

    /// **The conformance test.** EBU Tech 3341 specifies that a 1 kHz sine at
    /// −23 dBFS in both channels reads −23.0 LUFS ±0.1.
    ///
    /// This is the whole reason the −0.691 offset exists: the K-weighting
    /// contributes about +0.691 dB at 1 kHz, and the two cancel so the case
    /// comes out round. If the filters are wrong, this misses.
    #[test]
    fn a_1khz_sine_at_minus_23_dbfs_reads_minus_23_lufs() {
        let measured = integrated(&sine(-23.0, 5.0), SR).get();
        assert!(
            (measured - -23.0).abs() < 0.1,
            "EBU Tech 3341 requires -23.0 ±0.1 LUFS, measured {measured}"
        );
    }

    /// The same relationship has to hold elsewhere on the scale, or the filters
    /// happen to be right at one point by luck.
    #[test]
    fn the_scale_is_linear_in_decibels() {
        for level in [-40.0, -30.0, -23.0, -12.0, -6.0] {
            let measured = integrated(&sine(level, 4.0), SR).get();
            assert!(
                (measured - level).abs() < 0.2,
                "a {level} dBFS sine should read {level} LUFS, measured {measured}"
            );
        }
    }

    /// Halving the amplitude is −6 dB, and must move the measurement by −6 LU.
    #[test]
    fn halving_the_amplitude_costs_six_lu() {
        let loud = integrated(&sine(-12.0, 4.0), SR).get();
        let quiet = integrated(&sine(-18.0, 4.0), SR).get();
        assert!(
            ((loud - quiet) - 6.0).abs() < 0.1,
            "expected 6 LU between them, got {}",
            loud - quiet
        );
    }

    #[test]
    fn silence_is_reported_as_silence_rather_than_a_number() {
        let silence = vec![0.0f32; SR as usize * 2 * 3];
        assert!(integrated(&silence, SR).is_silent());
    }

    /// The relative gate exists for this: a quiet passage must not drag the
    /// measurement down and make the track come back too loud.
    #[test]
    fn a_quiet_intro_does_not_drag_the_measurement_down() {
        let mut track = vec![0.0f32; SR as usize * 2 * 4]; // 4 s of silence
        track.extend(sine(-14.0, 8.0));

        let measured = integrated(&track, SR).get();
        assert!(
            (measured - -14.0).abs() < 0.5,
            "the gate should ignore the silent intro; measured {measured}"
        );
    }

    /// What this is all for: bringing two tracks to the same perceived level.
    #[test]
    fn gain_to_reference_evens_two_tracks_out() {
        let loud = integrated(&sine(-8.0, 4.0), SR);
        let quiet = integrated(&sine(-26.0, 4.0), SR);

        let loud_gain = loud.gain_to(Lufs::REFERENCE);
        let quiet_gain = quiet.gain_to(Lufs::REFERENCE);

        assert!(loud_gain < 0.0, "a loud track should be turned down");
        assert!(quiet_gain > 0.0, "a quiet track should be turned up");
        // Applying each gain should land both at the reference.
        assert!((loud.get() + loud_gain - Lufs::REFERENCE).abs() < 0.01);
        assert!((quiet.get() + quiet_gain - Lufs::REFERENCE).abs() < 0.01);
    }

    /// There is no gain that makes silence louder, and returning infinity would
    /// blow up whatever used it.
    #[test]
    fn silence_asks_for_no_gain_rather_than_infinite_gain() {
        assert_eq!(Lufs::SILENCE.gain_to(Lufs::REFERENCE), 0.0);
    }

    #[test]
    fn something_shorter_than_one_block_is_not_guessed_at() {
        // 100 ms: less than the 400 ms the measurement needs.
        assert!(integrated(&sine(-20.0, 0.1), SR).is_silent());
        assert!(integrated(&[], SR).is_silent());
        assert!(integrated(&sine(-20.0, 1.0), 0).is_silent());
    }

    /// K-weighting is a frequency weighting, so it must actually weigh
    /// frequencies: bass counts for less than midrange at the same amplitude.
    #[test]
    fn low_frequencies_are_weighted_down() {
        let frames = SR as usize * 4;
        let tone = |hz: f32| -> Vec<f32> {
            (0..frames)
                .flat_map(|n| {
                    let v = (TAU * hz * n as f32 / SR as f32).sin() * 0.1;
                    [v, v]
                })
                .collect()
        };
        let bass = integrated(&tone(40.0), SR).get();
        let mid = integrated(&tone(1000.0), SR).get();
        assert!(
            bass < mid - 3.0,
            "40 Hz should weigh well under 1 kHz: {bass} vs {mid}"
        );
    }

    /// A track at 44.1 kHz must measure the same as one at 48 kHz, or a library
    /// of mixed rates would be levelled inconsistently.
    #[test]
    fn the_measurement_does_not_depend_on_the_sample_rate() {
        let at = |rate: u32| {
            let frames = (4.0 * f64::from(rate)) as usize;
            let samples: Vec<f32> = (0..frames)
                .flat_map(|n| {
                    let v = (TAU * 1000.0 * n as f32 / rate as f32).sin() * 0.0708;
                    [v, v]
                })
                .collect();
            integrated(&samples, rate).get()
        };
        let a = at(44_100);
        let b = at(48_000);
        assert!(
            (a - b).abs() < 0.15,
            "44.1k measured {a}, 48k measured {b} -- the filter design must \
             follow the rate"
        );
    }
}
