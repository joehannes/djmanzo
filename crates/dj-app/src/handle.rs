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
use dj_core::action::TransitionStyle;

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

// -- §29's AI hover ----------------------------------------------------------

/// Which side of a planned mix a deck is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The record the room is hearing, on its way out.
    Leaving,
    /// The record coming in.
    Arriving,
}

/// §29's *AI hover = suggestion*: what the assistant's plan does to one control.
///
/// Every field is about a mix djmanzo has already planned — see
/// [`crate::transition`] — rather than about a general opinion, because a
/// general opinion about where an EQ band should be is not a thing any
/// software has. A knob nothing in the plan touches produces no suggestion at
/// all; see [`suggested`].
#[derive(Debug, Clone, PartialEq)]
pub struct Suggested {
    pub control: Control,
    /// The value the plan takes it to at its furthest, when it names one.
    ///
    /// `None` for a gesture with no position: sync is a switch, and a number
    /// beside it would be an invention.
    pub to: Option<f64>,
    /// The action that gets there, exactly as [`dj_core::action::Action::parse`]
    /// takes it — so §29's last bullet holds for the hover as well as for the
    /// drag and the menu. A DJ who wants the plan's answer now can have it,
    /// and it goes through the bus like anything else.
    pub action: String,
    /// What the plan does, and why, in Rust's words.
    pub because: String,
}

