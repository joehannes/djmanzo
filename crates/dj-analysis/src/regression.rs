//! Scoring the beat tracker against hand-verified grids.
//!
//! # Why this exists separately from the tests
//!
//! M2 called for a *labelled regression set* — real tracks with grids somebody
//! verified by ear — and it is the one M2 item still open. It has stayed open
//! for a reason worth stating plainly: **the corpus cannot be written, only
//! collected.** It needs licensed music and a human who can hear whether a
//! downbeat is on the downbeat. Neither is something this repository can carry.
//!
//! What *can* be built, and is what this module is, is everything around the
//! corpus: the manifest format, the metrics, and the scoring. So the moment a
//! DJ points djmanzo at forty tracks they have gridded by hand, the answer
//! comes out — including the one number that has been waiting on it,
//! [`crate::tempo`]'s `CERTAIN_CORRELATION`, which is currently interpolated
//! between a synthetic click track (0.95) and white noise (0.014) with nothing
//! real in between.
//!
//! # The metrics are the standard ones
//!
//! Beat-tracking evaluation has settled conventions and there is no reason to
//! invent others. Two accuracy figures, reported separately because they mean
//! different things:
//!
//! - **Exact** — the estimate is within tolerance of the true tempo.
//! - **Octave-tolerant** — the estimate is within tolerance of the true tempo
//!   *or* of a simple multiple of it (half, double, third, triple).
//!
//! Keeping them apart is the whole point. Autocorrelation genuinely cannot
//! distinguish 80 from 160 — a curve periodic at one is periodic at the other —
//! so an octave error is a different failure from not finding the beat at all.
//! One costs the DJ a click on the alternative the analyser already offers; the
//! other means the grid is noise. A single blended score hides exactly the
//! distinction that decides what to fix.
//!
//! Phase is scored separately again, because a grid can have the right tempo
//! and sit between the beats, which sounds worse than either.

use crate::Analysis;
use dj_core::SampleRate;

/// One hand-verified track.
///
/// `bpm` and `downbeat_seconds` are the human's answer, not the analyser's.
#[derive(Debug, Clone, PartialEq)]
pub struct Labelled {
    /// Only used for reporting, so a failure names the track.
    pub name: String,
    pub bpm: f64,
    /// Where a beat actually falls, in seconds from the start of the file.
    ///
    /// Any beat, not necessarily the first and not necessarily a downbeat —
    /// the same convention [`dj_core::Beatgrid`] uses for its anchor, because
    /// asking a human to find bar one of a track that fades in is asking for a
    /// label nobody can produce reliably.
    pub downbeat_seconds: f64,
}

/// How close an estimate has to be to count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// Fraction of the true tempo. 0.02 — two percent — is the convention, and
    /// it is also about where a beatmatched mix starts to drift audibly over a
    /// few bars.
    pub bpm_fraction: f64,
    /// Fraction of one beat the anchor may be out by.
    ///
    /// An eighth of a beat at 128 BPM is 59 ms. Past roughly that a listener
    /// stops hearing "slightly early" and starts hearing a separate event.
    pub phase_fraction: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            bpm_fraction: 0.02,
            phase_fraction: 0.125,
        }
    }
}

/// What the analyser said about one labelled track, and whether it was right.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    pub name: String,
    pub expected_bpm: f64,
    /// `None` when the analyser declined to guess, which is not the same as
    /// being wrong and is counted separately.
    pub found_bpm: Option<f64>,
    pub confidence: f64,
    pub exact: bool,
    /// True when `exact` is true, or when the estimate is a simple multiple.
    pub octave_tolerant: bool,
    /// The multiple taken, when one was: 0.5, 2.0, and so on. `None` when the
    /// tempo was exact or wrong outright.
    pub octave: Option<f64>,
    /// `None` when there was no grid to check the phase of.
    pub phase_correct: Option<bool>,
}

impl Scored {
    /// An estimate that was offered and was wrong at any octave.
    ///
    /// Distinguished from a decline, which is the analyser working as intended:
    /// **an analyser that is confidently wrong is worse than one that says it
    /// does not know**, so the two must never be added together.
    #[must_use]
    pub fn confidently_wrong(&self) -> bool {
        self.found_bpm.is_some() && !self.octave_tolerant
    }
}

/// The whole run.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    pub tracks: Vec<Scored>,
}

