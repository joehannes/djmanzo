//! The library, with djmanzo's opinion beside each record.
//!
//! §76 asks for a toggle — *AI Lens* — that adds eight columns to the browser,
//! and closes with the line that governs the whole thing:
//!
//! > This must never replace the standard library view.
//!
//! So this module produces a **row per track that sits alongside** what the
//! browser already draws. Nothing here changes a title, a tempo or a key;
//! turning the lens off leaves the table djmanzo has always shown, untouched,
//! because the lens was never in it.
//!
//! # Every column says where it came from, or says it cannot
//!
//! Six of §76's eight are things djmanzo can work out. One is not, and the
//! honest thing is to name it rather than quietly ship seven columns and let a
//! DJ wonder which of the eight they were promised is missing:
//!
//! - **likely next** — [`dj_library::suggest`], the same scorer the Next rail
//!   uses. `None` when nothing is playing: a lens over a library with no deck
//!   running has nothing to be next *to*, and a number there would be a
//!   ranking against silence.
//! - **user affinity** — [`dj_library::learned`], years of plays.
//! - **crowd suitability** — **absent, and it stays absent.** It needs to know
//!   what the room is doing, which needs a camera or a microphone in it. This
//!   container has neither and neither does a laptop in a booth. Anything
//!   djmanzo printed in that column would be a number about a room it has
//!   never sensed.
//! - **current phase suitability** — the record's own function tags against
//!   the phase the night is in. Grounded in something a DJ wrote down rather
//!   than in a guess from the waveform.
//! - **transition risk** — the scorer's own bad news, kept as reasons rather
//!   than boiled to a number: a risk with no reason is a number a DJ cannot
//!   argue with.
//! - **novelty** and **familiarity** — one fact, read twice. See below.
//! - **function tags** — what the DJ said the record is for.
//!
//! # Novelty and familiarity are the same two numbers
//!
//! §76 lists them separately, and the reading that would make them genuinely
//! different is *the crowd's* — a record everyone knows against one nobody has
//! heard. djmanzo cannot know that; it would need to know the room.
//!
//! What it does know is this DJ's own history: how many times they have played
//! a record and how long ago. So novelty is "new to your sets" and familiarity
//! is "well worn in them", and they are computed from the same play count and
//! the same last-played date because they *are* the same fact from two ends.
//! Presenting them as two independent measurements would be dressing one
//! number up as two.

use dj_core::SessionPhase;
use dj_core::{TrackId, Trajectory};
use dj_library::functions::Function;
use dj_library::suggest::{self, Playing, Reason};
use dj_library::{LibraryTrack, learned::Learned};

/// How long a record has to be unplayed before it counts as fresh again.
///
/// Ninety days. Short enough that a record rested for a season comes back as
/// something to reach for, long enough that last month's workhorse does not.
const RESTED_DAYS: i64 = 90;

/// Plays after which a record is as familiar as it is going to get.
///
/// Twelve. Beyond that the difference between a record played twenty times and
/// one played forty is not a difference a DJ acts on, and a scale that kept
/// climbing would make one record permanently dominate the column.
const WELL_WORN: i64 = 12;

/// Why a mix into this record might not work.
///
/// Reasons rather than a score, because a risk a DJ cannot argue with is a
/// number they will learn to ignore. Every one of these is a
/// [`suggest::Reason`] the scorer already produces — the lens does not have a
/// second opinion about what makes a mix hard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    /// The keys clash.
    Keys,
    /// Further apart in tempo than the deck stretches cleanly.
    Tempo,
    /// No phrase structure, so the mix has to be made by ear.
    NoPhrase,
    /// Not analysed enough to judge. The honest risk of a record djmanzo has
    /// never listened to.
    Unknown,
}

