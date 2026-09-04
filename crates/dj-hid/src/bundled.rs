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

    /// Every deck verb djmanzo has, taken from the parser's own source.
    ///
    /// Read out of `action.rs` rather than kept as a list, because a list is
    /// the thing that goes stale: a verb added to the parser and forgotten here
    /// would leave this test quietly passing over a feature nobody can reach,
    /// which is precisely the failure it exists to catch.
    fn every_deck_verb() -> Vec<String> {
        let source = include_str!("../../dj-core/src/action.rs");
        let start = source
            .find("fn parse_deck_verb")
            .expect("the deck verb parser moved");
        let end = source[start..]
            .find("\nfn ")
            .map_or(source.len(), |offset| start + offset);
        let mut verbs = Vec::new();
        for line in source[start..end].lines() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with('"') {
                continue;
            }
            let Some(arrow) = trimmed.find("=>") else {
                continue;
            };
            // The left of `=>` is one or more quoted verbs joined by `|`.
            for piece in trimmed[..arrow].split('|') {
                let piece = piece.trim();
                if let Some(verb) = piece.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
                    verbs.push(verb.to_owned());
                }
            }
        }

        // The verbs with a sub-grammar of their own do not reach the match
        // above: they are handled in `Action::parse`'s `deck` branch, as
        // `verb == "..."`, because they take more than one word. Scraping only
        // the match left this guard silently not guarding them -- `fx` has
        // been invisible to it since it was written, and `hotcue_move` since it
        // was added. A guard that stops guarding is worse than none.
        for line in source.lines() {
            let Some(rest) = line.split_once("verb == \"") else {
                continue;
            };
            if let Some((verb, _)) = rest.1.split_once('"') {
                verbs.push(verb.to_owned());
            }
        }
        assert!(
            verbs.len() > 40,
            "only {} verbs were read out of the parser, so the extraction broke rather than \
             the vocabulary shrinking",
            verbs.len()
        );
        verbs
    }

    /// Every deck verb the bundled keyboard can send.
    fn keyboard_verbs() -> std::collections::HashSet<String> {
        let map = keyboard().expect("the bundled keyboard parses");
        let mut found = std::collections::HashSet::new();
        for key in &map.keys {
            for action in [key.press.as_deref(), key.release.as_deref()]
                .into_iter()
                .flatten()
            {
                // An action line may chain several with `&`.
                for one in action.split('&') {
                    let words: Vec<&str> = one.split_whitespace().collect();
                    if words.len() >= 3 && words[0] == "deck" {
                        found.insert(words[2].to_owned());
                    }
                }
            }
        }
        found
    }

    /// Verbs that deliberately have no key, and why.
    ///
    /// An allow-list rather than a smaller assertion, because "this one is fine
    /// to leave out" is a judgement and judgements should be written down. Every
    /// entry here is one of two things:
    ///
    /// - **continuous** -- it carries a position, and a key is a switch. A key
    ///   can kill an EQ band and cannot sweep one.
    /// - **covered by a toggle** -- the explicit on/off pair exists for scripts
    ///   and for controllers with two buttons; the keyboard uses the toggle.
    ///
    /// Anything else missing is a feature a DJ with no controller cannot use,
    /// which is what this list exists to keep visible.
    const KEYLESS: &[(&str, &str)] = &[
        ("jog", "continuous: a platter angle"),
        ("jog_touch", "continuous: paired with the platter"),
        ("jog_release", "continuous: paired with the platter"),
        ("seek", "continuous: a position in the track"),
        ("pitch", "continuous: a fader"),
        (
            "rate",
            "continuous: a fader, and `pitch` is the one a DJ names",
        ),
        ("volume", "continuous: a fader"),
        ("gain", "continuous: a trim"),
        ("filter", "continuous: a sweep"),
        ("stem_volume", "continuous"),
        ("stem_eq_low", "continuous"),
        ("stem_eq_mid", "continuous"),
        ("stem_eq_high", "continuous"),
        ("stem_filter", "continuous"),
        ("grid_scale", "continuous: a tempo multiplier"),
        ("grid_bpm", "continuous: a tempo, typed rather than pressed"),
        ("loop_move", "continuous: a distance"),
        (
            "loop_in_at",
            "continuous: a place in the record, which is a pointer's gesture.              The keyboard path to a loop is loop_in, loop_out and the halve              and double keys -- see §26",
        ),
        (
            "loop_out_at",
            "continuous: a place in the record, as loop_in_at",
        ),
        (
            "phrase_at",
            "continuous: a place in the record. A phrase boundary is dragged \
             where it is drawn; there is no keyboard gesture for that line, \
             over there",
        ),
        (
            "phrase_at",
            "continuous: a place in the record. A phrase boundary is dragged \
             where it is drawn; there is no keyboard gesture for \"that line, \
             over there\"",
        ),
        (
            "hotcue_move",
            "continuous: a place in the record. The keyboard sets a cue at the \
             playhead with the pad keys; moving one is a pointer's gesture",
        ),
        (
            "fx",
            "a sub-grammar of its own: a slot, a parameter and a value, which \
             is a rack rather than a key",
        ),
        ("slice_domain", "continuous: a length"),
        ("play", "covered by play_pause"),
        ("pause", "covered by play_pause"),
        ("playpause", "an alias of play_pause"),
        ("cue_on", "covered by cue_toggle"),
        ("cue_off", "covered by cue_toggle"),
        ("keylock_on", "covered by keylock_toggle"),
        ("keylock_off", "covered by keylock_toggle"),
        ("slip_on", "covered by slip_toggle"),
        ("slip_off", "covered by slip_toggle"),
        ("sync", "covered by sync_toggle"),
        ("sync_off", "covered by sync_toggle"),
        (
            "reverse_toggle",
            "the keyboard holds reverse rather than latching it: a track playing \
             backwards because a key was pressed and forgotten is not a state to \
             be one keystroke away from",
        ),
        ("stem_mute_on", "covered by stem_mute"),
        ("stem_mute_off", "covered by stem_mute"),
        (
            "stem_mute",
            "no key: four stems on two decks is eight more chords on an already \
             dense default, and stems need a separated track, so this lives on \
             the pad grid and the stems panel like the slicer does",
        ),
        ("stem_solo_on", "no key: soloing a stem is a panel decision"),
        (
            "stem_solo_off",
            "no key: soloing a stem is a panel decision",
        ),
        ("loop_save", "no key: saved loops are a browser feature"),
        ("loop_recall", "no key: saved loops are a browser feature"),
        ("slice", "no key: the slicer is a pad-grid instrument"),
        ("slice_off", "no key: the slicer is a pad-grid instrument"),
        (
            "grid_reset",
            "no key: undoing a grid edit is a panel decision",
        ),
    ];

    /// **A DJ with no controller can reach everything a DJ with one can.**
    ///
    /// The reason this is a test and not a review: the vocabulary grows, and a
    /// verb added to the engine with no key is invisible -- it works, it is
    /// tested, and the only person who finds out it is unreachable is somebody
    /// on a laptop in a booth. That is the shape of thing this project keeps
    /// finding, one layer up.
    ///
    /// It found three the first time it ran: **the crossfader could not be
    /// assigned**, **a beat grid could not be corrected**, and **sync could be
    /// engaged and never released** -- there was no toggle verb at all, so
    /// every key and every controller pad in existence could only turn it on.
    #[test]
    fn every_deck_verb_is_on_a_key_or_says_why_not() {
        let excused: std::collections::HashMap<&str, &str> = KEYLESS.iter().copied().collect();
        let reachable = keyboard_verbs();
        let mut orphans = Vec::new();
        for verb in every_deck_verb() {
            if reachable.contains(&verb) || excused.contains_key(verb.as_str()) {
                continue;
            }
            orphans.push(verb);
        }
        assert!(
            orphans.is_empty(),
            "these can be done with a controller and not with a keyboard, and nothing says \
             why: {}. Give each one a key, or add it to KEYLESS with the reason.",
            orphans.join(", ")
        );
    }

    /// **An excuse must still be needed.**
    ///
    /// One of these was written backwards on the first attempt: `reverse_on`
    /// and `reverse_off` were excused as "covered by reverse_toggle" when the
    /// keyboard binds exactly those two and not the toggle. Harmless, and
    /// exactly the kind of stale note that makes a list stop being read.
    #[test]
    fn nothing_is_excused_that_is_already_on_a_key() {
        let reachable = keyboard_verbs();
        let stale: Vec<&str> = KEYLESS
            .iter()
            .map(|(verb, _)| *verb)
            .filter(|verb| reachable.contains(*verb))
            .collect();
        assert!(
            stale.is_empty(),
            "these are excused from having a key and have one: {}",
            stale.join(", ")
        );
    }

    /// The allow-list must not outlive the thing it excuses.
    ///
    /// A verb renamed or removed leaves an entry here that excuses nothing,
    /// and the next verb with a similar name inherits an excuse written for a
    /// different feature.
    #[test]
    fn nothing_is_excused_that_does_not_exist() {
        let verbs: std::collections::HashSet<String> = every_deck_verb().into_iter().collect();
        for (verb, reason) in KEYLESS {
            assert!(
                verbs.contains(*verb),
                "KEYLESS excuses `{verb}` ({reason}), which is not a verb djmanzo has"
            );
        }
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