impl Report {
    #[must_use]
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    /// Fraction within tolerance of the true tempo.
    #[must_use]
    pub fn exact_accuracy(&self) -> f64 {
        self.fraction(|t| t.exact)
    }

    /// Fraction within tolerance of the true tempo or a simple multiple of it.
    #[must_use]
    pub fn octave_tolerant_accuracy(&self) -> f64 {
        self.fraction(|t| t.octave_tolerant)
    }

    /// Fraction where a tempo was offered and was wrong at every octave.
    ///
    /// The number to actually watch. Accuracy can be raised by guessing more,
    /// and this is what that costs.
    #[must_use]
    pub fn confidently_wrong_rate(&self) -> f64 {
        self.fraction(Scored::confidently_wrong)
    }

    /// Fraction the analyser declined to grid at all.
    #[must_use]
    pub fn decline_rate(&self) -> f64 {
        self.fraction(|t| t.found_bpm.is_none())
    }

    /// Fraction whose anchor landed within tolerance, among those that got a
    /// grid at the right tempo. A phase figure over tracks whose tempo is
    /// wrong would be measuring nothing.
    #[must_use]
    pub fn phase_accuracy(&self) -> f64 {
        let eligible: Vec<&Scored> = self
            .tracks
            .iter()
            .filter(|t| t.exact && t.phase_correct.is_some())
            .collect();
        if eligible.is_empty() {
            return 0.0;
        }
        let right = eligible
            .iter()
            .filter(|t| t.phase_correct == Some(true))
            .count();
        right as f64 / eligible.len() as f64
    }

    /// The confidence threshold that would best separate right from wrong.
    ///
    /// This is what calibrates `CERTAIN_CORRELATION`. It sweeps every
    /// confidence the run produced and returns the cut maximising *Youden's J*
    /// — the true-positive rate minus the false-positive rate — which is the
    /// standard answer to "where do I put the line" and, unlike raw accuracy,
    /// is not fooled by a corpus that is mostly one class.
    ///
    /// `None` when there is nothing to separate: no correct tracks, or no
    /// incorrect ones. A corpus the analyser gets entirely right cannot say
    /// where the threshold belongs, and reporting a number from it would be
    /// inventing one.
    #[must_use]
    pub fn best_confidence_threshold(&self) -> Option<f64> {
        let graded: Vec<(f64, bool)> = self
            .tracks
            .iter()
            .filter(|t| t.found_bpm.is_some())
            .map(|t| (t.confidence, t.octave_tolerant))
            .collect();

        let positives = graded.iter().filter(|(_, ok)| *ok).count();
        let negatives = graded.len() - positives;
        if positives == 0 || negatives == 0 {
            return None;
        }

        let mut best: Option<(f64, f64)> = None;
        for &(cut, _) in &graded {
            let true_positive =
                graded.iter().filter(|(c, ok)| *ok && *c >= cut).count() as f64 / positives as f64;
            let false_positive =
                graded.iter().filter(|(c, ok)| !*ok && *c >= cut).count() as f64 / negatives as f64;
            let j = true_positive - false_positive;
            if best.is_none_or(|(_, best_j)| j > best_j) {
                best = Some((cut, j));
            }
        }
        best.map(|(cut, _)| cut)
    }

    fn fraction(&self, mut pred: impl FnMut(&Scored) -> bool) -> f64 {
        if self.tracks.is_empty() {
            return 0.0;
        }
        self.tracks.iter().filter(|t| pred(t)).count() as f64 / self.tracks.len() as f64
    }

    /// A human-readable summary, for a CI log or a terminal.
    #[must_use]
    pub fn summary(&self) -> String {
        use std::fmt::Write as _;
        let mut out = format!("{} labelled tracks\n", self.len());
        let _ = writeln!(
            out,
            "  tempo exact            {:.1}%",
            self.exact_accuracy() * 100.0
        );
        let _ = writeln!(
            out,
            "  tempo within an octave {:.1}%",
            self.octave_tolerant_accuracy() * 100.0
        );
        let _ = writeln!(
            out,
            "  phase (of exact)       {:.1}%",
            self.phase_accuracy() * 100.0
        );
        let _ = writeln!(
            out,
            "  declined               {:.1}%",
            self.decline_rate() * 100.0
        );
        let _ = writeln!(
            out,
            "  CONFIDENTLY WRONG      {:.1}%",
            self.confidently_wrong_rate() * 100.0
        );
        match self.best_confidence_threshold() {
            Some(cut) => {
                let _ = writeln!(out, "  suggested threshold    {cut:.3}");
            }
            None => {
                let _ = writeln!(
                    out,
                    "  suggested threshold    (corpus is all one class -- cannot say)"
                );
            }
        }
        for track in self.tracks.iter().filter(|t| t.confidently_wrong()) {
            let _ = writeln!(
                out,
                "  wrong: {} expected {:.1}, found {:.1} at confidence {:.2}",
                track.name,
                track.expected_bpm,
                track.found_bpm.unwrap_or(0.0),
                track.confidence
            );
        }
        out
    }
}

