//! Phrases: the sixteen- and thirty-two-beat groups dance music is built from.
//!
//! A beat grid says where the beats are. It does not say which of them a DJ can
//! mix on, and those are not the same question: dropping a new track on beat 37
//! of a 32-beat phrase lands it three beats into the next one, and the result is
//! two tracks whose drums agree and whose *music* does not.
//!
//! What this finds is two numbers:
//!
//! - **How long a phrase is**, in beats. Usually 16 or 32; sometimes 8.
//! - **Which beat starts one.** A track's first downbeat is often not a phrase
//!   start — plenty of records open with a four- or eight-beat pickup.
//!
//! # How
//!
//! Phrase boundaries are where something *changes*: an element enters, a filter
//! opens, the drums drop out. So the signal to look for is novelty, measured
//! once per beat, and the question becomes which spacing of beats collects the
//! most of it.
//!
//! Per beat, the onset flux is summed into four frequency bands -- roughly
//! kick, body, snare and air -- giving a small vector. Novelty is the distance
//! between one beat's vector and the previous one's. Then for each candidate
//! phrase length, and each possible starting beat within it, the novelty
//! landing on those boundaries is scored.
//!
//! # The two traps
//!
//! **A threshold gets easier as phrases get longer.** A 32-beat candidate
//! averages a quarter as many beats as an 8-beat one, so on spiky novelty its
//! best phase reaches a given raw margin more easily — for being a smaller
//! sample, not a better answer. The score is therefore a **z-score** whose
//! denominator is the standard error, which carries the sample size, so
//! `MIN_CONFIDENCE` means the same thing at every length.
//!
//! Note what this does *not* do: it is not what chooses between lengths. That
//! is the midpoint rule below. The two jobs are easy to confuse, and an earlier
//! draft of this comment claimed the first did the second's work.
//!
//! **Beat boundaries do not land on hops.** A beat at 120 BPM is 43.07 hops
//! long, so every beat starts a little further into a hop than the last, and
//! the drift is itself periodic — around sixteen beats at that tempo, which is
//! squarely in the range being searched.
//!
//! What that does to a kick sitting on the downbeat is decide which beat it
//! belongs to. Weighting the boundary hop fractionally does not help: the hop's
//! energy is not uniformly spread inside it, and splitting it in proportion
//! puts part of every kick in the wrong beat, periodically. A metronome came
//! back with a sixteen-beat phrase at thirteen z that way.
//!
//! So each beat's span is **rounded to whole hops** — `round(start)` to
//! `round(end)` — and a transient on the downbeat then falls inside the same
//! beat every time, because the same rounding decided both where it is and
//! where the beat begins.
//!
//! That leaves spans of 43 or 44 hops, and a beat collecting one extra hop of
//! background collects a percent or two more energy — periodically, of course.
//! Two things make that harmless, and both are necessary:
//!
//! - a beat's feature is an **energy density**, divided by the length of its
//!   span, so a longer span is not a louder beat;
//! - novelty is **relative**: how much a beat differs from the one before it,
//!   as a fraction of the previous beat. A one-percent ripple then reads as a
//!   novelty of 0.01 whatever the track's absolute level, and
//!   [`MIN_CHANGE`] is a number one can reason about rather than a threshold
//!   tuned against a particular recording.
//!
//! **Significance is not prominence.** A z-score says how unlikely a pattern is
//! to be chance, and on a track whose beats are nearly identical a ripple of a
//! few percent is *wildly* unlikely — and musically nothing. Scored on z alone,
//! a metronome came back with a sixteen-beat phrase at thirteen z. So a
//! boundary also has to be **big**, which is what [`MIN_CHANGE`] asks and what
//! makes the relative novelty below worth its arithmetic.
//!
//! **A 16-beat track satisfies a 32-beat test.** Every boundary of a 32 is also
//! a boundary of a 16, so if the phrase really is 16, the 32 candidate lands on
//! real boundaries too and scores well. Length cannot be decided by boundary
//! strength alone. What separates them is the *midpoint*: for a genuine 32, the
//! beat halfway between two boundaries is unremarkable; for a 16 mislabelled as
//! a 32, it is another real boundary. So a length is only doubled when its
//! midpoints are measurably weaker than its boundaries.

use crate::onset::{BANDS, BandedOnset};
use dj_core::{Beatgrid, SampleRate};

