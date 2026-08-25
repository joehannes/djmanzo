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
