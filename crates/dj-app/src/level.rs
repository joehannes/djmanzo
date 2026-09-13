//! §8's adaptation levels, as the one axis §8 asks for.
//!
//! > The application needs multiple layers of adaptive behavior. Level 0 —
//! > Static. Level 1 — Remember. Level 2 — Suggest. Level 3 — Prepare. Level 4
//! > — Assist. Level 5 — Adaptive. Level 6 — Autopilot.
//!
//! # Almost all of it already existed, under other names
//!
//! Most of what §8's levels *describe* has shipped for a long time, spread
//! across the sections that own each piece. Level 2 is §10's `Posture::Suggest`,
//! Level 3 is its `Prepare` and §44's staging, Level 4 is `Assist` and the
//! assistant's reversible moves, Level 5 is §78's freedoms and §79's locks, and
//! Level 6 is the autopilot. Level 1 is [`crate::remembered`]'s nine.
//!
//! What did **not** exist is the level itself: a DJ who wanted "stage things
//! for me but do not move my screen" had to find the posture in one panel and
//! six locks in another, know which of the six mattered, and get both right.
//! §8's whole point is that this is *one* decision with a range, and that is
//! what this file is.
//!
//! # It sets; it does not own
//!
//! A level is a **starting point**, the same contract §7's arrangements and
//! §54's setups have and state. Setting one writes the posture and the locks;
//! nothing here holds them afterwards, and a DJ who then unlocks the theme has
//! unlocked the theme. That is not a gap in the design — a single axis that
//! owned six switches would be the axis arguing with the switches, and the
//! §79 panel would be a row of controls that silently sprang back.
//!
//! What djmanzo owes them instead is the truth: [`Level::departures`] says what
//! about the current state no longer matches the level that was set, in the
//! DJ's own words, so "you are at Prepare, except the theme still adapts" is
//! something the interface can say rather than something they have to work out.
//!
//! # §9, which this file is the most likely place to break
//!
//! > **Autonomy**: how much the machine is allowed to do. **Confidence**: how
//! > certain the system is. These are not the same. Make this distinction
//! > fundamental throughout the system.
//!
//! A level is autonomy and **only** autonomy. Nothing here reads a certainty,
//! and nothing here should ever gain a field that does: the tempting version of
//! this table is one where a confident djmanzo quietly acts a level higher,
//! which is precisely §9's *low confidence + high autonomy = invalid/unsafe*
//! arriving through the back door. Certainty lives on the context engine's
//! reading and on §9's warrant, and it decides whether a thing is *offered* —
//! never how far djmanzo may go with it. A test in this file asserts the
//! separation the only way a test can: that the same level answers the same
//! way whatever djmanzo believes.

use crate::cockpit::{Freedom, Permits};
use dj_assistant::Posture;

/// How far djmanzo may go. §8's seven, quietest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// §8's Level 0. The DJ controls everything; nothing is learned or moved.
    Static,
    /// §8's Level 1. Preferences survive a restart, and nothing else happens.
    Remember,
    /// §8's Level 2. Recommends, and never changes the state.
    Suggest,
    /// §8's Level 3. Stages the next record, and waits for commitment.
    Prepare,
    /// §8's Level 4. Makes the small reversible changes itself.
    Assist,
    /// §8's Level 5. The interface itself changes — density, theme, emphasis.
    Adaptive,
    /// §8's Level 6. Mixes, under guardrails, and can be interrupted.
    Autopilot,
}

impl Level {
    /// §8's seven, in §8's order.
    pub const ALL: [Self; 7] = [
        Self::Static,
        Self::Remember,
        Self::Suggest,
        Self::Prepare,
        Self::Assist,
        Self::Adaptive,
        Self::Autopilot,
    ];

