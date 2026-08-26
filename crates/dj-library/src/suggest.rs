//! What to play next, and why.
//!
//! A ranking over the library given what is playing now. Deterministic and
//! local: no model, no network, no learned weights. That is a deliberate floor
//! rather than a placeholder -- a DJ deciding what to drop next at 01:40 needs
//! an answer in the time it takes to look down, and needs to be able to see why
//! it was given.
//!
//! # Reasons are data, not prose
//!
//! Every suggestion carries [`Reason`]s: typed, enumerable, each with the
//! numbers behind it. [ADR-0005](../../../docs/adr/0005-assistant-speaks-only-actions.md)
//! makes the assistant speak in actions rather than sentences, and this is the
//! same principle one layer down. A rendered string could not be filtered on,
//! sorted by, or disagreed with; a `Reason::Harmonic { .. }` can be shown as a
//! chip, used to explain a rejection, and read back by the assistant when it
//! proposes a transition.
//!
//! # What this does *not* claim to know
//!
//! **Energy is approximated by loudness**, and they are not the same thing. A
//! sparse, tense record can be quieter than a wall-of-sound filler and carry a
//! room better. Integrated LUFS is what the analyser measures and it is a
//! defensible proxy for "how hard does this hit", but the honest name for the
//! reason is loudness, and that is what it is called below. A real energy
//! measure -- spectral flux over the track, percussive density, dynamic range --
//! belongs in `dj-analysis` and is not here yet.
//!
//! **Phrase compatibility is nearly free.** Phrase lengths in practice are 8, 16
//! and 32, and each divides the next, so two tracks that both have a phrase
//! structure will align. The real risk is a track with *no* detected structure,
//! which is what the phrase reason actually reports. Inventing a penalty for
//! 16-against-32 would be inventing a problem.

use crate::LibraryTrack;
use dj_core::{MusicalKey, TrackId};

/// What is playing, and what the DJ wants to happen next.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Playing {
    pub key: Option<MusicalKey>,
    pub bpm: Option<f64>,
    /// Integrated loudness of what is playing. See the module docs on why this
    /// is not called energy.
    pub lufs: Option<f64>,
    pub phrase_beats: Option<u32>,
}

/// Where the DJ wants the next track to take the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Trajectory {
    /// Harder than what is playing. The peak-hour default.
    Lift,
    /// About the same. Holding a plateau, which is most of a set.
    #[default]
    Hold,
    /// Softer. A come-down, or making room before a bigger record.
    Ease,
}

/// How far the deck can stretch a tempo before it stops sounding like itself.
///
/// Six percent each way. Beyond that keylock artefacts become audible on
/// sustained material and the drums start to sound wrong even with it on; it is
/// also roughly the range a pitch fader is usually set to. A track outside this
/// is not rejected -- half- and double-time still work -- but it is not scored
/// as a comfortable match.
const TEMPO_TOLERANCE: f64 = 0.06;

/// A reason a track was suggested, with the numbers behind it.
///
/// Typed rather than rendered, so the interface can show it as a chip and the
/// assistant can read it back when proposing a transition. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reason {
    /// Same key: the strongest harmonic match there is.
    SameKey(MusicalKey),
    /// A neighbour on the wheel, or the relative major/minor.
    Harmonic { from: MusicalKey, to: MusicalKey },
    /// The keys clash. Carried rather than hidden -- a DJ may want it anyway,
    /// and a suggestion that hides its worst feature is not a suggestion.
    KeyClash { from: MusicalKey, to: MusicalKey },
    /// Within the deck's comfortable pitch range.
    TempoFits { from: f64, to: f64 },
    /// Mixable at half or double time.
    TempoHalfOrDouble { from: f64, to: f64 },
    /// Would need more stretch than the deck can do cleanly.
    TempoFar { from: f64, to: f64 },
    /// Louder, quieter or level, and by how much. Named loudness rather than
    /// energy on purpose; see the module docs.
    Loudness { delta_db: f64 },
    /// The track has a phrase structure, so it can be mixed on a phrase.
    PhraseKnown { beats: u32 },
    /// No phrase structure was found -- mix by ear.
    PhraseUnknown,
    /// Not analysed enough to judge. Ranked last rather than dropped, because a
    /// library that is still analysing should not look empty.
    Unanalysed,
}

