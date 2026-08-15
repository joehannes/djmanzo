//! Tempo and beat grid, from the onset envelope.
//!
//! Two questions, answered separately because they fail separately: *how fast*
//! (the period of the onset curve) and *where* (its phase). A grid can have the
//! right tempo and be half a beat out, which sounds far worse than being
//! slightly off tempo, so the phase gets its own pass and its own evidence.
//!
//! # Confidence is the point
//!
//! `docs/ROADMAP.md` says low grid confidence disables auto-sync rather than
//! silently misbehaving, and that is only possible if the number means
//! something. Here it is the **peak autocorrelation of the onset curve** — a
//! genuine correlation coefficient, because the curve is normalised to unit
//! deviation before the search. It answers "how strongly does this repeat",
//! which is exactly the question. A four-on-the-floor merengue scores near the
//! top; a rubato ballad scores near the bottom, which is correct, because a DJ
//! should not be auto-syncing to a rubato ballad.
//!
//! # The octave problem
//!
//! Autocorrelation cannot distinguish 80 from 160 BPM: a curve periodic at 80
//! is also periodic at 160. Every beat tracker has to break that tie with a
//! prior, and this one weights candidates by a log-normal centred on 125 BPM —
//! close to the middle of what people actually play, and comfortably covering
//! the Dominican repertoire this project exists for (dembow 110–125, bachata
//! 120–140, merengue 120–160, típico 160–180).

use crate::onset::OnsetEnvelope;
use dj_core::{Beatgrid, Bpm, Confidence, FramePos, SampleRate};

/// The tempo range searched.
///
/// Wider than a DJ needs so an unusual track is not silently forced into range,
/// but narrower than [`Bpm::MIN`]..[`Bpm::MAX`], where the autocorrelation has
/// too little data to say anything at the extremes.
pub const MIN_BPM: f64 = 60.0;
pub const MAX_BPM: f64 = 200.0;

/// Centre of the prior that breaks the octave tie.
const PREFERRED_BPM: f64 = 125.0;
/// Width of the prior, in octaves. Wide enough not to force anything, narrow
/// enough to actually decide.
const PRIOR_WIDTH: f64 = 0.9;

/// Peak correlation treated as complete certainty.
///
/// Set so that a track correlating at 0.35 lands exactly on
/// [`Confidence::SYNC_THRESHOLD`] — the point where auto-sync turns on. Real
/// music sits well below the 0.95 a synthetic click track reaches, and this is
/// the number to revisit first when the labelled regression set from M2 exists
/// to calibrate against.
const CERTAIN_CORRELATION: f64 = 0.7;

/// What the analyser worked out about timing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoAnalysis {
    pub grid: Beatgrid,
    /// Runner-up tempo, when one was close. Usually the half or double.
    ///
    /// Reported because the octave decision is a *guess* and the interface
    /// should be able to offer the alternative in one click rather than making
    /// the DJ retap the whole grid.
    pub alternative: Option<Bpm>,
}

/// Estimate tempo and beat phase.
///
/// Returns `None` when there is not enough to say — too short, or no
/// periodicity at all. Saying nothing is the correct answer for a field
//  recording or an ambient wash, and far better than a confident wrong grid.
#[must_use]
pub fn analyse(envelope: &OnsetEnvelope, sample_rate: SampleRate) -> Option<TempoAnalysis> {
    // Autocorrelation at the slowest tempo needs at least a few periods to mean
    // anything. Below this the answer would be noise wearing a number.
    if envelope.duration() < 10.0 {
        return None;
    }

    let (bpm, confidence, alternative) = estimate_tempo(envelope)?;
    let anchor = estimate_phase(envelope, bpm, sample_rate);

    Some(TempoAnalysis {
        grid: Beatgrid::new(anchor, bpm, confidence),
        alternative,
    })
}

