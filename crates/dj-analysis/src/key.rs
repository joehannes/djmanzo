//! Key detection, reported on the Camelot wheel.
//!
//! Two steps. Build a **chroma vector** — how much energy the track spends on
//! each of the twelve pitch classes, with octaves folded together — and then
//! ask which of the twenty-four keys that distribution looks most like.
//!
//! The comparison uses the Krumhansl–Kessler profiles: the tonal hierarchies
//! measured experimentally in the 1980s by asking listeners how well each note
//! fitted a established key. They encode something real — the tonic and the
//! fifth dominate, the leading note is rare — and they are published data, not
//! anyone's code, so using them raises no licensing question at all.
//!
//! # What this will and will not get right
//!
//! It is good at music with a clear tonal centre, which is most of what a DJ
//! plays. It is weaker on tracks that modulate, on heavily percussive material
//! with little pitched content, and it has the classic relative-major/minor
//! ambiguity — C major and A minor share all seven notes, and are told apart
//! only by *emphasis*. The correlation score is reported so the interface can
//! show a weak result as weak rather than as fact.

use dj_core::{Mode, MusicalKey};
use rustfft::{FftPlanner, num_complex::Complex32};

/// Analysis window. Longer than the onset window: distinguishing adjacent
/// semitones in the bass needs frequency resolution, not time resolution.
const WINDOW: usize = 8192;
const HOP: usize = 4096;

/// Frequency range considered.
///
/// Below this is kick drum, which is pitched but not *harmonically* pitched and
/// swamps everything. Above it, the partials of a cymbal look like a chromatic
/// cluster and add nothing but noise.
const MIN_HZ: f32 = 65.0;
const MAX_HZ: f32 = 2_000.0;