    /// §8's own number for it, which is what a DJ will call it.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Static => 0,
            Self::Remember => 1,
            Self::Suggest => 2,
            Self::Prepare => 3,
            Self::Assist => 4,
            Self::Adaptive => 5,
            Self::Autopilot => 6,
        }
    }

    /// The slug it is stored and chosen by.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Remember => "remember",
            Self::Suggest => "suggest",
            Self::Prepare => "prepare",
            Self::Assist => "assist",
            Self::Adaptive => "adaptive",
            Self::Autopilot => "autopilot",
        }
    }

    /// §8's own name for it.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Static => "Static",
            Self::Remember => "Remember",
            Self::Suggest => "Suggest",
            Self::Prepare => "Prepare",
            Self::Assist => "Assist",
            Self::Adaptive => "Adaptive",
            Self::Autopilot => "Autopilot",
        }
    }

    /// What choosing it means, in the DJ's words.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Self::Static => "You control everything. Nothing is remembered and nothing moves.",
            Self::Remember => "Your settings come back tomorrow. Nothing else happens.",
            Self::Suggest => "djmanzo offers records and reasons, and never touches a control.",
            Self::Prepare => {
                "The next record arrives loaded, cued and gain-matched. The mix is yours."
            }
            Self::Assist => {
                "Small reversible things happen on their own. Big ones are still asked."
            }
            Self::Adaptive => "The screen follows the night as well: density, theme and emphasis.",
            Self::Autopilot => {
                "djmanzo mixes, says what it is doing, and stops the moment you touch anything."
            }
        }
    }

    /// The §10 posture this level sets.
    ///
    /// Not a second list. §10 is explicit that its six are *the main autonomy
    /// axis* and asks that they not be replaced, so this maps onto them rather
    /// than beside them. Two of §8's levels share a posture with a neighbour,
    /// and that is the honest answer rather than a gap: *Static* and *Remember*
    /// differ in what survives a restart and not in what the assistant does,
    /// and *Assist* and *Adaptive* differ in what the **interface** may do to
    /// itself, which is §78's axis and not §10's.
    #[must_use]
    pub const fn posture(self) -> Posture {
        match self {
            Self::Static | Self::Remember => Posture::Off,
            Self::Suggest => Posture::Suggest,
            Self::Prepare => Posture::Prepare,
            Self::Assist | Self::Adaptive => Posture::Assist,
            Self::Autopilot => Posture::Autopilot,
        }
    }

    /// Whether preferences survive a restart. §8's Level 1 is exactly this line.
    #[must_use]
    pub const fn remembers(self) -> bool {
        !matches!(self, Self::Static)
    }

    /// What the interface may change about itself at this level.
    ///
    /// All four of §78's freedoms from *Adaptive* up, and none below it. §8's
    /// Level 5 is the first that says *the interface itself changes*, and a
    /// level below it that let the theme follow the night would be Level 5
    /// happening at Level 3 with nothing saying so.
    #[must_use]
    pub const fn permits(self) -> Permits {
        if matches!(self, Self::Adaptive | Self::Autopilot) {
            Permits::everything()
        } else {
            Permits {
                rearrange: false,
                resize: false,
                retheme: false,
                restyle: false,
            }
        }
    }

    /// A level by its slug.
    ///
    /// `None` rather than a fallback: a stored level nobody recognises must not
    /// quietly become *Autopilot*, and must not quietly become *Static* either
    /// — either way djmanzo would be somewhere the DJ did not put it.
    #[must_use]
    pub fn parse(slug: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|l| l.slug() == slug)
    }

    /// What about the current state no longer matches this level.
    ///
    /// Empty when it does. A level is a starting point and a DJ may move any of
    /// the controls it set, so djmanzo's job afterwards is not to spring them
    /// back but to be able to say what changed — "Prepare, except the theme
    /// still adapts" is a true sentence and an interface that could not say it
    /// would be one where the level slowly stopped meaning anything.
    #[must_use]
    pub fn departures(self, posture: Posture, permits: Permits) -> Vec<&'static str> {
        let mut said = Vec::new();
        if posture != self.posture() {
            said.push(match posture as u8 > self.posture() as u8 {
                true => "the assistant is doing more than this level asks",
                false => "the assistant is doing less than this level asks",
            });
        }
        for (freedom, line) in [
            (
                Freedom::Rearrange,
                "panels still open and move on their own",
            ),
            (Freedom::Resize, "the density still follows the window"),
            (Freedom::Retheme, "the theme still follows the night"),
            (Freedom::Restyle, "the interface still answers the audio"),
        ] {
            if permits.allows(freedom) != self.permits().allows(freedom) {
                said.push(match permits.allows(freedom) {
                    true => line,
                    false => match freedom {
                        Freedom::Rearrange => "panels no longer move on their own",
                        Freedom::Resize => "the density no longer follows the window",
                        Freedom::Retheme => "the theme no longer follows the night",
                        Freedom::Restyle => "the interface no longer answers the audio",
                    },
                });
            }
        }
        said
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The load-bearing one: every level is reachable, and no two are the
    /// same place.**
    ///
    /// §8's seven have to be seven *different* answers, or the axis has fewer
    /// notches than it claims and a DJ moving one step gets nothing. The pair
    /// that makes this worth testing is *Assist* and *Adaptive*: they share a
    /// posture, so they are told apart only by §78's freedoms — and if those
    /// ever stopped differing, the axis would have a dead step in the middle
    /// with no error anywhere.
    #[test]
    fn each_level_is_somewhere_the_one_below_it_is_not() {
        for pair in Level::ALL.windows(2) {
            let (lower, upper) = (pair[0], pair[1]);
            let same = lower.posture() == upper.posture()
                && lower.permits() == upper.permits()
                && lower.remembers() == upper.remembers();
            assert!(
                !same,
                "{} and {} are the same place, so the axis has a dead step",
                lower.title(),
                upper.title()
            );
        }
    }

    /// **The axis only ever goes one way.**
    ///
    /// A level above another must not do *less*: the whole promise of a single
    /// control is that moving it up is moving djmanzo up. Checked on all three
    /// of the things a level sets, because they are set from three different
    /// tables and nothing else would notice one of them inverting.
    #[test]
    fn nothing_above_does_less_than_something_below() {
        for pair in Level::ALL.windows(2) {
            let (lower, upper) = (pair[0], pair[1]);
            assert!(
                upper.posture() as u8 >= lower.posture() as u8,
                "{} is quieter than {}",
                upper.title(),
                lower.title()
            );
            assert!(
                upper.remembers() >= lower.remembers(),
                "{} forgets what {} keeps",
                upper.title(),
                lower.title()
            );
            for freedom in [
                Freedom::Rearrange,
                Freedom::Resize,
                Freedom::Retheme,
                Freedom::Restyle,
            ] {
                assert!(
                    upper.permits().allows(freedom) >= lower.permits().allows(freedom),
                    "{} takes a freedom back that {} had",
                    upper.title(),
                    lower.title()
                );
            }
        }
    }

    /// **§9: a level is autonomy, and nothing else.**
    ///
    /// > Autonomy: how much the machine is allowed to do. Confidence: how
    /// > certain the system is. These are not the same.
    ///
    /// The tempting version of this table is one where a confident djmanzo
    /// quietly acts a level higher, which is §9's *low confidence + high
    /// autonomy = invalid/unsafe* arriving through the back door — because the
    /// same code path would then also act higher when it was merely *sure it
    /// was sure*. This asserts the separation the only way a test can: every
    /// question a level answers takes no reading, so there is no certainty it
    /// could consult. If a future signature here grows one, this test stops
    /// compiling, which is the point.
    #[test]
    fn what_djmanzo_may_do_never_depends_on_what_it_believes() {
        for level in Level::ALL {
            let answers = (level.posture(), level.remembers(), level.permits());
            for _ in 0..3 {
                assert_eq!(
                    (level.posture(), level.remembers(), level.permits()),
                    answers,
                    "{} answers differently on a second asking",
                    level.title()
                );
            }
        }
        // And the §10 postures a level can set are §10's own, never a seventh.
        for level in Level::ALL {
            assert!(Posture::ALL.contains(&level.posture()));
        }
    }

    /// **A level survives being written down, and an unknown slug is refused.**
    #[test]
    fn a_level_round_trips_and_a_stranger_is_not_guessed() {
        for level in Level::ALL {
            assert_eq!(Level::parse(level.slug()), Some(level));
        }
        assert_eq!(Level::parse(""), None);
        assert_eq!(Level::parse("Static"), None, "case is not forgiven");
        assert_eq!(Level::parse("level-3"), None);
    }

    /// **Departures are named, and a level that is met names nothing.**
    ///
    /// The half that makes a starting point honest. A DJ who set Prepare and
    /// then let the theme adapt is not at Prepare any more, and an interface
    /// that went on saying Prepare would be one where the level quietly stopped
    /// meaning anything.
    #[test]
    fn a_level_says_what_no_longer_matches_it() {
        let prepare = Level::Prepare;
        assert!(
            prepare
                .departures(prepare.posture(), prepare.permits())
                .is_empty(),
            "a state a level just set reads as a departure from it"
        );

        let themed = Permits {
            retheme: true,
            ..prepare.permits()
        };
        assert_eq!(
            prepare.departures(prepare.posture(), themed),
            ["the theme still follows the night"]
        );

        // Both directions, because the assistant can be turned down as well as
        // up and a DJ who quietened it deserves to be told the level no longer
        // describes them either way.
        assert_eq!(
            prepare.departures(Posture::Autopilot, prepare.permits()),
            ["the assistant is doing more than this level asks"]
        );
        assert_eq!(
            prepare.departures(Posture::Off, prepare.permits()),
            ["the assistant is doing less than this level asks"]
        );

        // And Adaptive, whose freedoms are all on, reports a lock as a
        // departure rather than as nothing.
        let locked = Permits {
            resize: false,
            ..Permits::everything()
        };
        assert_eq!(
            Level::Adaptive.departures(Posture::Assist, locked),
            ["the density no longer follows the window"]
        );
    }

    /// **Every level's words are its own, and none is written in markup.**
    ///
    /// These strings are shown in a picker. The rule earned itself in
    /// `crate::theme`, where a `*word*` written out of habit three lines below
    /// a doc comment reached the screen as asterisks.
    #[test]
    fn nothing_a_dj_reads_is_written_in_markup() {
        let mut seen = std::collections::BTreeSet::new();
        for level in Level::ALL {
            assert!(
                seen.insert(level.about()),
                "{} repeats a line",
                level.title()
            );
            for mark in ['*', '`', '_', '#'] {
                assert!(
                    !level.about().contains(mark),
                    "{}'s description contains `{mark}`, which the picker draws as itself",
                    level.title()
                );
            }
        }
        assert_eq!(seen.len(), Level::ALL.len());
    }
}