impl Reason {
    /// What this reason contributes to the score.
    ///
    /// Weights are stated here rather than scattered through the scorer, so the
    /// ranking can be argued with in one place.
    ///
    /// Harmonic and tempo dominate because they are the two that make a mix
    /// technically possible; loudness only shapes the order among tracks that
    /// already work. A key clash is a large negative rather than a
    /// disqualification -- DJs break that rule deliberately and often.
    #[must_use]
    pub fn weight(self) -> f64 {
        match self {
            Self::SameKey(_) => 3.0,
            Self::Harmonic { .. } => 2.0,
            Self::KeyClash { .. } => -2.5,
            Self::TempoFits { .. } => 3.0,
            Self::TempoHalfOrDouble { .. } => 1.0,
            Self::TempoFar { .. } => -3.0,
            // Loudness is scored by the trajectory, which needs to know what
            // was asked for -- see `score_loudness`. Zero here so it cannot be
            // counted twice.
            Self::Loudness { .. } => 0.0,
            Self::PhraseKnown { .. } => 0.5,
            Self::PhraseUnknown => 0.0,
            Self::Unanalysed => -10.0,
        }
    }
}

/// A ranked candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub track: TrackId,
    pub score: f64,
    pub reasons: Vec<Reason>,
}

/// Rank `candidates` for playing after `now`.
///
/// Returns every candidate, sorted best first, rather than a filtered top few:
/// the caller knows how many rows it has, and a DJ scrolling past the top
/// suggestion is looking for the one the ranking got wrong.
///
/// Ties break on `TrackId`, so the same library and the same playing track give
/// the same order every time. A suggester that shuffled equal candidates would
/// be impossible to test and unsettling to use.
#[must_use]
pub fn rank(now: &Playing, trajectory: Trajectory, candidates: &[LibraryTrack]) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = candidates
        .iter()
        .map(|t| score(now, trajectory, t))
        .collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.track.cmp(&b.track))
    });
    out
}

/// Score one candidate.
#[must_use]
pub fn score(now: &Playing, trajectory: Trajectory, candidate: &LibraryTrack) -> Suggestion {
    let mut reasons = Vec::new();

    let key = candidate.analysis.key();
    let bpm = candidate.analysis.bpm;
    if key.is_none() && bpm.is_none() {
        reasons.push(Reason::Unanalysed);
        return Suggestion {
            track: candidate.id,
            score: Reason::Unanalysed.weight(),
            reasons,
        };
    }

    if let (Some(from), Some(to)) = (now.key, key) {
        reasons.push(if from == to {
            Reason::SameKey(to)
        } else if from.is_compatible_with(to) {
            Reason::Harmonic { from, to }
        } else {
            Reason::KeyClash { from, to }
        });
    }

    if let (Some(from), Some(to)) = (now.bpm, bpm) {
        reasons.push(tempo_reason(from, to));
    }

    let mut total: f64 = reasons.iter().map(|r| r.weight()).sum();

    if let (Some(from), Some(to)) = (now.lufs, candidate.analysis.loudness_lufs) {
        let delta = to - from;
        reasons.push(Reason::Loudness { delta_db: delta });
        total += score_loudness(delta, trajectory);
    }

    reasons.push(match candidate.analysis.phrase_beats {
        Some(beats) => Reason::PhraseKnown { beats },
        None => Reason::PhraseUnknown,
    });
    total += reasons.last().map_or(0.0, |r| r.weight());

    Suggestion {
        track: candidate.id,
        score: total,
        reasons,
    }
}

/// Which tempo reason applies.
fn tempo_reason(from: f64, to: f64) -> Reason {
    // No guard for zero, negative or non-finite tempos, and that is deliberate:
    // every one of them already lands in `TempoFar` below. A NaN fails both
    // `<=` comparisons (NaN fails every comparison), a zero divides to an
    // infinity that fails them too, and a negative is nowhere near 1.0. A guard
    // was written here first and removed once mutation testing showed deleting
    // it changed nothing -- an unreachable branch reads like a safeguard and is
    // dead weight. The test below pins the behaviour whichever way it is
    // implemented.
    if (to / from - 1.0).abs() <= TEMPO_TOLERANCE {
        return Reason::TempoFits { from, to };
    }
    // Half and double time: a 140 record over a 70 one is an ordinary move, and
    // the ratio test above cannot see it because the numbers are miles apart.
    for factor in [0.5, 2.0] {
        if (to / (from * factor) - 1.0).abs() <= TEMPO_TOLERANCE {
            return Reason::TempoHalfOrDouble { from, to };
        }
    }
    Reason::TempoFar { from, to }
}

