//! The keyboard as a controller.
//!
//! # Why this is in the hardware crate
//!
//! A laptop keyboard *is* the controller for most of the people who will open
//! djmanzo for the first time, and for every DJ on a train. Treating it as one
//! rather than as a pile of UI shortcuts means it goes through the same
//! vocabulary, the same validation and the same file format as a controller
//! mapping — so it can be remapped, shared, and shown in the same list.
//!
//! # Why keys are named by physical position
//!
//! A binding names `KeyQ`, not `q`. That is the browser's `KeyboardEvent.code`:
//! the key in the position where a US layout has Q. On an AZERTY keyboard that
//! key is A and on QWERTZ it is still Q, but in all three it is *the key above
//! the left hand's ring finger*, which is what a mapping actually means. Naming
//! it by the character produced would move half the transport controls under a
//! French DJ's fingers the moment they switched layout.
//!
//! # The shape of a keyboard mapping
//!
//! ```toml
//! name = "Default"
//!
//! [[key]]
//! on = "Space"
//! press = "deck 1 play_pause"
//!
//! [[key]]
//! on = "shift+Space"
//! press = "deck 2 play_pause"
//!
//! [[key]]
//! on = "KeyQ"
//! press = "deck 1 censor_on"
//! release = "deck 1 censor_off"
//! ```
//!
//! `press` fires on key-down and `release` on key-up. A key with both is
//! momentary — held for as long as the finger is down — and a key with only
//! `press` latches. Auto-repeat is not a press: holding a key down does not
//! fire it eighty times a second.

use serde::{Deserialize, Serialize};

/// The modifiers a chord may carry, in the order they are written.
///
/// `meta` is Command on a Mac and the Windows key elsewhere. It is offered but
/// unused by the bundled mapping, because Command chords belong to the
/// operating system's menus and taking them would break Cmd-Q mid-set.
const MODIFIERS: [(&str, u8); 4] = [("ctrl", 1), ("alt", 2), ("shift", 4), ("meta", 8)];

/// A key, with its modifiers, in a form two spellings of the same chord share.
///
/// `shift+KeyA` and `Shift+keya` are the same chord and must hash the same, or
/// a mapping would work only when it was typed the way the lookup expected.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chord {
    /// The physical key, lowercased for comparison but kept for display.
    pub code: String,
    /// Bit set: ctrl 1, alt 2, shift 4, meta 8.
    pub modifiers: u8,
}

impl Chord {
    /// Parse `shift+alt+KeyA`. Modifiers may come in any order.
    pub fn parse(text: &str) -> Result<Self, KeyError> {
        let mut modifiers = 0u8;
        let mut code = None;
        for part in text.split('+') {
            let part = part.trim();
            if part.is_empty() {
                return Err(KeyError::BadKey(text.to_owned()));
            }
            let lowered = part.to_ascii_lowercase();
            match MODIFIERS.iter().find(|(name, _)| *name == lowered) {
                Some((_, bit)) => modifiers |= bit,
                // Two keys in one chord is a typo, not a two-key chord: no
                // keyboard reports which of them arrived first in a way a
                // mapping could act on.
                None if code.is_some() => return Err(KeyError::BadKey(text.to_owned())),
                None => code = Some(lowered),
            }
        }
        Ok(Chord {
            code: code.ok_or_else(|| KeyError::BadKey(text.to_owned()))?,
            modifiers,
        })
    }

    /// Build a chord from what a key event reports.
    #[must_use]
    pub fn from_event(code: &str, ctrl: bool, alt: bool, shift: bool, meta: bool) -> Self {
        let mut modifiers = 0u8;
        for (flag, bit) in [(ctrl, 1u8), (alt, 2), (shift, 4), (meta, 8)] {
            if flag {
                modifiers |= bit;
            }
        }
        Chord {
            code: code.to_ascii_lowercase(),
            modifiers,
        }
    }

    /// The canonical spelling, which is what the interface shows and what a
    /// lookup table is keyed by.
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (name, bit) in MODIFIERS {
            if self.modifiers & bit != 0 {
                out.push_str(name);
                out.push('+');
            }
        }
        out.push_str(&self.code);
        out
    }
}

/// One key and what it does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    /// `Space`, `KeyQ`, `shift+Digit1`.
    pub on: String,
    /// Sent on key-down, once, however long the key is held.
    #[serde(default)]
    pub press: Option<String>,
    /// Sent on key-up.
    #[serde(default)]
    pub release: Option<String>,
    /// What the key does, in words, for the shortcut sheet. Written by hand
    /// rather than derived from the action, because "cue" is what a DJ calls
    /// it and `deck 1 cue_press` is what the engine calls it.
    #[serde(default)]
    pub label: String,
    /// Which group it belongs to on the shortcut sheet.
    #[serde(default)]
    pub group: String,
}