impl Risk {
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Risk::Keys => "keys",
            Risk::Tempo => "tempo",
            Risk::NoPhrase => "no-phrase",
            Risk::Unknown => "unanalysed",
        }
    }

    /// What it means, in the words a DJ would use.
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Risk::Keys => "keys clash",
            Risk::Tempo => "too far in tempo",
            Risk::NoPhrase => "no phrase structure — mix by ear",
            Risk::Unknown => "not analysed yet",
        }
    }
}

/// One record, seen through the lens.
///
/// Every field is `Option` where djmanzo may have nothing to say, and an
/// absence is drawn as an absence. A lens that filled its blanks with zeroes
/// would rank an unanalysed record below a bad one.
#[derive(Debug, Clone, PartialEq)]
pub struct Lensed {
    pub track: TrackId,
    /// §76 "likely next": how well it follows what is playing, on the same
    /// scale the Next rail draws. `None` with nothing playing.
    pub likely_next: Option<f64>,
    /// §76 "user affinity": how much this DJ plays records like this one, 0 to
    /// 1. `None` until there is enough history to lean on.
    pub affinity: Option<f64>,
    /// §76 "current phase suitability": whether the record is *for* the part
    /// of the night the set is in. `None` before the night can be read, which
    /// is the first several minutes of every set.
    pub phase_fit: Option<f64>,
    /// §76 "transition risk", as reasons. Empty means nothing stood out —
    /// which is not the same as `likely_next` being high.
    pub risks: Vec<Risk>,
    /// §76 "novelty": 1 for a record never played, falling as it is used and
    /// climbing again as it rests. This DJ's sets, not the room's ears.
    pub novelty: f64,
    /// §76 "familiarity": how well worn it is in this DJ's own sets.
    pub familiarity: f64,
    /// §76 "function tags": what the DJ said it is for.
    pub functions: Vec<Function>,
}

/// What the lens needs to know about right now.
///
/// Handed in rather than reached for, so this module stays a pure function
/// over its inputs the way `suggest::score` does — it is the property that
/// makes both testable without a database or a deck.
#[derive(Debug, Clone, Copy)]
pub struct Now<'a> {
    /// The record playing, if one is. `None` is a real state: a DJ browsing
    /// before the first record goes on.
    pub playing: Option<&'a Playing>,
    /// Which record that is.
    ///
    /// Carried separately because [`Playing`] deliberately does not know — it
    /// is what a record *sounds like*, not which one it is. The lens needs the
    /// identity for one reason: a record cannot follow itself, so the row for
    /// the one on the deck gets no "likely next" and no risk rather than a
    /// score for a mix nobody can perform.
    pub playing_id: Option<TrackId>,
    pub trajectory: Trajectory,
    /// The phase the night is in, when djmanzo has read one.
    pub phase: Option<SessionPhase>,
    /// Unix seconds, for how long ago a record was last played.
    pub now: i64,
}

/// Put one record through the lens.
#[must_use]
pub fn look(
    track: &LibraryTrack,
    functions: &[Function],
    taste: Option<&Learned>,
    now: Now<'_>,
) -> Lensed {
    // Nothing to say about following the record that is playing: it is
    // already on. A score there would be an answer to a question nobody can
    // ask, sitting in a column of answers to real ones.
    let itself = now.playing_id == Some(track.id);
    let scored = now
        .playing
        .filter(|_| !itself)
        .map(|playing| suggest::score(playing, now.trajectory, track));

    Lensed {
        track: track.id,
        likely_next: scored.as_ref().map(suggest::Suggestion::confidence),
        affinity: taste
            .filter(|learned| learned.is_confident())
            .map(|learned| f64::from(learned.leaning_for(track)).clamp(0.0, 1.0)),
        phase_fit: now.phase.map(|phase| fit(phase, functions)),
        risks: scored.map(|s| risks(&s.reasons)).unwrap_or_default(),
        novelty: novelty(track, now.now),
        familiarity: familiarity(track),
        functions: functions.to_vec(),
    }
}

