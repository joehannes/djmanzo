//! What the assistant may do at each posture, as a table rather than as a
//! scattering of `if`s.
//!
//! [§72 of the directive](../../../docs/DIRECTIVE.md) asks for an explicit
//! matrix of capability against posture, and for it to be configurable. This is
//! that matrix. It exists because the alternative — each caller deciding for
//! itself whether the current posture permits what it is about to do — is how a
//! DJ ends up with an assistant that will not load a deck but will happily
//! reach for the crossfader, and with no single place to look to find out why.
//!
//! # It is the second of three gates, and the order matters
//!
//! 1. **[`crate::Takeover`]** — has a hand already claimed this control? Asked
//!    first, so nothing below can override a person.
//! 2. **This table** — does the posture permit this *kind* of thing at all?
//!    A fact about the level the DJ chose, and about nothing else.
//! 3. **[`crate::Warrant`]** — is the read of the night sure enough to act on?
//!    §9's other axis, asked last because it is the only one that changes
//!    minute to minute.
//!
//! All three must pass. They are separate because they answer different
//! questions, and a single combined check could not explain which of the three
//! said no — which is exactly what a DJ asks when the machine does nothing.
//!
//! # Silence is refusal
//!
//! [`Capability::of`] answers `None` for an action the matrix has no opinion
//! about, and callers must treat that as **no**. An action nobody thought to
//! classify is not one to let an autopilot try: the safe direction for a
//! default is the quiet one, and a new [`dj_core::Action`] variant should have
//! to be let in deliberately.

use crate::Posture;
use dj_core::{Action, DeckAction, MixerAction};
use serde::{Deserialize, Serialize};

/// A kind of thing the assistant might do.
///
/// The rows of §72's table, in its order. Deliberately coarse: this is about
/// *what a DJ would object to*, not about individual parameters. "EQ" is one
/// row because nobody thinks "the machine may touch my low band but not my
/// high one", and a matrix at parameter granularity would be 448 rows nobody
/// would ever read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Offer a record. Never audible, so it is allowed everywhere the
    /// assistant speaks at all.
    RecommendTrack,
    /// Put a record on a deck that is not playing.
    LoadNextDeck,
    /// Move a silent deck's playhead to where the mix will start.
    SetCue,
    /// Trim a silent deck to match the playing one.
    GainMatch,
    /// Engage or release sync.
    Sync,
    /// Move an EQ band.
    Eq,
    /// Engage, adjust or clear an effect.
    Fx,
    /// Move the crossfader. The one control the room hears immediately.
    Crossfader,
    /// Choose *what kind of thing* comes next, rather than offering one.
    ChooseDirection,
    /// Open, close or rearrange a surface.
    AdaptLayout,
}

