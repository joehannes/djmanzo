//! What the phase of the night asks of what djmanzo offers.
//!
//! [§17 of the directive](../../../docs/DIRECTIVE.md) is a list of six phases
//! and what each one should prioritise, and most of its entries are surfaces:
//! [`crate::cockpit::priorities`] is that half, and it opens panels. The
//! entries that are **not** surfaces are here.
//!
//! > ### Warm-up
//! > Prioritize: next-track candidates, **gradual energy**, longer
//! > transitions, understated visualization, low suggestion noise.
//! > ### Build
//! > Prioritize: compatible candidates, transitions, phrase structure,
//! > **energy trajectory**.
//! > ### Release
//! > Prioritize: **harmonic resolution**, **crowd cooling**, longer blends,
//! > reduced visual intensity.
//! > ### Closing
//! > Prioritize: **known anchors**, requests, end-of-set state, recording,
//! > history.
//!
//! Those five words are instructions about *what djmanzo should offer*, and
//! until this module existed nothing anywhere read them. The rail ranked every
//! night identically and defaulted to *Hold* from the first record to the last,
//! whatever the night was doing.
//!
//! # What is here and what is deliberately not
//!
//! **Longer transitions** and **longer blends** are already true and were
//! already true before this file: [`crate::plan`] proposes the longest
//! transition that leaves its tail margin intact, every time. A knob here
//! saying "prefer longer" would be a second description of a decision the
//! planner already makes that way, and the copy is the one that goes stale.
//!
//! **Low suggestion noise** belongs to §18 and is already a number:
//! `cockpit::Attention` sizes the rail, and two owners of how many suggestions
//! a DJ sees is exactly the shape this codebase keeps having to undo. It is
//! worth saying plainly that §18's budget is *widest* in warm-up, which is not
//! what §17's phrase asks for — §18 sizes by what the DJ is doing (nothing is
//! mixing yet, so there is room to think) and §17 by what the night is doing.
//! Reconciling them is a decision for whoever owns the budget, not a second
//! budget here.
//!
//! **Immediate control**, **stems** and **FX** are the deck, and a peak that
//! demanded the ranking do something about them would be answering the wrong
//! question.
//!
//! # The one rule the whole section rests on
//!
//! > The system may infer phase, but the DJ must always be able to override it.
//!
//! So nothing here decides anything. It is what djmanzo *starts from*: the rail
//! follows the night until the DJ presses a direction, and from then on the
//! direction is theirs for the rest of the night. That press is not a setting
//! to be found in a panel — it is the control that was already there.

use dj_core::{SessionPhase, Trajectory};

/// The one thing a phase asks the ranking to prefer, beyond a direction.
///
/// One thing, not a set. Two of §17's phases name something and four name
/// nothing, and a phase that asked for three would be asking the rail to be a
/// filter — which is not what any of these words mean and is not what the
/// weight they are worth can do. See [`Asks::PREFERENCE_IS_ONE_TILT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefer {
    /// The direction, and nothing else.
    ///
    /// Four of the six. A phase that credited something anyway would put a chip
    /// on every row of every rail saying nothing.
    Nothing,
    /// §17's *harmonic resolution*, at **Release**.
    ///
    /// Said as what it actually is rather than as more than it is: a key that
    /// **settles rather than travels**. Landing a set back on the key it opened
    /// in is the fuller meaning of the phrase and djmanzo does not know a set's
    /// root; what it does know is whether the next record agrees with the one
    /// playing, and preferring agreement while the floor is coming down is the
    /// honest half of the instruction.
    Resolution,
    /// §17's *known anchors*, at **Closing**.
    ///
    /// A record this DJ has actually played. Not a chart position and not a
    /// rating — an anchor is a record *this room* has heard this DJ play, and
    /// the only evidence djmanzo has of that is its own play count.
    Anchors,
}