/// A whole keyboard mapping file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeyMap {
    pub name: String,
    #[serde(default, rename = "key")]
    pub keys: Vec<KeyBinding>,
    /// Parsed chords, in step with `keys`, built once when the file loads.
    #[serde(skip)]
    chords: Vec<Chord>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error("{0:?} is not a key")]
    BadKey(String),
    #[error("{0:?} is not something djmanzo can do: {1}")]
    BadAction(String, String),
    #[error("the key {0:?} says nothing to do")]
    Silent(String),
    #[error("the key {0:?} is bound twice")]
    Twice(String),
    #[error("could not read the mapping: {0}")]
    Unreadable(String),
}

impl KeyMap {
    /// Read a keyboard mapping from TOML, checking every action it contains.
    pub fn parse(text: &str) -> Result<Self, KeyError> {
        let mut map: KeyMap =
            toml::from_str(text).map_err(|e| KeyError::Unreadable(e.to_string()))?;
        map.prepare()?;
        Ok(map)
    }

    fn prepare(&mut self) -> Result<(), KeyError> {
        self.chords = Vec::with_capacity(self.keys.len());
        for key in &self.keys {
            let chord = Chord::parse(&key.on)?;
            // A key bound twice is not a layered key, it is a mistake — and
            // the one that wins would depend on file order, which is exactly
            // the kind of thing nobody debugs at a gig.
            if self.chords.contains(&chord) {
                return Err(KeyError::Twice(key.on.clone()));
            }
            self.chords.push(chord);

            if key.press.is_none() && key.release.is_none() {
                return Err(KeyError::Silent(key.on.clone()));
            }
            for action in [&key.press, &key.release].into_iter().flatten() {
                dj_core::Action::parse(action)
                    .map_err(|e| KeyError::BadAction(action.clone(), e.to_string()))?;
            }
        }
        Ok(())
    }

    /// What a key event means. `down` false is a key-up.
    ///
    /// `repeat` is the operating system's auto-repeat, and is never a press: a
    /// finger resting on the cue key should not fire ninety cues.
    #[must_use]
    pub fn translate(&self, chord: &Chord, down: bool, repeat: bool) -> Option<&str> {
        if down && repeat {
            return None;
        }
        let index = self.chords.iter().position(|c| c == chord)?;
        let key = self.keys.get(index)?;
        let action = if down { &key.press } else { &key.release };
        action.as_deref()
    }

    /// Whether this mapping claims a chord at all — what the interface asks
    /// before deciding to swallow a key event.
    #[must_use]
    pub fn claims(&self, chord: &Chord) -> bool {
        self.chords.contains(chord)
    }

