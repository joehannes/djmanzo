//! §72's override matrix: what the assistant may do, at every posture.
//!
//! Until this existed the answer was four booleans on [`Posture`] —
//! `may_act`, `may_stage`, `may_mix`, `may_speak` — and the trouble with
//! `may_act` in particular is that it answered one question for four different
//! powers. Nudging an EQ, riding the pitch fader, switching sync on and pulling
//! the crossfader are not the same permission, and a DJ who wants the machine
//! to beatmatch but not to touch the tone had no way to say so and no way to
//! find out what it would do.
//!
//! So the matrix is written out, one row per thing djmanzo can actually do, and
//! **the booleans are derived from it**. One table, and the predicates every
//! caller already uses answer out of it — otherwise this would be a second
//! description of the same rules, free to disagree with the first.
//!
//! # Rows djmanzo does not have
//!
//! §72's example table lists EQ adjustment, FX and layout adaptation. djmanzo
//! does none of those on its own today: the autopilot stages, gain-matches and
//! runs the crossfader, and nothing else. Those rows are here, and they are
//! here saying **`No` at every posture**, because a matrix that promised a
//! power nothing exercises would be a specification pretending to be a
//! description. When one of them is built, its row is where the permission goes
//! — and the test below is what notices if a row starts lying.

use crate::Posture;
use serde::{Deserialize, Serialize};

/// One thing the assistant might do.
///
/// Named for the *effect* rather than the mechanism: a DJ deciding whether the
/// machine may "set the cue" is not thinking about which command that is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// Offer a record, with reasons. Never loads it.
    RecommendTrack,
    /// Put the next record on the deck that is not playing.
    LoadNextDeck,
    /// Cue that deck to where the mix would start.
    SetCue,
    /// Trim its gain to match what is playing.
    GainMatch,
    /// Lock its tempo to the playing deck's.
    Sync,
    /// Move an EQ band.
    Eq,
    /// Switch an effect on, or drive one.
    Fx,
    /// Move the crossfader — which is to say, perform the mix.
    Crossfader,
    /// Choose *what* plays next rather than proposing it.
    ChooseTrack,
    /// Rearrange the cockpit.
    AdaptLayout,
}

/// How far a posture may go with one capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Allowance {
    /// Not at all.
    No,
    /// Only where the room cannot hear the mistake, or only when asked.
    ///
    /// §72 writes "limited" in two cells and does not say what it means. Here
    /// it means **on a deck that is silent**: staging, cueing and gain-matching
    /// a record nobody can hear yet is a different act from moving a control
    /// that is audible, and that line is the one `Prepare` is built on.
    Limited,
    /// Freely.
    Yes,
}

impl Allowance {
    /// Whether it may happen at all.
    #[must_use]
    pub const fn permitted(self) -> bool {
        !matches!(self, Self::No)
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::Limited => "limited",
            Self::Yes => "yes",
        }
    }
}

impl Capability {
    /// Every capability, in the order §72 lists them.
    pub const ALL: [Capability; 10] = [
        Capability::RecommendTrack,
        Capability::LoadNextDeck,
        Capability::SetCue,
        Capability::GainMatch,
        Capability::Sync,
        Capability::Eq,
        Capability::Fx,
        Capability::Crossfader,
        Capability::ChooseTrack,
        Capability::AdaptLayout,
    ];

    /// A short name a DJ reads, not an identifier.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RecommendTrack => "Recommend a track",
            Self::LoadNextDeck => "Load the next deck",
            Self::SetCue => "Set the cue",
            Self::GainMatch => "Match the gain",
            Self::Sync => "Sync",
            Self::Eq => "Move an EQ band",
            Self::Fx => "Drive an effect",
            Self::Crossfader => "Move the crossfader",
            Self::ChooseTrack => "Choose what plays next",
            Self::AdaptLayout => "Rearrange the cockpit",
        }
    }

    /// Whether djmanzo can do this at all yet.
    ///
    /// Three of §72's rows describe powers nothing in this application
    /// exercises. Saying so beside the row is the difference between a matrix
    /// that describes the software and one that describes an intention.
    #[must_use]
    pub const fn built(self) -> bool {
        !matches!(self, Self::Eq | Self::Fx | Self::AdaptLayout)
    }
}

