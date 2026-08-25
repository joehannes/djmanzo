//! Building a mapping from inside the application.
//!
//! M4's promise is that adding a controller means editing a file rather than
//! rebuilding the application. That is only half true while the only way to
//! write the file is by hand from a manual: a DJ with an unsupported
//! controller and no documentation has nothing to type. So this is the other
//! half -- press the pad, pick what it should do, save.
//!
//! # The invariant everything here rests on
//!
//! **Anything the editor can produce, the parser can read.** An editor that
//! wrote a file the loader refused would be worse than no editor: the work is
//! lost at the moment it is saved, which is the moment a DJ stops paying
//! attention. [`Draft::to_toml`] and [`Mapping::parse`] are therefore tested
//! against each other over every shape a draft can take, not spot-checked.
//!
//! # Learning is describing, not guessing
//!
//! [`describe`] turns a message that arrived into the exact `on = "..."` text
//! the parser accepts. It does not invent a control kind or normalise a
//! channel: what comes back is what a person would have written having read
//! the manual, which is what makes the learned file editable afterwards.

use crate::mapping::{Binding, Encoding, Mapping, MappingError};
use crate::message::Message;

/// The `on = "..."` text for a message that just arrived.
///
/// Channels are 1-based here because a mapping file is read by a person, the
/// same convention [`Message::from_bytes`] uses on the way in. Numbers are hex
/// because that is how controller documentation writes them, so a learned file
/// and a hand-written one look alike.
#[must_use]
pub fn describe(message: Message) -> String {
    match message {
        Message::NoteOn { channel, note, .. } | Message::NoteOff { channel, note } => {
            format!("note {} {:#04x}", channel + 1, note)
        }
        Message::Control {
            channel,
            controller,
            ..
        } => format!("cc {} {:#04x}", channel + 1, controller),
        Message::PitchBend { channel, .. } => format!("bend {}", channel + 1),
    }
}

/// What a learned control should do.
///
/// Separate from [`Binding`] because a binding has five optional action fields
/// and only some combinations mean anything -- a control with both `press` and
/// `move` is not a control, it is two. Naming the intent makes the impossible
/// combinations unrepresentable rather than merely discouraged.
#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    /// A button that does something and stays done: a cue, a hot cue.
    Latching { press: String },
    /// A button that undoes itself: a censor, a kill, a brake.
    Momentary { press: String, release: String },
    /// A fader or knob. The action should contain `{value}`.
    Continuous {
        action: String,
        min: Option<f32>,
        max: Option<f32>,
    },
    /// An endless encoder.
    Encoder {
        up: String,
        down: String,
        encoding: Encoding,
    },
}

impl Role {
    /// This role as a binding on `trigger`.
    #[must_use]
    pub fn bind(&self, trigger: String) -> Binding {
        let mut binding = Binding {
            on: trigger,
            press: None,
            release: None,
            moved: None,
            turn_up: None,
            turn_down: None,
            encoding: Encoding::default(),
            min: None,
            max: None,
        };
        match self {
            Role::Latching { press } => binding.press = Some(press.clone()),
            Role::Momentary { press, release } => {
                binding.press = Some(press.clone());
                binding.release = Some(release.clone());
            }
            Role::Continuous { action, min, max } => {
                binding.moved = Some(action.clone());
                binding.min = *min;
                binding.max = *max;
            }
            Role::Encoder { up, down, encoding } => {
                binding.turn_up = Some(up.clone());
                binding.turn_down = Some(down.clone());
                binding.encoding = *encoding;
            }
        }
        binding
    }
}

/// A mapping being built.
///
/// Kept as its own type rather than editing a [`Mapping`] in place because a
/// `Mapping` carries parsed triggers and per-control state that a
/// half-finished draft has no business holding.
#[derive(Debug, Clone, Default)]
pub struct Draft {
    pub name: String,
    pub device: String,
    bindings: Vec<Binding>,
}

