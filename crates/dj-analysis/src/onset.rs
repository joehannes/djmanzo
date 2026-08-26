//! Finding where things happen: the onset detection function.
//!
//! Everything about tempo starts here. The idea is old and simple: take a
//! short-time spectrum, and whenever energy *appears* in a bin that was quieter
//! a moment ago, something started. Summing that across the spectrum gives a
//! curve with a bump at every drum hit, and the tempo problem becomes "what is
//! the period of this curve" rather than "where are the drums".
//!
//! Half-wave rectified: only *increases* count. Energy dying away is not an
//! onset, and counting it would put a second bump after every hit.
//!
//! # Choices worth stating
//!
//! **Log magnitude.** A kick and a hi-hat differ by tens of decibels, and on a
//! linear scale the hi-hats simply vanish. Compressing first is what lets one
//! curve carry both.
//!
//! **512-sample hop.** About 11 ms at 44.1 kHz, so two hits a demisemiquaver
//! apart at 180 BPM are still separate samples of the curve. Smaller costs time
//! for resolution nothing downstream uses.

use rustfft::{FftPlanner, num_complex::Complex32};

/// Frames between successive spectra.
pub const HOP: usize = 512;
/// Window length. Four times the hop, the usual overlap for this.
pub const WINDOW: usize = 2048;

/// The onset curve, plus the rate it is sampled at.
#[derive(Debug, Clone)]
pub struct OnsetEnvelope {
    /// One value per hop. Non-negative.
    pub values: Vec<f32>,
    /// Hops per second — the sample rate of `values`.
    pub rate: f64,
}

impl OnsetEnvelope {
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Seconds covered.
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.values.len() as f64 / self.rate
    }

    /// Convert a position in the envelope to a frame in the original audio.
    #[must_use]
    pub fn frame_at(&self, index: f64) -> f64 {
        index * HOP as f64
    }
}

/// How many frequency bands [`detect_bands`] splits the flux into.
pub const BANDS: usize = 4;

/// The onset curve, kept in bands rather than summed across the spectrum.
///
/// Same computation as [`detect`] — it is the *same loop*, and `detect` sums
/// what this keeps. Worth keeping for structure analysis, where the question is
/// not "did something happen" but "did something *different* happen": a filter
/// opening and a snare entering are both onsets and are not the same event, and
/// once the spectrum is summed away nothing can tell them apart.
#[derive(Debug, Clone)]
pub struct BandedOnset {
    /// One vector per hop, low band first.
    pub values: Vec<[f32; BANDS]>,
    /// Hops per second.
    pub rate: f64,
}

/// Band edges in hertz, low to high.
///
/// Roughly kick, body, snare and upper percussion, air. Chosen by what carries
/// phrase information in dance music rather than by an equal division of
/// anything: the interesting distinction is between a track with its kick in
/// and one without, and equal-width bands would put that boundary in the middle
/// of a band.
const EDGES: [f32; BANDS - 1] = [150.0, 800.0, 4000.0];

/// The onset curve in bands. See [`BandedOnset`].
#[must_use]
pub fn detect_bands(samples: &[f32], sample_rate: u32) -> BandedOnset {
    let (_, banded) = analyse(samples, sample_rate);
    banded
}

/// Compute the onset envelope of interleaved stereo audio.
#[must_use]
pub fn detect(samples: &[f32], sample_rate: u32) -> OnsetEnvelope {
    let (summed, _) = analyse(samples, sample_rate);
    summed
}

/// One pass over the audio, producing both shapes.
///
/// Together rather than twice: the FFT is the expensive part, and computing it
/// once for the tempo curve and again for the banded one would double the cost
/// of analysing a track to produce two views of the same numbers.
fn analyse(samples: &[f32], sample_rate: u32) -> (OnsetEnvelope, BandedOnset) {
    let frames = samples.len() / 2;
    let rate = f64::from(sample_rate) / HOP as f64;
    if frames < WINDOW || sample_rate == 0 {
        return (
            OnsetEnvelope {
                values: Vec::new(),
                rate,
            },
            BandedOnset {
                values: Vec::new(),
                rate,
            },
        );
    }

    // Mono for analysis. A DJ's stereo image is not information about tempo,
    // and summing halves the work.
    let mono: Vec<f32> = (0..frames)
        .map(|f| (samples[f * 2] + samples[f * 2 + 1]) * 0.5)
        .collect();

    let window = hann(WINDOW);
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(WINDOW);

    let bins = WINDOW / 2 + 1;
    let mut previous = vec![0.0f32; bins];
    let mut scratch = vec![Complex32::new(0.0, 0.0); WINDOW];
    let mut values = Vec::with_capacity(mono.len() / HOP);
    let mut banded: Vec<[f32; BANDS]> = Vec::with_capacity(mono.len() / HOP);
    // Bin index of each band edge, worked out once.
    let hz_per_bin = f64::from(sample_rate) / WINDOW as f64;
    let edge_bins: [usize; BANDS - 1] = std::array::from_fn(|i| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            ((f64::from(EDGES[i]) / hz_per_bin) as usize).min(bins)
        }
    });

    let mut start = 0;
    while start + WINDOW <= mono.len() {
        for (i, slot) in scratch.iter_mut().enumerate() {
            *slot = Complex32::new(mono[start + i] * window[i], 0.0);
        }
        fft.process(&mut scratch);

        let mut flux = 0.0f32;
        let mut per_band = [0.0f32; BANDS];
        for bin in 0..bins {
            // Log magnitude: a kick and a hi-hat are tens of dB apart, and on a
            // linear scale the hi-hats disappear entirely.
            let magnitude = (1.0 + scratch[bin].norm()).ln();
            let rise = magnitude - previous[bin];
            // Half-wave rectified: energy dying away is not an onset, and
            // counting it would put a second bump after every hit.
            if rise > 0.0 {
                flux += rise;
                let band = edge_bins.iter().filter(|edge| bin >= **edge).count();
                per_band[band] += rise;
            }
            previous[bin] = magnitude;
        }
        values.push(flux);
        banded.push(per_band);
        start += HOP;
    }

    normalise(&mut values);
    // The banded curve is deliberately **not** normalised. `normalise` centres
    // on zero, which is right for the autocorrelation the tempo search does and
    // wrong here: structure analysis compares one beat's bands against
    // another's, and a band that has gone negative because the track got
    // quieter overall is not a band with less energy in it.
    (
        OnsetEnvelope { values, rate },
        BandedOnset {
            values: banded,
            rate,
        },
    )
}