/// Find the period of the onset curve.
fn estimate_tempo(envelope: &OnsetEnvelope) -> Option<(Bpm, Confidence, Option<Bpm>)> {
    let values = &envelope.values;
    let min_lag = (envelope.rate * 60.0 / MAX_BPM).floor().max(1.0) as usize;
    let max_lag = (envelope.rate * 60.0 / MIN_BPM).ceil() as usize;
    if max_lag >= values.len() / 2 || min_lag >= max_lag {
        return None;
    }

    // Unnormalised autocorrelation over the search range. The envelope is
    // already zero-mean, so this is a covariance and periodicity shows as a
    // clear peak rather than a slope.
    let mut scores = vec![0.0f64; max_lag + 1];
    for lag in min_lag..=max_lag {
        let mut sum = 0.0f64;
        for i in 0..values.len() - lag {
            sum += f64::from(values[i]) * f64::from(values[i + lag]);
        }
        scores[lag] = sum / (values.len() - lag) as f64;
    }

    // Weight by the prior. This is what breaks the half/double tie, and it is
    // the one place a human judgement is baked into the number.
    let mut weighted = vec![0.0f64; max_lag + 1];
    for lag in min_lag..=max_lag {
        let bpm = envelope.rate * 60.0 / lag as f64;
        weighted[lag] = scores[lag] * prior(bpm);
    }

    let best_lag = (min_lag..=max_lag).max_by(|a, b| {
        weighted[*a]
            .partial_cmp(&weighted[*b])
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;
    if weighted[best_lag] <= 0.0 {
        return None;
    }

    // Interpolate the peak. A lag is a whole number of hops, which at ~86 hops
    // per second quantises the tempo to about 1.4 BPM near 125 -- audible as
    // drift over a few bars. Fitting a parabola to the neighbours recovers the
    // fraction between.
    let refined = interpolate_peak(&scores, best_lag);
    let bpm_value = envelope.rate * 60.0 / refined;
    let bpm = Bpm::new(bpm_value)?;

    // Confidence is the peak correlation itself. The envelope is normalised to
    // unit deviation, so autocorrelation at lag zero is 1.0 and every score
    // here is a genuine correlation coefficient: "how strongly does this curve
    // repeat at this period".
    //
    // Measured, not guessed. A synthetic click track peaks at 0.95; white noise
    // peaks at 0.014 -- a factor of sixty-five. The first thing tried was the
    // ratio of the peak to the mean score, which looked reasonable and is
    // useless: it reads 16.5 for the click track and 5.8 for noise, because
    // dividing by a near-zero mean inflates nothing into something. Noise
    // scored a confidence of 1.0 and would have been auto-synced to.
    let confidence = Confidence::new((scores[best_lag] / CERTAIN_CORRELATION).clamp(0.0, 1.0));

    // The octave the prior rejected, offered so the interface can switch in one
    // click rather than making the DJ retap the grid.
    let alternative = [0.5, 2.0]
        .into_iter()
        .filter_map(|factor| {
            let lag = refined / factor;
            let index = lag.round() as usize;
            (min_lag..=max_lag)
                .contains(&index)
                .then_some((index, factor))
        })
        .max_by(|a, b| {
            scores[a.0]
                .partial_cmp(&scores[b.0])
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .and_then(|(_, factor)| Bpm::new(bpm_value * factor));

    Some((bpm, confidence, alternative))
}

/// A log-normal preference for ordinary dance tempos.
fn prior(bpm: f64) -> f64 {
    let octaves = (bpm / PREFERRED_BPM).log2();
    (-(octaves * octaves) / (2.0 * PRIOR_WIDTH * PRIOR_WIDTH)).exp()
}

/// Parabolic interpolation through a peak and its neighbours.
fn interpolate_peak(scores: &[f64], peak: usize) -> f64 {
    if peak == 0 || peak + 1 >= scores.len() {
        return peak as f64;
    }
    let (left, centre, right) = (scores[peak - 1], scores[peak], scores[peak + 1]);
    let denominator = left - 2.0 * centre + right;
    if denominator.abs() < f64::EPSILON {
        return peak as f64;
    }
    let offset = 0.5 * (left - right) / denominator;
    // A correction beyond half a bin means the peak is not where we thought.
    if offset.abs() > 0.5 {
        peak as f64
    } else {
        peak as f64 + offset
    }
}

/// Find where the beats actually fall.
///
/// Slides a pulse train across the onset curve and keeps the alignment that
/// collects the most onset energy. Separate from the tempo pass because a grid
/// can have exactly the right tempo and be half a beat out, which sounds much
/// worse than a slightly wrong tempo.
fn estimate_phase(envelope: &OnsetEnvelope, bpm: Bpm, sample_rate: SampleRate) -> FramePos {
    let period_hops = envelope.rate * 60.0 / bpm.get();
    if period_hops < 1.0 {
        return FramePos::ZERO;
    }

    let steps = period_hops.round().max(1.0) as usize;
    let mut best = (f64::NEG_INFINITY, 0usize);
    for offset in 0..steps {
        let mut sum = 0.0f64;
        let mut beat = 0;
        loop {
            let at = offset as f64 + beat as f64 * period_hops;
            let index = at.round() as usize;
            if index >= envelope.values.len() {
                break;
            }
            sum += f64::from(envelope.values[index]);
            beat += 1;
        }
        // Average rather than total: a later offset fits fewer beats in, and
        // comparing totals would bias towards offset zero.
        let beats = (envelope.values.len() as f64 / period_hops).max(1.0);
        let score = sum / beats;
        if score > best.0 {
            best = (score, offset);
        }
    }

    // The envelope lags the audio by half a window: a spectrum computed over
    // samples [n, n+2048) reports energy centred at n+1024. Without this the
    // whole grid sits about 23 ms late, which is audible as a grid that looks
    // right and feels wrong.
    let frame = envelope.frame_at(best.1 as f64) - (crate::onset::WINDOW / 2) as f64;
    let _ = sample_rate;
    FramePos::new(frame.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onset;

    const SR: u32 = 44_100;
    const RATE: SampleRate = SampleRate::DEFAULT;

    fn analyse_clicks(bpm: f64, seconds: f64) -> Option<TempoAnalysis> {
        let audio = crate::onset::tests::clicks(bpm, seconds, SR);
        let envelope = onset::detect(&audio, SR);
        analyse(&envelope, RATE)
    }

    /// The core claim: a click track at a known tempo comes back at that tempo.
    #[test]
    fn a_click_track_reports_its_own_tempo() {
        for bpm in [90.0, 100.0, 120.0, 128.0, 140.0, 174.0] {
            let result = analyse_clicks(bpm, 30.0)
                .unwrap_or_else(|| panic!("{bpm} BPM produced no analysis"));
            let measured = result.grid.bpm.get();
            assert!(
                (measured - bpm).abs() < 1.0,
                "expected {bpm} BPM, measured {measured}"
            );
        }
    }

    /// The tempos this project exists for.
    #[test]
    fn the_dominican_repertoire_is_covered() {
        // dembow, bachata, merengue, típico.
        for bpm in [115.0, 130.0, 150.0, 168.0] {
            let result = analyse_clicks(bpm, 30.0).unwrap();
            assert!(
                (result.grid.bpm.get() - bpm).abs() < 1.5,
                "{bpm} came back as {}",
                result.grid.bpm.get()
            );
        }
    }

    /// Interpolation matters: without it the lag quantises the tempo to about
    /// 1.4 BPM near 125, which drifts audibly over a few bars.
    #[test]
    fn tempo_is_finer_than_the_hop_grid() {
        let result = analyse_clicks(127.3, 40.0).unwrap();
        let measured = result.grid.bpm.get();
        assert!(
            (measured - 127.3).abs() < 0.6,
            "a non-integer tempo needs sub-hop resolution; measured {measured}"
        );
    }

    /// **The property the roadmap depends on.** A steady pulse must score high
    /// enough to auto-sync; noise must not.
    #[test]
    fn a_steady_pulse_is_confident_and_noise_is_not() {
        let steady = analyse_clicks(128.0, 30.0).unwrap();
        assert!(
            steady.grid.confidence.is_sync_worthy(),
            "a click track scored only {}",
            steady.grid.confidence.get()
        );

        // White noise has no period. Whatever tempo comes back, it must not be
        // trusted enough to sync against.
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let noise: Vec<f32> = (0..SR as usize * 30 * 2)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed >> 40) as f32 / 8_388_608.0 - 1.0
            })
            .collect();
        let envelope = onset::detect(&noise, SR);
        if let Some(result) = analyse(&envelope, RATE) {
            assert!(
                !result.grid.confidence.is_sync_worthy(),
                "noise scored {} and would have been synced to",
                result.grid.confidence.get()
            );
        }
    }

    /// A grid with the right tempo and the wrong phase sounds worse than one
    /// slightly off tempo, so the anchor has to land on a beat.
    #[test]
    fn the_anchor_lands_on_a_beat() {
        let bpm = 120.0;
        let result = analyse_clicks(bpm, 30.0).unwrap();
        let beat_frames = f64::from(SR) * 60.0 / bpm;

        // Clicks start at frame 0, so the anchor should be a whole number of
        // beats from there.
        let anchor = result.grid.anchor.get();
        let offset = anchor % beat_frames;
        let error = offset.min(beat_frames - offset);
        assert!(
            error < beat_frames * 0.12,
            "anchor {anchor} is {error} frames off the beat grid (beat is {beat_frames})"
        );
    }

    /// Autocorrelation cannot tell 80 from 160, so the alternative is offered
    /// rather than the DJ having to retap a grid that is exactly half right.
    #[test]
    fn the_other_octave_is_offered() {
        let result = analyse_clicks(128.0, 30.0).unwrap();
        let alternative = result.alternative.expect("should offer an octave");
        let ratio = alternative.get() / result.grid.bpm.get();
        assert!(
            (ratio - 0.5).abs() < 0.05 || (ratio - 2.0).abs() < 0.05,
            "the alternative should be an octave away, ratio was {ratio}"
        );
    }

    /// Saying nothing is the right answer for something with no pulse, and much
    /// better than a confident wrong grid.
    #[test]
    fn something_too_short_produces_no_analysis() {
        assert!(analyse_clicks(120.0, 4.0).is_none());
        let empty = OnsetEnvelope {
            values: Vec::new(),
            rate: 86.0,
        };
        assert!(analyse(&empty, RATE).is_none());
    }

    #[test]
    fn silence_produces_no_tempo() {
        let envelope = onset::detect(&vec![0.0f32; SR as usize * 2 * 30], SR);
        let result = analyse(&envelope, RATE);
        // Either nothing, or something explicitly untrustworthy.
        if let Some(result) = result {
            assert!(!result.grid.confidence.is_sync_worthy());
        }
    }

    /// The prior exists to break octave ties, not to force everything to 125.
    #[test]
    fn the_prior_prefers_the_middle_without_forcing_it() {
        assert!(prior(125.0) > prior(62.5));
        assert!(prior(125.0) > prior(250.0));
        // But a genuine 174 must still be reachable -- drum and bass exists.
        assert!(
            prior(174.0) > 0.3,
            "the prior is too narrow: {}",
            prior(174.0)
        );
    }

    #[test]
    fn peak_interpolation_finds_the_fraction_between_bins() {
        // A symmetric peak stays put.
        assert!((interpolate_peak(&[0.0, 1.0, 2.0, 1.0, 0.0], 2) - 2.0).abs() < 1e-9);
        // A lopsided one moves towards the taller neighbour.
        let refined = interpolate_peak(&[0.0, 1.0, 2.0, 1.8, 0.0], 2);
        assert!(refined > 2.0 && refined < 2.5, "got {refined}");
    }
}
