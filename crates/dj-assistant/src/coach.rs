//! Watching a DJ work, and saying something useful about it.
//!
//! A learner is not an expert with fewer features, and a manual is not
//! teaching. What actually moves somebody forward, in order:
//!
//! 1. **A word for what the hand just did.** Most of early DJing is doing
//!    something that worked and not knowing what it was called, which means
//!    not being able to do it on purpose.
//! 2. **The specific error.** "That was off" teaches nothing. "You came in
//!    three beats before the phrase, which is why the drums fought" is a thing
//!    that can be done differently next time.
//! 3. **One thing at a time**, and the thing is a doing.
//!
//! # Why this reads the action log rather than the audio
//!
//! Because every action is already timestamped on one bus
//! ([ADR-0003](../../../docs/adr/0003-action-bus-and-parameter-registry.md)),
//! so what the DJ did is *known*, exactly, with no detection involved. A
//! listener trying to infer a bass swap from a spectrum would be guessing at
//! something the log states outright — and would be wrong in exactly the noisy,
//! unarguable way that makes a learner distrust the whole feature.
//!
//! What the log cannot say is whether it *sounded* good, and this module
//! therefore does not claim to. It reports what happened and where it sat
//! against the grid. That is a smaller claim than most software makes and it
//! is one that holds.
//!
//! # Why nothing here is scored
//!
//! No marks, no streaks, no percentage. A DJ practising at home is not
//! revising for an exam, and a number attached to a mix invites playing for
//! the number. Observations and notes; the DJ decides what they mean.

use crate::technique;
use dj_core::{Action, DeckAction, DeckId, MixerAction};
use std::time::Duration;

/// One thing that happened, with when.
///
/// The coach's input. Deliberately not `dj_control::TimedEvent`: this crate
/// has no business knowing about the bus, and a pair is all the recogniser
/// needs. The app maps one to the other.
#[derive(Debug, Clone, PartialEq)]
pub struct Moment {
    pub at: Duration,
    pub action: Action,
}

impl Moment {
    #[must_use]
    pub fn new(at: Duration, action: Action) -> Self {
        Self { at, action }
    }
}

/// Where a deck sat against its phrase when something happened.
///
/// Supplied by the caller rather than derived, because the coach has no decks.
/// `beat_within_phrase` is 0 at the top of a breath.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Footing {
    pub deck: DeckId,
    pub beat_within_phrase: u32,
    pub phrase_beats: u32,
    /// Seconds of the record still to run.
    pub remaining: f32,
}

/// A technique the coach recognised, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct Observed {
    pub technique: &'static technique::Technique,
    pub at: Duration,
}

/// Something worth saying, once.
///
/// Three fields and not one string, because the three do different jobs: a DJ
/// who reads only `what` still learns something, and a DJ who is mid-set reads
/// only `fix`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// What happened.
    pub what: String,
    /// Why it sounded the way it did.
    pub why: String,
    /// What to do differently.
    pub fix: String,
}

/// How close to a phrase boundary still counts as on it.
///
/// One beat either side. Half a beat would fail DJs whose grid is a few
/// milliseconds out through no fault of theirs; four beats would call a whole
/// bar of lateness correct, and a bar late is audible to everybody in the
/// room.
const PHRASE_SLACK: u32 = 1;

/// How long two EQ moves may be apart and still be one gesture.
///
/// Two seconds. A bass swap done deliberately is two hands moving nearly
/// together; two records whose lows were adjusted a minute apart are not a
/// swap, and calling them one would teach the wrong name for the wrong thing.
const ONE_GESTURE: Duration = Duration::from_secs(2);

/// A low EQ at or below this is out of the way.
const LOW_IS_OUT: f32 = 0.25;

/// A low EQ at or above this is in the room.
const LOW_IS_IN: f32 = 0.75;