/// Centre on zero and scale to unit deviation.
///
/// Autocorrelation of a curve with a large mean is dominated by the mean, which
/// would bury the periodicity that is the whole point.
fn normalise(values: &mut [f32]) {
    if values.is_empty() {
        return;
    }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    let variance =
        values.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / values.len() as f32;
    let deviation = variance.sqrt();
    if deviation <= f32::EPSILON {
        values.fill(0.0);
        return;
    }
    for value in values.iter_mut() {
        *value = (*value - mean) / deviation;
    }
}

fn hann(length: usize) -> Vec<f32> {
    (0..length)
        .map(|n| {
            let phase = std::f32::consts::TAU * n as f32 / length as f32;
            0.5 - 0.5 * phase.cos()
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: u32 = 44_100;

    /// A click track: a short burst of noise every `period_seconds`.
    pub(crate) fn clicks(bpm: f64, seconds: f64, rate: u32) -> Vec<f32> {
        let frames = (seconds * f64::from(rate)) as usize;
        let period = f64::from(rate) * 60.0 / bpm;
        let mut out = vec![0.0f32; frames * 2];
        let mut seed = 0x2545_F491_4F6C_DD1Du64;

        let mut beat = 0;
        loop {
            let at = (beat as f64 * period).round() as usize;
            if at >= frames {
                break;
            }
            // 12 ms of decaying noise: broadband, like a real drum hit.
            let length = (0.012 * f64::from(rate)) as usize;
            for i in 0..length.min(frames - at) {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let noise = (seed >> 40) as f32 / 8_388_608.0 - 1.0;
                let decay = 1.0 - (i as f32 / length as f32);
                let v = noise * decay * decay * 0.7;
                out[(at + i) * 2] = v;
                out[(at + i) * 2 + 1] = v;
            }
            beat += 1;
        }
        out
    }

    #[test]
    fn a_click_track_produces_peaks_at_the_clicks() {
        let envelope = detect(&clicks(120.0, 8.0, SR), SR);
        assert!(!envelope.is_empty());

        // 120 BPM is one click every 0.5 s; at ~86 hops/s that is ~43 hops.
        let expected_spacing = envelope.rate * 0.5;

        // The largest values should sit near multiples of that spacing.
        let mut peaks: Vec<usize> = Vec::new();
        for (i, value) in envelope.values.iter().enumerate() {
            if *value > 2.0
                && peaks
                    .last()
                    .is_none_or(|last| i - last > expected_spacing as usize / 2)
            {
                peaks.push(i);
            }
        }
        assert!(peaks.len() > 8, "found only {} peaks", peaks.len());

        for pair in peaks.windows(2) {
            let gap = (pair[1] - pair[0]) as f64;
            assert!(
                (gap - expected_spacing).abs() < expected_spacing * 0.25,
                "click spacing {gap} is not near {expected_spacing}"
            );
        }
    }

    /// Only increases count. A sustained tone that fades has no onsets after
    /// the first, and counting the decay would double every beat.
    #[test]
    fn a_decay_is_not_an_onset() {
        let frames = SR as usize * 3;
        let samples: Vec<f32> = (0..frames)
            .flat_map(|n| {
                let decay = 1.0 - (n as f32 / frames as f32);
                let v = (TAU * 440.0 * n as f32 / SR as f32).sin() * decay * 0.5;
                [v, v]
            })
            .collect();

        let envelope = detect(&samples, SR);
        // The attack is one bump at the very start; the rest should be quiet.
        let tail = &envelope.values[envelope.values.len() / 4..];
        let loud = tail.iter().filter(|v| **v > 2.0).count();
        assert!(loud < tail.len() / 50, "{loud} onsets found in a decay");
    }

    #[test]
    fn silence_produces_a_flat_envelope() {
        let envelope = detect(&vec![0.0f32; SR as usize * 2 * 3], SR);
        assert!(envelope.values.iter().all(|v| v.abs() < 1e-6));
    }

    #[test]
    fn something_shorter_than_a_window_is_not_guessed_at() {
        assert!(detect(&[0.0; 64], SR).is_empty());
        assert!(detect(&[], SR).is_empty());
        assert!(detect(&clicks(120.0, 2.0, SR), 0).is_empty());
    }

    #[test]
    fn the_envelope_reports_its_own_timebase() {
        let envelope = detect(&clicks(120.0, 4.0, SR), SR);
        assert!((envelope.rate - f64::from(SR) / HOP as f64).abs() < 1e-9);
        assert!((envelope.duration() - 4.0).abs() < 0.2);
        // And maps back to frames in the original audio.
        assert!((envelope.frame_at(10.0) - 5120.0).abs() < 1e-9);
    }
}