/// The multiples an octave-tolerant score accepts.
///
/// Halves, doubles and the triplet relations. Not an arbitrary ratio: these are
/// the ones a periodicity estimate genuinely confuses, and admitting more would
/// turn the tolerant score into "any tempo at all".
const OCTAVES: [f64; 6] = [0.25, 1.0 / 3.0, 0.5, 2.0, 3.0, 4.0];

/// Score one analysis against one label.
#[must_use]
pub fn score(
    label: &Labelled,
    analysis: &Analysis,
    sample_rate: SampleRate,
    tol: Tolerance,
) -> Scored {
    let Some(tempo) = analysis.tempo else {
        return Scored {
            name: label.name.clone(),
            expected_bpm: label.bpm,
            found_bpm: None,
            confidence: 0.0,
            exact: false,
            octave_tolerant: false,
            octave: None,
            phase_correct: None,
        };
    };

    let found = tempo.grid.bpm.get();
    let window = label.bpm * tol.bpm_fraction;
    let exact = (found - label.bpm).abs() <= window;

    let octave = OCTAVES.iter().copied().find(|&multiple| {
        (found - label.bpm * multiple).abs() <= label.bpm * multiple * tol.bpm_fraction
    });

    Scored {
        name: label.name.clone(),
        expected_bpm: label.bpm,
        found_bpm: Some(found),
        confidence: tempo.grid.confidence.get(),
        exact,
        octave_tolerant: exact || octave.is_some(),
        octave: if exact { None } else { octave },
        phase_correct: Some(phase_within(label, &tempo, sample_rate, tol)),
    }
}