/// Name what happened in this stretch of the log.
///
/// Order is the order things happened, so reading the list back is watching
/// the mix again.
#[must_use]
pub fn observe(moments: &[Moment]) -> Vec<Observed> {
    let mut seen = Vec::new();

    for (i, moment) in moments.iter().enumerate() {
        let Action::Deck { deck, action } = &moment.action else {
            continue;
        };

        // The moves that are their own name. Nothing to infer: the DJ pressed
        // the thing, and the log says so.
        let named = match action {
            DeckAction::Backspin(_) => Some("backspin"),
            DeckAction::Brake(_) => Some("brake"),
            DeckAction::LoopRoll(_) => Some("loop roll"),
            DeckAction::SetCensor(true) => Some("censor"),
            DeckAction::PhraseJump(_) => Some("phrase jump"),
            DeckAction::LoopPhrases(_) => Some("loop the intro"),
            DeckAction::Sync | DeckAction::SyncToggle => Some("sync"),
            _ => None,
        };
        if let Some(name) = named
            && let Some(t) = technique::by_name(name)
        {
            seen.push(Observed {
                technique: t,
                at: moment.at,
            });
            continue;
        }

        // A bass swap is not an action; it is two actions meaning one thing.
        // One deck's low leaves as another's arrives, close enough together to
        // have been one intention.
        if let DeckAction::SetEqLow(level) = action
            && *level <= LOW_IS_OUT
            && let Some(t) = technique::by_name("bass swap")
            && moments[i + 1..]
                .iter()
                .take_while(|later| later.at.saturating_sub(moment.at) <= ONE_GESTURE)
                .any(|later| {
                    matches!(
                        &later.action,
                        Action::Deck { deck: other, action: DeckAction::SetEqLow(up) }
                            if other != deck && *up >= LOW_IS_IN
                    )
                })
        {
            seen.push(Observed {
                technique: t,
                at: moment.at,
            });
        }
    }

    seen
}

/// What is worth saying about how a record was brought in.
///
/// One note at most, because §12's rule is one thing at a time: a learner
/// handed three corrections applies none of them.
#[must_use]
pub fn critique_entry(footing: Footing) -> Option<Note> {
    let off = off_phrase(footing.beat_within_phrase, footing.phrase_beats);
    if off == 0 {
        return None;
    }

    let (side, beats) = if off > 0 {
        ("after", off.unsigned_abs())
    } else {
        ("before", off.unsigned_abs())
    };
    let plural = if beats == 1 { "" } else { "s" };

    Some(Note {
        what: format!(
            "You came in {beats} beat{plural} {side} the phrase on deck {}.",
            // `human_number`, not `index() + 1`: the 1-based convention has
            // one home in `DeckId` and a second copy here would eventually
            // disagree with it in exactly one message.
            footing.deck.human_number()
        ),
        // The specific reason, not a verdict. A learner who is told *why*
        // it fought can hear it happening next time.
        why: "The two records' phrases were offset, so their drums and \
              accents landed in different places and fought each other."
            .to_string(),
        fix: "A phrase is a breath. Wait for the next one rather than \
              coming in mid-breath — the phrase jump moves a whole breath \
              at a time, so it lands in the right place by itself."
            .to_string(),
    })
}

/// Signed distance to the nearest phrase boundary, in beats.
///
/// Negative is early, positive is late, zero is close enough. Signed because
/// early and late are different mistakes made for different reasons — early is
/// nerves about dead air, late is not having decided.
#[must_use]
fn off_phrase(beat_within_phrase: u32, phrase_beats: u32) -> i32 {
    if phrase_beats == 0 {
        return 0;
    }
    let beat = i64::from(beat_within_phrase % phrase_beats);
    let len = i64::from(phrase_beats);
    // Late from the last boundary, or early for the next -- whichever is
    // nearer. A record two beats before the next phrase is early by two, not
    // late by thirty.
    let late = beat;
    let early = beat - len;
    let off = if late <= -early { late } else { early };
    if off.unsigned_abs() <= u64::from(PHRASE_SLACK) {
        0
    } else {
        // Safe: bounded by phrase_beats, which is a u32 beat count.
        i32::try_from(off).unwrap_or(0)
    }
}

/// What is worth saying about a mix that has both basses up.
///
/// The most common blend mistake and the one nobody hears in headphones,
/// because two lows sum to something that only sounds wrong on a system with
/// real low end -- which is to say, in front of people.
#[must_use]
pub fn critique_lows(a: f32, b: f32) -> Option<Note> {
    if a < LOW_IS_IN || b < LOW_IS_IN {
        return None;
    }
    Some(Note {
        what: "Both records have their bass up.".to_string(),
        why: "Two basses do not add up to more bass. They add up to mud — \
              and it will sound fine in the headphones and wrong in the room."
            .to_string(),
        fix: "A riverbed has room for one channel. Pull one low down as the \
              other comes up, so the bass hands over rather than doubling."
            .to_string(),
    })
}

/// The one thing to work on next, given what the DJ can already do.
///
/// Named techniques are what they have shown; the answer is the easiest thing
/// they have not. Easiest, not most impressive: a learner sent at a flare
/// scratch after two nights stops being a learner.
#[must_use]
pub fn next_lesson(shown: &[&str], rig: technique::Rig) -> Option<&'static technique::Technique> {
    technique::catalogue()
        .iter()
        .filter(|t| rig.allows(t.needs))
        .filter(|t| !shown.iter().any(|s| s.eq_ignore_ascii_case(t.name)))
        .min_by_key(|t| (t.difficulty, t.name))
}

