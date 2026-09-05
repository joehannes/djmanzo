//! How much the assistant does, and what "good" means right now.
//!
//! Almost every question about assistant behaviour turns out to be one of two
//! questions, and keeping them separate is what stops this becoming a pile of
//! switches. See [ASSISTANT.md](../../../docs/ASSISTANT.md) part two.
//!
//! - [`Posture`] is **how much it does** -- from silent to mixing.
//! - [`Occasion`] is **what good means** -- the same two records are the right
//!   and the wrong answer depending on the room.
//!
//! They are deliberately independent. "Autopilot while I learn" and "say
//! nothing at peak time" are both coherent requests, and a single combined
//! setting could express neither.

use dj_core::Trajectory;
use serde::{Deserialize, Serialize};

/// How much the assistant does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Posture {
    /// Nothing at all. The machine is quiet.
    Off,
    /// Records the set but neither acts nor speaks. For practice you intend to
    /// review afterwards, where commentary during would be a distraction.
    Watch,
    /// Offers, with reasons, and never acts.
    #[default]
    Suggest,
    /// **Stages without committing.**
    ///
    /// The level most software skips and the one most working DJs would leave
    /// it on. A DJ two minutes from the end of a record does not want advice
    /// and does not want the machine to mix -- they want the next track already
    /// loaded, cued to the phrase and gain-matched, so that the only remaining
    /// act is theirs.
    Prepare,
    /// Does the small things, asks about the big ones.
    Assist,
    /// Mixes, and narrates what it is doing.
    Autopilot,
}

impl Posture {
    /// Every posture, quietest first, for an interface that offers them.
    pub const ALL: [Posture; 6] = [
        Posture::Off,
        Posture::Watch,
        Posture::Suggest,
        Posture::Prepare,
        Posture::Assist,
        Posture::Autopilot,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Watch => "watch",
            Self::Suggest => "suggest",
            Self::Prepare => "prepare",
            Self::Assist => "assist",
            Self::Autopilot => "autopilot",
        }
    }

    /// Parse a posture by name.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|p| p.name() == word.trim().to_ascii_lowercase())
    }

    /// Whether it may move a control on its own.
    ///
    /// The single question most callers ask. `Prepare` answers **false**:
    /// staging a deck is not moving a control the audience can hear, and
    /// conflating the two is what makes "prepare" collapse into "assist".
    ///
    /// Read out of §72's matrix rather than written here, and `Sync` is the row
    /// that answers it: sync is the first thing this application does that a
    /// room can hear. The three predicates below come from the same table for
    /// the same reason — two descriptions of one rule are free to disagree, and
    /// the one they disagree about is what an autopilot does to a live mix. See
    /// [`crate::authority`].
    #[must_use]
    pub fn may_act(self) -> bool {
        self.allows(crate::Capability::Sync).permitted()
    }

    /// Whether it may load and cue a deck that is not playing.
    #[must_use]
    pub fn may_stage(self) -> bool {
        self.allows(crate::Capability::LoadNextDeck).permitted()
    }

    /// Whether it may perform a transition unasked.
    #[must_use]
    pub fn may_mix(self) -> bool {
        self.allows(crate::Capability::Crossfader).permitted()
    }

    /// Whether it may say anything unprompted.
    ///
    /// `Watch` is silent by design -- it is for practice you will review later,
    /// and commentary during is the thing being avoided.
    #[must_use]
    pub fn may_speak(self) -> bool {
        !matches!(self, Self::Off | Self::Watch)
    }

    /// Whether the session is being recorded for later review.
    #[must_use]
    pub fn records(self) -> bool {
        !matches!(self, Self::Off)
    }
}