impl Draft {
    #[must_use]
    pub fn new(name: impl Into<String>, device: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            device: device.into(),
            bindings: Vec::new(),
        }
    }

    /// Start from a mapping that already exists, to edit it.
    #[must_use]
    pub fn from_mapping(mapping: &Mapping) -> Self {
        Self {
            name: mapping.name.clone(),
            device: mapping.device.clone(),
            bindings: mapping.bindings.clone(),
        }
    }

    #[must_use]
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Give `trigger` a role, replacing whatever was on that control.
    ///
    /// Replacing rather than appending is the point: a DJ who binds the same
    /// pad twice meant to change their mind. Two bindings on one control would
    /// both fire, which is never what anybody wants and is invisible in a file
    /// until the pad does two things at once.
    ///
    /// # Errors
    /// If the trigger or any of the role's actions would not parse. Checked
    /// here, at the moment of binding, so the message names the control the DJ
    /// is looking at rather than appearing when the file is next opened.
    pub fn bind(&mut self, trigger: &str, role: &Role) -> Result<(), MappingError> {
        let binding = role.bind(trigger.to_owned());
        check(&binding)?;
        self.unbind(trigger);
        self.bindings.push(binding);
        Ok(())
    }

    /// Remove whatever is on `trigger`. Returns whether anything was there.
    pub fn unbind(&mut self, trigger: &str) -> bool {
        let before = self.bindings.len();
        self.bindings.retain(|b| b.on != trigger);
        self.bindings.len() != before
    }

    /// What `trigger` currently does, if anything.
    #[must_use]
    pub fn binding(&self, trigger: &str) -> Option<&Binding> {
        self.bindings.iter().find(|b| b.on == trigger)
    }

    /// The draft as a mapping file.
    ///
    /// # Errors
    /// If the draft cannot be written as TOML, which would mean a name or an
    /// action containing something TOML cannot hold.
    pub fn to_toml(&self) -> Result<String, MappingError> {
        let mapping = Mapping::from_parts(
            self.name.clone(),
            self.device.clone(),
            self.bindings.clone(),
        );
        let body = toml::to_string_pretty(&mapping)
            .map_err(|e| MappingError::Unreadable(e.to_string()))?;
        Ok(format!("{HEADER}{body}"))
    }

    /// The draft as a `Mapping`, parsed the same way a file would be.
    ///
    /// Deliberately goes through the text rather than constructing a `Mapping`
    /// directly: what a DJ gets when they reload their saved file is exactly
    /// what this returns, so a draft that cannot be saved cannot be used
    /// either.
    ///
    /// # Errors
    /// As [`Mapping::parse`].
    pub fn build(&self) -> Result<Mapping, MappingError> {
        Mapping::parse(&self.to_toml()?)
    }
}

/// Written at the top of every saved mapping, because a DJ who opens the file
/// later should be able to tell where it came from and that hand-editing is
/// expected.
const HEADER: &str = "\
# Written by djmanzo's mapping editor.
#
# Safe to edit by hand: every action here is parsed when the file loads, so a
# typo is a message when you choose the mapping rather than a control that
# quietly does nothing. See docs/CONTROLLERS.md for the vocabulary.

";