/// Whether a crossfader move was a cut or a blend.
///
/// The distinction a learner most wants named, and the log answers it exactly:
/// how far it travelled, and in how long.
#[must_use]
pub fn crossfade_shape(moments: &[Moment]) -> Option<&'static technique::Technique> {
    let moves: Vec<_> = moments
        .iter()
        .filter_map(|m| match m.action {
            Action::Mixer(MixerAction::Crossfader(x)) => Some((m.at, x)),
            _ => None,
        })
        .collect();

    let first = moves.first()?;
    let last = moves.last()?;
    let travelled = (last.1 - first.1).abs();
    // Under a third of the way across is not a transition at all -- it is a
    // DJ leaning on the fader.
    if travelled < 0.33 {
        return None;
    }

    let took = last.0.saturating_sub(first.0);
    // Two seconds is roughly four beats at a danceable tempo: fast enough
    // that the room hears an edit rather than a mix.
    if took <= Duration::from_secs(2) {
        technique::by_name("cut")
    } else {
        technique::by_name("long blend")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::DeckId;

    /// By the number a DJ sees, so a test that says `deck(2)` means deck 2.
    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).expect("valid deck")
    }

    fn at(secs: f32, action: Action) -> Moment {
        Moment::new(Duration::from_secs_f32(secs), action)
    }

    fn eq_low(d: u8, level: f32) -> Action {
        Action::Deck {
            deck: deck(d),
            action: DeckAction::SetEqLow(level),
        }
    }

    /// **A move the DJ pressed is named, not guessed at.**
    #[test]
    fn the_moves_that_are_their_own_name_are_named() {
        let log = vec![
            at(
                1.0,
                Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Backspin(None),
                },
            ),
            at(
                2.0,
                Action::Deck {
                    deck: deck(1),
                    action: DeckAction::LoopRoll(Some(0.25)),
                },
            ),
        ];
        let names: Vec<_> = observe(&log).iter().map(|o| o.technique.name).collect();
        assert_eq!(names, vec!["backspin", "loop roll"]);
    }

    /// **A bass swap is two hands moving together, and is recognised as one
    /// thing.**
    ///
    /// This is the case the whole recogniser exists for: the DJ did something
    /// that worked and has no word for it.
    #[test]
    fn one_low_leaving_as_another_arrives_is_a_bass_swap() {
        let log = vec![at(10.0, eq_low(1, 0.0)), at(10.4, eq_low(2, 1.0))];
        let names: Vec<_> = observe(&log).iter().map(|o| o.technique.name).collect();
        assert_eq!(names, vec!["bass swap"]);
    }

    /// **Both lows going up is not a swap — it is the mistake.**
    ///
    /// The one recognition that must not be generous. A swap is a handover:
    /// one low leaves as the other arrives. Two lows arriving together is
    /// precisely what `critique_lows` exists to warn about, and naming it
    /// "bass swap" would have the coach congratulating a DJ for the most
    /// common blend error there is.
    #[test]
    fn two_lows_arriving_together_is_not_a_handover() {
        let log = vec![at(10.0, eq_low(1, 1.0)), at(10.4, eq_low(2, 1.0))];
        assert!(
            observe(&log).is_empty(),
            "the coach named the mistake it is supposed to catch"
        );
        // And the other half of the pair does catch it.
        assert!(critique_lows(1.0, 1.0).is_some());
    }

    /// **Two unrelated EQ moves are not a bass swap.**
    ///
    /// A minute apart they are two decisions, and naming them one would teach
    /// the wrong word for the wrong thing — worse than saying nothing.
    #[test]
    fn lows_moved_a_minute_apart_are_two_decisions() {
        let log = vec![at(10.0, eq_low(1, 0.0)), at(70.0, eq_low(2, 1.0))];
        assert!(observe(&log).is_empty());
    }

    /// **Turning one deck's own low down and up again is not a swap either.**
    #[test]
    fn one_deck_riding_its_own_low_is_not_a_swap() {
        let log = vec![at(10.0, eq_low(1, 0.0)), at(10.4, eq_low(1, 1.0))];
        assert!(observe(&log).is_empty());
    }

    /// **The note names the number of beats and which side.**
    ///
    /// Early and late are different mistakes: early is nerves about dead air,
    /// late is not having decided. A note that said only "off" would lose the
    /// half of the message a DJ can act on.
    #[test]
    fn the_error_is_specific_about_how_far_and_which_way() {
        let early = critique_entry(Footing {
            deck: deck(2),
            beat_within_phrase: 29,
            phrase_beats: 32,
            remaining: 60.0,
        })
        .expect("three beats early is worth saying");
        assert!(early.what.contains('3'), "{}", early.what);
        assert!(early.what.contains("before"), "{}", early.what);
        assert!(early.what.contains("deck 2"), "{}", early.what);

        let late = critique_entry(Footing {
            deck: deck(1),
            beat_within_phrase: 4,
            phrase_beats: 32,
            remaining: 60.0,
        })
        .expect("four beats late is worth saying");
        assert!(late.what.contains("after"), "{}", late.what);
        assert!(late.what.contains('4'), "{}", late.what);
    }

    /// **On the phrase, the coach says nothing.**
    ///
    /// Including one beat either side. A DJ whose grid is a few milliseconds
    /// out is not making a mistake, and being corrected for one is how people
    /// learn to ignore the coach.
    #[test]
    fn a_mix_that_landed_is_left_alone() {
        for beat in [0, 1, 31] {
            assert_eq!(
                critique_entry(Footing {
                    deck: deck(1),
                    beat_within_phrase: beat,
                    phrase_beats: 32,
                    remaining: 60.0,
                }),
                None,
                "beat {beat} of 32 was corrected"
            );
        }
    }

    /// **Nearness wraps.**
    ///
    /// Two beats before the next phrase is early by two, not late by thirty.
    /// Getting this wrong would tell a DJ who was very slightly early that
    /// they were most of a phrase late.
    #[test]
    fn lateness_is_measured_to_the_nearest_boundary_either_way() {
        assert_eq!(off_phrase(30, 32), -2);
        assert_eq!(off_phrase(2, 32), 2);
        assert_eq!(off_phrase(16, 32), 16);
    }

    #[test]
    fn a_phrase_of_no_length_is_not_a_phrase() {
        assert_eq!(off_phrase(5, 0), 0);
    }

    /// **Both basses up is caught; one is not.**
    #[test]
    fn two_basses_at_once_are_worth_mentioning() {
        assert!(critique_lows(1.0, 1.0).is_some());
        assert!(critique_lows(1.0, 0.0).is_none());
        assert!(critique_lows(0.5, 0.5).is_none());
    }

    /// **The next lesson is the easiest thing not yet shown.**
    ///
    /// Not the most impressive. A learner sent at a transformer scratch after
    /// two nights stops being a learner.
    #[test]
    fn the_next_thing_to_learn_is_the_easiest_one_left() {
        let next = next_lesson(&[], technique::Rig::laptop()).expect("something to learn");
        assert_eq!(next.difficulty, technique::Difficulty::First);
    }

    /// **What has been shown is not offered again.**
    #[test]
    fn a_technique_already_shown_is_not_the_next_lesson() {
        let rig = technique::Rig::laptop();
        let first = next_lesson(&[], rig).expect("something to learn");
        let second = next_lesson(&[first.name], rig).expect("something else to learn");
        assert_ne!(first.name, second.name);
    }

    /// **The lesson is one the DJ's rig can actually perform.**
    #[test]
    fn nothing_is_set_as_homework_that_the_rig_cannot_do() {
        // Every technique this laptop can do has been shown, so the only
        // things left need hardware -- and there should be nothing to offer.
        let rig = technique::Rig::laptop();
        let shown: Vec<&str> = technique::catalogue()
            .iter()
            .filter(|t| rig.allows(t.needs))
            .map(|t| t.name)
            .collect();
        assert_eq!(next_lesson(&shown, rig), None);
    }

    /// **A slow fader is a blend and a fast one is a cut.**
    #[test]
    fn the_shape_of_a_crossfade_says_which_move_it_was() {
        let quick = vec![
            at(0.0, Action::Mixer(MixerAction::Crossfader(-1.0))),
            at(0.2, Action::Mixer(MixerAction::Crossfader(1.0))),
        ];
        assert_eq!(crossfade_shape(&quick).map(|t| t.name), Some("cut"));

        let slow = vec![
            at(0.0, Action::Mixer(MixerAction::Crossfader(-1.0))),
            at(30.0, Action::Mixer(MixerAction::Crossfader(1.0))),
        ];
        assert_eq!(crossfade_shape(&slow).map(|t| t.name), Some("long blend"));
    }

    /// **Leaning on the fader is not a transition.**
    #[test]
    fn a_fader_that_barely_moved_is_not_named_at_all() {
        let nudge = vec![
            at(0.0, Action::Mixer(MixerAction::Crossfader(0.0))),
            at(0.2, Action::Mixer(MixerAction::Crossfader(0.1))),
        ];
        assert_eq!(crossfade_shape(&nudge), None);
        assert_eq!(crossfade_shape(&[]), None);
    }
}