/// Krumhansl–Kessler major profile, starting at C.
const MAJOR_PROFILE: [f64; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
/// Krumhansl–Kessler minor profile, starting at C.
const MINOR_PROFILE: [f64; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

/// A detected key and how much to believe it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyAnalysis {
    pub key: MusicalKey,
    /// Correlation with the winning profile, −1..=1. Above about 0.7 is a clear
    /// result; below 0.5 the track probably has no single key.
    pub correlation: f64,
    /// The runner-up, which for tonal music is usually the relative
    /// major/minor. Offered because that pair is genuinely ambiguous and a DJ
    /// can tell them apart by ear in a second.
    pub alternative: Option<MusicalKey>,
}

/// How much energy the track spends on each pitch class, C first.
#[must_use]
pub fn chroma(samples: &[f32], sample_rate: u32) -> [f64; 12] {
    let mut totals = [0.0f64; 12];
    let frames = samples.len() / 2;
    if frames < WINDOW || sample_rate == 0 {
        return totals;
    }

    let mono: Vec<f32> = (0..frames)
        .map(|f| (samples[f * 2] + samples[f * 2 + 1]) * 0.5)
        .collect();

    let window = hann(WINDOW);
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(WINDOW);
    let mut scratch = vec![Complex32::new(0.0, 0.0); WINDOW];

    let bin_hz = sample_rate as f32 / WINDOW as f32;
    let first_bin = (MIN_HZ / bin_hz).ceil() as usize;
    let last_bin = ((MAX_HZ / bin_hz).floor() as usize).min(WINDOW / 2);

    let mut start = 0;
    while start + WINDOW <= mono.len() {
        for (i, slot) in scratch.iter_mut().enumerate() {
            *slot = Complex32::new(mono[start + i] * window[i], 0.0);
        }
        fft.process(&mut scratch);

        for (offset, value) in scratch[first_bin..last_bin].iter().enumerate() {
            let magnitude = value.norm();
            if magnitude <= 0.0 {
                continue;
            }
            let hz = (first_bin + offset) as f32 * bin_hz;
            totals[pitch_class(hz)] += f64::from(magnitude);
        }
        start += HOP;
    }
    totals
}

/// Which of the twelve pitch classes a frequency belongs to, C = 0.
///
/// A440 as the reference, and MIDI note 69. Folding octaves together is the
/// whole point of chroma: a bass C and a piano C are the same harmonic fact.
fn pitch_class(hz: f32) -> usize {
    let midi = 69.0 + 12.0 * (hz / 440.0).log2();
    // MIDI 60 is middle C, so subtracting keeps C at index 0.
    let class = (midi.round() as i64 - 60).rem_euclid(12);
    class as usize
}

/// Work out the key.
///
/// Returns `None` for material with no pitched content to speak of.
#[must_use]
pub fn detect(samples: &[f32], sample_rate: u32) -> Option<KeyAnalysis> {
    let chroma = chroma(samples, sample_rate);
    from_chroma(&chroma)
}

/// The decision half, separated so it can be tested against a chroma vector
/// built by hand rather than only through an FFT.
#[must_use]
pub fn from_chroma(chroma: &[f64; 12]) -> Option<KeyAnalysis> {
    if chroma.iter().sum::<f64>() <= 0.0 {
        return None;
    }

    // Correlate against all 24 keys: 12 rotations of each profile.
    let mut scored: Vec<(f64, usize, Mode)> = Vec::with_capacity(24);
    for tonic in 0..12usize {
        for (profile, mode) in [(&MAJOR_PROFILE, Mode::Major), (&MINOR_PROFILE, Mode::Minor)] {
            let rotated: Vec<f64> = (0..12).map(|i| profile[(i + 12 - tonic) % 12]).collect();
            scored.push((correlation(chroma, &rotated), tonic, mode));
        }
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let (correlation, tonic, mode) = scored[0];
    let key = to_camelot(tonic, mode)?;
    let alternative = scored
        .get(1)
        .and_then(|(_, tonic, mode)| to_camelot(*tonic, *mode));

    Some(KeyAnalysis {
        key,
        correlation,
        alternative,
    })
}

/// Pearson correlation. Both vectors are centred first, so what is compared is
/// the *shape* of the distribution rather than how loud the track was.
fn correlation(a: &[f64; 12], b: &[f64]) -> f64 {
    let mean_a = a.iter().sum::<f64>() / 12.0;
    let mean_b = b.iter().sum::<f64>() / 12.0;

    let mut covariance = 0.0;
    let mut variance_a = 0.0;
    let mut variance_b = 0.0;
    for i in 0..12 {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        covariance += da * db;
        variance_a += da * da;
        variance_b += db * db;
    }
    let denominator = (variance_a * variance_b).sqrt();
    if denominator <= 0.0 {
        0.0
    } else {
        covariance / denominator
    }
}

/// Pitch class and mode to a Camelot wheel position.
///
/// The wheel is arranged in fifths, so a step of seven semitones is one hour.
/// C major is 8B and A minor is 8A by definition, which fixes the offset.
fn to_camelot(tonic: usize, mode: Mode) -> Option<MusicalKey> {
    // Distance round the circle of fifths from the mode's 8-o'clock anchor.
    let anchor = match mode {
        Mode::Major => 0, // C
        Mode::Minor => 9, // A
    };
    let steps = ((tonic as i64 - anchor) * 7).rem_euclid(12);
    let hour = ((8 + steps - 1).rem_euclid(12) + 1) as u8;
    MusicalKey::new(hour, mode)
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
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: u32 = 44_100;

    /// Frequency of a pitch class in a given octave. C4 = 261.63 Hz.
    fn hz(pitch_class: usize, octave: i32) -> f32 {
        let midi = 60 + pitch_class as i32 + (octave - 4) * 12;
        440.0 * 2.0f32.powf((midi as f32 - 69.0) / 12.0)
    }

    /// Play a set of pitch classes as sustained tones.
    fn chord(classes: &[usize], seconds: f64) -> Vec<f32> {
        let frames = (seconds * f64::from(SR)) as usize;
        (0..frames)
            .flat_map(|n| {
                let mut v = 0.0f32;
                for class in classes {
                    for octave in 3..=5 {
                        v += (TAU * hz(*class, octave) * n as f32 / SR as f32).sin();
                    }
                }
                let v = v / (classes.len() * 3) as f32 * 0.5;
                [v, v]
            })
            .collect()
    }

    // -- the Camelot mapping, which is pure arithmetic and must be exact ------

    /// The two anchors the whole wheel is defined by.
    #[test]
    fn c_major_is_8b_and_a_minor_is_8a() {
        assert_eq!(to_camelot(0, Mode::Major).unwrap().camelot(), "8B");
        assert_eq!(to_camelot(9, Mode::Minor).unwrap().camelot(), "8A");
    }

    /// A step round the wheel is a fifth. Getting this backwards would make
    /// every harmonic-mixing suggestion wrong in a way that still looks tidy.
    #[test]
    fn one_hour_is_one_fifth() {
        // C(8B) → G(9B) → D(10B) → A(11B) → E(12B) → B(1B)
        for (pitch_class, expected) in [(7, "9B"), (2, "10B"), (9, "11B"), (4, "12B"), (11, "1B")] {
            assert_eq!(
                to_camelot(pitch_class, Mode::Major).unwrap().camelot(),
                expected
            );
        }
        // A(8A) → E(9A) → B(10A)
        for (pitch_class, expected) in [(4, "9A"), (11, "10A")] {
            assert_eq!(
                to_camelot(pitch_class, Mode::Minor).unwrap().camelot(),
                expected
            );
        }
    }

    /// Every pitch class must land on a distinct hour, or two keys collide.
    #[test]
    fn all_twenty_four_keys_are_distinct() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for tonic in 0..12 {
            for mode in [Mode::Major, Mode::Minor] {
                let key = to_camelot(tonic, mode).unwrap();
                assert!(seen.insert(key.camelot()), "collision at {tonic} {mode:?}");
            }
        }
        assert_eq!(seen.len(), 24);
    }

    /// The Camelot mapping has to agree with the standard names dj-core prints,
    /// or the two notations would disagree in the interface.
    #[test]
    fn camelot_and_standard_notation_agree() {
        assert_eq!(to_camelot(0, Mode::Major).unwrap().standard(), "C");
        assert_eq!(to_camelot(9, Mode::Minor).unwrap().standard(), "Am");
        assert_eq!(to_camelot(7, Mode::Major).unwrap().standard(), "G");
        assert_eq!(to_camelot(4, Mode::Minor).unwrap().standard(), "Em");
    }

    // -- detection ----------------------------------------------------------

    /// A C major triad should read as C major.
    #[test]
    fn a_c_major_chord_reads_as_c_major() {
        let result = detect(&chord(&[0, 4, 7], 6.0), SR).expect("should detect something");
        assert_eq!(
            result.key.camelot(),
            "8B",
            "detected {} ({})",
            result.key.camelot(),
            result.key.standard()
        );
    }

    /// And a minor one as minor. This is the harder half: A minor shares every
    /// note with C major and differs only by emphasis.
    #[test]
    fn an_a_minor_chord_reads_as_a_minor() {
        let result = detect(&chord(&[9, 0, 4], 6.0), SR).expect("should detect something");
        assert_eq!(
            result.key.camelot(),
            "8A",
            "detected {}",
            result.key.standard()
        );
    }

    /// A full scale carries more evidence than a triad and should be at least
    /// as confident.
    #[test]
    fn a_c_major_scale_reads_as_c_major() {
        // C D E F G A B, weighted towards the tonic as real music is.
        let mut weighted = [0.0f64; 12];
        for (class, weight) in [
            (0, 5.0),
            (2, 2.0),
            (4, 3.0),
            (5, 2.0),
            (7, 4.0),
            (9, 2.0),
            (11, 1.5),
        ] {
            weighted[class] = weight;
        }
        let result = from_chroma(&weighted).unwrap();
        assert_eq!(result.key.camelot(), "8B");
        assert!(result.correlation > 0.7, "weak: {}", result.correlation);
    }

    /// The relative major/minor pair is genuinely ambiguous, so the runner-up
    /// should be the other one — a DJ can tell them apart by ear instantly.
    #[test]
    fn the_relative_key_is_offered_as_the_alternative() {
        let mut weighted = [0.0f64; 12];
        for (class, weight) in [
            (0, 5.0),
            (4, 3.0),
            (7, 4.0),
            (2, 2.0),
            (5, 2.0),
            (9, 2.5),
            (11, 1.5),
        ] {
            weighted[class] = weight;
        }
        let result = from_chroma(&weighted).unwrap();
        let alternative = result.alternative.expect("should offer one");
        // Relative keys share a Camelot hour and differ only by ring.
        assert_eq!(
            alternative.hour(),
            result.key.hour(),
            "the runner-up {} is not the relative of {}",
            alternative.camelot(),
            result.key.camelot()
        );
        assert_ne!(alternative.mode(), result.key.mode());
    }

    /// Transposing the music transposes the answer, by exactly one hour per
    /// fifth. This is the property that makes harmonic mixing work at all.
    #[test]
    fn transposing_the_input_moves_the_key_round_the_wheel() {
        let base = detect(&chord(&[0, 4, 7], 6.0), SR).unwrap();
        // Up a fifth: G major.
        let fifth = detect(&chord(&[7, 11, 2], 6.0), SR).unwrap();
        let expected = base.key.hour() % 12 + 1;
        assert_eq!(
            fifth.key.hour(),
            expected,
            "{} should be one hour above {}",
            fifth.key.camelot(),
            base.key.camelot()
        );
    }

    #[test]
    fn silence_has_no_key() {
        assert!(detect(&vec![0.0f32; SR as usize * 2 * 4], SR).is_none());
        assert!(from_chroma(&[0.0; 12]).is_none());
    }

    #[test]
    fn something_too_short_has_no_key() {
        assert!(detect(&[0.0; 128], SR).is_none());
        assert!(detect(&chord(&[0, 4, 7], 2.0), 0).is_none());
    }

    /// Chroma folds octaves together: the same note in two octaves is one
    /// harmonic fact, which is the entire premise.
    #[test]
    fn octaves_fold_onto_one_pitch_class() {
        assert_eq!(pitch_class(hz(0, 3)), pitch_class(hz(0, 5)));
        assert_eq!(pitch_class(hz(0, 4)), 0, "middle C should be class 0");
        assert_eq!(pitch_class(hz(9, 4)), 9, "A should be class 9");
        assert_eq!(pitch_class(440.0), 9, "A440 should be class 9");
    }

    /// Correlation compares shape, not level: making a track twice as loud must
    /// not change its key.
    #[test]
    fn loudness_does_not_affect_the_result() {
        let quiet = [1.0, 0.2, 0.4, 0.2, 0.6, 0.5, 0.2, 0.8, 0.2, 0.4, 0.2, 0.3];
        let loud: [f64; 12] = std::array::from_fn(|i| quiet[i] * 1000.0);
        let a = from_chroma(&quiet).unwrap();
        let b = from_chroma(&loud).unwrap();
        assert_eq!(a.key, b.key);
        assert!((a.correlation - b.correlation).abs() < 1e-9);
    }
}
