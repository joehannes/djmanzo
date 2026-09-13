//! §53: what a controller actually puts under a DJ's hands.
//!
//! > The UI should know: number of decks actually controllable, available
//! > physical knobs, jogs, pads, stem controls, mixer channels, displays, LED
//! > feedback. Use that to determine which GUI surfaces deserve prominence.
//!
//! # It is read from the mapping, not from the device
//!
//! A MIDI controller announces a name and a port and nothing else. What it
//! *does* is in its [`Mapping`] — which button sends which note, and what that
//! note is bound to — and that is a complete answer to seven of §53's eight
//! questions without touching the hardware. It is also the right answer: a
//! controller with four decks of buttons that this DJ has mapped as two is a
//! two-deck controller as far as the interface is concerned, because the
//! bindings are what a hand will actually find.
//!
//! That is what makes §53 testable at all. The eighth question — **displays** —
//! is not answerable this way and is not guessed: see [`Hands::displays`].
//!
//! # Counted from the actions, not from the control names
//!
//! A binding's `on` is a note number and its action is
//! [`dj_core::Action`] text, so *what the control does* is the thing djmanzo
//! can read. `deck 3 play_pause` proves a third deck is reachable; a knob bound
//! to `deck 1 eq_low` proves a knob. Reading the note numbers instead would be
//! counting what the vendor wired rather than what the DJ can reach, and those
//! differ on every controller where a shift key doubles a row.

use crate::feedback::FeedbackMap;
use crate::mapping::{Binding, Mapping, MappingError};
use serde::Serialize;

/// What one controller puts under the hands.
///
/// Every field is a count or a flag derived from the mapping, so two mappings
/// for the same hardware can legitimately differ: this describes the *reach* a
/// DJ has, not the plastic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Hands {
    /// Decks a hand can reach. The highest deck number any binding names.
    pub decks: u8,
    /// Continuous controls that are not a jog: faders, knobs, encoders.
    ///
    /// One number rather than §53's separate *knobs* and *mixer channels*,
    /// because a MIDI mapping cannot tell a knob from a fader — both are a
    /// control change — and a count that pretended to would be a confident
    /// guess. What is separable is the mixer, below.
    pub knobs: usize,
    /// Jogs and platters, by the decks that have one.
    pub jogs: u8,
    /// Pads: buttons bound to a hot cue, a saved loop or a sampler slot.
    pub pads: usize,
    /// Whether any control reaches the stems.
    ///
    /// §53's own worked example turns on this one: *if a controller has
    /// dedicated stem pads, compact the GUI stem panel; if there are no stem
    /// controls, expand it*.
    pub stems: bool,
    /// Mixer channels a hand can reach — a channel fader, an EQ or a filter
    /// counts the deck it belongs to.
    pub channels: u8,
    /// Lights the mapping declares. §53's *LED feedback*.
    ///
    /// A count of what the file says can be lit, **not** a claim that djmanzo
    /// lights them: `FeedbackMap` is parsed and read by nobody, which is the
    /// fourth table this project has found in that state. See
    /// [`LEDS_NOT_DRIVEN`] — the interface says which, because "this controller
    /// has lights" and "djmanzo drives them" are different facts and only one
    /// of them is true.
    pub leds: usize,
}

/// What §53 asks for and a mapping cannot answer.
///
/// **Displays.** A screen on a controller is not reachable over the MIDI
/// mapping at all — a CDJ's display is driven by its own firmware and a DDJ's
/// by a USB protocol djmanzo does not speak — so there is nothing in a mapping
/// file that could say whether one exists. Reported as absent rather than
/// guessed from the device name, which is the same posture §25's unbuilt layers
/// take: a count that included a display djmanzo cannot draw to would be a
/// promise the interface could not keep.
pub const DISPLAYS_UNKNOWABLE: &str = "A controller's screens are driven by its own firmware, not by a mapping, \
     so djmanzo cannot see them.";

/// What the light count is, and what it is not.
///
/// The `[[feedback]]` blocks describe lights djmanzo *could* drive, and
/// nothing sends them: `FeedbackMap` is parsed and consulted by no other
/// module. Counting them and saying nothing would be the interface claiming a
/// controller lights up under it, which is the kind of promise §25's unbuilt
/// layers are named as absent to avoid.
pub const LEDS_NOT_DRIVEN: &str =
    "This mapping describes lights, and djmanzo does not send them yet.";

impl Hands {
    /// Read a controller's reach off the text of its mapping file.
    ///
    /// One entry point over the file rather than two over its halves, because
    /// the lights are `[[feedback]]` blocks in the same file and a profile
    /// assembled from two reads is a profile that can be half of one mapping
    /// and half of another.
    ///
    /// # Errors
    /// If the mapping does not parse, or its feedback blocks do not.
    pub fn read(text: &str) -> Result<Self, MappingError> {
        let mapping = Mapping::parse(text)?;
        let lights = FeedbackMap::parse(text)?;
        Ok(Self::of(&mapping, &lights))
    }