/// What "good" means right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Occasion {
    /// Explanation over speed; everything reversible.
    Learning,
    /// Honest critique afterwards rather than encouragement during.
    Practice,
    /// Novelty and odd pairings over safety.
    Experimenting,
    /// Patience and space. Long blends, room to breathe.
    WarmUp,
    /// Energy and phrase-locked accuracy over subtlety.
    Peak,
    /// Descent, memory, familiarity.
    Close,
    /// Invisibility. No dead air, no drama.
    Background,
    /// The room's wishes over the arc of the set.
    Requests,
    /// No particular occasion.
    #[default]
    Open,
}

impl Occasion {
    pub const ALL: [Occasion; 9] = [
        Occasion::Learning,
        Occasion::Practice,
        Occasion::Experimenting,
        Occasion::WarmUp,
        Occasion::Peak,
        Occasion::Close,
        Occasion::Background,
        Occasion::Requests,
        Occasion::Open,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Learning => "learning",
            Self::Practice => "practice",
            Self::Experimenting => "experimenting",
            Self::WarmUp => "warm_up",
            Self::Peak => "peak",
            Self::Close => "close",
            Self::Background => "background",
            Self::Requests => "requests",
            Self::Open => "open",
        }
    }

    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|o| o.name() == word.trim().to_ascii_lowercase())
    }

    /// Where the next record should take the room.
    ///
    /// The suggester already takes this; the occasion is where it comes from.
    #[must_use]
    pub fn trajectory(self) -> Trajectory {
        match self {
            Self::WarmUp | Self::Peak => Trajectory::Lift,
            Self::Close | Self::Background => Trajectory::Ease,
            _ => Trajectory::Hold,
        }
    }

    /// How long a transition should be, in beats, when nothing else decides.
    ///
    /// A warm-up blend is long because there is time and nobody is waiting; a
    /// peak-time one is shorter because the floor notices the seam. Background
    /// is longest of all: the ideal is that nobody can say when it happened.
    #[must_use]
    pub fn transition_beats(self) -> u32 {
        match self {
            Self::Background => 64,
            Self::WarmUp | Self::Close => 32,
            Self::Peak | Self::Requests => 16,
            // Learning and practice want to *see* the transition, and an eight
            // beat mix is short enough to watch happen.
            Self::Learning | Self::Practice => 8,
            Self::Experimenting | Self::Open => 32,
        }
    }

    /// How much explanation to offer, 0..=2.
    ///
    /// Zero is "say nothing unless asked". Two is "name what just happened".
    /// Not a boolean, because the middle -- an occasional word at the moment it
    /// matters -- is where most DJs would set it.
    #[must_use]
    pub fn verbosity(self) -> u8 {
        match self {
            Self::Learning => 2,
            Self::Practice | Self::Experimenting => 1,
            _ => 0,
        }
    }

    /// Whether a mistake here is expensive.
    ///
    /// What the interface uses to decide how far away to put the destructive
    /// controls. A booth is dark and loud, and a mis-click at peak time is
    /// heard by everyone; alone at home it costs nothing.
    #[must_use]
    pub fn mistakes_are_costly(self) -> bool {
        matches!(
            self,
            Self::Peak | Self::Background | Self::Requests | Self::Close
        )
    }

    /// How willing to suggest something unexpected, 0.0..=1.0.
    #[must_use]
    pub fn appetite_for_risk(self) -> f32 {
        match self {
            Self::Experimenting => 1.0,
            Self::Learning | Self::Practice => 0.6,
            Self::WarmUp | Self::Open => 0.4,
            Self::Peak | Self::Close => 0.2,
            // The room asked for a specific record. Now is not the time to be
            // interesting.
            Self::Background | Self::Requests => 0.05,
        }
    }
}

/// A named bundle of both dials and what they imply.
///
/// What the interface offers as a single choice: "warm-up", "peak", "teach me".
/// A pack is a starting point rather than a lock -- either dial can be moved
/// afterwards, and doing so does not leave the pack, it changes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pack {
    pub name: &'static str,
    pub posture: Posture,
    pub occasion: Occasion,
    /// One line, for the button's tooltip. Says what changes, not what it is
    /// called.
    pub summary: &'static str,
}