/// The bad news among a suggestion's reasons.
///
/// Taken from the scorer's own output rather than re-derived: a lens with its
/// own idea of what makes a mix risky would eventually disagree with the rail
/// about the same pair of records, and both would be on screen at once.
fn risks(reasons: &[Reason]) -> Vec<Risk> {
    let mut found: Vec<Risk> = reasons
        .iter()
        .filter_map(|reason| match reason {
            Reason::KeyClash { .. } => Some(Risk::Keys),
            Reason::TempoFar { .. } => Some(Risk::Tempo),
            Reason::PhraseUnknown => Some(Risk::NoPhrase),
            Reason::Unanalysed => Some(Risk::Unknown),
            _ => None,
        })
        .collect();
    found.sort_unstable();
    found.dedup();
    found
}

/// Whether a record is for this part of the night.
///
/// From the DJ's own function tags, which is the only grounded answer there
/// is: a guess from loudness would call every quiet record a closer.
///
/// A record with no tags scores in the middle rather than at zero. It has not
/// been judged unsuitable — nobody has said anything about it at all, and
/// sorting the untagged half of a collection to the bottom would make the lens
/// a filter for how much tagging a DJ has done.
fn fit(phase: SessionPhase, functions: &[Function]) -> f64 {
    if functions.is_empty() {
        return 0.5;
    }
    let wanted: &[Function] = match phase {
        SessionPhase::WarmUp => &[Function::Opener, Function::Safe],
        SessionPhase::Heat => &[Function::Builder, Function::Singalong],
        SessionPhase::Peak => &[Function::Peak, Function::Singalong],
        SessionPhase::Cooldown => &[Function::FloorReset, Function::Safe],
        SessionPhase::ChillOut => &[Function::Closer],
    };
    if functions.iter().any(|f| wanted.contains(f)) {
        1.0
    } else if functions.contains(&Function::Safe) || functions.contains(&Function::TransitionTool) {
        // Works in almost any room, or exists to get between two of them.
        0.6
    } else {
        // Tagged, and tagged as something else. A real answer: this is a peak
        // record and the night is warming up.
        0.2
    }
}

/// New to this DJ's sets, 1 down to 0.
///
/// A record never played is wholly new. One played recently is not. One played
/// often but rested for a season is new *again*, which is how a DJ actually
/// thinks about a crate they have not opened since the spring.
fn novelty(track: &LibraryTrack, now: i64) -> f64 {
    if track.stats.play_count == 0 {
        return 1.0;
    }
    let Some(last) = track.stats.last_played else {
        // Played, but djmanzo does not know when. Not new, and not stale
        // either — the middle is the honest place for it.
        return 0.5;
    };
    #[allow(clippy::cast_precision_loss)]
    let days = (now - last).max(0) as f64 / 86_400.0;
    #[allow(clippy::cast_precision_loss)]
    let rested = RESTED_DAYS as f64;
    (days / rested).clamp(0.0, 1.0)
}