impl Prefer {
    /// What to say on the row this preference credited, in the DJ's terms.
    ///
    /// `None` for [`Prefer::Nothing`], which is how a fold knows there is
    /// nothing to credit: the words and the credit are the same fact, so a
    /// phase cannot end up silently tilting a rail with nothing on screen
    /// saying why — which is the shape a rail reordering itself for no visible
    /// reason takes, and it reads as the ranking being broken.
    #[must_use]
    pub const fn words(self) -> Option<&'static str> {
        match self {
            Self::Nothing => None,
            Self::Resolution => Some("a key that settles"),
            Self::Anchors => Some("a record you play"),
        }
    }
}

/// What one phase asks of the ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asks {
    /// Which way the rail ranks, until the DJ presses a direction themselves.
    pub trajectory: Trajectory,
    /// The one thing this phase asks for beyond that direction.
    pub prefer: Prefer,
    /// §17's own words for this row, for the line the rail draws.
    ///
    /// Quoted rather than paraphrased: the rail is saying *why it is ranking
    /// this way*, and the answer is that the directive says so. A sentence
    /// invented here would be djmanzo explaining itself in words nobody can
    /// check against anything.
    pub words: &'static str,
}

impl Asks {
    /// The most a phase's preference may move a candidate's score.
    ///
    /// Three quarters of a point — the same bound taste, §81's profile and
    /// §16's knowledge pack get, quoted from the same scale a same-key match is
    /// worth three on and a key clash minus two and a half. It is the fourth tilt
    /// on one score, and
    /// `commands::chosen_pack_tests::every_tilt_at_once_still_cannot_cross_a_key_relation`
    /// is where the four are held below what a single key relation is worth.
    ///
    /// **A phase asks. It does not overrule the mixing**, and it does not
    /// overrule the DJ: this is the smallest thing in §17, because a reading
    /// djmanzo made about the night is the weakest evidence on the rail.
    pub const PREFERENCE_IS_ONE_TILT: f64 = 0.75;
}