/// How much the loudness change is worth, given where the DJ wants to go.
///
/// A separate function because it is the one score that depends on intent
/// rather than on the two tracks alone: three decibels louder is the right
/// answer for `Lift` and the wrong one for `Ease`.
///
/// Scaled so a change in the wanted direction is worth up to about a key match,
/// and tapers rather than growing without bound -- a record twelve decibels
/// louder is not four times better than one three decibels louder, it is a
/// different record entirely.
fn score_loudness(delta_db: f64, trajectory: Trajectory) -> f64 {
    if !delta_db.is_finite() {
        return 0.0;
    }
    // Three decibels is about the smallest step a room notices, so it is the
    // unit here.
    let steps = delta_db / 3.0;
    let wanted = match trajectory {
        Trajectory::Lift => steps,
        Trajectory::Ease => -steps,
        // Level is the goal, so any change in either direction costs.
        Trajectory::Hold => -steps.abs(),
    };
    // tanh: rewards the right direction, saturates rather than running away.
    2.0 * wanted.tanh()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoredAnalysis;
    use dj_core::{Mode, SampleRate};

    fn key(hour: u8, mode: Mode) -> MusicalKey {
        MusicalKey::new(hour, mode).unwrap()
    }

    /// A candidate with just enough filled in to be judged.
    fn track(byte: u8, bpm: Option<f64>, k: Option<MusicalKey>, lufs: Option<f64>) -> LibraryTrack {
        LibraryTrack {
            id: TrackId::from_bytes([byte; 32]),
            path: std::path::PathBuf::from(format!("/music/{byte}.wav")),
            tags: crate::Tags::default(),
            duration_frames: 44_100 * 300,
            sample_rate: SampleRate::DEFAULT,
            channels: 2,
            file_size: None,
            file_modified: None,
            added_at: 0,
            analysis: StoredAnalysis {
                bpm,
                key_hour: k.map(MusicalKey::hour),
                key_mode: k.map(MusicalKey::mode),
                loudness_lufs: lufs,
                phrase_beats: Some(16),
                phrase_anchor: Some(0),
                ..StoredAnalysis::default()
            },
            stats: crate::PlayStats::default(),
            colour: None,
        }
    }

    fn playing() -> Playing {
        Playing {
            key: Some(key(8, Mode::Minor)),
            bpm: Some(128.0),
            lufs: Some(-8.0),
            phrase_beats: Some(16),
        }
    }

    fn reasons_of(s: &Suggestion) -> Vec<Reason> {
        s.reasons.clone()
    }

    /// **The same key at the same tempo wins.**
    ///
    /// The baseline the whole ranking is built on. If this does not hold,
    /// nothing else about the scorer is worth arguing over.
    #[test]
    fn the_easiest_mix_ranks_first() {
        let easy = track(1, Some(128.0), Some(key(8, Mode::Minor)), Some(-8.0));
        let clash = track(2, Some(128.0), Some(key(2, Mode::Major)), Some(-8.0));
        let far = track(3, Some(174.0), Some(key(8, Mode::Minor)), Some(-8.0));

        let ranked = rank(&playing(), Trajectory::Hold, &[clash, far, easy]);
        assert_eq!(ranked[0].track, TrackId::from_bytes([1; 32]));
        assert!(
            reasons_of(&ranked[0]).contains(&Reason::SameKey(key(8, Mode::Minor))),
            "the winner did not say why it won"
        );
    }

    /// **A key clash is reported, not hidden.**
    ///
    /// A suggestion that conceals its worst feature is not a suggestion. DJs
    /// break the harmonic rule deliberately, and one who does should be able to
    /// see that is what they are doing.
    #[test]
    fn a_clashing_key_still_says_so() {
        let clash = track(2, Some(128.0), Some(key(2, Mode::Major)), Some(-8.0));
        let scored = score(&playing(), Trajectory::Hold, &clash);
        assert!(
            scored
                .reasons
                .iter()
                .any(|r| matches!(r, Reason::KeyClash { .. })),
            "the clash was scored but not stated: {:?}",
            scored.reasons
        );
    }

    /// **Same key beats merely compatible.**
    ///
    /// A neighbour on the wheel works; the same key always works. With the two
    /// weighted equally the ranking still looked sensible in every other test,
    /// because nothing compared them directly.
    #[test]
    fn the_same_key_outranks_a_compatible_one() {
        let same = track(2, Some(128.0), Some(key(8, Mode::Minor)), Some(-8.0));
        let neighbour = track(1, Some(128.0), Some(key(9, Mode::Minor)), Some(-8.0));

        // The neighbour has the *lower* id, so an id tie-break would put it
        // first. Only the key weight can produce the expected order.
        let ranked = rank(&playing(), Trajectory::Hold, &[neighbour, same]);
        assert_eq!(
            ranked[0].track,
            TrackId::from_bytes([2; 32]),
            "a neighbouring key ranked level with or above the same key"
        );
    }

    /// **Loudness shapes the order; it does not decide it.**
    ///
    /// The loudness score saturates on purpose. Without that, a record twenty
    /// decibels louder scores so highly that it outranks one that is actually
    /// mixable -- and the suggester starts recommending tracks the deck cannot
    /// pitch into range, because they are loud.
    #[test]
    fn a_very_loud_track_does_not_outrank_a_mixable_one() {
        // Wrong tempo by a mile, and enormously louder.
        let shouty = track(1, Some(200.0), Some(key(8, Mode::Minor)), Some(15.0));
        // Right tempo, right key, only slightly louder.
        let mixable = track(2, Some(128.0), Some(key(8, Mode::Minor)), Some(-6.0));

        let ranked = rank(&playing(), Trajectory::Lift, &[shouty, mixable]);
        assert_eq!(
            ranked[0].track,
            TrackId::from_bytes([2; 32]),
            "an unmixable record won on loudness alone"
        );
    }

    /// **A key clash costs, rather than merely being labelled.**
    ///
    /// Without this, flipping the clash weight from negative to positive passed
    /// every other test: the same-key track still won on its own merits, so
    /// nothing noticed that clashing had become a *bonus*. The label and the
    /// arithmetic have to agree.
    #[test]
    fn a_key_clash_lowers_the_score() {
        let clash = track(2, Some(128.0), Some(key(2, Mode::Major)), Some(-8.0));
        let friendly = track(2, Some(128.0), Some(key(9, Mode::Minor)), Some(-8.0));

        let clashing = score(&playing(), Trajectory::Hold, &clash);
        let compatible = score(&playing(), Trajectory::Hold, &friendly);
        assert!(
            clashing.score < compatible.score,
            "a clashing key scored {} against {} for a compatible one",
            clashing.score,
            compatible.score
        );
        // And against no key information at all: a known clash is worse than an
        // unknown, because the DJ has been told it will fight.
        let mut keyless = track(2, Some(128.0), None, Some(-8.0));
        keyless.analysis.key_hour = None;
        keyless.analysis.key_mode = None;
        let unknown = score(&playing(), Trajectory::Hold, &keyless);
        assert!(
            clashing.score < unknown.score,
            "a known clash scored {} against {} for an unknown key",
            clashing.score,
            unknown.score
        );
    }

    /// **Half and double time count as mixable.**
    ///
    /// 174 over 87 is an ordinary move. The plain ratio test cannot see it --
    /// the numbers are nowhere near each other -- so a scorer without the
    /// explicit check would rank every drum-and-bass record as unmixable with
    /// every house one.
    #[test]
    fn double_time_is_recognised_as_mixable() {
        let now = Playing {
            bpm: Some(87.0),
            ..playing()
        };
        let dnb = track(4, Some(174.0), Some(key(8, Mode::Minor)), Some(-8.0));
        let scored = score(&now, Trajectory::Hold, &dnb);
        assert!(
            scored
                .reasons
                .iter()
                .any(|r| matches!(r, Reason::TempoHalfOrDouble { .. })),
            "87 to 174 was not recognised as double time: {:?}",
            scored.reasons
        );
        assert!(scored.score > 0.0, "a double-time match scored negative");
    }

    /// **The trajectory changes the order, not just the labels.**
    ///
    /// The one score that depends on intent. A louder record is the right
    /// answer for `Lift` and the wrong one for `Ease`, and if that does not
    /// reverse the ranking the trajectory is decoration.
    #[test]
    fn asking_to_lift_and_asking_to_ease_give_opposite_orders() {
        let loud = track(5, Some(128.0), Some(key(8, Mode::Minor)), Some(-5.0));
        let quiet = track(6, Some(128.0), Some(key(8, Mode::Minor)), Some(-12.0));
        let pool = vec![loud, quiet];

        let lifting = rank(&playing(), Trajectory::Lift, &pool);
        let easing = rank(&playing(), Trajectory::Ease, &pool);

        assert_eq!(
            lifting[0].track,
            TrackId::from_bytes([5; 32]),
            "asked to lift, it did not put the louder record first"
        );
        assert_eq!(
            easing[0].track,
            TrackId::from_bytes([6; 32]),
            "asked to ease, it did not put the quieter record first"
        );
    }

    /// Holding a plateau wants neither: the closest in loudness wins.
    ///
    /// The level track deliberately has the **highest** id. An earlier version
    /// gave it the lowest, and the test passed with the loudness score removed
    /// entirely -- the id tie-break was putting it first and the assertion was
    /// measuring nothing. Mutation testing found that.
    #[test]
    fn holding_prefers_the_track_closest_in_loudness() {
        let loud = track(7, Some(128.0), Some(key(8, Mode::Minor)), Some(-3.0));
        let quiet = track(8, Some(128.0), Some(key(8, Mode::Minor)), Some(-16.0));
        let level = track(9, Some(128.0), Some(key(8, Mode::Minor)), Some(-8.2));

        let ranked = rank(&playing(), Trajectory::Hold, &[loud, quiet, level]);
        assert_eq!(
            ranked[0].track,
            TrackId::from_bytes([9; 32]),
            "holding did not prefer the record closest in level"
        );
    }

    /// **An unanalysed track ranks last but is still offered.**
    ///
    /// Dropping it would make a library that is still analysing look empty,
    /// which is the state every new install is in for its first hour.
    #[test]
    fn an_unanalysed_track_is_last_but_present() {
        let good = track(1, Some(128.0), Some(key(8, Mode::Minor)), Some(-8.0));
        let unknown = track(2, None, None, None);

        let ranked = rank(&playing(), Trajectory::Hold, &[unknown, good]);
        assert_eq!(ranked.len(), 2, "the unanalysed track was dropped");
        assert_eq!(ranked[1].track, TrackId::from_bytes([2; 32]));
        assert!(reasons_of(&ranked[1]).contains(&Reason::Unanalysed));
    }

    /// **The same library gives the same order twice.**
    ///
    /// Ties break on the track id rather than on whatever order the rows
    /// arrived in. A suggester that shuffled equal candidates would be
    /// impossible to test and unsettling to use -- the list would move under
    /// the cursor.
    #[test]
    fn equal_candidates_are_ordered_stably() {
        let a = track(1, Some(128.0), Some(key(8, Mode::Minor)), Some(-8.0));
        let b = track(2, Some(128.0), Some(key(8, Mode::Minor)), Some(-8.0));

        let one = rank(&playing(), Trajectory::Hold, &[a.clone(), b.clone()]);
        let two = rank(&playing(), Trajectory::Hold, &[b, a]);
        assert_eq!(
            one.iter().map(|s| s.track).collect::<Vec<_>>(),
            two.iter().map(|s| s.track).collect::<Vec<_>>(),
            "the same candidates in a different input order ranked differently"
        );
    }

    /// A track with no phrase structure says so, rather than being penalised
    /// for a mismatch that does not exist. See the module docs.
    #[test]
    fn a_track_without_phrases_is_reported_not_punished() {
        let mut no_phrase = track(3, Some(128.0), Some(key(8, Mode::Minor)), Some(-8.0));
        no_phrase.analysis.phrase_beats = None;
        no_phrase.analysis.phrase_anchor = None;

        let scored = score(&playing(), Trajectory::Hold, &no_phrase);
        assert!(scored.reasons.contains(&Reason::PhraseUnknown));
        assert!(
            scored.score > 0.0,
            "a perfectly mixable track was sunk for having no phrase structure"
        );
    }

    /// A nonsense tempo does not become a comfortable match by arithmetic
    /// accident -- dividing by zero or comparing against a NaN both land in the
    /// "too far" branch rather than passing the tolerance test.
    #[test]
    fn a_nonsense_tempo_is_not_a_match() {
        for bad in [0.0, -120.0, f64::NAN, f64::INFINITY] {
            let odd = track(4, Some(bad), Some(key(8, Mode::Minor)), Some(-8.0));
            let scored = score(&playing(), Trajectory::Hold, &odd);
            assert!(
                scored
                    .reasons
                    .iter()
                    .any(|r| matches!(r, Reason::TempoFar { .. })),
                "{bad} was treated as a tempo match: {:?}",
                scored.reasons
            );
        }
    }
}
