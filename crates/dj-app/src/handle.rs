//! What a control is, and what its gestures do.
//!
//! §29 asks for controls that are "compact but powerful" through progressive
//! disclosure, and gives the example of a tiny EQ knob carrying seven things at
//! once — drag, shift-drag, double-click, right-click, long hold, an AI hover,
//! and the same parameter over MIDI. It closes with the warning that matters:
//! *do not turn every knob into a huge widget.*
//!
//! # The seventh bullet is the design
//!
//! > MIDI = same underlying parameter.
//!
//! That is [ADR-0003](../../../docs/adr/0003-action-bus-and-parameter-registry.md)
//! stated as a control requirement. Every gesture on a control has to end up as
//! the same [`dj_core::action::Action`] a controller would send, or the drag
//! and the MIDI knob are two paths that will eventually disagree. So this
//! module answers each gesture with **action text**, not with a number for the
//! interface to interpret: what the DJ does with a knob is what a mapping does
//! with it, spelled the same way.
//!
//! # Why the answers live here rather than at the call site
//!
//! They were at the call sites, and that is the two-copies problem again. Every
//! `SvgKnob` in the interface passed its own `ondblclick` naming its own idea of
//! where the control resets to — three EQ bands each spelling out `1`, the
//! filter spelling out `0` — and nothing made a fourth call site agree, or made
//! any of them wrong when a range changed. A control's unity point is a fact
//! about the *parameter*, not about the place it happens to be drawn.
//!
//! # The three levels, concretely
//!
//! - **Level 1, immediate value.** The readout. It is the interface's, because
//!   it is about how a number reads rather than what it is: "kill" for a
//!   band at zero, "LP 60%" for a filter.
//! - **Level 2, direct adjustment.** Drag, shift-drag for fine, double-click to
//!   [`Handle::unity`]. The fine ratio and the unity point are here.
//! - **Level 3, contextual options.** [`Handle::options`] — a short list of
//!   whole moves a DJ would otherwise reach across the interface for, each one
//!   an action djmanzo already accepts.
//!
//! Short on purpose. §29's warning is about widgets that grow, and a menu of
//! twelve is a widget that grew.

use dj_core::DeckId;

/// How much finer a shift-drag is than a drag.
///
/// A quarter. Fine enough that a band can be set to a value a plain drag
/// skates past, coarse enough that the knob still moves under the hand — a
/// tenth feels broken rather than precise.
pub const FINE: f64 = 0.25;

/// One option on a control's contextual menu.
///
/// The label is what a DJ reads; the action is what djmanzo does. Both, rather
/// than a label the interface maps to a command of its own, because a menu
/// whose entries are not actions is a second vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Option_ {
    pub label: String,
    /// Exactly the text [`dj_core::action::Action::parse`] takes.
    pub action: String,
}

/// A control djmanzo knows the gestures for.
///
/// Deliberately the small set §29 is about — the knobs and faders on a deck —
/// rather than every parameter in the registry. A table of four hundred
/// entries would be a table nobody keeps true.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    EqLow,
    EqMid,
    EqHigh,
    Filter,
    Volume,
    Pitch,
}

impl Control {
    /// Every control, in the order a deck draws them.
    pub const ALL: [Control; 6] = [
        Control::EqLow,
        Control::EqMid,
        Control::EqHigh,
        Control::Filter,
        Control::Volume,
        Control::Pitch,
    ];

    /// The verb this control is, as the action grammar spells it.
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Control::EqLow => "eq_low",
            Control::EqMid => "eq_mid",
            Control::EqHigh => "eq_high",
            Control::Filter => "filter",
            Control::Volume => "volume",
            Control::Pitch => "pitch",
        }
    }

    /// Read back what [`Self::verb`] wrote.
    #[must_use]
    pub fn parse(verb: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.verb() == verb)
    }

    /// Where a double-click puts it.
    ///
    /// The one fact every call site was spelling out for itself. Unity for a
    /// gain-like control is 1, centre for a bipolar one is 0 — and which is
    /// which is a property of the parameter, not of the panel.
    #[must_use]
    pub const fn unity(self) -> f64 {
        match self {
            Control::EqLow | Control::EqMid | Control::EqHigh | Control::Volume => 1.0,
            Control::Filter | Control::Pitch => 0.0,
        }
    }
}

/// Everything a control's gestures need, for one deck.
#[derive(Debug, Clone, PartialEq)]
pub struct Handle {
    pub control: Control,
    /// What a double-click sends.
    pub reset: String,
    /// How much finer a shift-drag is. See [`FINE`].
    pub fine: f64,
    /// §29's level three, and never more than a handful of it.
    pub options: Vec<Option_>,
}

