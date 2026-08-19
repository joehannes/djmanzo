//! The effect vocabulary.
//!
//! Names and shapes only — no audio. It lives here rather than in `dj-dsp`
//! because the action parser, the assistant, the controller mappings and the
//! interface all need to say "echo" long before anything makes a sound, and
//! [ADR-0003](../../docs/adr/0003-action-bus-and-parameter-registry.md) has one
//! vocabulary for all of them. `dj-dsp` supplies the arithmetic behind each
//! name.

use serde::{Deserialize, Serialize};

/// How many effects can run at once, per deck and on the master.
///
/// Three. Enough to stack a filter under an echo under a gate, which is the
/// deepest thing anyone does live; a fourth would mostly be a slot nobody can
/// reach in time. Each slot costs a delay line whether or not it is in use, so
/// this number is also a memory budget.
pub const FX_SLOTS: usize = 3;

/// Which effect a slot is running.
///
/// `None` is a state rather than an absence so a slot can always report
/// something, and the interface can show an empty slot distinctly from one that
/// is merely switched off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EffectKind {
    #[default]
    None,
    Echo,
    Gate,
    Crush,
    Flanger,
}

impl EffectKind {
    /// Every effect, in the order they are offered.
    pub const ALL: [EffectKind; 5] = [
        EffectKind::None,
        EffectKind::Echo,
        EffectKind::Gate,
        EffectKind::Crush,
        EffectKind::Flanger,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            EffectKind::None => "none",
            EffectKind::Echo => "echo",
            EffectKind::Gate => "gate",
            EffectKind::Crush => "crush",
            EffectKind::Flanger => "flanger",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// Whether the beat control means anything for this effect.
    ///
    /// Crush has no time in it, so a beat control on it would be a knob that
    /// does nothing — and a knob that does nothing is worse than no knob. The
    /// interface uses this to hide it rather than to grey it out: a control
    /// that is absent asks no questions.
    #[must_use]
    pub const fn is_timed(self) -> bool {
        matches!(
            self,
            EffectKind::Echo | EffectKind::Gate | EffectKind::Flanger
        )
    }

    /// What the `amount` knob does, in the DJ's words.
    ///
    /// One knob per effect, named for what it changes rather than for the
    /// parameter it happens to drive. "Feedback" is a thing a DJ hears;
    /// "coefficient" is not.
    #[must_use]
    pub const fn amount_label(self) -> &'static str {
        match self {
            EffectKind::None => "",
            EffectKind::Echo => "feedback",
            EffectKind::Gate => "width",
            EffectKind::Crush => "grit",
            EffectKind::Flanger => "depth",
        }
    }

    /// Position in [`Self::ALL`], which is how the parameter registry carries
    /// it: the registry holds `f32`, and an index is the honest way to put a
    /// small enum through one.
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|kind| *kind == self).unwrap_or(0)
    }

    #[must_use]
    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(EffectKind::None)
    }
}

/// Where in a deck's chain a slot sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Placement {
    /// Before the channel fader. The effect's tail survives the fader coming
    /// down, so an echo can be thrown and then faded out from under.
    #[default]
    PreFader,
    /// After the fader. The effect hears what the room hears, and pulling the
    /// fader takes the tail with it.
    PostFader,
}

/// One change to one slot.
///
/// A single action variant carrying a small sub-grammar rather than a verb per
/// slot per control: three slots times six controls times two targets would be
/// thirty-six verbs, and the vocabulary is read by a model on every request
/// where every token is paid for.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FxChange {
    Select(EffectKind),
    SetEnabled(bool),
    ToggleEnabled,
    /// Dry-to-wet mix, 0..=1.
    Wet(f32),
    /// Length in beats. Ignored by effects with no time in them.
    Beats(f32),
    /// The effect's own knob, 0..=1. See [`EffectKind::amount_label`].
    Amount(f32),
    Place(Placement),
}

impl std::fmt::Display for FxChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FxChange::Select(kind) => write!(f, "{}", kind.name()),
            FxChange::SetEnabled(true) => write!(f, "on"),
            FxChange::SetEnabled(false) => write!(f, "off"),
            FxChange::ToggleEnabled => write!(f, "toggle"),
            FxChange::Wet(v) => write!(f, "wet {v}"),
            FxChange::Beats(v) => write!(f, "beats {v}"),
            FxChange::Amount(v) => write!(f, "amount {v}"),
            FxChange::Place(Placement::PreFader) => write!(f, "pre"),
            FxChange::Place(Placement::PostFader) => write!(f, "post"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_round_trips_through_its_name() {
        for kind in EffectKind::ALL {
            assert_eq!(EffectKind::parse(kind.name()), Some(kind));
        }
        assert_eq!(EffectKind::parse("gramophone"), None);
    }

    /// The registry carries the kind as an index, so the mapping has to survive
    /// the trip in both directions or a slot would come back as a different
    /// effect than the one that is running.
    #[test]
    fn every_effect_round_trips_through_its_index() {
        for kind in EffectKind::ALL {
            assert_eq!(EffectKind::from_index(kind.index()), kind);
        }
        // A registry read before anything was written is zero, which must mean
        // "no effect" rather than an arbitrary one.
        assert_eq!(EffectKind::from_index(0), EffectKind::None);
        assert_eq!(EffectKind::from_index(999), EffectKind::None);
    }

    /// Every effect that has a knob has to say what the knob does, or the
    /// interface has an unlabelled control.
    #[test]
    fn every_effect_names_its_knob() {
        for kind in EffectKind::ALL {
            if kind == EffectKind::None {
                continue;
            }
            assert!(
                !kind.amount_label().is_empty(),
                "{} has an unnamed knob",
                kind.name()
            );
        }
    }

    #[test]
    fn only_the_timed_effects_claim_to_be_timed() {
        assert!(EffectKind::Echo.is_timed());
        assert!(EffectKind::Gate.is_timed());
        assert!(EffectKind::Flanger.is_timed());
        assert!(!EffectKind::Crush.is_timed());
        assert!(!EffectKind::None.is_timed());
    }
}