/// The packs offered by default.
#[must_use]
pub fn packs() -> &'static [Pack] {
    &[
        Pack {
            name: "Teach me",
            posture: Posture::Suggest,
            occasion: Occasion::Learning,
            summary: "Names what you just did, catches the specific mistake, \
                      short transitions you can watch happen.",
        },
        Pack {
            name: "Practising",
            posture: Posture::Watch,
            occasion: Occasion::Practice,
            summary: "Silent while you play, records everything, and has \
                      opinions afterwards.",
        },
        Pack {
            name: "Messing about",
            posture: Posture::Suggest,
            occasion: Occasion::Experimenting,
            summary: "Suggests the pairings it would not risk in front of a \
                      room.",
        },
        Pack {
            name: "Warm-up",
            posture: Posture::Prepare,
            occasion: Occasion::WarmUp,
            summary: "Stages the next record cued and gain-matched. Long \
                      blends. You still do the mixing.",
        },
        Pack {
            name: "Peak",
            posture: Posture::Prepare,
            occasion: Occasion::Peak,
            summary: "Everything ready, nothing touched. Destructive controls \
                      moved out of the way.",
        },
        Pack {
            name: "Closing",
            posture: Posture::Prepare,
            occasion: Occasion::Close,
            summary: "Suggests the descent, and remembers what has already \
                      been played.",
        },
        Pack {
            name: "Hands off",
            posture: Posture::Autopilot,
            occasion: Occasion::Background,
            summary: "Mixes on its own, as invisibly as it can. Touch anything \
                      to take over.",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Prepare stages but does not act.**
    ///
    /// The distinction the whole posture ladder exists for, and the one that
    /// collapses if nobody checks it: staging a deck that is not playing is not
    /// moving a control the audience can hear. A DJ who set this level and
    /// found the crossfader moving would never trust it again.
    #[test]
    fn prepare_stages_without_acting() {
        assert!(Posture::Prepare.may_stage(), "prepare cannot stage");
        assert!(!Posture::Prepare.may_act(), "prepare moved a live control");
        assert!(
            !Posture::Prepare.may_mix(),
            "prepare performed a transition"
        );
    }

    /// **Only autopilot mixes.**
    #[test]
    fn only_autopilot_mixes() {
        for posture in Posture::ALL {
            assert_eq!(
                posture.may_mix(),
                posture == Posture::Autopilot,
                "{} disagrees about whether it may mix",
                posture.name()
            );
        }
    }

    /// **Watch is silent, and that is the point of it.**
    ///
    /// It is for practice you intend to review afterwards; commentary during is
    /// the thing being avoided. It still records, which is what separates it
    /// from Off.
    #[test]
    fn watch_records_but_says_nothing() {
        assert!(!Posture::Watch.may_speak(), "watch spoke");
        assert!(Posture::Watch.records(), "watch recorded nothing to review");
        assert!(!Posture::Off.records(), "off recorded anyway");
    }

    /// **The ladder is monotonic.**
    ///
    /// Each rung may do everything the one below it may. Without this the
    /// names mean nothing -- a DJ moving from Assist to Autopilot expecting
    /// *more* would occasionally get less, and there would be no way to reason
    /// about the setting at all.
    #[test]
    fn each_posture_may_do_everything_the_quieter_one_may() {
        for pair in Posture::ALL.windows(2) {
            let (lower, higher) = (pair[0], pair[1]);
            for (what, low, high) in [
                ("speak", lower.may_speak(), higher.may_speak()),
                ("stage", lower.may_stage(), higher.may_stage()),
                ("act", lower.may_act(), higher.may_act()),
                ("mix", lower.may_mix(), higher.may_mix()),
                ("record", lower.records(), higher.records()),
            ] {
                assert!(
                    high || !low,
                    "{} may {what} but {} above it may not",
                    lower.name(),
                    higher.name()
                );
            }
        }
    }

    /// Names round-trip, because they cross into settings files and the action
    /// vocabulary.
    #[test]
    fn postures_and_occasions_round_trip_by_name() {
        for posture in Posture::ALL {
            assert_eq!(Posture::parse(posture.name()), Some(posture));
        }
        for occasion in Occasion::ALL {
            assert_eq!(Occasion::parse(occasion.name()), Some(occasion));
        }
        assert_eq!(Posture::parse("  AUTOPILOT "), Some(Posture::Autopilot));
        assert!(Posture::parse("nonsense").is_none());
    }

    /// **Peak and warm-up want different transitions.**
    ///
    /// If the occasion did not change this it would be a label. A warm-up blend
    /// is long because there is time and nobody is waiting; at peak the floor
    /// notices the seam.
    #[test]
    fn the_occasion_changes_the_transition_length() {
        assert!(
            Occasion::WarmUp.transition_beats() > Occasion::Peak.transition_beats(),
            "a peak-time mix was planned as long as a warm-up one"
        );
        assert!(
            Occasion::Background.transition_beats() >= Occasion::WarmUp.transition_beats(),
            "background should be the least noticeable of all"
        );
    }

    /// **Learning is short enough to watch happen.**
    ///
    /// A sixty-four beat blend is two minutes of nothing visibly changing,
    /// which is the worst possible way to be shown what a transition is.
    #[test]
    fn learning_transitions_are_short_enough_to_see() {
        assert!(Occasion::Learning.transition_beats() <= 8);
        assert_eq!(Occasion::Learning.verbosity(), 2);
    }

    /// **A room that asked for a record does not want to be surprised.**
    #[test]
    fn requests_and_background_take_the_least_risk() {
        let cautious = Occasion::Requests.appetite_for_risk();
        assert!(cautious < Occasion::Peak.appetite_for_risk());
        assert!(Occasion::Experimenting.appetite_for_risk() > Occasion::Peak.appetite_for_risk());
    }

    /// **Where a mistake is expensive, the occasion says so.**
    ///
    /// What the interface reads to decide how far away to put eject and
    /// load-over-playing. Alone at home a mis-click costs nothing; at peak time
    /// it is heard by everyone.
    #[test]
    fn the_costly_occasions_are_the_ones_with_an_audience() {
        assert!(Occasion::Peak.mistakes_are_costly());
        assert!(Occasion::Background.mistakes_are_costly());
        assert!(!Occasion::Learning.mistakes_are_costly());
        assert!(!Occasion::Experimenting.mistakes_are_costly());
    }

    /// A warm-up lifts, a close eases. If these were the same the trajectory
    /// would not be worth passing to the suggester.
    #[test]
    fn the_occasion_decides_where_the_room_is_going() {
        assert_eq!(Occasion::WarmUp.trajectory(), Trajectory::Lift);
        assert_eq!(Occasion::Close.trajectory(), Trajectory::Ease);
        assert_eq!(Occasion::Open.trajectory(), Trajectory::Hold);
    }

    /// **Every pack is a coherent pair.**
    ///
    /// A pack that said "autopilot while learning" would be offering to do the
    /// thing the learner is there to practise. Caught here rather than in a
    /// review, because packs are the part most likely to be added to casually.
    #[test]
    fn no_pack_offers_to_do_the_learning_for_you() {
        for pack in packs() {
            if matches!(pack.occasion, Occasion::Learning | Occasion::Practice) {
                assert!(
                    !pack.posture.may_mix(),
                    "the {:?} pack mixes on behalf of someone who is trying to learn",
                    pack.name
                );
            }
        }
    }

    /// Every pack has a summary that says what changes, and no two share a
    /// name.
    #[test]
    fn packs_are_named_and_described() {
        let mut names: Vec<&str> = packs().iter().map(|p| p.name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "two packs share a name");
        for pack in packs() {
            assert!(
                pack.summary.len() > 20,
                "the {:?} pack does not say what it changes",
                pack.name
            );
        }
    }
}