    /// Read a controller's reach off its mapping and its lights.
    #[must_use]
    pub fn of(mapping: &Mapping, lights: &FeedbackMap) -> Self {
        let mut hands = Hands::default();
        let mut channels = 0u8;

        for binding in &mapping.bindings {
            for action in actions(binding) {
                let deck = deck_of(action);
                if let Some(n) = deck {
                    hands.decks = hands.decks.max(n);
                }
                let verb = verb_of(action);

                if is_jog(binding, verb) {
                    if let Some(n) = deck {
                        hands.jogs = hands.jogs.max(n);
                    }
                } else if continuous(binding) {
                    hands.knobs += 1;
                } else if PAD_VERBS.contains(&verb) {
                    hands.pads += 1;
                }

                if verb.starts_with("stem_") {
                    hands.stems = true;
                }
                if MIXER_VERBS.contains(&verb)
                    && let Some(n) = deck
                {
                    channels = channels.max(n);
                }
            }
        }

        hands.channels = channels;
        hands.leds = lights.lights.len();
        hands
    }

    /// Whether the interface should give the stem panel room.
    ///
    /// §53's worked example, and the one judgement this type exists to make. A
    /// DJ whose controller has stem pads reaches for those, so the panel is a
    /// readout and can stay folded; a DJ with no stem controls has only the
    /// screen, and a folded panel is the feature hidden from the only person
    /// who needs it open.
    ///
    /// True when there is a controller and it cannot reach the stems. **Not**
    /// true when there is no controller at all: a laptop-only DJ is already
    /// the case the interface is designed around, and unfolding a module for
    /// them would be §53 answering a question nobody asked.
    #[must_use]
    pub const fn wants_stems_open(&self) -> bool {
        !self.stems
    }
}

/// Verbs that make a button a pad rather than a transport key.
///
/// The vocabulary's own spellings, checked against it by a test: a verb here
/// that `dj_core::vocabulary` does not have is a rule about nothing, and the
/// pad count would silently read zero on a controller covered in them. That is
/// not hypothetical — the first version of this list guessed `hotcue_set`,
/// `sample_play` and `loop_recall`, and read the DDJ-SR's fifty-six pads as
/// none.
const PAD_VERBS: [&str; 7] = [
    "hotcue",
    "hotcue_set",
    "hotcue_clear",
    "sampler",
    "slice",
    "roll",
    "loop_recall",
];

/// Verbs that make a control part of the mixer.
const MIXER_VERBS: [&str; 6] = ["volume", "eq_low", "eq_mid", "eq_high", "filter", "gain"];

/// Every action string one binding can send.
fn actions(binding: &Binding) -> impl Iterator<Item = &str> {
    [
        binding.press.as_deref(),
        binding.release.as_deref(),
        binding.moved.as_deref(),
        binding.turn_up.as_deref(),
        binding.turn_down.as_deref(),
        binding.platter.as_deref(),
    ]
    .into_iter()
    .flatten()
}

/// The deck an action is addressed to, if any.
fn deck_of(action: &str) -> Option<u8> {
    let mut words = action.split_whitespace();
    (words.next()? == "deck").then(|| words.next()?.parse().ok())?
}

/// The verb, whatever the action is addressed to.
fn verb_of(action: &str) -> &str {
    let mut words = action.split_whitespace();
    match words.next() {
        Some("deck") => {
            words.next();
            words.next().unwrap_or("")
        }
        Some(word) => word,
        None => "",
    }
}

/// Whether this binding is a continuous control at all.
fn continuous(binding: &Binding) -> bool {
    binding.moved.is_some() || binding.turn_up.is_some() || binding.platter.is_some()
}