impl Capability {
    pub const ALL: [Capability; 10] = [
        Capability::RecommendTrack,
        Capability::LoadNextDeck,
        Capability::SetCue,
        Capability::GainMatch,
        Capability::Sync,
        Capability::Eq,
        Capability::Fx,
        Capability::Crossfader,
        Capability::ChooseDirection,
        Capability::AdaptLayout,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Capability::RecommendTrack => "recommend_track",
            Capability::LoadNextDeck => "load_next_deck",
            Capability::SetCue => "set_cue",
            Capability::GainMatch => "gain_match",
            Capability::Sync => "sync",
            Capability::Eq => "eq",
            Capability::Fx => "fx",
            Capability::Crossfader => "crossfader",
            Capability::ChooseDirection => "choose_direction",
            Capability::AdaptLayout => "adapt_layout",
        }
    }

    /// As §72 writes it, for the panel that shows the matrix.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Capability::RecommendTrack => "Recommend track",
            Capability::LoadNextDeck => "Load next deck",
            Capability::SetCue => "Set cue",
            Capability::GainMatch => "Gain match",
            Capability::Sync => "Sync",
            Capability::Eq => "EQ adjustment",
            Capability::Fx => "FX",
            Capability::Crossfader => "Crossfader",
            Capability::ChooseDirection => "Genre / track selection",
            Capability::AdaptLayout => "Layout adaptation",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|c| c.name() == name.trim().to_ascii_lowercase())
    }

    /// Whether the room hears this the moment it happens.
    ///
    /// The line the whole table is really drawn along: everything below it can
    /// be undone before anybody notices, and everything above it cannot.
    #[must_use]
    pub const fn is_audible(self) -> bool {
        matches!(
            self,
            Capability::Sync | Capability::Eq | Capability::Fx | Capability::Crossfader
        )
    }

    /// Which capability an action belongs to, if any.
    ///
    /// `None` means the matrix has no opinion, and every caller must read that
    /// as a refusal — see the module note on why silence is refusal.
    #[must_use]
    pub fn of(action: &Action) -> Option<Self> {
        match action {
            Action::Deck { action, .. } => Self::of_deck(action),
            Action::Mixer(action) => Self::of_mixer(action),
        }
    }

    fn of_deck(action: &DeckAction) -> Option<Self> {
        match action {
            DeckAction::Seek(_) | DeckAction::Cue => Some(Capability::SetCue),
            DeckAction::SetGainDb(_) => Some(Capability::GainMatch),
            DeckAction::SetEqLow(_)
            | DeckAction::SetEqMid(_)
            | DeckAction::SetEqHigh(_)
            | DeckAction::SetFilter(_) => Some(Capability::Eq),
            DeckAction::Fx { .. } => Some(Capability::Fx),
            DeckAction::Sync | DeckAction::SyncOff => Some(Capability::Sync),
            // A channel fader is as audible as the crossfader and is the
            // obvious way around a refusal to touch one, so it is the same row.
            DeckAction::SetVolume(_) => Some(Capability::Crossfader),
            _ => None,
        }
    }

    fn of_mixer(action: &MixerAction) -> Option<Self> {
        match action {
            MixerAction::Crossfader(_) => Some(Capability::Crossfader),
            // The master rack is the same rack in a different place, and an
            // assistant refused a deck effect would otherwise reach for it.
            MixerAction::Fx { .. } => Some(Capability::Fx),
            // A stem swap is two decks' audio changing at once. Nothing the
            // room hears is more of an event, so it is the crossfader row.
            MixerAction::StemSwap { .. } | MixerAction::StemSwapOff => Some(Capability::Crossfader),
            MixerAction::MasterGainDb(_) => Some(Capability::Crossfader),
            _ => None,
        }
    }
}

/// How far a posture may go with one capability.
///
/// Three values rather than two because §72's own table has three: `limited`
/// is a real cell in it, and collapsing it into yes or no would lose the
/// distinction the directive drew between an assistant that may nudge an EQ
/// and one that may kill a band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Allowance {
    #[default]
    No,
    /// Small, reversible moves only. What "limited" means numerically is the
    /// caller's business — see [`Allowance::most_change`] for the one number
    /// this crate is willing to state.
    Limited,
    Yes,
}

impl Allowance {
    pub const ALL: [Allowance; 3] = [Allowance::No, Allowance::Limited, Allowance::Yes];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Allowance::No => "no",
            Allowance::Limited => "limited",
            Allowance::Yes => "yes",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|a| a.name() == name.trim().to_ascii_lowercase())
    }

    /// Whether anything at all may happen.
    #[must_use]
    pub const fn permits(self) -> bool {
        !matches!(self, Allowance::No)
    }

    /// How far a continuous control may move from where it is, as a fraction
    /// of its range.
    ///
    /// A fifth. Chosen so that "limited" cannot kill a band or open a filter
    /// all the way — the two EQ moves an audience hears as an event rather
    /// than as a mix — while still allowing the nudge that gain-staging a
    /// blend actually needs. `Yes` is unbounded and `No` is zero, so a caller
    /// can use this number without branching on the variant.
    #[must_use]
    pub const fn most_change(self) -> f32 {
        match self {
            Allowance::No => 0.0,
            Allowance::Limited => 0.2,
            Allowance::Yes => f32::INFINITY,
        }
    }
}

