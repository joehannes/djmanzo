//! A bundle of moves the assistant has prepared, offered before any of it
//! happens.
//!
//! [§44 of the directive](../../../docs/DIRECTIVE.md) asks for non-trivial AI
//! action to be a **transaction**: load the record, cue it, match its gain,
//! engage sync, run the mix — staged together, shown as one thing with Accept,
//! Modify and Reject, and only then carried out as ordinary actions that log
//! and replay like everything else.
//!
//! # Why a bundle rather than five decisions
//!
//! The autopilot already decides one step at a time and that is right for a
//! machine doing the work. It is wrong for a machine *asking*: a DJ shown
//! "load deck 2" has been told almost nothing, because the question they are
//! actually answering is whether the next three minutes should go the way
//! djmanzo thinks. Five separate prompts is also five chances to say yes to
//! half a plan, which is the state nobody wants — a record loaded and cued
//! that nothing is going to mix.
//!
//! # Accepting is not a second way of doing things
//!
//! Every step is a [`crate::autopilot::Step`] and accepting runs it through
//! `commands::perform_step`, the same function the automatic tick uses. There
//! is deliberately no execution path here at all. A transaction that carried
//! out its own version of a load would be a second implementation to keep in
//! agreement with the first, and the first one is the one under test.
//!
//! # Three gates, and this asks the second
//!
//! Every step names the [`Capability`] it belongs to and carries what the
//! current posture allows — see `dj_assistant::authority` for why that is a
//! table rather than a scattering of checks. A step the posture refuses is
//! **shown and disabled**, never hidden: "the machine wants to do this and your
//! level does not allow it" is a thing a DJ should be able to see, and the
//! obvious response — turn it up one notch — is unavailable to somebody who was
//! never told.
//!
//! # It goes stale rather than going wrong
//!
//! A transaction is built against a particular record on a particular deck. If
//! that record leaves, or another one arrives, the plan is about a set that no
//! longer exists. It is dropped rather than drawn — the same rule
//! [`crate::transition`] follows, for the same reason.

use dj_assistant::{Allowance, Authority, Capability, Posture};
use dj_core::{DeckId, TrackId};
use serde::Serialize;

use crate::autopilot::{CueTo, Step};

/// One move inside a transaction.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Move {
    /// What it does, in one line, for the person deciding.
    pub about: String,
    /// The row of §72's matrix it belongs to.
    pub capability: String,
    /// What the current posture allows for that row.
    pub allowance: String,
    /// Whether it will run when the transaction is accepted.
    ///
    /// This is what **Modify** is: a DJ who wants the record loaded and cued
    /// but intends to bring it in themselves turns the last move off rather
    /// than rejecting the whole plan and doing it all by hand.
    pub chosen: bool,
    #[serde(skip)]
    step: Step,
}

impl Move {
    /// Whether the posture permits this at all.
    #[must_use]
    pub fn permitted(&self) -> bool {
        self.allowance != Allowance::No.name()
    }
}

/// A prepared bundle, waiting for an answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Staged {
    /// What the whole thing is, in the words §44 uses.
    pub headline: String,
    /// Why now — the autopilot's own reasoning, not a restatement.
    pub because: String,
    pub moves: Vec<Move>,
    /// The deck the room is hearing, and what is on it.
    ///
    /// The staleness check. Serialised because the panel says which deck the
    /// plan is about, and hidden state a panel cannot see is state that
    /// surprises somebody.
    pub live_deck: u8,
    #[serde(skip)]
    live_track: Option<TrackId>,
}

impl Staged {
    /// Whether this plan is still about the set that is playing.
    #[must_use]
    pub fn still_current(&self, live_deck: DeckId, live_track: Option<TrackId>) -> bool {
        self.live_deck == live_deck.human_number() && self.live_track == live_track
    }

    /// Turn one move on or off. **Modify**, in §44's three words.
    ///
    /// A move the posture refuses cannot be turned on: the matrix is not a
    /// suggestion, and a checkbox that let a DJ tick past it would make the
    /// posture meaningless without ever saying so.
    ///
    /// # Errors
    /// When the index is not a move, or the posture refuses it.
    pub fn choose(&mut self, index: usize, chosen: bool) -> Result<(), String> {
        let entry = self
            .moves
            .get_mut(index)
            .ok_or_else(|| format!("there is no move {index}"))?;
        if chosen && !entry.permitted() {
            return Err(format!(
                "{} is not something this posture may do",
                entry.about
            ));
        }
        entry.chosen = chosen;
        Ok(())
    }

    /// The moves that will actually run, in order.
    #[must_use]
    pub fn chosen(&self) -> Vec<(usize, &Step)> {
        self.moves
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.chosen && entry.permitted())
            .map(|(index, entry)| (index, &entry.step))
            .collect()
    }

    /// Whether anything is left to do.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chosen().is_empty()
    }
}