/// The phrase structure of a track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhraseAnalysis {
    /// Phrase length in beats.
    pub beats: u32,
    /// The beat index, counted from the grid's anchor, on which a phrase
    /// starts. Always less than `beats`.
    pub anchor: u32,
    /// How much stronger the boundaries are than the rest of the track, as a
    /// z-score. Carried rather than thresholded away because a caller drawing
    /// markers wants to know how much to trust them.
    pub confidence: f32,
}

/// Candidate phrase lengths, shortest first.
///
/// Shortest first because the search promotes upward: 8 is tested, then 8 is
/// doubled to 16 only if the midpoints justify it. Starting from the longest
/// and working down would have to answer the same question in reverse and get
/// the same answer, with the accident above waiting at every step.
const CANDIDATES: &[u32] = &[8, 16, 32];

/// How many z above the track's own novelty a boundary set has to be.
///
/// Two. Low enough to find structure in a track that is mostly texture, high
/// enough that noise does not produce markers -- and markers a DJ cannot trust
/// are worse than none, because they will be mixed on.
const MIN_CONFIDENCE: f32 = 2.0;

/// How much a boundary beat must differ from the one before it.
///
/// Fifteen percent, and the percentage is meaningful because novelty is
/// relative — see the module documentation. This is the gate that separates a
/// phrase boundary from an arithmetic artefact: hop quantisation leaves a
/// ripple of one or two percent in any track, perfectly periodic and perfectly
/// inaudible, and `MIN_LIFT` alone cannot reject it because a ripple against a
/// near-silent baseline is a large *ratio*.
const MIN_CHANGE: f32 = 0.15;

/// How much weaker a midpoint has to be than a boundary before a length is
/// doubled.
///
/// A midpoint at 70% of the boundary strength is a real boundary being called a
/// midpoint, so the shorter length was right. Below that, doubling.
const MIDPOINT_RATIO: f32 = 0.7;

/// Find the phrase structure, if there is one.
///
/// `None` when the track is too short to have phrases, when the grid is
/// unusable, or when no candidate is convincing. **A `None` is a real answer**:
/// a live recording or an ambient piece may simply have no phrase structure,
/// and inventing one would put markers on a waveform that mean nothing.
#[must_use]
pub fn phrases(
    onset: &BandedOnset,
    grid: &Beatgrid,
    rate: SampleRate,
    frames: u64,
) -> Option<PhraseAnalysis> {
    let features = beat_features(onset, grid, rate, frames)?;
    let novelty = novelty(&features);
    // The longest candidate needs at least four phrases to say anything, and
    // fewer than that is arithmetic on noise.
    if novelty.len() < (CANDIDATES[0] * 4) as usize {
        return None;
    }

    let mut best: Option<PhraseAnalysis> = None;
    for &length in CANDIDATES {
        if novelty.len() < (length * 4) as usize {
            break;
        }
        let Some((anchor, confidence, change)) = best_phase(&novelty, length) else {
            continue;
        };
        if confidence < MIN_CONFIDENCE || change < MIN_CHANGE {
            continue;
        }
        match best {
            // Promote to the longer length only when the beats halfway between
            // its boundaries are measurably quieter than the boundaries. See
            // the module documentation.
            Some(shorter) if !midpoints_are_quiet(&novelty, length, anchor) => {
                best = Some(shorter);
            }
            _ => {
                best = Some(PhraseAnalysis {
                    beats: length,
                    anchor,
                    confidence,
                });
            }
        }
    }
    best
}

/// One small vector per beat: the onset flux summed in four bands.
///
/// Each beat's span is rounded to whole hops at both ends, which is what keeps
/// a transient on the downbeat inside the beat it belongs to. See the module
/// documentation -- getting this wrong invents phrases in a metronome.
pub(crate) fn beat_features(
    onset: &BandedOnset,
    grid: &Beatgrid,
    rate: SampleRate,
    frames: u64,
) -> Option<Vec<[f32; BANDS]>> {
    if onset.values.is_empty() || onset.rate <= 0.0 {
        return None;
    }
    let first = grid.beat_index_at(dj_core::FramePos::new(0.0), rate);
    #[allow(clippy::cast_precision_loss)]
    let last = grid.beat_index_at(dj_core::FramePos::new(frames as f64), rate);
    if last <= first {
        return None;
    }

    let hops = onset.values.len();
    let mut out = Vec::with_capacity((last - first) as usize);
    for index in first..last {
        let start = grid.beat_position(index, rate).get() / rate.as_f64() * onset.rate;
        let end = grid.beat_position(index + 1, rate).get() / rate.as_f64() * onset.rate;
        if end <= start {
            return None;
        }
        #[allow(clippy::cast_precision_loss)]
        if start >= hops as f64 {
            break;
        }
        let mut bands = [0.0f32; BANDS];
        let hops_used = accumulate(&onset.values, start.max(0.0), end, &mut bands);
        if hops_used == 0 {
            break;
        }
        // Density, not total. A beat whose span rounded to one hop more than
        // its neighbour's is not a louder beat, and treating it as one leaves a
        // periodic ripple exactly where phrases are looked for.
        #[allow(clippy::cast_precision_loss)]
        let span = hops_used as f32;
        for band in &mut bands {
            *band /= span;
        }
        out.push(bands);
    }
    (out.len() >= 8).then_some(out)
}