/// What the assistant's plan would do to this deck's controls.
///
/// **§29's last unbuilt gesture.** Its list for a knob is drag, shift-drag,
/// double-click, right-click, long hold, *AI hover*, MIDI — and the hover was
/// the one thing missing, on the reasoning that the assistant stages whole
/// moves rather than single parameter values. That is no longer true. §68's
/// transition object carries a style, and [`crate::shape`] is the one table
/// saying what a style does beyond the two channel faders, in the values the
/// automix actually sends. So the answer exists; it only had to be asked per
/// control.
///
/// # Sparse on purpose
///
/// Four of the six controls get nothing, every time, and that is the honest
/// answer rather than a gap: no transition style djmanzo performs touches the
/// mid band, the high band or the filter. A hover that said *"the assistant
/// would leave this where it is"* on every knob of every deck would be six
/// tooltips saying nothing, which is how a DJ learns to stop reading tooltips.
///
/// # What is deliberately not here
///
/// The gain trim. `autopilot::Step::MatchGain` is a real value the assistant
/// would set, and it is a *trim*, not one of §29's six knobs — the channel
/// fader is `volume` and they are different parameters. A hover that put a
/// trim figure on the volume knob would be pointing at the wrong control while
/// looking authoritative.
///
/// Stems and effects are the same story from the other end: a style moves both
/// and neither is a knob. The pair view says what the style does to them, in
/// [`crate::shape::Shape::words`], which is the same table read for a panel
/// rather than for a knob.
#[must_use]
pub fn suggested(deck: DeckId, side: Side, style: TransitionStyle, beats: u32) -> Vec<Suggested> {
    let n = deck.human_number();
    let shape = crate::shape::shape(style);
    let mut out = Vec::new();

    // The channel faders, which are the transition. Only where there is an
    // overlap for them to move across: a cut stops one deck on the tick the
    // other starts, and there is no fade in it to describe.
    if shape.overlaps {
        let (to, word) = match side {
            Side::Leaving => (0.0, "down"),
            Side::Arriving => (1.0, "up"),
        };
        out.push(Suggested {
            control: Control::Volume,
            to: Some(to),
            action: format!("deck {n} volume {to}"),
            because: format!("The assistant brings this fader {word} over {beats} beats."),
        });
    }

    // The bass swap, where the style has one.
    if let crate::shape::Eq::HandOverLows { done_by } = shape.eq {
        #[allow(clippy::cast_possible_truncation)]
        let percent = (done_by * 100.0).round() as i64;
        let (to, word) = match side {
            Side::Leaving => (0.0, "out of"),
            Side::Arriving => (1.0, "into"),
        };
        out.push(Suggested {
            control: Control::EqLow,
            to: Some(to),
            action: format!("deck {n} eq_low {to}"),
            because: format!(
                "The low end is handed {word} this deck by {percent}% through, then put back."
            ),
        });
    }

    // Sync, which is what the pitch fader is for at the moment a record comes
    // in. Only the arriving deck: the record the room is already hearing is
    // the tempo reference, and syncing it to the one that has not started is
    // the wrong way round.
    if side == Side::Arriving {
        out.push(Suggested {
            control: Control::Pitch,
            to: None,
            action: format!("deck {n} sync"),
            because: "The assistant engages sync so this record comes in on the beat.".to_owned(),
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::action::Action;

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).expect("a deck")
    }

    /// **The hover's actions are actions too.**
    ///
    /// The same guard as the menu's, for the same reason: §29's last bullet
    /// says a drag, a MIDI CC and everything else must end up as one
    /// parameter, and a hover offering a verb the parser rejects is that
    /// promise broken in the newest place.
    #[test]
    fn every_suggestion_parses_as_an_action() {
        for style in TransitionStyle::ALL {
            for side in [Side::Leaving, Side::Arriving] {
                for found in suggested(deck(2), side, style, 32) {
                    assert!(
                        Action::parse(&found.action).is_ok(),
                        "{style:?} on the {side:?} deck suggests `{}`, which is not an action",
                        found.action
                    );
                    assert!(
                        !found.because.is_empty(),
                        "a suggestion for {:?} says nothing",
                        found.control
                    );
                }
            }
        }
    }

    /// **The hover only speaks about controls the plan actually moves.**
    ///
    /// The load-bearing rule of this gesture. No style djmanzo performs
    /// touches the mid band, the high band or the filter, and a hover that
    /// said "the assistant would leave this where it is" on every knob of
    /// every deck would be six tooltips saying nothing — which is how a DJ
    /// learns to stop reading tooltips. Silence here is an answer.
    #[test]
    fn the_knobs_no_style_touches_are_never_given_a_suggestion() {
        for style in TransitionStyle::ALL {
            for side in [Side::Leaving, Side::Arriving] {
                let touched: Vec<Control> = suggested(deck(1), side, style, 32)
                    .into_iter()
                    .map(|s| s.control)
                    .collect();
                for quiet in [Control::EqMid, Control::EqHigh, Control::Filter] {
                    assert!(
                        !touched.contains(&quiet),
                        "{style:?} claims to move {quiet:?}, which no style does"
                    );
                }
            }
        }
    }

    /// **A cut has no fade, and the hover does not invent one.**
    ///
    /// `Shape::overlaps` is false for exactly one style because a cut stops
    /// one deck on the tick the other starts. A volume suggestion there would
    /// be describing a fader travel that never happens — and it would look
    /// exactly as confident as the four that do.
    #[test]
    fn a_cut_suggests_nothing_about_the_faders() {
        for side in [Side::Leaving, Side::Arriving] {
            let cut: Vec<Control> = suggested(deck(1), side, TransitionStyle::Cut, 8)
                .into_iter()
                .map(|s| s.control)
                .collect();
            assert!(
                !cut.contains(&Control::Volume),
                "a cut suggested a fader move on the {side:?} deck"
            );
            let blend: Vec<Control> = suggested(deck(1), side, TransitionStyle::Blend, 8)
                .into_iter()
                .map(|s| s.control)
                .collect();
            assert!(blend.contains(&Control::Volume), "a blend has no fade");
        }
    }

    /// **The two sides are opposite, and neither is the other's copy.**
    ///
    /// The fader comes down on the record leaving and up on the one arriving,
    /// and the low end goes the same way. A version that answered the same
    /// thing for both decks would be right half the time and look right all of
    /// it.
    #[test]
    fn the_deck_going_out_and_the_deck_coming_in_are_told_apart() {
        let out = suggested(deck(1), Side::Leaving, TransitionStyle::Blend, 32);
        let into = suggested(deck(2), Side::Arriving, TransitionStyle::Blend, 32);

        let value = |found: &[Suggested], control: Control| {
            found
                .iter()
                .find(|s| s.control == control)
                .and_then(|s| s.to)
        };
        assert_eq!(value(&out, Control::Volume), Some(0.0));
        assert_eq!(value(&into, Control::Volume), Some(1.0));
        // A blend hands the low end over, so the two decks are opposite there
        // as well. `Fade` does not, and says nothing about it at all.
        assert_eq!(value(&out, Control::EqLow), Some(0.0));
        assert_eq!(value(&into, Control::EqLow), Some(1.0));
        assert_eq!(
            suggested(deck(1), Side::Leaving, TransitionStyle::Fade, 32)
                .iter()
                .find(|s| s.control == Control::EqLow),
            None,
            "a fade claimed to move the low band, which `shape` says it does not"
        );
        // And the deck number in the action is the deck asked about.
        assert!(
            out.iter().all(|s| s.action.starts_with("deck 1 ")),
            "a suggestion for deck 1 names another deck"
        );
        assert!(into.iter().all(|s| s.action.starts_with("deck 2 ")));
    }

    /// **Sync is offered to the record coming in, and to nothing else.**
    ///
    /// The record the room is already hearing is the tempo reference; syncing
    /// it to the one that has not started is the wrong way round, and it is
    /// the kind of wrong that is only obvious once it has happened to a full
    /// dancefloor. It carries no value, because sync is a switch and a number
    /// beside it would be an invention.
    #[test]
    fn only_the_arriving_deck_is_offered_sync_and_it_carries_no_number() {
        for style in TransitionStyle::ALL {
            let arriving = suggested(deck(2), Side::Arriving, style, 16);
            let pitch = arriving
                .iter()
                .find(|s| s.control == Control::Pitch)
                .unwrap_or_else(|| panic!("{style:?} did not offer sync to the arriving deck"));
            assert_eq!(pitch.to, None, "sync was given a position");
            assert_eq!(pitch.action, "deck 2 sync");

            assert!(
                suggested(deck(1), Side::Leaving, style, 16)
                    .iter()
                    .all(|s| s.control != Control::Pitch),
                "{style:?} offered to sync the record that is already playing"
            );
        }
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