    /// The parsed chords, in step with `keys`.
    #[must_use]
    pub fn chords(&self) -> &[Chord] {
        &self.chords
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(text: &str) -> KeyMap {
        KeyMap::parse(text).unwrap_or_else(|e| panic!("\n{text}\n{e}"))
    }

    fn press(code: &str) -> Chord {
        Chord::from_event(code, false, false, false, false)
    }

    #[test]
    fn a_key_down_and_up_are_different_actions() {
        let m = map(r#"
            name = "Test"
            [[key]]
            on = "KeyQ"
            press = "deck 1 censor_on"
            release = "deck 1 censor_off"
        "#);
        assert_eq!(
            m.translate(&press("KeyQ"), true, false),
            Some("deck 1 censor_on")
        );
        assert_eq!(
            m.translate(&press("KeyQ"), false, false),
            Some("deck 1 censor_off")
        );
    }

    /// A latching key has nothing to say when the finger comes up, and must
    /// not fall back to the press action.
    #[test]
    fn a_latching_key_is_silent_on_release() {
        let m = map(r#"
            name = "Test"
            [[key]]
            on = "Space"
            press = "deck 1 play_pause"
        "#);
        assert_eq!(
            m.translate(&press("Space"), true, false),
            Some("deck 1 play_pause")
        );
        assert_eq!(m.translate(&press("Space"), false, false), None);
    }

    /// The reason auto-repeat is handled here rather than in the interface:
    /// every caller would have to remember, and the one that forgot would fire
    /// thirty cue jumps a second.
    #[test]
    fn auto_repeat_is_not_a_press() {
        let m = map(r#"
            name = "Test"
            [[key]]
            on = "Space"
            press = "deck 1 play_pause"
        "#);
        assert_eq!(m.translate(&press("Space"), true, true), None);
    }

    /// A key-up is delivered even when it repeats, because it cannot: there is
    /// no such thing as an auto-repeating release, and refusing one would leave
    /// a censor stuck on.
    #[test]
    fn a_release_is_delivered_whatever_the_repeat_flag_says() {
        let m = map(r#"
            name = "Test"
            [[key]]
            on = "KeyQ"
            press = "deck 1 censor_on"
            release = "deck 1 censor_off"
        "#);
        assert_eq!(
            m.translate(&press("KeyQ"), false, true),
            Some("deck 1 censor_off")
        );
    }

    #[test]
    fn shift_makes_a_different_key() {
        let m = map(r#"
            name = "Test"
            [[key]]
            on = "Space"
            press = "deck 1 play_pause"
            [[key]]
            on = "shift+Space"
            press = "deck 2 play_pause"
        "#);
        let shifted = Chord::from_event("Space", false, false, true, false);
        assert_eq!(
            m.translate(&press("Space"), true, false),
            Some("deck 1 play_pause")
        );
        assert_eq!(
            m.translate(&shifted, true, false),
            Some("deck 2 play_pause")
        );
    }

    /// An unshifted binding must not answer a shifted key. Otherwise every
    /// shift layer would fire both halves.
    #[test]
    fn a_plain_binding_does_not_answer_a_shifted_key() {
        let m = map(r#"
            name = "Test"
            [[key]]
            on = "Space"
            press = "deck 1 play_pause"
        "#);
        let shifted = Chord::from_event("Space", false, false, true, false);
        assert_eq!(m.translate(&shifted, true, false), None);
        assert!(!m.claims(&shifted));
    }

    #[test]
    fn modifiers_may_be_written_in_any_order_and_any_case() {
        let a = Chord::parse("shift+alt+KeyA").unwrap();
        let b = Chord::parse("ALT+Shift+keya").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.text(), "alt+shift+keya");
    }

    #[test]
    fn every_modifier_is_understood() {
        let all = Chord::parse("ctrl+alt+shift+meta+KeyA").unwrap();
        assert_eq!(all.modifiers, 1 | 2 | 4 | 8);
        assert_eq!(all, Chord::from_event("KeyA", true, true, true, true));
    }

    #[test]
    fn a_chord_with_no_key_is_refused() {
        assert!(Chord::parse("shift+").is_err());
        assert!(Chord::parse("shift").is_err());
        assert!(Chord::parse("").is_err());
    }

    #[test]
    fn two_keys_in_one_chord_are_refused() {
        assert!(matches!(
            Chord::parse("KeyA+KeyB"),
            Err(KeyError::BadKey(_))
        ));
    }

    /// The check that makes a shared mapping safe: it cannot say anything the
    /// vocabulary does not already contain.
    #[test]
    fn an_action_the_vocabulary_does_not_have_is_refused_when_the_file_loads() {
        let bad = KeyMap::parse(
            r#"
            name = "Test"
            [[key]]
            on = "Space"
            press = "deck 1 launch_the_missiles"
        "#,
        );
        assert!(matches!(bad, Err(KeyError::BadAction(_, _))), "{bad:?}");
    }

    #[test]
    fn a_key_bound_twice_is_refused() {
        let bad = KeyMap::parse(
            r#"
            name = "Test"
            [[key]]
            on = "Space"
            press = "deck 1 play_pause"
            [[key]]
            on = "Space"
            press = "deck 2 play_pause"
        "#,
        );
        assert!(matches!(bad, Err(KeyError::Twice(_))), "{bad:?}");
    }

    /// Bound twice means the same chord, not the same spelling of it.
    #[test]
    fn the_same_chord_spelled_differently_still_counts_as_twice() {
        let bad = KeyMap::parse(
            r#"
            name = "Test"
            [[key]]
            on = "shift+Space"
            press = "deck 1 play_pause"
            [[key]]
            on = "SHIFT+space"
            press = "deck 2 play_pause"
        "#,
        );
        assert!(matches!(bad, Err(KeyError::Twice(_))), "{bad:?}");
    }

    #[test]
    fn a_key_that_does_nothing_is_refused() {
        let bad = KeyMap::parse(
            r#"
            name = "Test"
            [[key]]
            on = "Space"
        "#,
        );
        assert!(matches!(bad, Err(KeyError::Silent(_))), "{bad:?}");
    }
}