/// Well worn in this DJ's sets, 0 up to 1.
///
/// Deliberately *not* `1 - novelty`. Familiarity is how often, novelty is how
/// recently — a record played forty times and rested a year is both very
/// familiar and freshly playable, and a lens that made them complementary
/// could not say so.
fn familiarity(track: &LibraryTrack) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let worn = track.stats.play_count.clamp(0, WELL_WORN) as f64;
    #[allow(clippy::cast_precision_loss)]
    let most = WELL_WORN as f64;
    worn / most
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::{Mode, MusicalKey, SampleRate};
    use dj_library::{PlayStats, StoredAnalysis, Tags};
    use std::path::PathBuf;

    const DAY: i64 = 86_400;
    const NOW: i64 = 1_800_000_000;

    fn track(byte: u8) -> LibraryTrack {
        LibraryTrack {
            id: TrackId::from_bytes([byte; 32]),
            path: PathBuf::from("/music/x.flac"),
            tags: Tags::default(),
            duration_frames: 48_000 * 200,
            sample_rate: SampleRate::DEFAULT,
            channels: 2,
            file_size: None,
            file_modified: None,
            added_at: 0,
            analysis: StoredAnalysis::default(),
            stats: PlayStats::default(),
            colour: None,
        }
    }

    /// An analysed record: a tempo, a key and a phrase, so the scorer has
    /// something to judge rather than answering `Unanalysed` to everything.
    fn analysed(byte: u8, bpm: f64, hour: u8) -> LibraryTrack {
        let mut t = track(byte);
        t.analysis.bpm = Some(bpm);
        t.analysis.key_hour = MusicalKey::new(hour, Mode::Minor).map(|k| k.hour());
        t.analysis.key_mode = Some(Mode::Minor);
        t.analysis.phrase_beats = Some(16);
        t.analysis.phrase_anchor = Some(0);
        t
    }

    fn playing(bpm: f64, hour: u8) -> Playing {
        Playing::of(&analysed(99, bpm, hour))
    }

    fn quiet() -> Now<'static> {
        Now {
            playing: None,
            playing_id: None,
            trajectory: Trajectory::Hold,
            phase: None,
            now: NOW,
        }
    }

    /// **A column djmanzo cannot answer stays empty.**
    ///
    /// With nothing playing there is nothing to be next *to*, and before the
    /// night can be read there is no phase to fit. A lens that filled those
    /// with zero would rank every record as a bad follow-on to silence — and
    /// worse, would look exactly like a considered judgement.
    #[test]
    fn nothing_playing_means_no_opinion_rather_than_a_bad_one() {
        let seen = look(&analysed(1, 124.0, 8), &[], None, quiet());
        assert_eq!(seen.likely_next, None, "ranked against silence");
        assert_eq!(seen.phase_fit, None, "a phase was invented");
        assert_eq!(seen.affinity, None, "affinity without any history");
        assert!(seen.risks.is_empty(), "a risk about no transition");
        // The two that do not need a deck are still answered.
        assert_eq!(seen.novelty, 1.0);
        assert_eq!(seen.familiarity, 0.0);
    }

    /// **Novelty is not the opposite of familiarity.**
    ///
    /// A record played forty times and then rested a year is *both* well worn
    /// and freshly playable, and that is a real and useful state — it is the
    /// crate you have not opened since the spring. A lens that made the two
    /// complementary could not express it.
    #[test]
    fn a_well_worn_record_can_also_be_fresh_again() {
        let mut rested = analysed(1, 124.0, 8);
        rested.stats.play_count = 40;
        rested.stats.last_played = Some(NOW - 365 * DAY);

        let seen = look(&rested, &[], None, quiet());
        assert_eq!(seen.familiarity, 1.0, "forty plays is not well worn");
        assert_eq!(seen.novelty, 1.0, "a year rested is not fresh again");
        assert!(
            (seen.novelty + seen.familiarity - 1.0).abs() > 0.5,
            "novelty and familiarity are complementary, so one of them says nothing"
        );

        // And the same record played yesterday is familiar and not fresh.
        let mut yesterday = rested.clone();
        yesterday.stats.last_played = Some(NOW - DAY);
        let seen = look(&yesterday, &[], None, quiet());
        assert_eq!(seen.familiarity, 1.0);
        assert!(seen.novelty < 0.05, "played yesterday and still novel");
    }

    /// A record nobody has played is wholly new, whatever else is true of it.
    #[test]
    fn a_record_never_played_is_new() {
        let seen = look(&analysed(1, 124.0, 8), &[], None, quiet());
        assert_eq!(seen.novelty, 1.0);
        assert_eq!(seen.familiarity, 0.0);
    }

    /// **The risks are the scorer's own bad news, not a second opinion.**
    ///
    /// A lens with its own idea of what makes a mix hard would eventually
    /// disagree with the Next rail about the same two records, with both on
    /// screen at once.
    #[test]
    fn the_risks_are_the_ones_the_scorer_names() {
        // 124 into 98 with clashing keys: too far to stretch, and the keys
        // fight.
        let far = analysed(1, 98.0, 1);
        let now = Now {
            playing: Some(&playing(124.0, 8)),
            ..quiet()
        };
        let seen = look(&far, &[], None, now);
        assert!(seen.risks.contains(&Risk::Tempo), "{:?}", seen.risks);
        assert!(seen.risks.contains(&Risk::Keys), "{:?}", seen.risks);

        // And a record that fits names none of them.
        let close = analysed(2, 125.0, 8);
        let seen = look(&close, &[], None, now);
        assert!(seen.risks.is_empty(), "{:?}", seen.risks);
        assert!(seen.likely_next.unwrap() > 0.7);
    }

    /// An unanalysed record's risk is that djmanzo has not listened to it,
    /// which is the honest answer rather than a low score.
    #[test]
    fn an_unanalysed_record_says_so_rather_than_scoring_badly() {
        let now = Now {
            playing: Some(&playing(124.0, 8)),
            ..quiet()
        };
        let seen = look(&track(1), &[], None, now);
        assert_eq!(seen.risks, vec![Risk::Unknown]);
    }

    /// **An untagged record is not judged unsuitable.**
    ///
    /// Nobody has said anything about it. Sorting the untagged half of a
    /// collection to the bottom would turn the lens into a measure of how much
    /// tagging the DJ has got round to.
    #[test]
    fn a_record_with_no_function_tags_sits_in_the_middle() {
        let now = Now {
            phase: Some(SessionPhase::Peak),
            ..quiet()
        };
        let untagged = look(&analysed(1, 124.0, 8), &[], None, now);
        assert_eq!(untagged.phase_fit, Some(0.5));

        // A record tagged as something else *is* judged, because somebody said.
        let opener = look(&analysed(2, 124.0, 8), &[Function::Opener], None, now);
        assert_eq!(opener.phase_fit, Some(0.2));
        assert!(
            opener.phase_fit < untagged.phase_fit,
            "an opener at peak ranked above a record nobody has described"
        );
    }

    /// The phase fit follows the night, and every phase has something it wants.
    #[test]
    fn each_phase_has_records_that_are_for_it() {
        for phase in SessionPhase::ALL {
            let anything = SessionPhase::ALL
                .iter()
                .flat_map(|_| Function::ALL)
                .any(|f| fit(phase, &[f]) >= 1.0);
            assert!(anything, "nothing is for {phase:?}");
        }
        assert_eq!(fit(SessionPhase::Peak, &[Function::Peak]), 1.0);
        assert_eq!(fit(SessionPhase::WarmUp, &[Function::Opener]), 1.0);
        assert_eq!(fit(SessionPhase::ChillOut, &[Function::Closer]), 1.0);
        // Safe works anywhere, which is what the tag means.
        assert!(fit(SessionPhase::Peak, &[Function::Safe]) >= 0.6);
    }

    /// **A record cannot follow itself.**
    ///
    /// The row for the record on the deck gets no "likely next" and no risk —
    /// a score there would be an answer to a question nobody can ask, sitting
    /// in a column of answers to real ones. Found by turning the lens on with
    /// a record playing and reading the table: the playing record scored 0.84
    /// as a follow-on to itself.
    #[test]
    fn the_record_that_is_playing_is_not_ranked_against_itself() {
        let on_deck = analysed(7, 124.0, 8);
        let now = Now {
            playing: Some(&playing(124.0, 8)),
            playing_id: Some(on_deck.id),
            ..quiet()
        };

        let seen = look(&on_deck, &[], None, now);
        assert_eq!(seen.likely_next, None, "a record was ranked against itself");
        assert!(
            seen.risks.is_empty(),
            "a risk in mixing a record into itself"
        );

        // Every other record is still judged.
        let other = look(&analysed(8, 125.0, 8), &[], None, now);
        assert!(other.likely_next.unwrap() > 0.7);
    }
}