/// Sum whole hops from `from` to `to` into `into`, rounding both ends.
///
/// Rounded rather than split. See the module documentation: the energy inside a
/// hop is not spread evenly through it, so weighting a boundary hop in
/// proportion puts part of a downbeat's transient into the previous beat — and
/// which part, periodically. Rounding the boundary instead keeps a transient
/// and the beat it belongs to on the same side of the same decision.
fn accumulate(values: &[[f32; BANDS]], from: f64, to: f64, into: &mut [f32; BANDS]) -> usize {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let first = (from.round().max(0.0) as usize).min(values.len());
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let last = (to.round().max(0.0) as usize).min(values.len());
    let span = &values[first..last.max(first)];
    for hop in span {
        for (slot, value) in into.iter_mut().zip(hop.iter()) {
            *slot += value;
        }
    }
    span.len()
}

/// How different each beat is from the one before it, **as a fraction of it**.
///
/// Relative rather than absolute, and that is what makes `MIN_CHANGE` a number
/// with a meaning: 0.2 is "a fifth more energy arrived than the last beat had",
/// on any track at any level. An absolute distance would need a threshold tuned
/// to one recording's loudness, and would call the quantisation ripple in a
/// quiet track a phrase boundary while missing a real one in a loud track.
fn novelty(features: &[[f32; BANDS]]) -> Vec<f32> {
    if features.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(features.len());
    // The first beat has nothing before it. Zero rather than dropped, so beat
    // indices in the result still line up with beat indices in the grid -- an
    // off-by-one here would put every marker one beat early.
    out.push(0.0);
    for pair in features.windows(2) {
        let arrived: f32 = pair[0]
            .iter()
            .zip(pair[1].iter())
            // Half-wave rectified, like the onset curve it came from: a phrase
            // boundary is where something *arrives*. Energy dying away is the
            // end of the last phrase, not the start of the next.
            .map(|(before, after)| (after - before).max(0.0).powi(2))
            .sum();
        let previous: f32 = pair[0].iter().map(|band| band.powi(2)).sum();
        // The epsilon is not defensive: a beat of silence followed by anything
        // is an infinite relative change, and a track that starts from nothing
        // would otherwise report its first sound as the strongest boundary it
        // will ever have.
        out.push(arrived.sqrt() / (previous.sqrt() + f32::EPSILON.max(1e-6)));
    }
    out
}

/// The starting beat that collects the most novelty, how unlikely that pattern
/// is, and how much the boundary beats actually change.
///
/// Two numbers because they answer different questions and a boundary must pass
/// both. The z-score asks whether the pattern could be chance. The change asks
/// whether anything audible happens at all -- which z cannot, because a tiny
/// ripple against a near-uniform track is wildly unlikely and inaudible.
///
/// A third gate lived here for a while: a *lift*, the ratio of boundary novelty
/// to the track's average. It was removed because no test could be built that
/// it passed and the two above failed. Keeping an unearned gate is worse than
/// having one fewer -- it looks like a safeguard and is a coincidence.
fn best_phase(novelty: &[f32], length: u32) -> Option<(u32, f32, f32)> {
    let total: f32 = novelty.iter().sum();
    let count = novelty.len() as f32;
    let mean = total / count;
    let variance = novelty.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / count;
    let deviation = variance.sqrt();
    if deviation <= f32::EPSILON {
        // A perfectly flat track has no boundaries to find, and dividing by
        // this would report infinite confidence in an arbitrary phase.
        return None;
    }

    if mean <= f32::EPSILON {
        return None;
    }
    let mut best: Option<(u32, f32, f32)> = None;
    for phase in 0..length {
        let hits: Vec<f32> = novelty
            .iter()
            .skip(phase as usize)
            .step_by(length as usize)
            .copied()
            .collect();
        if hits.len() < 4 {
            continue;
        }
        let hit_mean = hits.iter().sum::<f32>() / hits.len() as f32;
        // The standard error, not the standard deviation: it carries the
        // sample size, so `MIN_CONFIDENCE` is as hard to clear for a 32-beat
        // candidate averaging a handful of beats as for an 8-beat one
        // averaging four times as many.
        let error = deviation / (hits.len() as f32).sqrt();
        let z = (hit_mean - mean) / error;
        if best.is_none_or(|(_, previous, _)| z > previous) {
            best = Some((phase, z, hit_mean));
        }
    }
    best
}