/// The gestures for one control on one deck.
///
/// The deck number is threaded through every action, because an action without
/// one is not an action djmanzo can perform — and a menu that acted on
/// "whichever deck" would be the one thing a DJ cannot risk mid-mix.
#[must_use]
pub fn handle(deck: DeckId, control: Control) -> Handle {
    let n = deck.human_number();
    let verb = control.verb();
    let say = |label: &str, action: String| Option_ {
        label: label.to_owned(),
        action,
    };

    // Whole moves, not shades of the drag the knob already does. A menu entry
    // that sets a band to 0.7 is a worse drag; one that kills it is a thing a
    // DJ does with a hand they do not have free.
    let options = match control {
        Control::EqLow | Control::EqMid | Control::EqHigh => vec![
            say("Kill", format!("deck {n} {verb} 0")),
            say("Unity", format!("deck {n} {verb} 1")),
            say("Full", format!("deck {n} {verb} 4")),
        ],
        Control::Filter => vec![
            say("Off", format!("deck {n} filter 0")),
            say("Low-pass", format!("deck {n} filter -0.6")),
            say("High-pass", format!("deck {n} filter 0.6")),
        ],
        Control::Volume => vec![
            say("Down", format!("deck {n} volume 0")),
            say("Full", format!("deck {n} volume 1")),
        ],
        Control::Pitch => vec![
            say("Zero", format!("deck {n} pitch 0")),
            say("Sync", format!("deck {n} sync")),
        ],
    };

    Handle {
        control,
        reset: format!("deck {n} {verb} {}", control.unity()),
        fine: FINE,
        options,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::action::Action;

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).expect("a deck")
    }

    /// **Every gesture is an action djmanzo already accepts.**
    ///
    /// §29's last bullet — "MIDI = same underlying parameter" — is ADR-0003 as
    /// a control requirement: a drag, a double-click, a menu entry and a MIDI
    /// CC have to end up as the same action, or they are paths that will
    /// eventually disagree. A control offering a verb the parser rejects is
    /// that disagreement, shipped.
    #[test]
    fn every_gesture_parses_as_an_action() {
        for control in Control::ALL {
            for n in [1u8, 4] {
                let h = handle(deck(n), control);
                assert!(
                    Action::parse(&h.reset).is_ok(),
                    "reset is not an action: {}",
                    h.reset
                );
                for option in &h.options {
                    assert!(
                        Action::parse(&option.action).is_ok(),
                        "{} is not an action: {}",
                        option.label,
                        option.action
                    );
                }
            }
        }
    }

    /// **A control's actions name the deck they are for.**
    ///
    /// A menu that acted on "whichever deck" is the one thing a DJ cannot risk
    /// mid-mix: the wrong record goes quiet in front of a room.
    #[test]
    fn every_action_names_its_own_deck() {
        for control in Control::ALL {
            for n in [1u8, 2, 3, 4] {
                let h = handle(deck(n), control);
                assert!(h.reset.starts_with(&format!("deck {n} ")), "{}", h.reset);
                for option in &h.options {
                    assert!(
                        option.action.starts_with(&format!("deck {n} ")),
                        "{} on deck {n}: {}",
                        option.label,
                        option.action
                    );
                }
            }
        }
    }

    /// **A double-click goes where the parameter's unity is, not where a panel
    /// thought it was.**
    ///
    /// Every `SvgKnob` in the interface used to pass its own `ondblclick`
    /// naming its own idea of the reset point. A gain-like control resets to
    /// unity and a bipolar one to centre, and which is which is a fact about
    /// the parameter.
    #[test]
    fn a_reset_goes_to_the_parameters_own_unity() {
        assert_eq!(handle(deck(1), Control::EqLow).reset, "deck 1 eq_low 1");
        assert_eq!(handle(deck(2), Control::Filter).reset, "deck 2 filter 0");
        assert_eq!(handle(deck(3), Control::Volume).reset, "deck 3 volume 1");
        assert_eq!(handle(deck(4), Control::Pitch).reset, "deck 4 pitch 0");
    }

    /// **The menu stays short.**
    ///
    /// §29's own warning is "do not turn every knob into a huge widget", and a
    /// contextual menu is exactly where a knob grows into one.
    #[test]
    fn no_control_offers_more_than_a_handful() {
        for control in Control::ALL {
            let h = handle(deck(1), control);
            assert!(
                (1..=4).contains(&h.options.len()),
                "{control:?} offers {} options",
                h.options.len()
            );
        }
    }

    /// A shift-drag is finer than a drag, and not so fine the knob stops
    /// moving under the hand.
    ///
    /// The number, not `FINE`: written against the constant this would move
    /// with it and defend nothing — the same mistake `profile`'s threshold
    /// test made. A quarter is the judgement, and every control carries it.
    #[test]
    fn a_shift_drag_is_a_quarter_of_a_drag() {
        for control in Control::ALL {
            assert_eq!(
                handle(deck(1), control).fine,
                0.25,
                "{control:?} disagrees about what fine means"
            );
        }
    }

    /// Every control round-trips through its verb, so a stored or serialised
    /// one is the same control on the other side.
    #[test]
    fn a_control_survives_being_written_down() {
        for control in Control::ALL {
            assert_eq!(Control::parse(control.verb()), Some(control));
        }
        assert_eq!(Control::parse("eq_lo"), None, "a near miss is not a match");
    }
}