/// Whether the grid's anchor lands on a real beat.
///
/// Compared modulo the beat period, not directly: the grid's anchor is "some
/// beat", so an anchor a whole number of beats away from the label is right and
/// must not be counted as a phase error. That is the same convention
/// [`dj_core::Beatgrid`] documents, and testing against it any other way would
/// fail correct grids for being anchored somewhere else in the track.
fn phase_within(
    label: &Labelled,
    tempo: &crate::TempoAnalysis,
    sample_rate: SampleRate,
    tol: Tolerance,
) -> bool {
    let rate = f64::from(sample_rate.get());
    let beat_seconds = 60.0 / label.bpm;
    let anchor_seconds = tempo.grid.anchor.get() / rate;

    let offset = (anchor_seconds - label.downbeat_seconds).rem_euclid(beat_seconds);
    // Distance to the nearest beat in either direction, so an anchor a hair
    // *before* one is as close as a hair after.
    let error = offset.min(beat_seconds - offset);
    error <= beat_seconds * tol.phase_fraction
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::{Beatgrid, Bpm, Confidence, FramePos};

    const SR: SampleRate = SampleRate::DEFAULT;

    fn analysis(bpm: f64, anchor_seconds: f64, confidence: f32) -> Analysis {
        Analysis {
            tempo: Some(crate::TempoAnalysis {
                grid: Beatgrid::new(
                    FramePos::new(anchor_seconds * f64::from(SR.get())),
                    Bpm::new(bpm).unwrap(),
                    Confidence::new(f64::from(confidence)),
                ),
                alternative: None,
            }),
            key: None,
            loudness: crate::loudness::Lufs::SILENCE,
        }
    }

    fn label(bpm: f64, downbeat: f64) -> Labelled {
        Labelled {
            name: "test".to_owned(),
            bpm,
            downbeat_seconds: downbeat,
        }
    }

    // -- tempo -------------------------------------------------------------

    #[test]
    fn an_exact_tempo_scores_exact() {
        let scored = score(
            &label(128.0, 0.0),
            &analysis(128.0, 0.0, 0.9),
            SR,
            Tolerance::default(),
        );
        assert!(scored.exact);
        assert!(scored.octave_tolerant);
        assert_eq!(scored.octave, None, "an exact hit took no multiple");
    }

    #[test]
    fn two_percent_out_is_still_exact_and_three_is_not() {
        let tol = Tolerance::default();
        assert!(score(&label(100.0, 0.0), &analysis(102.0, 0.0, 0.9), SR, tol).exact);
        assert!(!score(&label(100.0, 0.0), &analysis(103.0, 0.0, 0.9), SR, tol).exact);
    }

    /// The distinction the whole module exists for.
    #[test]
    fn a_halved_tempo_is_tolerant_but_not_exact() {
        let scored = score(
            &label(140.0, 0.0),
            &analysis(70.0, 0.0, 0.9),
            SR,
            Tolerance::default(),
        );
        assert!(!scored.exact, "70 is not 140");
        assert!(scored.octave_tolerant, "but it is the half");
        assert_eq!(scored.octave, Some(0.5));
        assert!(!scored.confidently_wrong());
    }

    #[test]
    fn a_doubled_tempo_is_tolerant_too() {
        let scored = score(
            &label(70.0, 0.0),
            &analysis(140.0, 0.0, 0.9),
            SR,
            Tolerance::default(),
        );
        assert_eq!(scored.octave, Some(2.0));
    }

    /// A tempo that is not the truth and not a multiple of it. This is the
    /// failure that matters, and it must not hide inside the tolerant score.
    #[test]
    fn an_unrelated_tempo_is_confidently_wrong() {
        let scored = score(
            &label(128.0, 0.0),
            &analysis(97.0, 0.0, 0.9),
            SR,
            Tolerance::default(),
        );
        assert!(!scored.octave_tolerant);
        assert!(scored.confidently_wrong());
    }

    /// Declining is the analyser working as designed, not a wrong answer.
    #[test]
    fn declining_to_grid_is_not_counted_as_wrong() {
        let none = Analysis {
            tempo: None,
            key: None,
            loudness: crate::loudness::Lufs::SILENCE,
        };
        let scored = score(&label(128.0, 0.0), &none, SR, Tolerance::default());
        assert_eq!(scored.found_bpm, None);
        assert!(
            !scored.confidently_wrong(),
            "saying nothing is not being wrong"
        );
        assert_eq!(scored.phase_correct, None, "no grid, no phase to check");
    }

    // -- phase -------------------------------------------------------------

    #[test]
    fn an_anchor_on_the_beat_is_in_phase() {
        let scored = score(
            &label(120.0, 1.0),
            &analysis(120.0, 1.0, 0.9),
            SR,
            Tolerance::default(),
        );
        assert_eq!(scored.phase_correct, Some(true));
    }

    /// The anchor is "some beat", so a whole number of beats away is correct.
    /// Scoring it any other way would fail every correct grid anchored
    /// somewhere other than where the human happened to click.
    #[test]
    fn an_anchor_a_whole_number_of_beats_away_is_in_phase() {
        // 120 BPM is 0.5 s a beat; eight beats later is four seconds.
        let scored = score(
            &label(120.0, 1.0),
            &analysis(120.0, 5.0, 0.9),
            SR,
            Tolerance::default(),
        );
        assert_eq!(scored.phase_correct, Some(true));
    }

    #[test]
    fn an_anchor_half_a_beat_out_is_not() {
        // 0.25 s at 120 BPM is exactly half a beat -- the worst case.
        let scored = score(
            &label(120.0, 1.0),
            &analysis(120.0, 1.25, 0.9),
            SR,
            Tolerance::default(),
        );
        assert_eq!(scored.phase_correct, Some(false));
    }

    #[test]
    fn an_anchor_slightly_early_is_as_good_as_slightly_late() {
        let tol = Tolerance::default();
        // A twentieth of a beat either side, well inside the eighth allowed.
        let early = score(&label(120.0, 1.0), &analysis(120.0, 0.975, 0.9), SR, tol);
        let late = score(&label(120.0, 1.0), &analysis(120.0, 1.025, 0.9), SR, tol);
        assert_eq!(early.phase_correct, Some(true));
        assert_eq!(late.phase_correct, Some(true));
    }

    // -- the report --------------------------------------------------------

    fn report_of(rows: &[(f64, f64, f32)]) -> Report {
        Report {
            tracks: rows
                .iter()
                .map(|&(expected, found, confidence)| {
                    score(
                        &label(expected, 0.0),
                        &analysis(found, 0.0, confidence),
                        SR,
                        Tolerance::default(),
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn the_two_accuracies_differ_where_the_octave_is_wrong() {
        // Two exact, one halved, one unrelated.
        let report = report_of(&[
            (128.0, 128.0, 0.9),
            (100.0, 100.0, 0.9),
            (140.0, 70.0, 0.8),
            (120.0, 91.0, 0.3),
        ]);
        assert!((report.exact_accuracy() - 0.5).abs() < 1e-9);
        assert!((report.octave_tolerant_accuracy() - 0.75).abs() < 1e-9);
        assert!((report.confidently_wrong_rate() - 0.25).abs() < 1e-9);
    }

    #[test]
    fn phase_is_scored_only_over_tracks_whose_tempo_was_right() {
        // One right tempo in phase, one wrong tempo whose phase is meaningless.
        let report = Report {
            tracks: vec![
                score(
                    &label(120.0, 0.0),
                    &analysis(120.0, 0.0, 0.9),
                    SR,
                    Tolerance::default(),
                ),
                score(
                    &label(120.0, 0.0),
                    &analysis(91.0, 0.13, 0.4),
                    SR,
                    Tolerance::default(),
                ),
            ],
        };
        assert!(
            (report.phase_accuracy() - 1.0).abs() < 1e-9,
            "the wrong-tempo track must not drag the phase figure down"
        );
    }

    #[test]
    fn an_empty_report_reports_zero_rather_than_dividing_by_nothing() {
        let report = Report::default();
        assert!(report.is_empty());
        assert_eq!(report.exact_accuracy(), 0.0);
        assert_eq!(report.phase_accuracy(), 0.0);
        assert_eq!(report.best_confidence_threshold(), None);
    }

    // -- calibration -------------------------------------------------------

    /// The point of the whole module: find where confidence stops predicting
    /// correctness. Right answers confident, wrong answers not.
    #[test]
    fn the_threshold_lands_between_the_confident_right_and_the_unsure_wrong() {
        let report = report_of(&[
            (128.0, 128.0, 0.80),
            (124.0, 124.0, 0.75),
            (120.0, 97.0, 0.30),
            (100.0, 133.0, 0.25),
        ]);
        let cut = report
            .best_confidence_threshold()
            .expect("both classes present");
        assert!(
            (0.30..=0.75).contains(&cut),
            "a threshold of {cut} does not separate them"
        );
    }

    /// A corpus with nothing wrong in it cannot say where the line goes, and
    /// must say so rather than return a number somebody would then act on.
    #[test]
    fn a_corpus_of_one_class_yields_no_threshold() {
        let all_right = report_of(&[(128.0, 128.0, 0.9), (100.0, 100.0, 0.4)]);
        assert_eq!(all_right.best_confidence_threshold(), None);

        let all_wrong = report_of(&[(128.0, 97.0, 0.9), (100.0, 133.0, 0.4)]);
        assert_eq!(all_wrong.best_confidence_threshold(), None);
    }

    /// Declines carry no confidence of their own and must not be graded as
    /// though they did — otherwise a corpus the analyser mostly declined would
    /// drag the suggested threshold toward zero.
    #[test]
    fn declines_do_not_take_part_in_the_threshold() {
        let none = Analysis {
            tempo: None,
            key: None,
            loudness: crate::loudness::Lufs::SILENCE,
        };
        let mut report = report_of(&[(128.0, 128.0, 0.8), (120.0, 97.0, 0.3)]);
        let with_declines = {
            report
                .tracks
                .push(score(&label(90.0, 0.0), &none, SR, Tolerance::default()));
            report.clone()
        };
        let without = report_of(&[(128.0, 128.0, 0.8), (120.0, 97.0, 0.3)]);
        assert_eq!(
            with_declines.best_confidence_threshold(),
            without.best_confidence_threshold()
        );
    }

    #[test]
    fn the_summary_names_the_tracks_that_were_wrong() {
        let report = Report {
            tracks: vec![score(
                &Labelled {
                    name: "some track.flac".to_owned(),
                    bpm: 128.0,
                    downbeat_seconds: 0.0,
                },
                &analysis(97.0, 0.0, 0.9),
                SR,
                Tolerance::default(),
            )],
        };
        let text = report.summary();
        assert!(text.contains("some track.flac"), "{text}");
        assert!(text.contains("CONFIDENTLY WRONG"), "{text}");
    }
}