impl Posture {
    /// How far this posture may go with one capability. §72's matrix.
    #[must_use]
    pub const fn allows(self, what: Capability) -> Allowance {
        use Allowance::{Limited, No, Yes};
        use Capability as C;
        use Posture as P;

        // Nothing at all, and `Watch` is silent by design -- it is for practice
        // you review afterwards, and commentary during is the thing being
        // avoided.
        if matches!(self, P::Off | P::Watch) {
            return No;
        }
        // Not built. Named rather than quietly permitted -- see `built`.
        if !what.built() {
            return No;
        }

        match what {
            // Everything that speaks may offer one.
            C::RecommendTrack => Yes,
            // Prepare is exactly this: ready the silent deck and stop.
            C::LoadNextDeck | C::SetCue | C::GainMatch => match self {
                P::Suggest => No,
                P::Prepare => Limited,
                _ => Yes,
            },
            // The first thing the room can hear.
            C::Sync => match self {
                P::Assist | P::Autopilot => Yes,
                _ => No,
            },
            // Performing the mix, and choosing what is in it: the two things
            // only Autopilot does.
            C::Crossfader | C::ChooseTrack => match self {
                P::Autopilot => Yes,
                _ => No,
            },
            C::Eq | C::Fx | C::AdaptLayout => No,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The matrix is the source of the booleans, not a copy of them.**
    ///
    /// These four predicates were the rules before this table existed and
    /// every caller still uses them. Written out here as they were, so a change
    /// to the matrix that would change what the assistant does fails loudly
    /// rather than silently altering an autopilot.
    #[test]
    fn the_derived_predicates_match_the_rules_they_replaced() {
        for posture in Posture::ALL {
            let was_act = matches!(posture, Posture::Assist | Posture::Autopilot);
            let was_stage = matches!(
                posture,
                Posture::Prepare | Posture::Assist | Posture::Autopilot
            );
            let was_mix = matches!(posture, Posture::Autopilot);

            assert_eq!(posture.may_act(), was_act, "may_act at {}", posture.name());
            assert_eq!(
                posture.may_stage(),
                was_stage,
                "may_stage at {}",
                posture.name()
            );
            assert_eq!(posture.may_mix(), was_mix, "may_mix at {}", posture.name());
        }
    }

    /// **A quieter posture is never allowed more.** The whole point of an
    /// ordered list of postures: a DJ who turns it down expects less, not
    /// different.
    #[test]
    fn nothing_is_more_permitted_at_a_quieter_posture() {
        for what in Capability::ALL {
            let mut previous = Allowance::No;
            for posture in Posture::ALL {
                let now = posture.allows(what);
                assert!(
                    now >= previous,
                    "{:?} is {} at {} but {} at the quieter one",
                    what,
                    now.name(),
                    posture.name(),
                    previous.name()
                );
                previous = now;
            }
        }
    }

    /// Off is off. Not "mostly off".
    #[test]
    fn a_posture_that_is_off_permits_nothing() {
        for what in Capability::ALL {
            assert_eq!(Posture::Off.allows(what), Allowance::No);
            assert_eq!(Posture::Watch.allows(what), Allowance::No);
        }
    }

    /// **A row for something djmanzo cannot do says no everywhere.** A matrix
    /// that permitted a power nothing exercises would read as a description of
    /// the software and be a description of an intention.
    #[test]
    fn an_unbuilt_capability_is_permitted_nowhere() {
        for what in Capability::ALL.into_iter().filter(|c| !c.built()) {
            for posture in Posture::ALL {
                assert_eq!(
                    posture.allows(what),
                    Allowance::No,
                    "{:?} is permitted at {} and does not exist",
                    what,
                    posture.name()
                );
            }
        }
        // And the ones that are built are permitted somewhere, or the row is
        // describing nothing.
        for what in Capability::ALL.into_iter().filter(|c| c.built()) {
            assert!(
                Posture::ALL.iter().any(|p| p.allows(what).permitted()),
                "{what:?} is built and permitted nowhere"
            );
        }
    }
}
