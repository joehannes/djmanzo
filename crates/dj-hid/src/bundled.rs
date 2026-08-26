//! The mappings djmanzo ships with.
//!
//! These are compiled into the binary rather than installed as files, so a
//! fresh install on a machine with nothing configured still has a working
//! keyboard and the common controllers. A user mapping of the same name found
//! on disk takes precedence — the bundled one is the floor, not the ceiling.

use crate::keys::{KeyError, KeyMap};
use crate::mapping::{Mapping, MappingError};

/// The keyboard, for a laptop with nothing plugged into it.
pub const KEYBOARD_DEFAULT: &str = include_str!("../mappings/keyboard-default.toml");

/// Every bundled controller mapping, as `(file stem, text)`.
pub const CONTROLLERS: &[(&str, &str)] = &[
    (
        "generic-2-deck",
        include_str!("../mappings/generic-2-deck.toml"),
    ),
    (
        "motorised-platter",
        include_str!("../mappings/motorised-platter.toml"),
    ),
    ("generic-hid", include_str!("../mappings/generic-hid.toml")),
    (
        "scripted-shift",
        include_str!("../mappings/scripted-shift.toml"),
    ),
    (
        "pioneer-ddj-sr",
        include_str!("../mappings/pioneer-ddj-sr.toml"),
    ),
    (
        "pioneer-cdj-3000",
        include_str!("../mappings/pioneer-cdj-3000.toml"),
    ),
    (
        "pioneer-ddj-200",
        include_str!("../mappings/pioneer-ddj-200.toml"),
    ),
    (
        "pioneer-ddj-2deck",
        include_str!("../mappings/pioneer-ddj-2deck.toml"),
    ),
];

/// The default keyboard mapping, parsed.
///
/// # Errors
/// Only if the bundled file is broken, which a test in this module rules out
/// at build time — but it is returned rather than unwrapped so a broken build
/// says so instead of taking the application down on launch.
pub fn keyboard() -> Result<KeyMap, KeyError> {
    KeyMap::parse(KEYBOARD_DEFAULT)
}