/// What §17's phase asks of the ranking.
///
/// `None` is §17's **Setup** — the stretch before anything has read as a phase,
/// which `dj_core::context` produces honestly at the start of every night. The
/// mapping from §17's six names onto djmanzo's five plus that unread state is
/// [`crate::cockpit::priorities`]'s, stated once there.
#[must_use]
pub fn asks(phase: Option<SessionPhase>) -> Asks {
    match phase {
        // Setup. Nothing is playing, so there is no trajectory to have, and
        // §17's setup list names no energy word at all. Holding is the answer
        // that adds nothing rather than the answer that guesses.
        None => Asks {
            trajectory: Trajectory::Hold,
            prefer: Prefer::Nothing,
            words: "setting up",
        },
        // Warm-up: *gradual energy*. Gradual energy is still energy going up.
        // The *gradual* half is the transition length, which §17 names in the
        // same breath — longer transitions — and which the planner already
        // gives by proposing the longest that fits.
        Some(SessionPhase::WarmUp) => Asks {
            trajectory: Trajectory::Lift,
            prefer: Prefer::Nothing,
            words: "gradual energy",
        },
        // Build: *energy trajectory*. The same direction as warm-up, and that
        // is the honest answer rather than a difference invented to make the
        // table look discriminating: both phases are the night going up. What
        // separates them in §17 is what is on screen and how long the mixes
        // are, and both of those are somebody else's column.
        Some(SessionPhase::Heat) => Asks {
            trajectory: Trajectory::Lift,
            prefer: Prefer::Nothing,
            words: "an energy trajectory",
        },
        // Peak: §17 names no energy word here, and that absence is the
        // instruction. A peak that kept asking for louder would run out of
        // room — there is nowhere above the top — so the rail holds, which is
        // what staying at a peak is.
        Some(SessionPhase::Peak) => Asks {
            trajectory: Trajectory::Hold,
            prefer: Prefer::Nothing,
            words: "holding the top",
        },
        // Release: *crowd cooling*, and *harmonic resolution* with it.
        Some(SessionPhase::Cooldown) => Asks {
            trajectory: Trajectory::Ease,
            prefer: Prefer::Resolution,
            words: "crowd cooling, and a key that settles",
        },
        // Closing: *known anchors*. Still easing — a closing set that climbed
        // would be a closing set that had not noticed — and now preferring the
        // records this DJ actually plays.
        Some(SessionPhase::ChillOut) => Asks {
            trajectory: Trajectory::Ease,
            prefer: Prefer::Anchors,
            words: "known anchors",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every phase has a row, and the two §17 names something for are the
    /// two that ask for something.**
    ///
    /// The load-bearing shape of the table rather than its contents: a row
    /// added for a new phase with `Prefer::Nothing` and no words would compile,
    /// serialise and do nothing, which is the failure this codebase keeps
    /// meeting from the other side.
    #[test]
    fn every_phase_asks_for_something_and_says_so_in_the_directives_words() {
        for phase in SessionPhase::ALL {
            let row = asks(Some(phase));
            assert!(
                !row.words.trim().is_empty(),
                "{phase:?} asks for something and does not say what"
            );
        }
        assert!(!asks(None).words.trim().is_empty());

        // The two §17 names a preference for, and only those two.
        assert_eq!(
            asks(Some(SessionPhase::Cooldown)).prefer,
            Prefer::Resolution
        );
        assert_eq!(asks(Some(SessionPhase::ChillOut)).prefer, Prefer::Anchors);
        for phase in [SessionPhase::WarmUp, SessionPhase::Heat, SessionPhase::Peak] {
            assert_eq!(
                asks(Some(phase)).prefer,
                Prefer::Nothing,
                "{phase:?} prefers something §17 does not name for it"
            );
        }
        assert_eq!(asks(None).prefer, Prefer::Nothing);
    }

    /// **A preference that credits a record says why, and one that credits
    /// nothing says nothing.**
    ///
    /// The two halves are one fact. A `Prefer` that scored without words would
    /// reorder a rail for a cause nothing on screen names, and one with words
    /// but no credit would be a chip on a row that had not moved.
    #[test]
    fn every_preference_that_asks_for_something_has_words_for_it() {
        assert_eq!(Prefer::Nothing.words(), None);
        for prefer in [Prefer::Resolution, Prefer::Anchors] {
            let words = prefer.words().expect("a preference that asks says what");
            assert!(!words.trim().is_empty());
        }
    }

    /// **The night rises, holds, and comes down — in that order.**
    ///
    /// §17's five energy words in the order its own sections are in. A table
    /// that eased during the build or lifted at the close would be readable,
    /// compile and be wrong about the one thing it is for, and the failure
    /// would show up as a rail quietly offering the wrong half of a library at
    /// the worst moment of a night.
    #[test]
    fn the_arc_of_the_night_is_up_then_level_then_down() {
        let direction = |phase| asks(Some(phase)).trajectory;
        assert_eq!(direction(SessionPhase::WarmUp), Trajectory::Lift);
        assert_eq!(direction(SessionPhase::Heat), Trajectory::Lift);
        assert_eq!(direction(SessionPhase::Peak), Trajectory::Hold);
        assert_eq!(direction(SessionPhase::Cooldown), Trajectory::Ease);
        assert_eq!(direction(SessionPhase::ChillOut), Trajectory::Ease);
        // And nothing read yet is not "the night is going up".
        assert_eq!(asks(None).trajectory, Trajectory::Hold);
    }

    /// The tilt is quoted from the same scale as the other three, and is the
    /// smallest claim on the rail rather than a bigger one.
    #[test]
    fn a_phase_asks_for_no_more_than_the_dj_themselves_gets() {
        assert!(
            (Asks::PREFERENCE_IS_ONE_TILT - dj_library::learned::Learned::MOST_IT_MAY_MOVE).abs()
                < f64::EPSILON,
            "a reading djmanzo made about the night outweighs what the DJ plays"
        );
    }
}