/// Whether the beats halfway between this length's boundaries are quiet enough
/// for the length to be believed.
fn midpoints_are_quiet(novelty: &[f32], length: u32, anchor: u32) -> bool {
    let half = length / 2;
    let at = |offset: u32| -> f32 {
        let hits: Vec<f32> = novelty
            .iter()
            .skip(((anchor + offset) % length) as usize)
            .step_by(length as usize)
            .copied()
            .collect();
        if hits.is_empty() {
            return 0.0;
        }
        hits.iter().sum::<f32>() / hits.len() as f32
    };
    let boundary = at(0);
    if boundary <= f32::EPSILON {
        return false;
    }
    at(half) / boundary < MIDPOINT_RATIO
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::{Bpm, Confidence, FramePos};

    const RATE: u32 = 44_100;
    const BPM: f64 = 120.0;

    /// Hops per beat, computed the way the grid computes it.
    ///
    /// **Not `hops / beats`.** That divides a truncated hop count by the beat
    /// count and lands a fraction of a hop away from where the grid actually
    /// puts each beat -- a drift that accumulates over a few hundred beats
    /// until a fixture's kick falls into the neighbouring beat, periodically.
    /// Two of the tests below were measuring that drift and calling it a
    /// phrase.
    fn hops_per_beat() -> f64 {
        f64::from(RATE) * 60.0 / BPM / crate::onset::HOP as f64
    }

    fn grid() -> Beatgrid {
        Beatgrid::new(
            FramePos::new(0.0),
            Bpm::new(BPM).unwrap(),
            Confidence::new(0.9),
        )
    }

    /// A track built to order: `beats` beats long, with a burst in the upper
    /// bands on every beat whose index satisfies the pattern.
    ///
    /// Synthetic rather than recorded because the point is to know the answer.
    /// A real track's phrasing is a matter of opinion in places; this one's is
    /// arithmetic, so a wrong answer is unambiguous.
    ///
    /// # Why the shapes are smooth
    ///
    /// The first version placed the kick with `within < 0.15`, which is a
    /// truncation: a beat is 43.06 hops at this tempo, so that gave six hops of
    /// kick on some beats and seven on others, in a pattern repeating every
    /// sixteen beats. The "featureless" metronome therefore had a real
    /// sixteen-beat structure in it, the detector found it, and the test that
    /// was supposed to prove no false positives was manufacturing one.
    ///
    /// Everything here is now a smooth function of fractional beat phase and
    /// spans enough hops to be sampled properly, so every beat carries the same
    /// energy however it falls between hops.
    fn track(beats: usize, boundary: impl Fn(usize) -> bool) -> (BandedOnset, u64) {
        let hop_rate = f64::from(RATE) / crate::onset::HOP as f64;
        let seconds = beats as f64 * 60.0 / BPM;
        let hops = (seconds * hop_rate) as usize;
        let per_beat = hops_per_beat();

        /// A raised cosine over the first `width` of the beat: 1 at the
        /// downbeat, falling to 0. Wide enough that a beat's total does not
        /// depend on where its hops land.
        fn pulse(phase: f64, width: f64) -> f32 {
            if phase >= width {
                return 0.0;
            }
            (0.5 * (1.0 + (std::f64::consts::PI * phase / width).cos())) as f32
        }

        let mut values = vec![[0.0f32; crate::onset::BANDS]; hops];
        for (index, hop) in values.iter_mut().enumerate() {
            let exact = index as f64 / per_beat;
            let beat = exact.floor() as usize;
            let phase = exact - exact.floor();
            // Every beat gets a kick in the low band, so the track has a pulse
            // whether or not it has phrases.
            hop[0] = 0.05 + pulse(phase, 0.3);
            // A boundary beat brings in the upper bands -- an element entering,
            // which is what a phrase boundary actually sounds like, and which a
            // detector reading only loudness could not tell from a louder kick.
            if boundary(beat) {
                hop[2] = 2.0 * pulse(phase, 0.5);
                hop[3] = 1.0 * pulse(phase, 0.5);
            }
        }
        let frames = (seconds * f64::from(RATE)) as u64;
        (
            BandedOnset {
                values,
                rate: hop_rate,
            },
            frames,
        )
    }

    fn analyse(onset: &BandedOnset, frames: u64) -> Option<PhraseAnalysis> {
        phrases(onset, &grid(), SampleRate::new(RATE).unwrap(), frames)
    }

    /// **A sixteen-beat phrase is found, and its starting beat with it.**
    #[test]
    fn a_sixteen_beat_phrase_is_found_where_it_starts() {
        let (onset, frames) = track(16 * 12, |beat| beat.is_multiple_of(16));
        let found = analyse(&onset, frames).expect("a phrase structure");
        assert_eq!(found.beats, 16, "the phrase length was misread");
        assert_eq!(found.anchor, 0, "the phrase start was misread");
    }

    /// **A phrase that does not start on beat zero is still found.**
    ///
    /// Plenty of records open with a four- or eight-beat pickup, so the first
    /// downbeat is not the first phrase. A detector that assumed otherwise
    /// would put every marker on the wrong beat for those tracks, which is
    /// worse than putting none: a DJ would mix on them.
    #[test]
    fn a_phrase_offset_from_the_first_beat_is_found() {
        let (onset, frames) = track(16 * 12, |beat| beat % 16 == 5);
        let found = analyse(&onset, frames).expect("a phrase structure");
        assert_eq!(found.beats, 16);
        assert_eq!(found.anchor, 5, "the offset phrase start was misread");
    }

    /// **Thirty-two is not reported for a sixteen-beat track.**
    ///
    /// The trap the whole midpoint rule exists for: every boundary of a 32 is
    /// also a boundary of a 16, so a 32-beat candidate lands on real boundaries
    /// and scores well on a track whose phrases are 16. Told apart only by what
    /// is halfway between.
    #[test]
    fn a_sixteen_beat_track_is_not_called_thirty_two() {
        let (onset, frames) = track(32 * 10, |beat| beat.is_multiple_of(16));
        let found = analyse(&onset, frames).expect("a phrase structure");
        assert_eq!(
            found.beats, 16,
            "a 16-beat track was reported as 32, so every other marker is wrong"
        );
    }

    /// **And a genuine thirty-two is not shortened to sixteen.**
    ///
    /// The other direction of the same rule. A detector that always answered 16
    /// would pass the test above and be no use.
    #[test]
    fn a_thirty_two_beat_track_is_reported_as_thirty_two() {
        let (onset, frames) = track(32 * 10, |beat| beat.is_multiple_of(32));
        let found = analyse(&onset, frames).expect("a phrase structure");
        assert_eq!(found.beats, 32, "a 32-beat track was reported as 16");
        assert_eq!(found.anchor % 32, 0);
    }

    /// **A track with no phrase structure gets no markers.**
    ///
    /// A live recording or an ambient piece may genuinely have none, and
    /// inventing one puts marks on a waveform that mean nothing -- which a DJ
    /// will then mix on. `None` is a real answer.
    #[test]
    fn a_track_without_phrases_reports_none() {
        let (onset, frames) = track(16 * 12, |_| false);
        let found = analyse(&onset, frames);
        assert!(
            found.is_none(),
            "a featureless track was given a phrase structure: {found:?}"
        );
    }

    /// **A sharp metronome is not a phrase structure either.**
    ///
    /// The other fixture is smooth, which is fair to the detector and unfair to
    /// the test: a real kick drum is a transient of a few milliseconds, far
    /// shorter than a hop. Summed in whole hops, a beat's energy then depends
    /// on where the beat falls between two hops, and that dependence is
    /// periodic -- so a plain drum machine would be given a phrase structure.
    /// Fractional edges are what prevent it, and only a sharp fixture shows it.
    #[test]
    fn a_sharp_metronome_is_not_given_a_phrase_structure() {
        let hop_rate = f64::from(RATE) / crate::onset::HOP as f64;
        let beats = 16 * 12;
        let seconds = f64::from(beats) * 60.0 / BPM;
        let hops = (seconds * hop_rate) as usize;
        let per_beat = hops_per_beat();

        // Built beat by beat rather than hop by hop, which matters: a hop-wise
        // rule with a window narrower than a hop gives a kick to *some* beats
        // and not others, which is a structure and not a metronome. An earlier
        // version of this test did exactly that and was measuring its own
        // fixture.
        //
        // The shape is what a real onset envelope makes of a transient: the
        // analysis window is four hops long, so a kick of a few milliseconds
        // appears smeared across about that many, at the same size every time.
        let mut values = vec![[0.0f32; crate::onset::BANDS]; hops];
        for beat in 0..beats {
            let start = f64::from(beat) * per_beat;
            for offset in 0..4u32 {
                let index = (start.round() as usize) + offset as usize;
                if let Some(hop) = values.get_mut(index) {
                    hop[0] += 20.0 * (1.0 - f64::from(offset) / 4.0) as f32;
                }
            }
        }
        let onset = BandedOnset {
            values,
            rate: hop_rate,
        };
        let found = analyse(&onset, (seconds * f64::from(RATE)) as u64);
        assert!(
            found.is_none(),
            "a drum machine with one identical kick per beat was given a phrase structure: {found:?}"
        );
    }

    /// **A pattern that is unmistakable and inaudible is not a structure.**
    ///
    /// The case the effect-size gate exists for, and the reason a z-score
    /// cannot be the only criterion. On a track whose beats are nearly
    /// identical, a periodic ripple of a few percent is enormously unlikely to
    /// be chance -- a huge z -- and nobody would ever hear it. Markers there are
    /// markers a DJ mixes on.
    #[test]
    fn a_pattern_too_faint_to_hear_is_not_reported_however_regular() {
        let hop_rate = f64::from(RATE) / crate::onset::HOP as f64;
        let beats = 16 * 12;
        let seconds = f64::from(beats) * 60.0 / BPM;
        let hops = (seconds * hop_rate) as usize;
        let per_beat = hops_per_beat();

        let mut values = vec![[0.0f32; crate::onset::BANDS]; hops];
        for (index, hop) in values.iter_mut().enumerate() {
            let exact = index as f64 / per_beat;
            let beat = exact.floor() as usize;
            let phase = exact - exact.floor();
            let shape = if phase < 0.3 {
                (0.5 * (1.0 + (std::f64::consts::PI * phase / 0.3).cos())) as f32
            } else {
                0.0
            };
            // Perfectly regular, every sixteenth beat, and three percent.
            hop[0] = if beat.is_multiple_of(16) {
                shape * 1.03
            } else {
                shape
            };
        }
        let onset = BandedOnset {
            values,
            rate: hop_rate,
        };
        let found = analyse(&onset, (seconds * f64::from(RATE)) as u64);
        assert!(
            found.is_none(),
            "a three-percent ripple was reported as a phrase structure: {found:?}"
        );
    }

    /// **A track where every beat is a surprise has no phrase boundaries.**
    ///
    /// The case `MIN_LIFT` exists for, and the one `MIN_CHANGE` cannot catch.
    /// Dense, restless percussion changes by a lot every beat, so every
    /// candidate phase clears "did anything audible happen" — and none of them
    /// stands out from the others, which is what having no phrase structure
    /// means. Without the lift gate the detector would pick whichever phase won
    /// by noise and mark it, confidently.
    #[test]
    fn a_track_that_changes_every_beat_has_no_boundaries() {
        let hop_rate = f64::from(RATE) / crate::onset::HOP as f64;
        let beats = 16 * 12;
        let seconds = f64::from(beats) * 60.0 / BPM;
        let hops = (seconds * hop_rate) as usize;
        let per_beat = hops_per_beat();

        // A fixed, reproducible pattern rather than a random one: a test that
        // fails one run in twenty is a test that gets deleted.
        let restless = |beat: usize| -> f32 {
            let scrambled = (beat.wrapping_mul(2_654_435_761)) % 1000;
            #[allow(clippy::cast_precision_loss)]
            {
                0.4 + scrambled as f32 / 1000.0
            }
        };

        let mut values = vec![[0.0f32; crate::onset::BANDS]; hops];
        for (index, hop) in values.iter_mut().enumerate() {
            let exact = index as f64 / per_beat;
            let beat = exact.floor() as usize;
            let phase = exact - exact.floor();
            let shape = if phase < 0.3 {
                (0.5 * (1.0 + (std::f64::consts::PI * phase / 0.3).cos())) as f32
            } else {
                0.0
            };
            hop[0] = shape * restless(beat);
            hop[2] = shape * restless(beat + 7);
        }
        let onset = BandedOnset {
            values,
            rate: hop_rate,
        };
        let found = analyse(&onset, (seconds * f64::from(RATE)) as u64);
        assert!(
            found.is_none(),
            "a track that changes on every beat was given a phrase structure: {found:?}"
        );
    }

    /// **A phrase marked by a break is still found, and in the right place.**
    ///
    /// Half the phrase boundaries in dance music are things stopping, not
    /// starting: the drums drop out for eight bars. Novelty here is half-wave
    /// rectified — only arrivals count — so a break produces nothing at its
    /// start and a burst at its *end*. That is not a bug and it is worth
    /// pinning: the end of a break is itself a phrase boundary, so the anchor
    /// comes out the same either way.
    #[test]
    fn a_phrase_marked_by_a_break_is_found_at_the_right_beat() {
        let hop_rate = f64::from(RATE) / crate::onset::HOP as f64;
        let beats = 16 * 12;
        let seconds = f64::from(beats) * 60.0 / BPM;
        let hops = (seconds * hop_rate) as usize;
        let per_beat = hops_per_beat();

        let mut values = vec![[0.0f32; crate::onset::BANDS]; hops];
        for (index, hop) in values.iter_mut().enumerate() {
            let exact = index as f64 / per_beat;
            let beat = exact.floor() as usize;
            let phase = exact - exact.floor();
            let shape = if phase < 0.3 {
                (0.5 * (1.0 + (std::f64::consts::PI * phase / 0.3).cos())) as f32
            } else {
                0.0
            };
            // Sixteen beats of full band, then sixteen with the drums gone.
            let playing = (beat / 16).is_multiple_of(2);
            hop[0] = shape * if playing { 1.0 } else { 0.1 };
            hop[2] = shape * if playing { 2.0 } else { 0.0 };
        }
        let onset = BandedOnset {
            values,
            rate: hop_rate,
        };
        let found = analyse(&onset, (seconds * f64::from(RATE)) as u64)
            .expect("a break every sixteen beats is a phrase structure");
        assert_eq!(
            found.anchor % 16,
            0,
            "the phrase was anchored at beat {}, which is not where the drums \
             come back",
            found.anchor
        );
    }

    // ---------------------------------------------------------------- units
    //
    // The tests above drive the whole detector, and a detector has enough
    // slack that a wrong helper can still give right answers on a synthetic
    // track. These pin the helpers directly, which is how the two properties
    // the thresholds *depend on* get to be facts rather than intentions.

    /// **Novelty is relative**, which is what makes `MIN_CHANGE` a percentage.
    ///
    /// A beat carrying half again the energy of the last one is a novelty of
    /// 0.5 whether the track is loud or quiet. If this were an absolute
    /// distance, `MIN_CHANGE` would be an energy in arbitrary units, and the
    /// same 15% would reject a whole quiet record and accept nothing in a loud
    /// one.
    #[test]
    fn novelty_is_a_fraction_of_the_previous_beat_not_a_distance() {
        let quiet = novelty(&[[1.0, 0.0, 0.0, 0.0], [1.5, 0.0, 0.0, 0.0]]);
        let loud = novelty(&[[100.0, 0.0, 0.0, 0.0], [150.0, 0.0, 0.0, 0.0]]);
        assert!(
            (quiet[1] - 0.5).abs() < 0.01,
            "half again as much energy read as {}",
            quiet[1]
        );
        assert!(
            (quiet[1] - loud[1]).abs() < 0.01,
            "the same relative change read as {} quietly and {} loudly",
            quiet[1],
            loud[1]
        );
    }

    /// **A beat's feature is a density**, so a span one hop longer is not a
    /// louder beat.
    ///
    /// Beat spans round to 43 or 44 hops at this tempo, in a pattern that
    /// repeats every sixteen beats -- squarely in the range of periods being
    /// searched. Totals would carry that ripple into the novelty; densities do
    /// not.
    #[test]
    fn a_beat_feature_is_energy_per_hop_not_energy() {
        let hop_rate = f64::from(RATE) / crate::onset::HOP as f64;
        // Constant energy everywhere, so any variation between beats can only
        // have come from the spans being different lengths.
        let onset = BandedOnset {
            values: vec![[1.0, 0.0, 0.0, 0.0]; 8000],
            rate: hop_rate,
        };
        let features = beat_features(
            &onset,
            &grid(),
            SampleRate::new(RATE).unwrap(),
            u64::from(RATE) * 90,
        )
        .expect("features");
        let first = features[0][0];
        for (index, beat) in features.iter().enumerate() {
            assert!(
                (beat[0] - first).abs() < 1e-4,
                "beat {index} read {} against beat 0's {first}, so the span length \
                 leaked into the feature",
                beat[0]
            );
        }
    }

    /// **More examples of a pattern is more confidence in it.**
    ///
    /// This is what the standard error buys, and it is easy to lose: scored
    /// against the plain deviation, a boundary seen four times and the same
    /// boundary seen forty times would score identically, and
    /// `MIN_CONFIDENCE` would be satisfied by a pattern glimpsed twice at the
    /// start of a track.
    ///
    /// Stated this way rather than as "z is comparable across phrase lengths",
    /// which was the first attempt and was wrong: two tracks with boundaries
    /// every 8 and every 32 beats are not the same pattern -- the second has a
    /// quieter baseline -- so their z-scores have no reason to match.
    #[test]
    fn a_pattern_seen_more_often_is_more_confident() {
        let build = |phrases: usize| -> Vec<f32> {
            (0..16 * phrases)
                .map(|beat| if beat.is_multiple_of(16) { 2.0 } else { 0.0 })
                .collect()
        };
        let glimpsed = best_phase(&build(4), 16).expect("a phase");
        let established = best_phase(&build(64), 16).expect("a phase");
        assert!(
            established.1 > glimpsed.1 * 2.0,
            "the same boundary seen 64 times scored {} against {} for four times, so \
             the confidence does not grow with the evidence",
            established.1,
            glimpsed.1
        );
    }

    /// Silence is not structure either, and must not divide by zero on the way
    /// to saying so.
    #[test]
    fn silence_reports_none_rather_than_dividing_by_zero() {
        let hop_rate = f64::from(RATE) / crate::onset::HOP as f64;
        let onset = BandedOnset {
            values: vec![[0.0; crate::onset::BANDS]; 4000],
            rate: hop_rate,
        };
        assert!(analyse(&onset, u64::from(RATE) * 45).is_none());
    }

    /// A track too short to contain four phrases cannot be measured, and says
    /// so rather than reporting a structure inferred from two examples.
    #[test]
    fn a_track_too_short_to_measure_reports_none() {
        let (onset, frames) = track(20, |beat| beat.is_multiple_of(8));
        assert!(analyse(&onset, frames).is_none());
    }

    /// The confidence is a real number about this track, not a constant.
    #[test]
    fn a_clear_structure_is_more_confident_than_a_faint_one() {
        let (clear, clear_frames) = track(16 * 12, |beat| beat.is_multiple_of(16));
        let strong = analyse(&clear, clear_frames).expect("a phrase structure");

        // The same phrasing, but the boundaries barely louder than the beats.
        let hop_rate = f64::from(RATE) / crate::onset::HOP as f64;
        let seconds = 16.0 * 12.0 * 60.0 / BPM;
        let hops = (seconds * hop_rate) as usize;
        let per_beat = hops_per_beat();
        let mut values = vec![[0.0f32; crate::onset::BANDS]; hops];
        for (index, hop) in values.iter_mut().enumerate() {
            let exact = index as f64 / per_beat;
            let beat = exact.floor() as usize;
            let phase = exact - exact.floor();
            let shape = if phase < 0.3 {
                (0.5 * (1.0 + (std::f64::consts::PI * phase / 0.3).cos())) as f32
            } else {
                0.0
            };
            hop[0] = 0.05 + shape;
            if beat.is_multiple_of(16) {
                hop[2] = 0.15 * shape;
            }
        }
        let faint = BandedOnset {
            values,
            rate: hop_rate,
        };
        let weak = analyse(&faint, (seconds * f64::from(RATE)) as u64);

        assert!(
            weak.is_none_or(|w| w.confidence < strong.confidence),
            "a faint structure was as convincing as an obvious one"
        );
    }
}