/// Every bundled controller mapping, parsed.
///
/// # Errors
/// As [`keyboard`].
pub fn controllers() -> Result<Vec<Mapping>, MappingError> {
    CONTROLLERS
        .iter()
        .map(|(_, text)| Mapping::parse(text))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The one test that matters here: a mapping that ships broken is a
    /// keyboard that does nothing on somebody's first launch, and the file is
    /// hand-written TOML with two hundred action strings in it.
    #[test]
    fn the_bundled_keyboard_parses() {
        let map = keyboard().unwrap_or_else(|e| panic!("bundled keyboard is broken: {e}"));
        assert!(map.keys.len() > 40, "only {} keys", map.keys.len());
    }

    /// The commented-out `[audio]` example is documentation, and documentation
    /// that does not parse is worse than none: a DJ uncomments it, the mapping
    /// stops loading, and the file they were told to copy is the reason.
    ///
    /// So it is uncommented here and put through the real parser.
    #[test]
    fn the_documented_audio_example_parses_when_uncommented() {
        let (_, text) = CONTROLLERS
            .iter()
            .find(|(stem, _)| *stem == "generic-2-deck")
            .expect("the generic mapping is bundled");

        // The example is indented under `#   `, which is what tells it apart
        // from the prose around it.
        let example: String = text
            .lines()
            .filter_map(|line| line.strip_prefix("#   "))
            .map(|line| format!("{line}\n"))
            .collect();
        assert!(
            example.contains("[audio]"),
            "the documented example has moved or lost its indentation:\n{example}"
        );

        let uncommented = format!(
            "name = \"Example\"\ndevice = \"MIDI\"\n\n{example}\n             [[binding]]\non = \"note 1 0x0B\"\npress = \"deck 1 play_pause\"\n"
        );
        let mapping = Mapping::parse(&uncommented).unwrap_or_else(|e| {
            panic!("the documented example does not parse: {e}\n{uncommented}")
        });

        let routing = mapping
            .audio
            .expect("the example is an audio preset")
            .routing()
            .expect("the example is a usable arrangement");
        // And it demonstrates the case the section exists for. An example
        // showing master on 1-2 would be indistinguishable from the guess, so
        // it would teach a reader nothing about when to write one at all.
        assert_ne!(
            routing.master,
            (0, 1),
            "the example shows the arrangement djmanzo already guesses"
        );
    }

    /// A bundled script has to *work*, not merely parse. The shift mapping is
    /// documentation as much as it is a mapping, and documentation that does
    /// the wrong thing is worse than none.
    #[test]
    fn the_bundled_script_actually_shifts() {
        let (_, text) = CONTROLLERS
            .iter()
            .find(|(stem, _)| *stem == "scripted-shift")
            .expect("it is bundled");
        let mapping = Mapping::parse(text).expect("it parses");
        let source = mapping.script.as_ref().expect("it has a script");
        let script = crate::script::Script::load(
            "scripted-shift",
            source,
            std::sync::Arc::new(dj_control::ParameterRegistry::new()),
        )
        .expect("the script loads");

        use crate::script::Event;
        assert_eq!(
            script.on_control("note 1 0x02", Event::Press, 1.0).unwrap(),
            vec!["deck 1 hotcue 2"]
        );

        script.on_control("note 1 0x3f", Event::Press, 1.0).unwrap();
        assert_eq!(
            script.on_control("note 1 0x02", Event::Press, 1.0).unwrap(),
            vec!["deck 1 hotcue_set 2"],
            "shift was held and the pad did the unshifted thing"
        );

        script
            .on_control("note 1 0x3f", Event::Release, 0.0)
            .unwrap();
        assert_eq!(
            script.on_control("note 1 0x02", Event::Press, 1.0).unwrap(),
            vec!["deck 1 hotcue 2"],
            "shift was released and the pad stayed shifted"
        );

        // A pad the script does not know about is nothing, not a guess.
        assert!(
            script
                .on_control("note 1 0x7f", Event::Press, 1.0)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn every_bundled_controller_parses() {
        for (name, text) in CONTROLLERS {
            Mapping::parse(text)
                .unwrap_or_else(|e| panic!("bundled mapping {name} is broken: {e}"));
        }
    }

    /// Every key on the sheet needs a name and a group, or the shortcut sheet
    /// shows a blank row and the DJ has to read the TOML to find out what the
    /// key does.
    #[test]
    fn every_bundled_key_says_what_it_is_and_where_it_belongs() {
        for key in keyboard().unwrap().keys {
            assert!(!key.label.trim().is_empty(), "{} has no label", key.on);
            assert!(!key.group.trim().is_empty(), "{} has no group", key.on);
        }
    }

    /// **Held moves must undo themselves.** A censor, a kill or a brake that
    /// only has a press leaves the deck stuck the moment a finger comes up —
    /// and the bundled mapping says "(hold)" on the sheet, so the label would
    /// be lying.
    #[test]
    fn every_key_labelled_hold_has_a_release() {
        for key in keyboard().unwrap().keys {
            if key.label.contains("(hold)") {
                assert!(
                    key.release.is_some(),
                    "{} is labelled a hold and has no release",
                    key.on
                );
            }
        }
    }

    /// And the other direction, which is the one a hand edit gets wrong: a key
    /// with a release *is* momentary, so it has to say so.
    #[test]
    fn every_key_with_a_release_is_labelled_a_hold() {
        for key in keyboard().unwrap().keys {
            if key.release.is_some() {
                assert!(
                    key.label.contains("(hold)"),
                    "{} has a release and is not labelled a hold: {:?}",
                    key.on,
                    key.label
                );
            }
        }
    }

    /// One bundled mapping by name, for the tests that drive a specific one.
    fn bundled(stem: &str) -> Mapping {
        let (_, text) = CONTROLLERS
            .iter()
            .find(|(name, _)| *name == stem)
            .unwrap_or_else(|| panic!("no bundled mapping called {stem}"));
        Mapping::parse(text).expect("the bundled file parses")
    }

    /// **The DDJ-SR's faders arrive in two halves and are read as one.**
    ///
    /// Every fader and knob on this controller is a 14-bit pair, and the whole
    /// reason `cc14` exists is that binding the high byte alone would put the
    /// pitch fader back on 128 steps. A mapping that quietly bound only the
    /// high byte would still work -- which is why this drives the actual pair
    /// and checks that the low byte changes the answer.
    #[test]
    fn the_ddj_sr_pitch_fader_reads_all_fourteen_bits() {
        let mut map = bundled("pioneer-ddj-sr");
        let mut at = |msb: u8, lsb: u8| {
            map.translate(crate::Message::Control {
                channel: 0,
                controller: 0x00,
                value: msb,
            });
            map.translate(crate::Message::Control {
                channel: 0,
                controller: 0x20,
                value: lsb,
            })
            .join("")
        };
        let coarse = at(64, 0);
        let fine = at(64, 1);
        assert!(!coarse.is_empty(), "the pitch fader is not bound at all");
        assert_ne!(
            coarse, fine,
            "one low-byte step moved the pitch fader nowhere, so it is still seven bits"
        );
    }

    /// **A deck's controls are on the channel the vendor puts them on.**
    ///
    /// Pioneer splits one controller across seven MIDI channels and puts a
    /// deck's pads on a *different* channel from its transport. Getting that
    /// wrong is the easiest mistake to make transcribing the table, and it
    /// would show up as deck 2 answering deck 1's pads.
    #[test]
    fn the_ddj_sr_keeps_its_two_decks_apart() {
        let mut map = bundled("pioneer-ddj-sr");
        // Play on channel 1 is deck 1; the same note on channel 2 is deck 2.
        let play = |channel| crate::Message::NoteOn {
            channel,
            note: 0x0B,
            velocity: 127,
        };
        assert_eq!(map.translate(play(0)), vec!["deck 1 play_pause"]);
        assert_eq!(map.translate(play(1)), vec!["deck 2 play_pause"]);
        // The pads live on channels 8 and 9, not 1 and 2.
        let pad = |channel| crate::Message::NoteOn {
            channel,
            note: 0x00,
            velocity: 127,
        };
        assert_eq!(map.translate(pad(7)), vec!["deck 1 hotcue 1"]);
        assert_eq!(map.translate(pad(8)), vec!["deck 2 hotcue 1"]);
    }

    /// **A CDJ's platter is a speed around 64, and both directions work.**
    ///
    /// The platter does not send a delta: it sends how fast it is turning,
    /// with 64 for stopped. Read as an unsigned fader it would drive the deck
    /// forwards at half speed while standing still, which is the failure that
    /// looks like the software is possessed.
    #[test]
    fn the_cdj_platter_turns_both_ways_around_a_still_centre() {
        let mut map = bundled("pioneer-cdj-3000");
        let jog = |value| crate::Message::Control {
            channel: 0,
            controller: 0x10,
            value,
        };
        let number = |actions: Vec<String>| -> f32 {
            actions
                .first()
                .and_then(|a| a.rsplit(' ').next())
                .and_then(|n| n.parse().ok())
                .unwrap_or_else(|| panic!("the platter produced no movement"))
        };
        let still = number(map.translate(jog(64)));
        let forward = number(map.translate(jog(127)));
        let backward = number(map.translate(jog(0)));
        // Exactly nothing, not nearly nothing. Read as a fader position, 64
        // out of 127 lands a hair above zero and the deck creeps forwards
        // under a hand that is not touching it -- which over a set is a track
        // sliding out of time on its own, and is the reason `centred` exists.
        assert_eq!(
            still, 0.0,
            "a stopped platter moved the deck by {still} of a turn"
        );
        assert!(forward > 0.0, "forwards moved the deck by {forward}");
        assert!(backward < 0.0, "backwards moved the deck by {backward}");
    }

    /// The two hands are meant to mirror each other. If deck 1 gains a move
    /// and deck 2 does not, the layout stops being learnable — you can no
    /// longer reason "same shape, other hand".
    #[test]
    fn the_two_decks_have_the_same_moves() {
        let map = keyboard().unwrap();
        let moves = |deck: &str| -> HashSet<String> {
            map.keys
                .iter()
                .filter(|k| k.group == deck)
                .map(|k| k.label.clone())
                .collect()
        };
        let one = moves("Deck 1");
        let two = moves("Deck 2");
        assert_eq!(one, two, "the decks do not mirror");
        assert!(one.len() > 15, "only {} moves per deck", one.len());
    }

    /// Command chords are the operating system's. Taking Cmd-Q would quit the
    /// application, and Cmd-W would close the window, mid-set.
    #[test]
    fn the_bundled_keyboard_leaves_command_alone() {
        for chord in keyboard().unwrap().chords() {
            assert_eq!(chord.modifiers & 8, 0, "{} uses meta", chord.text());
        }
    }
}