/// The matrix: what each posture may do, with the DJ's changes applied.
///
/// Starts as §72's table verbatim. [`Authority::set`] changes one cell, which
/// is what "make this system configurable" asks for, with one exception that
/// is not configurable — see [`Authority::set`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authority {
    /// Only the cells a DJ has changed. Absent means the default applies, so a
    /// stored file does not freeze a table djmanzo may improve.
    changed: Vec<Cell>,
}

/// One changed cell, in a form that survives being written to disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub capability: Capability,
    pub posture: Posture,
    pub allowance: Allowance,
}

impl Default for Authority {
    fn default() -> Self {
        Self::new()
    }
}

impl Authority {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            changed: Vec::new(),
        }
    }

    /// §72's table, verbatim.
    ///
    /// `Off` and `Watch` are absent from the directive's table because they are
    /// not in question: off does nothing and watch only records. They answer
    /// [`Allowance::No`] to everything, and unlike every other cell that
    /// cannot be changed.
    #[must_use]
    pub const fn default_for(capability: Capability, posture: Posture) -> Allowance {
        use Allowance::{Limited, No, Yes};
        use Capability as C;
        match posture {
            Posture::Off | Posture::Watch => No,
            Posture::Suggest => match capability {
                C::RecommendTrack | C::AdaptLayout => Yes,
                _ => No,
            },
            Posture::Prepare => match capability {
                C::RecommendTrack | C::AdaptLayout | C::LoadNextDeck | C::SetCue | C::GainMatch => {
                    Yes
                }
                _ => No,
            },
            Posture::Assist => match capability {
                C::RecommendTrack
                | C::AdaptLayout
                | C::LoadNextDeck
                | C::SetCue
                | C::GainMatch
                | C::Sync => Yes,
                C::Eq | C::Fx => Limited,
                C::Crossfader | C::ChooseDirection => No,
            },
            Posture::Autopilot => Yes,
        }
    }

    /// What this matrix says, with the DJ's changes applied.
    #[must_use]
    pub fn allows(&self, capability: Capability, posture: Posture) -> Allowance {
        self.changed
            .iter()
            .find(|cell| cell.capability == capability && cell.posture == posture)
            .map_or_else(|| Self::default_for(capability, posture), |c| c.allowance)
    }

    /// Change one cell.
    ///
    /// Setting a cell back to its default removes it rather than storing it, so
    /// the stored form stays a list of *disagreements* with djmanzo rather than
    /// a frozen copy of a table that may improve.
    ///
    /// # Errors
    /// `Off` and `Watch` cannot be widened. They are what those postures *mean*
    /// — "do nothing" and "record only" — and a matrix that let Watch reach for
    /// a crossfader would make the word a lie rather than a setting.
    pub fn set(
        &mut self,
        capability: Capability,
        posture: Posture,
        allowance: Allowance,
    ) -> Result<(), String> {
        if matches!(posture, Posture::Off | Posture::Watch) && allowance.permits() {
            return Err(format!(
                "{} means the assistant does not act; choose a louder posture instead",
                posture.name()
            ));
        }
        self.changed
            .retain(|cell| !(cell.capability == capability && cell.posture == posture));
        if allowance != Self::default_for(capability, posture) {
            self.changed.push(Cell {
                capability,
                posture,
                allowance,
            });
        }
        Ok(())
    }

    /// Put every cell back to djmanzo's answer.
    pub fn reset(&mut self) {
        self.changed.clear();
    }

    /// The cells the DJ has changed.
    #[must_use]
    pub fn changes(&self) -> &[Cell] {
        &self.changed
    }

    /// Whether one action is permitted at one posture, and how far.
    ///
    /// [`Allowance::No`] for an action the matrix has no opinion about. See the
    /// module note on why silence is refusal.
    #[must_use]
    pub fn allows_action(&self, action: &Action, posture: Posture) -> Allowance {
        Capability::of(action).map_or(Allowance::No, |c| self.allows(c, posture))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §72's table, transcribed from the directive and compared against the
    /// code rather than against memory.
    ///
    /// Written out in full on purpose: this is the one place the directive
    /// states a table, and a test that re-derived it from the same `match` it
    /// is checking would assert nothing at all.
    #[test]
    fn the_matrix_is_the_one_the_directive_writes_down() {
        use Allowance::{Limited, No, Yes};
        use Capability as C;
        use Posture::{Assist, Autopilot, Prepare, Suggest};
        let expected = [
            (C::RecommendTrack, [Yes, Yes, Yes, Yes]),
            (C::LoadNextDeck, [No, Yes, Yes, Yes]),
            (C::SetCue, [No, Yes, Yes, Yes]),
            (C::GainMatch, [No, Yes, Yes, Yes]),
            (C::Sync, [No, No, Yes, Yes]),
            (C::Eq, [No, No, Limited, Yes]),
            (C::Fx, [No, No, Limited, Yes]),
            (C::Crossfader, [No, No, No, Yes]),
            (C::ChooseDirection, [No, No, No, Yes]),
            (C::AdaptLayout, [Yes, Yes, Yes, Yes]),
        ];
        let authority = Authority::new();
        for (capability, row) in expected {
            for (posture, want) in [Suggest, Prepare, Assist, Autopilot].into_iter().zip(row) {
                assert_eq!(
                    authority.allows(capability, posture),
                    want,
                    "{} at {}",
                    capability.name(),
                    posture.name()
                );
            }
        }
    }

    /// A louder posture never does less than a quieter one.
    ///
    /// The property that makes the levels a *dial* rather than five unrelated
    /// modes, and the one a DJ relies on when they turn it up one notch.
    #[test]
    fn every_posture_may_do_everything_the_quieter_one_may() {
        let authority = Authority::new();
        for capability in Capability::ALL {
            for pair in Posture::ALL.windows(2) {
                let quieter = authority.allows(capability, pair[0]);
                let louder = authority.allows(capability, pair[1]);
                assert!(
                    louder >= quieter,
                    "{} does less at {} than at {}",
                    capability.name(),
                    pair[1].name(),
                    pair[0].name()
                );
            }
        }
    }

    /// Off and Watch do nothing, whatever anybody stores.
    #[test]
    fn the_quiet_postures_cannot_be_widened() {
        let mut authority = Authority::new();
        for posture in [Posture::Off, Posture::Watch] {
            for capability in Capability::ALL {
                assert_eq!(authority.allows(capability, posture), Allowance::No);
                let refused = authority.set(capability, posture, Allowance::Yes);
                assert!(refused.is_err(), "{} was widened", posture.name());
                assert_eq!(authority.allows(capability, posture), Allowance::No);
            }
        }
        assert!(authority.changes().is_empty());
    }

    /// Configurable in both directions — narrowing and widening are both the
    /// DJ's call — and a cell set back to its default stops being stored.
    #[test]
    fn a_changed_cell_is_kept_and_a_restored_one_is_forgotten() {
        let mut authority = Authority::new();
        // "Autopilot, but never the crossfader" is a coherent request.
        authority
            .set(Capability::Crossfader, Posture::Autopilot, Allowance::No)
            .expect("narrowing autopilot is allowed");
        assert_eq!(
            authority.allows(Capability::Crossfader, Posture::Autopilot),
            Allowance::No
        );
        assert_eq!(authority.changes().len(), 1);

        // And so is "let Assist have the EQ properly".
        authority
            .set(Capability::Eq, Posture::Assist, Allowance::Yes)
            .expect("widening a loud posture is allowed");
        assert_eq!(authority.changes().len(), 2);

        authority
            .set(Capability::Crossfader, Posture::Autopilot, Allowance::Yes)
            .expect("restoring is allowed");
        assert_eq!(
            authority.changes().len(),
            1,
            "a cell back at its default is still stored"
        );

        authority.reset();
        assert!(authority.changes().is_empty());
        assert_eq!(
            authority.allows(Capability::Eq, Posture::Assist),
            Allowance::Limited
        );
    }

    /// An action nobody classified is not one an autopilot may try.
    #[test]
    fn an_unclassified_action_is_refused_rather_than_waved_through() {
        let authority = Authority::new();
        let deck = dj_core::DeckId::from_human(1).expect("deck 1");
        // Transport is deliberately unclassified: starting and stopping a
        // record is not one of §72's rows, and the safe reading of silence is
        // no.
        let play = Action::Deck {
            deck,
            action: DeckAction::Play,
        };
        assert_eq!(Capability::of(&play), None);
        assert_eq!(
            authority.allows_action(&play, Posture::Autopilot),
            Allowance::No
        );
    }

    /// The classifier agrees with the row a DJ would expect.
    #[test]
    fn actions_land_in_the_row_they_belong_to() {
        let deck = dj_core::DeckId::from_human(2).expect("deck 2");
        let of = |action| Capability::of(&action);
        assert_eq!(
            of(Action::Deck {
                deck,
                action: DeckAction::SetEqLow(0.0)
            }),
            Some(Capability::Eq)
        );
        assert_eq!(
            of(Action::Deck {
                deck,
                action: DeckAction::SetGainDb(-2.0)
            }),
            Some(Capability::GainMatch)
        );
        assert_eq!(
            of(Action::Deck {
                deck,
                action: DeckAction::Sync
            }),
            Some(Capability::Sync)
        );
        assert_eq!(
            of(Action::Mixer(MixerAction::Crossfader(0.5))),
            Some(Capability::Crossfader)
        );
        // A channel fader is the way around a refused crossfader, so it is the
        // same row rather than an unguarded one.
        assert_eq!(
            of(Action::Deck {
                deck,
                action: DeckAction::SetVolume(0.0)
            }),
            Some(Capability::Crossfader)
        );
    }

    /// "Limited" has to mean something a caller can act on.
    #[test]
    fn limited_is_a_real_bound_and_the_others_are_its_ends() {
        assert_eq!(Allowance::No.most_change(), 0.0);
        assert!(Allowance::Limited.most_change() > 0.0);
        assert!(Allowance::Limited.most_change() < 1.0);
        assert!(Allowance::Yes.most_change().is_infinite());
        assert!(!Allowance::No.permits());
        assert!(Allowance::Limited.permits());
    }

    /// Names round-trip, because they are stored and sent over a wire.
    #[test]
    fn capabilities_and_allowances_round_trip_by_name() {
        for capability in Capability::ALL {
            assert_eq!(Capability::parse(capability.name()), Some(capability));
            assert!(!capability.title().is_empty());
        }
        for allowance in Allowance::ALL {
            assert_eq!(Allowance::parse(allowance.name()), Some(allowance));
        }
        assert_eq!(Capability::parse("nothing at all"), None);
    }

    /// The audible line is the one the table is drawn along: nothing audible
    /// is permitted before Assist.
    #[test]
    fn nothing_the_room_hears_is_permitted_before_assist() {
        let authority = Authority::new();
        for capability in Capability::ALL.into_iter().filter(|c| c.is_audible()) {
            for posture in [
                Posture::Off,
                Posture::Watch,
                Posture::Suggest,
                Posture::Prepare,
            ] {
                assert_eq!(
                    authority.allows(capability, posture),
                    Allowance::No,
                    "{} is audible and permitted at {}",
                    capability.name(),
                    posture.name()
                );
            }
        }
    }
}