/// Check a binding the way loading a file would.
fn check(binding: &Binding) -> Result<(), MappingError> {
    crate::mapping::Trigger::parse(&binding.on)?;

    let actions = [
        &binding.press,
        &binding.release,
        &binding.moved,
        &binding.turn_up,
        &binding.turn_down,
    ];
    if actions.iter().all(|a| a.is_none()) {
        return Err(MappingError::Silent(binding.on.clone()));
    }
    for action in actions.into_iter().flatten() {
        // `{value}` is substituted at dispatch time, so it has to be stood in
        // for to check the rest of the line -- the same way the loader does.
        let probe = action.replace("{value}", "0.5");
        dj_core::Action::parse(&probe)
            .map_err(|e| MappingError::BadAction(action.clone(), e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn latch(action: &str) -> Role {
        Role::Latching {
            press: action.to_owned(),
        }
    }

    // -- learning ----------------------------------------------------------

    /// What comes back has to be what a person would have written having read
    /// the manual -- otherwise the learned file is not editable afterwards.
    #[test]
    fn a_learned_control_is_described_the_way_a_file_writes_it() {
        assert_eq!(
            describe(Message::NoteOn {
                channel: 0,
                note: 0x0B,
                velocity: 127
            }),
            "note 1 0x0b"
        );
        assert_eq!(
            describe(Message::Control {
                channel: 1,
                controller: 0x16,
                value: 64
            }),
            "cc 2 0x16"
        );
        assert_eq!(
            describe(Message::PitchBend {
                channel: 0,
                value: 8192
            }),
            "bend 1"
        );
    }

    /// A pad going down and coming up is one control, so learning either has
    /// to name the same thing -- otherwise releasing a pad would bind a second
    /// control that is really the first one again.
    #[test]
    fn a_note_off_describes_the_same_control_as_its_note_on() {
        let on = describe(Message::NoteOn {
            channel: 2,
            note: 0x20,
            velocity: 100,
        });
        let off = describe(Message::NoteOff {
            channel: 2,
            note: 0x20,
        });
        assert_eq!(on, off);
    }

    /// **The join between the two halves.** Whatever `describe` produces, the
    /// parser must accept -- that is the entire learn path in one assertion.
    #[test]
    fn everything_learnable_is_parseable() {
        let messages = [
            Message::NoteOn {
                channel: 0,
                note: 0,
                velocity: 1,
            },
            Message::NoteOn {
                channel: 15,
                note: 127,
                velocity: 127,
            },
            Message::NoteOff {
                channel: 7,
                note: 64,
            },
            Message::Control {
                channel: 0,
                controller: 0,
                value: 0,
            },
            Message::Control {
                channel: 15,
                controller: 127,
                value: 127,
            },
            Message::PitchBend {
                channel: 0,
                value: 0,
            },
            Message::PitchBend {
                channel: 15,
                value: 16_383,
            },
        ];
        for message in messages {
            let text = describe(message);
            let trigger = crate::mapping::Trigger::parse(&text)
                .unwrap_or_else(|e| panic!("learned {text:?} and could not parse it: {e}"));
            assert!(
                trigger.matches(message),
                "learned {text:?} from a message it then does not match"
            );
        }
    }

    // -- binding -----------------------------------------------------------

    #[test]
    fn binding_a_control_puts_the_action_on_it() {
        let mut draft = Draft::new("Mine", "Some Controller");
        draft
            .bind("note 1 0x0b", &latch("deck 1 play_pause"))
            .unwrap();

        let binding = draft.binding("note 1 0x0b").expect("it was just bound");
        assert_eq!(binding.press.as_deref(), Some("deck 1 play_pause"));
        assert_eq!(binding.release, None, "a latch has no release");
    }

    #[test]
    fn a_momentary_control_gets_both_halves() {
        let mut draft = Draft::new("Mine", "");
        draft
            .bind(
                "note 1 0x0c",
                &Role::Momentary {
                    press: "deck 1 censor_on".to_owned(),
                    release: "deck 1 censor_off".to_owned(),
                },
            )
            .unwrap();

        let binding = draft.binding("note 1 0x0c").unwrap();
        assert_eq!(binding.press.as_deref(), Some("deck 1 censor_on"));
        assert_eq!(binding.release.as_deref(), Some("deck 1 censor_off"));
    }

    #[test]
    fn a_continuous_control_keeps_its_range() {
        let mut draft = Draft::new("Mine", "");
        draft
            .bind(
                "cc 1 0x07",
                &Role::Continuous {
                    action: "deck 1 eq_high {value}".to_owned(),
                    min: Some(0.0),
                    max: Some(4.0),
                },
            )
            .unwrap();

        let binding = draft.binding("cc 1 0x07").unwrap();
        assert_eq!(binding.moved.as_deref(), Some("deck 1 eq_high {value}"));
        assert_eq!(binding.max, Some(4.0));
    }

    /// **Binding the same pad twice is changing your mind, not adding a
    /// second job.** Two bindings on one control would both fire, which is
    /// invisible in a file until the pad does two things at once.
    #[test]
    fn binding_a_control_again_replaces_what_was_there() {
        let mut draft = Draft::new("Mine", "");
        draft
            .bind("note 1 0x0b", &latch("deck 1 play_pause"))
            .unwrap();
        draft.bind("note 1 0x0b", &latch("deck 1 cue")).unwrap();

        assert_eq!(draft.bindings().len(), 1, "the control was bound twice");
        assert_eq!(
            draft.binding("note 1 0x0b").unwrap().press.as_deref(),
            Some("deck 1 cue")
        );
    }

    #[test]
    fn unbinding_says_whether_there_was_anything_to_remove() {
        let mut draft = Draft::new("Mine", "");
        draft.bind("note 1 0x0b", &latch("deck 1 cue")).unwrap();

        assert!(draft.unbind("note 1 0x0b"));
        assert!(!draft.unbind("note 1 0x0b"), "removed it twice");
        assert!(draft.is_empty());
    }

    /// **The message has to arrive while the DJ is looking at the control.**
    /// Checking at save time would report it when the pad is long forgotten.
    #[test]
    fn a_bad_action_is_refused_at_the_moment_of_binding() {
        let mut draft = Draft::new("Mine", "");
        let error = draft
            .bind("note 1 0x0b", &latch("deck 1 plya"))
            .expect_err("that is not an action");
        assert!(
            error.to_string().contains("plya"),
            "the message should name the typo: {error}"
        );
        assert!(draft.is_empty(), "a refused binding was still added");
    }

    #[test]
    fn a_control_that_cannot_be_parsed_is_refused() {
        let mut draft = Draft::new("Mine", "");
        assert!(draft.bind("sysex 1", &latch("deck 1 cue")).is_err());
        assert!(draft.is_empty());
    }

    /// A `{value}` action is checked with the placeholder stood in for, the
    /// same way the loader does it -- otherwise every fader would look broken.
    #[test]
    fn a_value_placeholder_is_checked_the_way_the_loader_checks_it() {
        let mut draft = Draft::new("Mine", "");
        draft
            .bind(
                "cc 1 0x16",
                &Role::Continuous {
                    action: "deck 1 volume {value}".to_owned(),
                    min: None,
                    max: None,
                },
            )
            .expect("a fader is a normal thing to bind");
    }

    // -- saving ------------------------------------------------------------

    /// **The invariant the whole editor rests on.** A file the loader refuses
    /// loses the DJ's work at the moment they stop paying attention.
    #[test]
    fn everything_the_editor_writes_the_loader_reads() {
        let mut draft = Draft::new("Every shape", "Some Controller");
        draft
            .bind("note 1 0x0b", &latch("deck 1 play_pause"))
            .unwrap();
        draft
            .bind(
                "note 1 0x0c",
                &Role::Momentary {
                    press: "deck 1 censor_on".to_owned(),
                    release: "deck 1 censor_off".to_owned(),
                },
            )
            .unwrap();
        draft
            .bind(
                "cc 1 0x07",
                &Role::Continuous {
                    action: "deck 1 eq_high {value}".to_owned(),
                    min: Some(0.0),
                    max: Some(4.0),
                },
            )
            .unwrap();
        draft
            .bind(
                "bend 1",
                &Role::Continuous {
                    action: "deck 1 pitch {value}".to_owned(),
                    min: Some(-1.0),
                    max: Some(1.0),
                },
            )
            .unwrap();
        for encoding in [Encoding::Signed, Encoding::Offset, Encoding::Absolute] {
            draft
                .bind(
                    &format!("cc 1 {:#04x}", 0x30 + encoding as u8),
                    &Role::Encoder {
                        up: "deck 1 beatjump 1".to_owned(),
                        down: "deck 1 beatjump -1".to_owned(),
                        encoding,
                    },
                )
                .unwrap();
        }

        let text = draft.to_toml().expect("a draft has to be writable");
        let reloaded = Mapping::parse(&text)
            .unwrap_or_else(|e| panic!("the editor wrote a file it cannot read: {e}\n\n{text}"));

        assert_eq!(reloaded.name, "Every shape");
        assert_eq!(reloaded.device, "Some Controller");
        assert_eq!(reloaded.bindings.len(), draft.bindings().len());
    }

    /// Reloading a saved file and editing it again has to be lossless, or a
    /// DJ's mapping quietly degrades every time they open it.
    #[test]
    fn a_saved_mapping_survives_a_round_trip_through_the_editor() {
        let mut draft = Draft::new("Round trip", "Device");
        draft
            .bind("note 1 0x0b", &latch("deck 1 play_pause"))
            .unwrap();
        draft
            .bind(
                "cc 1 0x07",
                &Role::Continuous {
                    action: "deck 1 eq_high {value}".to_owned(),
                    min: Some(0.0),
                    max: Some(4.0),
                },
            )
            .unwrap();

        let once = draft.to_toml().unwrap();
        let reopened = Draft::from_mapping(&Mapping::parse(&once).unwrap());
        let twice = reopened.to_toml().unwrap();

        assert_eq!(once, twice, "the mapping changed by being opened");
    }

    /// The bundled mappings are the ones most likely to be opened and edited,
    /// so they are the strongest available test of the round trip.
    #[test]
    fn a_bundled_mapping_can_be_opened_edited_and_saved() {
        for (name, text) in crate::bundled::CONTROLLERS {
            let original = Mapping::parse(text).expect("the bundled file parses");
            let mut draft = Draft::from_mapping(&original);
            let before = draft.bindings().len();

            draft
                .bind("note 9 0x7f", &latch("deck 1 eject"))
                .expect("adding a control to a bundled mapping");

            let saved = draft.to_toml().expect("writable");
            let reloaded = Mapping::parse(&saved)
                .unwrap_or_else(|e| panic!("editing {name} produced an unreadable file: {e}"));

            assert_eq!(reloaded.bindings.len(), before + 1);
        }
    }

    /// A draft that has been through the parser is what a DJ actually gets.
    #[test]
    fn building_a_draft_gives_a_usable_mapping() {
        let mut draft = Draft::new("Usable", "");
        draft
            .bind("note 1 0x0b", &latch("deck 1 play_pause"))
            .unwrap();

        let mapping = draft.build().expect("a bound draft builds");
        assert_eq!(mapping.bindings.len(), 1);
    }

    /// An empty draft is a normal state -- a DJ who has opened the editor and
    /// not yet pressed anything -- and must not be an error.
    #[test]
    fn an_empty_draft_is_a_valid_mapping_with_nothing_in_it() {
        let draft = Draft::new("Nothing yet", "");
        let mapping = draft.build().expect("an empty mapping is still a mapping");
        assert!(mapping.bindings.is_empty());
    }

    /// The header is there so a DJ opening the file later knows it is theirs
    /// to edit. It has to be comments, or it would not parse.
    #[test]
    fn the_header_is_comments_and_survives_parsing() {
        let draft = Draft::new("Header", "");
        let text = draft.to_toml().unwrap();
        assert!(text.starts_with('#'));
        assert!(Mapping::parse(&text).is_ok());
    }
}