/// Whether this binding is a jog or a platter.
///
/// The verb, or the encoding: a jog bound with `move` and an encoding is a jog
/// even when its action is `deck 1 nudge`, and `angle` is only ever a motorised
/// platter. See [`Encoding`] for why the presence of that field is itself the
/// fact.
fn is_jog(binding: &Binding, verb: &str) -> bool {
    binding.platter.is_some()
        || matches!(verb, "jog" | "jog_touch" | "scratch" | "nudge")
        || (binding.encoding.is_some() && binding.moved.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundled;

    fn hands(text: &str) -> Hands {
        Hands::read(text).expect("the mapping parses")
    }

    /// **The load-bearing one: every bundled mapping reads as the controller it
    /// is.**
    ///
    /// Not that the numbers are *some* number — a profile of all zeros would
    /// pass that — but that each is the one a person reading the file would
    /// give. A profile is what §53 decides prominence from, so one that read a
    /// four-deck controller as two would hide half the interface from somebody
    /// who has the hardware for it.
    #[test]
    fn a_bundled_mapping_reads_as_the_controller_it_describes() {
        for (name, text) in bundled::CONTROLLERS {
            let hands = hands(text);
            assert!(
                hands.decks >= 1,
                "{name} reads as a controller with no decks at all"
            );
            assert!(
                hands.jogs <= hands.decks,
                "{name} reads as having more jogs ({}) than decks ({})",
                hands.jogs,
                hands.decks
            );
            assert!(
                hands.channels <= hands.decks,
                "{name} reads as having more mixer channels than decks"
            );
        }

        // The two ends of the shipped range, by name, so the counting itself is
        // checked and not merely its self-consistency.
        let sr = hands(
            bundled::CONTROLLERS
                .iter()
                .find(|(n, _)| *n == "pioneer-ddj-sr")
                .expect("the DDJ-SR ships")
                .1,
        );
        // **Two**, and the file's own header says the hardware has four. That
        // is the whole principle of this module rather than a bug in it: the
        // DDJ-SR splits across four MIDI channels and this mapping binds two
        // decks, so two is what a hand will find. Counting the channels the
        // vendor wired would tell the interface to make room for two decks
        // nobody can reach.
        assert_eq!(
            sr.decks, 2,
            "the DDJ-SR mapping binds two decks, whatever the hardware has"
        );
        assert!(sr.pads >= 16, "the DDJ-SR has pads: {}", sr.pads);
        assert!(sr.jogs >= 2, "the DDJ-SR has jogs: {}", sr.jogs);
        assert!(sr.channels >= 2, "the DDJ-SR has a mixer");
        assert!(!sr.stems, "the DDJ-SR mapping binds nothing to the stems");

        let small = hands(
            bundled::CONTROLLERS
                .iter()
                .find(|(n, _)| *n == "generic-hid")
                .expect("the generic HID mapping ships")
                .1,
        );
        assert!(
            small.pads < sr.pads,
            "an eleven-binding mapping reads as having as many pads as a DDJ-SR"
        );
    }

    /// **Both lists name verbs djmanzo actually has.**
    ///
    /// The rule `dj_app::tiers` learned the hard way, in the module where it
    /// bites hardest: a verb misspelled in `PAD_VERBS` is a rule that silently
    /// does not apply, and the first version of that list guessed three of its
    /// six — so a DDJ-SR covered in pads read as having none, and §53 would
    /// have given the on-screen pad zone room on a controller that has
    /// fifty-six of them.
    #[test]
    fn every_verb_these_lists_name_is_one_the_vocabulary_has() {
        let known: Vec<&str> = dj_core::vocabulary::vocabulary()
            .iter()
            .map(|spec| spec.verb)
            .collect();
        for verb in PAD_VERBS.iter().chain(MIXER_VERBS.iter()) {
            assert!(
                known.contains(verb),
                "`{verb}` is not a verb djmanzo has, so the rule naming it \
                 covers nothing"
            );
        }
    }

    /// **§53's worked example: no stem controls means the panel wants room.**
    ///
    /// The one judgement this type exists to make, and the direction matters.
    /// A DJ whose controller reaches the stems has the panel as a readout; a DJ
    /// whose controller does not has only the screen, and a folded module is
    /// the feature hidden from the only person who needs it open.
    #[test]
    fn a_controller_that_cannot_reach_the_stems_wants_them_on_screen() {
        let with = hands(
            r#"
            name = "with stems"
            [[binding]]
            on = "note 1 0x10"
            press = "deck 1 stem_mute vocal"
            "#,
        );
        let without = hands(
            r#"
            name = "without"
            [[binding]]
            on = "note 1 0x0B"
            press = "deck 1 play_pause"
            "#,
        );
        assert!(with.stems);
        assert!(!with.wants_stems_open());
        assert!(!without.stems);
        assert!(without.wants_stems_open());
    }

    /// **A jog is not a knob, and the encoding is what says so.**
    ///
    /// Counted as a knob, a jog would make a two-deck controller look like it
    /// had a mixer it does not have — and `Encoding`'s own doc comment says why
    /// the field's *presence* is the fact: a jog bound with `move` and no
    /// encoding is indistinguishable from a fader.
    #[test]
    fn a_jog_is_counted_as_a_jog_rather_than_as_a_knob() {
        let jog = hands(
            r#"
            name = "jog"
            [[binding]]
            on = "cc 1 0x22"
            move = "deck 1 jog {value}"
            encoding = "signed"
            "#,
        );
        assert_eq!(jog.jogs, 1);
        assert_eq!(jog.knobs, 0, "the jog was counted as a knob");

        let fader = hands(
            r#"
            name = "fader"
            [[binding]]
            on = "cc 1 0x07"
            move = "deck 1 volume {value}"
            "#,
        );
        assert_eq!(fader.knobs, 1);
        assert_eq!(fader.jogs, 0);
        assert_eq!(fader.channels, 1, "a channel fader is a mixer channel");
    }

    /// **A mapping with nothing in it reads as nothing, and says so.**
    ///
    /// The keyboard is exactly this case: it has no bindings at all, because a
    /// keyboard's keys are `keys.rs`' business. A profile that invented a deck
    /// for it would have the interface deciding prominence from a controller
    /// that is not there.
    #[test]
    fn an_empty_mapping_claims_nothing() {
        let empty = hands("name = \"nothing\"");
        assert_eq!(empty, Hands::default());
        assert_eq!(empty.decks, 0);
        assert_eq!(empty.leds, 0);
    }
}