/// What happened when a transaction was accepted.
///
/// Partial success is a real outcome and is reported as one. A load can fail
/// because the file moved; a mix can fail because a hand landed on the
/// crossfader in the same second. Reporting "accepted" and leaving a DJ to
/// discover that only half of it happened is the failure this type exists to
/// prevent.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Outcome {
    /// What was carried out, in order, in words.
    pub done: Vec<String>,
    /// The move that stopped it, and why. `None` means all of them ran.
    pub stopped: Option<Stopped>,
}

/// The move that failed, and what it said.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stopped {
    pub at: usize,
    pub about: String,
    pub because: String,
}

/// Build the bundle for what the assistant would do next.
///
/// `authority` and `posture` decide what each move is allowed to be; they do
/// not decide what is *in* the bundle. That distinction is deliberate: a DJ at
/// Prepare should still be shown that the mix is the fourth move and that their
/// level is why it is greyed, rather than being shown a three-move plan that
/// stops mid-air for no stated reason.
#[must_use]
pub fn build(
    situation: &crate::autopilot::Situation,
    plan: Option<&crate::plan::Plan>,
    authority: &Authority,
    posture: Posture,
    because: &str,
    live_track: Option<TrackId>,
) -> Option<Staged> {
    let idle = situation.idle?;
    let mut moves = Vec::new();

    let mut add = |step: Step, about: String, capability: Capability| {
        let allowance = authority.allows(capability, posture);
        moves.push(Move {
            about,
            capability: capability.name().to_owned(),
            allowance: allowance.name().to_owned(),
            // Everything the posture permits is on by default. A plan that
            // arrived with its moves switched off would be a plan the DJ has
            // to assemble, which is the work it exists to save.
            chosen: allowance.permits(),
            step,
        });
    };

    // Nothing staged yet: the load is the first move. Something staged
    // already: the DJ or an earlier tick put it there and it stays.
    if situation.staged.is_none() {
        let track = situation.next?;
        add(
            Step::Stage { deck: idle, track },
            format!("Load the next record onto deck {}", idle.human_number()),
            Capability::LoadNextDeck,
        );
    }

    add(
        Step::Cue {
            deck: idle,
            at: CueTo::PhraseStart,
        },
        format!(
            "Cue deck {} to the start of its first phrase",
            idle.human_number()
        ),
        Capability::SetCue,
    );

    if let Some(db) = situation.gain_offset_db.filter(|db| db.abs() > 0.5) {
        add(
            Step::MatchGain { deck: idle, db },
            format!(
                "Trim deck {} by {db:+.1} dB to match deck {}",
                idle.human_number(),
                situation.live.human_number()
            ),
            Capability::GainMatch,
        );
    }

    add(
        Step::Sync { deck: idle },
        format!("Engage sync on deck {}", idle.human_number()),
        Capability::Sync,
    );

    if let Some(planned) = plan {
        let beats = situation
            .occasion
            .transition_beats()
            .min(planned.length_beats);
        add(
            Step::Mix {
                from: situation.live,
                to: idle,
                style: planned.style,
                beats,
            },
            format!(
                "Mix deck {} into deck {} over {beats} beats, {}",
                situation.live.human_number(),
                idle.human_number(),
                planned.style.as_str()
            ),
            Capability::Crossfader,
        );
    }

    (!moves.is_empty()).then(|| Staged {
        headline: "Prepared next transition".to_owned(),
        because: because.to_owned(),
        moves,
        live_deck: situation.live.human_number(),
        live_track,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{Incoming, Outgoing};
    use dj_assistant::Occasion;
    use dj_core::{Phrase, SampleRate};

    fn deck(number: u8) -> DeckId {
        DeckId::from_human(number).expect("a deck")
    }

    fn situation(staged: bool) -> crate::autopilot::Situation {
        crate::autopilot::Situation {
            posture: Posture::Autopilot,
            occasion: Occasion::Open,
            certainty: dj_core::Certainty::Fair,
            live: deck(1),
            outgoing: Outgoing {
                position: 0.0,
                length: 48_000.0 * 300.0,
                bpm: 124.0,
                phrase: Phrase::new(16, 0),
                key: None,
                sample_rate: SampleRate::DEFAULT,
                grid_anchor: 0.0,
            },
            idle: Some(deck(2)),
            staged: staged.then(|| {
                (
                    TrackId::from_bytes([7; 32]),
                    Incoming {
                        bpm: 124.0,
                        phrase: Phrase::new(16, 0),
                        key: None,
                    },
                )
            }),
            next: Some(TrackId::from_bytes([9; 32])),
            gain_offset_db: Some(-2.5),
        }
    }

    fn built(posture: Posture) -> Staged {
        let situation = crate::autopilot::Situation {
            posture,
            ..situation(false)
        };
        let planned = crate::plan::plan(
            &situation.outgoing,
            &Incoming {
                bpm: 124.0,
                phrase: Phrase::new(16, 0),
                key: None,
            },
        );
        build(
            &situation,
            planned.as_ref(),
            &Authority::new(),
            posture,
            "60s left on deck 1",
            Some(TrackId::from_bytes([1; 32])),
        )
        .expect("a situation with somewhere to go stages something")
    }

    /// §44's example, as a bundle: the load, the cue, the trim, sync and the
    /// mix, offered as one thing.
    #[test]
    fn the_whole_transition_is_staged_as_one_thing() {
        let staged = built(Posture::Autopilot);
        assert_eq!(staged.headline, "Prepared next transition");
        let rows: Vec<&str> = staged
            .moves
            .iter()
            .map(|entry| entry.capability.as_str())
            .collect();
        assert_eq!(
            rows,
            vec![
                "load_next_deck",
                "set_cue",
                "gain_match",
                "sync",
                "crossfader"
            ]
        );
        assert!(staged.moves.iter().all(|entry| entry.chosen));
        assert!(!staged.is_empty());
    }

    /// **A move the posture refuses is shown and disabled, never dropped.**
    ///
    /// The whole argument for putting the matrix on screen: a DJ at Prepare can
    /// see that the mix is the last move and that their level is why it will
    /// not happen. A plan that silently stopped after the trim would leave them
    /// wondering what djmanzo thinks the point of a cued deck is.
    #[test]
    fn a_move_the_posture_refuses_is_shown_greyed_rather_than_hidden() {
        let staged = built(Posture::Prepare);
        let mix = staged
            .moves
            .last()
            .expect("the mix is still in the plan at Prepare");
        assert_eq!(mix.capability, "crossfader");
        assert!(!mix.permitted());
        assert!(!mix.chosen, "a refused move was on by default");

        // And it cannot be ticked past.
        let mut staged = staged;
        let refused = staged.choose(staged.moves.len() - 1, true);
        assert!(refused.is_err(), "the matrix was ticked past");

        // What is left is the silent half, and it is real work: the load, the
        // cue and the trim. Sync is refused at Prepare too — it is audible the
        // moment the fader moves — so three, not four.
        assert_eq!(staged.chosen().len(), 3);
        let refused: Vec<&str> = staged
            .moves
            .iter()
            .filter(|entry| !entry.permitted())
            .map(|entry| entry.capability.as_str())
            .collect();
        assert_eq!(refused, vec!["sync", "crossfader"]);
    }

    /// Modify is turning a move off, and it leaves the rest intact.
    #[test]
    fn a_move_can_be_turned_off_without_rejecting_the_plan() {
        let mut staged = built(Posture::Autopilot);
        let before = staged.chosen().len();
        staged
            .choose(before - 1, false)
            .expect("turning off is fine");
        assert_eq!(staged.chosen().len(), before - 1);
        // And back on, because it is permitted.
        staged.choose(before - 1, true).expect("turning on is fine");
        assert_eq!(staged.chosen().len(), before);
        assert!(staged.choose(99, false).is_err());
    }

    /// A record already on the deck is not loaded twice.
    #[test]
    fn a_deck_that_already_has_the_record_is_not_loaded_again() {
        let situation = situation(true);
        let staged = build(
            &situation,
            None,
            &Authority::new(),
            Posture::Autopilot,
            "staged already",
            None,
        )
        .expect("there is still cueing and trimming to do");
        assert!(
            staged
                .moves
                .iter()
                .all(|entry| entry.capability != "load_next_deck"),
            "staged a load onto a deck that already had the record"
        );
    }

    /// A plan is about a particular record on a particular deck.
    #[test]
    fn a_plan_stops_being_current_when_the_record_changes() {
        let track = TrackId::from_bytes([1; 32]);
        let staged = built(Posture::Autopilot);
        assert!(staged.still_current(deck(1), Some(track)));
        assert!(
            !staged.still_current(deck(1), Some(TrackId::from_bytes([2; 32]))),
            "a plan about a record that has been ejected is still current"
        );
        assert!(!staged.still_current(deck(2), Some(track)));
    }

    /// Nowhere to put a record is nothing to prepare.
    #[test]
    fn nothing_is_staged_with_no_free_deck() {
        let situation = crate::autopilot::Situation {
            idle: None,
            ..situation(false)
        };
        assert!(
            build(
                &situation,
                None,
                &Authority::new(),
                Posture::Autopilot,
                "",
                None
            )
            .is_none()
        );
    }
}
