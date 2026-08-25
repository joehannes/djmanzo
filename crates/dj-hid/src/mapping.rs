//! A mapping file, and what it does to a message.
//!
//! # The shape of a mapping
//!
//! ```toml
//! name = "Generic 2-deck"
//! device = "MIDI Mix"
//!
//! [[binding]]
//! on = "note 1 36"
//! press = "deck 1 play_pause"
//!
//! [[binding]]
//! on = "note 1 37"
//! press = "deck 1 censor_on"
//! release = "deck 1 censor_off"
//!
//! [[binding]]
//! on = "cc 1 7"
//! move = "deck 1 volume {value}"
//!
//! [[binding]]
//! on = "cc 1 20"
//! turn_up = "deck 1 beatjump 1"
//! turn_down = "deck 1 beatjump -1"
//! ```
//!
//! `{value}` is the control's position, scaled into the range the action wants.
//! It is the same idea as `{deck}` in a preset, and deliberately the same
//! spelling.
//!
//! # Why a mapping cannot do anything the interface cannot
//!
//! Every line a mapping produces is parsed by [`dj_core::Action::parse`] before
//! it goes anywhere. A file from a stranger can rebind every control on their
//! own controller and cannot invent a capability, because there is no way to
//! say anything the vocabulary does not already contain. That is what makes
//! mappings safe to share.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// What a binding listens for.
///
/// Channels are 1-based here, matching what a controller's own documentation
/// prints, and 0-based on the wire. The conversion happens once, in
/// [`Trigger::matches`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Trigger {
    /// A pad or button. `note <channel> <note>`.
    Note { channel: u8, note: u8 },
    /// A knob, fader or encoder. `cc <channel> <controller>`.
    Control { channel: u8, controller: u8 },
    /// A high-resolution control that uses the pitch wheel. `bend <channel>`.
    Bend { channel: u8 },
}

impl Trigger {
    /// Parse the `on = "..."` line.
    pub fn parse(text: &str) -> Result<Self, MappingError> {
        let mut words = text.split_whitespace();
        let kind = words.next().ok_or(MappingError::EmptyTrigger)?;
        let number = |word: Option<&str>| -> Result<u8, MappingError> {
            let word = word.ok_or_else(|| MappingError::BadTrigger(text.to_owned()))?;
            // Hex is offered because controller documentation is usually
            // written in it, and transcribing 0x0B as 11 by hand is a mistake
            // waiting to be made.
            let parsed = match word.strip_prefix("0x").or_else(|| word.strip_prefix("0X")) {
                Some(hex) => u8::from_str_radix(hex, 16),
                None => word.parse(),
            };
            parsed.map_err(|_| MappingError::BadTrigger(text.to_owned()))
        };

        match kind {
            "note" => Ok(Trigger::Note {
                channel: number(words.next())?,
                note: number(words.next())?,
            }),
            "cc" => Ok(Trigger::Control {
                channel: number(words.next())?,
                controller: number(words.next())?,
            }),
            "bend" => Ok(Trigger::Bend {
                channel: number(words.next())?,
            }),
            _ => Err(MappingError::BadTrigger(text.to_owned())),
        }
    }

    /// Whether `message` is this trigger. Channel 0 in a file means "any",
    /// because some controllers are configurable and a mapping should not
    /// break when somebody moves theirs to channel 3.
    #[must_use]
    pub fn matches(self, message: crate::Message) -> bool {
        let same_channel = |wanted: u8| wanted == 0 || wanted == message.channel() + 1;
        match (self, message) {
            (Trigger::Note { channel, note }, crate::Message::NoteOn { note: got, .. })
            | (Trigger::Note { channel, note }, crate::Message::NoteOff { note: got, .. }) => {
                same_channel(channel) && note == got
            }
            (
                Trigger::Control {
                    channel,
                    controller,
                },
                crate::Message::Control {
                    controller: got, ..
                },
            ) => same_channel(channel) && controller == got,
            (Trigger::Bend { channel }, crate::Message::PitchBend { .. }) => same_channel(channel),
            _ => false,
        }
    }
}

/// How an encoder reports movement.
///
/// A jog wheel or endless knob sends one of three things and the hardware's
/// manual says which. Getting it wrong reverses the control, so it is written
/// in the mapping rather than inferred from the traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Encoding {
    /// 1..=63 clockwise, 127..=65 anticlockwise, 0 and 64 still. The common
    /// case, and so the default.
    #[default]
    Signed,
    /// 64 is still, above is clockwise, below is anticlockwise.
    Offset,
    /// A position, not a delta. Direction comes from the previous value.
    Absolute,
}

/// One control on the hardware, and what it does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    /// `note 1 36`, `cc 1 7`, `bend 1`.
    pub on: String,
    /// Sent when a button goes down.
    #[serde(default)]
    pub press: Option<String>,
    /// Sent when it comes up. A binding with both is momentary; one with only
    /// `press` latches, which is the difference between a censor and a cue.
    #[serde(default)]
    pub release: Option<String>,
    /// Sent when a continuous control moves. May contain `{value}`.
    #[serde(default, rename = "move")]
    pub moved: Option<String>,
    /// Sent when an encoder turns clockwise.
    #[serde(default)]
    pub turn_up: Option<String>,
    /// Sent anticlockwise.
    #[serde(default)]
    pub turn_down: Option<String>,
    /// Which encoder convention this control uses. Only consulted when the
    /// binding has `turn_up` or `turn_down`.
    #[serde(default)]
    pub encoding: Encoding,
    /// Sent when a **motorised platter** reports its angle. Contains `{value}`,
    /// which is filled with the movement since the last report in revolutions.
    ///
    /// Distinct from `move` because the number means something different: a
    /// fader's value is a position and a platter's is an angle that wraps, and
    /// reading a wrap as a position would be a whole revolution of audio every
    /// time the record passes zero. See [`crate::platter`].
    #[serde(default)]
    pub platter: Option<String>,
    /// Steps in one revolution of that platter, from the device's manual.
    ///
    /// Required alongside `platter`: without it there is no way to tell a
    /// movement from a wrap, and every device counts differently.
    #[serde(default)]
    pub resolution: Option<u32>,
    /// What `{value}` runs between. Defaults to 0..=1, which is what most of
    /// the vocabulary wants; an EQ wants 0..=4 and a pitch fader -1..=1.
    #[serde(default)]
    pub min: Option<f32>,
    #[serde(default)]
    pub max: Option<f32>,
}

/// A whole mapping file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Mapping {
    pub name: String,
    /// The port name to look for, matched loosely — a device announces itself
    /// with a different suffix on every platform.
    #[serde(default)]
    pub device: String,
    #[serde(default, rename = "binding")]
    pub bindings: Vec<Binding>,
    /// Which sockets on this controller carry which bus.
    ///
    /// In the mapping rather than a settings panel because it is the same fact
    /// about the same piece of hardware as which pad is play, and a DJ should
    /// not have to find it separately with a crowd waiting. See
    /// [`crate::audio`].
    #[serde(default)]
    pub audio: Option<crate::audio::AudioPreset>,

    /// Parsed triggers, built once when the file loads.
    #[serde(skip)]
    triggers: Vec<Trigger>,
    /// The last value seen for each control, so an encoder knows which way it
    /// turned and a fader can be asked what it is set to.
    #[serde(skip)]
    last: HashMap<Trigger, u8>,
    /// One unwrapper per motorised platter, keyed by the control it is on.
    ///
    /// Per mapping rather than per message because a platter's angle only
    /// means anything against the previous one.
    #[serde(skip)]
    platters: HashMap<Trigger, crate::platter::AbsolutePlatter>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MappingError {
    #[error("a binding has no `on`")]
    EmptyTrigger,
    #[error("{0:?} is not a control this understands")]
    BadTrigger(String),
    #[error("{0:?} is not something djmanzo can do: {1}")]
    BadAction(String, String),
    #[error("a binding on {0:?} says nothing to do")]
    Silent(String),
    #[error("could not read the mapping: {0}")]
    Unreadable(String),
    #[error("{0:?} is not a parameter djmanzo has")]
    UnknownParameter(String),
    #[error("the light on {0:?} is both a switch and a range; it can only be one")]
    AmbiguousFeedback(String),
    #[error("the platter on {0:?} has no `resolution`, so a wrap cannot be told from a turn")]
    NoResolution(String),
    #[error("{0:?} has a `resolution` but is not a platter")]
    ResolutionWithoutPlatter(String),
    #[error("the platter on {0:?} cannot be followed: {1}")]
    BadPlatter(String, String),
    #[error("the audio preset is not usable: {0}")]
    BadAudioPreset(String),
}

impl Mapping {
    /// Build a mapping from its parts, without going through a file.
    ///
    /// For the editor, which assembles one control at a time. The result is
    /// **not** prepared: its triggers are unparsed, so it is only good for
    /// writing back out. Anything that wants to run a mapping goes through
    /// [`Mapping::parse`], which is what makes a saved file and a built one
    /// the same thing.
    ///
    /// The audio preset is a parameter rather than a default because a part
    /// that can be forgotten is a part that will be: an editor that dropped it
    /// would silently unroute a controller the moment a DJ renamed a pad.
    #[must_use]
    pub fn from_parts(
        name: String,
        device: String,
        bindings: Vec<Binding>,
        audio: Option<crate::audio::AudioPreset>,
    ) -> Self {
        Self {
            name,
            device,
            bindings,
            audio,
            ..Self::default()
        }
    }

    /// Read a mapping from TOML, checking every action it contains.
    ///
    /// **Every action is parsed here, when the file loads, not when a pad is
    /// pressed.** A typo in a mapping should be a message when you choose the
    /// mapping, not a control that silently does nothing an hour into a set.
    pub fn parse(text: &str) -> Result<Self, MappingError> {
        let mut mapping: Mapping =
            toml::from_str(text).map_err(|e| MappingError::Unreadable(e.to_string()))?;
        mapping.prepare()?;
        Ok(mapping)
    }

    fn prepare(&mut self) -> Result<(), MappingError> {
        // The audio preset is checked here for the same reason the actions
        // are: a layout where the master and the cue overlap puts the next
        // track through the speakers, and there is no moment later at which
        // finding that out is any use.
        if let Some(preset) = &self.audio {
            preset
                .routing()
                .map_err(|e| MappingError::BadAudioPreset(e.to_string()))?;
        }

        self.triggers = Vec::with_capacity(self.bindings.len());
        for binding in &self.bindings {
            self.triggers.push(Trigger::parse(&binding.on)?);

            let says_something = binding.press.is_some()
                || binding.release.is_some()
                || binding.moved.is_some()
                || binding.turn_up.is_some()
                || binding.turn_down.is_some()
                || binding.platter.is_some();
            if !says_something {
                return Err(MappingError::Silent(binding.on.clone()));
            }

            // A platter needs its resolution, and the resolution has to be one
            // a wrap can be told from a movement in. Both checked here, when
            // the file loads, so a platter that could never work says so
            // before a DJ puts a hand on it.
            if binding.platter.is_some() {
                let steps = binding
                    .resolution
                    .ok_or_else(|| MappingError::NoResolution(binding.on.clone()))?;
                let unwrapper = crate::platter::AbsolutePlatter::new(steps)
                    .map_err(|e| MappingError::BadPlatter(binding.on.clone(), e.to_string()))?;
                self.platters
                    .insert(Trigger::parse(&binding.on)?, unwrapper);
            } else if binding.resolution.is_some() {
                return Err(MappingError::ResolutionWithoutPlatter(binding.on.clone()));
            }

            for action in [
                &binding.press,
                &binding.release,
                &binding.moved,
                &binding.turn_up,
                &binding.turn_down,
                &binding.platter,
            ]
            .into_iter()
            .flatten()
            {
                // `{value}` stands in for a number, so it is filled with one
                // before checking. A template that only parses when the fader
                // happens to be at 0.5 is not a template that works.
                let probe = action.replace("{value}", "0.5");
                dj_core::Action::parse(&probe)
                    .map_err(|e| MappingError::BadAction(action.clone(), e.to_string()))?;
            }
        }
        Ok(())
    }

    /// What this message means, as action text.
    ///
    /// Empty when nothing is bound to it, which is most messages on a busy
    /// controller.
    pub fn translate(&mut self, message: crate::Message) -> Vec<String> {
        let mut out = Vec::new();
        for (index, trigger) in self.triggers.clone().into_iter().enumerate() {
            if !trigger.matches(message) {
                continue;
            }
            let Some(binding) = self.bindings.get(index) else {
                continue;
            };

            match message {
                crate::Message::NoteOn { .. } => {
                    if let Some(action) = &binding.press {
                        out.push(action.clone());
                    }
                }
                crate::Message::NoteOff { .. } => {
                    if let Some(action) = &binding.release {
                        out.push(action.clone());
                    }
                }
                crate::Message::Control { value, .. } => {
                    if let Some(template) = binding.platter.clone() {
                        // A 7-bit control is a coarse platter, but a real one:
                        // some devices send the angle's high byte only.
                        if let Some(turns) = self
                            .platters
                            .get_mut(&trigger)
                            .map(|platter| platter.advance(u32::from(value)))
                        {
                            out.push(fill(&template, turns));
                        }
                        continue;
                    }
                    let previous = self.last.insert(trigger, value);
                    if binding.turn_up.is_some() || binding.turn_down.is_some() {
                        if let Some(action) = turned(binding, value, previous) {
                            out.push(action);
                        }
                    } else if let Some(template) = &binding.moved {
                        out.push(fill(template, scale(binding, f32::from(value) / 127.0)));
                    }
                }
                crate::Message::PitchBend { value, .. } => {
                    // Fourteen bits is what a motorised platter's angle
                    // actually needs: 3600 steps does not fit in seven.
                    if let Some(template) = binding.platter.clone() {
                        if let Some(turns) = self
                            .platters
                            .get_mut(&trigger)
                            .map(|platter| platter.advance(u32::from(value)))
                        {
                            out.push(fill(&template, turns));
                        }
                        continue;
                    }
                    if let Some(template) = &binding.moved {
                        out.push(fill(template, scale(binding, f32::from(value) / 16_383.0)));
                    }
                }
            }
        }
        out
    }

    /// The triggers this mapping listens to, for a learn mode to show.
    #[must_use]
    pub fn triggers(&self) -> &[Trigger] {
        &self.triggers
    }

    /// Whether this mapping is for `port`, matched loosely.
    ///
    /// A device announces itself as "MIDI Mix" on one platform and
    /// "MIDI Mix:MIDI Mix MIDI 1 24:0" on another, so an exact match would work
    /// on the machine it was written on and nowhere else.
    #[must_use]
    pub fn fits(&self, port: &str) -> bool {
        if self.device.is_empty() {
            return false;
        }
        port.to_lowercase().contains(&self.device.to_lowercase())
    }
}

/// Which way an encoder turned.
///
/// **The convention is declared, not guessed.** Three are in the wild and the
/// same byte means opposite things in two of them: 30 is thirty clicks
/// clockwise to a signed encoder and a position just below centre to an
/// absolute one. An earlier version read the byte and fell back to comparing
/// with the last value, which meant an absolute encoder turned down from 60 to
/// 30 produced *beat jump forward* — the wrong direction, silently, on real
/// hardware. Controllers document which they send, so a mapping can say.
fn turned(binding: &Binding, value: u8, previous: Option<u8>) -> Option<String> {
    let up = match binding.encoding {
        // Two's complement: 1..=63 is that many clicks clockwise, 127 counts
        // back down from zero. 0 and 64 are no movement.
        Encoding::Signed => match value {
            1..=63 => true,
            65..=127 => false,
            _ => return None,
        },
        // Binary offset: 64 is the centre, and how far from it is how fast.
        Encoding::Offset => match value.cmp(&64) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => return None,
        },
        // Absolute: only the change from last time says anything, so the very
        // first message says nothing at all.
        Encoding::Absolute => match previous {
            Some(previous) if value > previous => true,
            Some(previous) if value < previous => false,
            _ => return None,
        },
    };
    if up {
        binding.turn_up.clone()
    } else {
        binding.turn_down.clone()
    }
}

/// Put a 0..=1 position into the range the binding asked for.
fn scale(binding: &Binding, position: f32) -> f32 {
    let min = binding.min.unwrap_or(0.0);
    let max = binding.max.unwrap_or(1.0);
    min + position * (max - min)
}

/// Fill `{value}` in, rounded to three places.
///
/// Three because a 7-bit control has 128 steps and printing a full `f32` of
/// them gives `0.6929134` — a number with five digits of precision the hardware
/// never had.
/// Put a number into a `{value}` template.
///
/// Six decimal places, trailing zeros trimmed. It used to be three, which was
/// enough for a knob and wrong for the two controls that need better:
///
/// - one step of a 3600-step motorised platter is 0.000278 of a revolution,
///   which rounds to **zero** at three places -- the platter would be dead
///   until it was turned fast enough to cover two steps between reports;
/// - a 14-bit pitch fader across ±1 moves 0.000122 a step, so the fourteen
///   bits the bundled mapping deliberately asks for were being quantised back
///   to about a thousand.
///
/// Six places keeps everything the wire can carry -- a 14-bit control has
/// 16,384 steps, and six places resolves a million -- while still writing
/// `0.5` rather than `0.500000` for the common case.
fn fill(template: &str, value: f32) -> String {
    let text = format!("{value:.6}");
    let trimmed = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        &text
    };
    // An empty string would be a template filled with nothing, which does not
    // parse; `-0` and `0.000000` both trim to something, but guard anyway.
    let filled = if trimmed.is_empty() { "0" } else { trimmed };
    template.replace("{value}", filled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Message;

    fn mapping(toml: &str) -> Mapping {
        Mapping::parse(toml).unwrap_or_else(|e| panic!("{toml}\n\n{e}"))
    }

    #[test]
    fn a_button_presses_and_releases() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 censor_on"
            release = "deck 1 censor_off"
            "#,
        );

        assert_eq!(
            map.translate(Message::NoteOn {
                channel: 0,
                note: 36,
                velocity: 127
            }),
            vec!["deck 1 censor_on"]
        );
        assert_eq!(
            map.translate(Message::NoteOff {
                channel: 0,
                note: 36
            }),
            vec!["deck 1 censor_off"]
        );
    }

    /// A binding with no `release` latches. That is the difference between a
    /// censor pad and a cue button, and it is expressed by what the file omits
    /// rather than by a mode word.
    #[test]
    fn a_button_with_no_release_says_nothing_on_the_way_up() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 play_pause"
            "#,
        );
        assert_eq!(
            map.translate(Message::NoteOff {
                channel: 0,
                note: 36
            }),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_fader_fills_in_where_it_is() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "cc 1 7"
            move = "deck 1 volume {value}"
            "#,
        );
        assert_eq!(
            map.translate(Message::Control {
                channel: 0,
                controller: 7,
                value: 127
            }),
            vec!["deck 1 volume 1"]
        );
        assert_eq!(
            map.translate(Message::Control {
                channel: 0,
                controller: 7,
                value: 0
            }),
            vec!["deck 1 volume 0"]
        );
    }

    /// Not every control runs 0 to 1. An EQ runs to 4 and a pitch fader from
    /// -1, and a mapping that could not say so would need a different action
    /// for every range.
    #[test]
    fn a_binding_can_say_what_range_it_covers() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "cc 1 20"
            move = "deck 1 eq_low {value}"
            min = 0.0
            max = 4.0

            [[binding]]
            on = "cc 1 21"
            move = "deck 1 pitch {value}"
            min = -1.0
            max = 1.0
            "#,
        );
        assert_eq!(
            map.translate(Message::Control {
                channel: 0,
                controller: 20,
                value: 127
            }),
            vec!["deck 1 eq_low 4"]
        );
        // Centre of a 7-bit control is 63 or 64, and neither is exactly the
        // middle: 64 of 127 across -1..=1 is 0.007874 above centre. This used
        // to be written as `0.008`, because `fill` rounded to three places to
        // keep the number tidy -- which also rounded a platter step to nothing
        // and collapsed adjacent 14-bit fader steps. The number is now what
        // the control actually said.
        assert_eq!(
            map.translate(Message::Control {
                channel: 0,
                controller: 21,
                value: 64
            }),
            vec!["deck 1 pitch 0.007874"]
        );
    }

    fn encoder(encoding: &str) -> Mapping {
        mapping(&format!(
            r#"
            name = "Test"
            [[binding]]
            on = "cc 1 30"
            encoding = "{encoding}"
            turn_up = "deck 1 beatjump 1"
            turn_down = "deck 1 beatjump -1"
            "#
        ))
    }

    fn turn(map: &mut Mapping, value: u8) -> Vec<String> {
        map.translate(Message::Control {
            channel: 0,
            controller: 30,
            value,
        })
    }

    /// Signed is the common case and so the default: 1 is one click clockwise,
    /// 127 is one anticlockwise, 0 and 64 are the encoder sitting still.
    #[test]
    fn a_signed_encoder_reads_the_byte_as_a_delta() {
        let mut map = encoder("signed");
        assert_eq!(turn(&mut map, 1), vec!["deck 1 beatjump 1"]);
        assert_eq!(turn(&mut map, 127), vec!["deck 1 beatjump -1"]);
        assert_eq!(
            turn(&mut map, 63),
            vec!["deck 1 beatjump 1"],
            "fast clockwise"
        );
        assert_eq!(turn(&mut map, 65), vec!["deck 1 beatjump -1"], "fast anti");
        assert_eq!(turn(&mut map, 0), Vec::<String>::new(), "still");
        assert_eq!(turn(&mut map, 64), Vec::<String>::new(), "still");
    }

    /// Signed is what a mapping gets when it does not say, because most
    /// hardware sends it and a file should not have to state the obvious.
    #[test]
    fn an_encoder_that_says_nothing_is_signed() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "cc 1 30"
            turn_up = "deck 1 beatjump 1"
            turn_down = "deck 1 beatjump -1"
            "#,
        );
        assert_eq!(turn(&mut map, 1), vec!["deck 1 beatjump 1"]);
        assert_eq!(turn(&mut map, 127), vec!["deck 1 beatjump -1"]);
    }

    /// Binary offset centres on 64 and the distance from it is the speed.
    #[test]
    fn an_offset_encoder_reads_the_byte_around_its_centre() {
        let mut map = encoder("offset");
        assert_eq!(turn(&mut map, 65), vec!["deck 1 beatjump 1"]);
        assert_eq!(turn(&mut map, 63), vec!["deck 1 beatjump -1"]);
        assert_eq!(turn(&mut map, 127), vec!["deck 1 beatjump 1"], "hard spin");
        assert_eq!(turn(&mut map, 0), vec!["deck 1 beatjump -1"], "hard spin");
        assert_eq!(turn(&mut map, 64), Vec::<String>::new(), "centred");
    }

    /// Absolute sends a position, so the first message says nothing — there is
    /// nothing to compare it with yet.
    #[test]
    fn an_absolute_encoder_needs_a_previous_value_first() {
        let mut map = encoder("absolute");
        assert_eq!(turn(&mut map, 64), Vec::<String>::new(), "no history yet");
        assert_eq!(turn(&mut map, 70), vec!["deck 1 beatjump 1"]);
        assert_eq!(turn(&mut map, 64), vec!["deck 1 beatjump -1"]);
        assert_eq!(turn(&mut map, 64), Vec::<String>::new(), "did not move");
    }

    /// The reason the convention is declared rather than guessed. 30 is a
    /// position below centre on an absolute encoder and thirty clicks
    /// clockwise on a signed one. Reading the byte alone would send a DJ
    /// turning *down* from 60 to 30 a beat jump *forward*.
    #[test]
    fn the_same_byte_means_opposite_things_in_two_conventions() {
        let mut absolute = encoder("absolute");
        assert_eq!(turn(&mut absolute, 60), Vec::<String>::new());
        assert_eq!(turn(&mut absolute, 30), vec!["deck 1 beatjump -1"]);

        let mut signed = encoder("signed");
        assert_eq!(turn(&mut signed, 60), vec!["deck 1 beatjump 1"]);
        assert_eq!(turn(&mut signed, 30), vec!["deck 1 beatjump 1"]);
    }

    #[test]
    fn a_high_resolution_fader_uses_all_fourteen_bits() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "bend 1"
            move = "deck 1 volume {value}"
            "#,
        );
        assert_eq!(
            map.translate(Message::PitchBend {
                channel: 0,
                value: 16_383
            }),
            vec!["deck 1 volume 1"]
        );
        // A 14-bit control resolves steps a 7-bit one cannot, and this now
        // tests that rather than asserting the opposite: 8191 and 8192 are
        // adjacent positions and must not write the same number. They both
        // used to be `0.5`, because `fill` rounded to three places -- so this
        // test's name claimed fourteen bits while its assertion proved eleven.
        let below = map.translate(Message::PitchBend {
            channel: 0,
            value: 8_191,
        });
        let above = map.translate(Message::PitchBend {
            channel: 0,
            value: 8_192,
        });
        assert_ne!(
            below, above,
            "two adjacent 14-bit positions wrote the same number"
        );

        // And the value is the truth about where the fader is: 8192 of 16383
        // is a half-step above centre, not exactly centre.
        assert_eq!(above, vec!["deck 1 volume 0.500031"]);
    }

    /// **The property that makes a mapping from a stranger safe to load.**
    /// Every action is checked when the file is read, so a typo is a message
    /// now rather than a dead control an hour into a set — and nothing a
    /// mapping can say escapes the vocabulary.
    #[test]
    fn a_mapping_that_says_something_impossible_is_refused() {
        let bad = Mapping::parse(
            r#"
            name = "Test"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 self_destruct"
            "#,
        );
        assert!(matches!(bad, Err(MappingError::BadAction(_, _))), "{bad:?}");
    }

    /// And a template is checked *with a value in it*, or one that only parses
    /// when the fader happens to be at a particular place would pass.
    #[test]
    fn a_template_is_checked_with_a_number_in_it() {
        let bad = Mapping::parse(
            r#"
            name = "Test"
            [[binding]]
            on = "cc 1 7"
            move = "deck 1 volume {value} extra"
            "#,
        );
        assert!(bad.is_err(), "a malformed template was accepted");
    }

    #[test]
    fn a_binding_that_does_nothing_is_refused() {
        let bad = Mapping::parse(
            r#"
            name = "Test"
            [[binding]]
            on = "note 1 36"
            "#,
        );
        assert!(matches!(bad, Err(MappingError::Silent(_))), "{bad:?}");
    }

    #[test]
    fn an_unreadable_control_is_named_in_the_error() {
        let bad = Mapping::parse(
            r#"
            name = "Test"
            [[binding]]
            on = "wheel 1 3"
            press = "deck 1 play"
            "#,
        );
        match bad {
            Err(MappingError::BadTrigger(what)) => assert!(what.contains("wheel")),
            other => panic!("{other:?}"),
        }
    }

    /// Controller documentation is written in hex, and transcribing 0x0B as 11
    /// by hand is a mistake waiting to happen.
    #[test]
    fn a_control_can_be_written_in_hex() {
        assert_eq!(
            Trigger::parse("note 1 0x24").unwrap(),
            Trigger::Note {
                channel: 1,
                note: 36
            }
        );
    }

    /// Channel 0 means any. Some controllers are configurable, and a mapping
    /// should not stop working because somebody moved theirs to channel 3.
    #[test]
    fn channel_zero_listens_to_every_channel() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "note 0 36"
            press = "deck 1 play_pause"
            "#,
        );
        for channel in 0..16 {
            assert_eq!(
                map.translate(Message::NoteOn {
                    channel,
                    note: 36,
                    velocity: 100
                })
                .len(),
                1,
                "channel {channel} was ignored"
            );
        }
    }

    /// And a mapping that names a channel listens only to that one, or two
    /// decks sharing a note number would fire together.
    #[test]
    fn a_named_channel_ignores_the_others() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "note 2 36"
            press = "deck 2 play_pause"
            "#,
        );
        assert!(
            map.translate(Message::NoteOn {
                channel: 0,
                note: 36,
                velocity: 100
            })
            .is_empty()
        );
        assert_eq!(
            map.translate(Message::NoteOn {
                channel: 1,
                note: 36,
                velocity: 100
            }),
            vec!["deck 2 play_pause"]
        );
    }

    #[test]
    fn nothing_is_said_about_a_control_nobody_mapped() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 play_pause"
            "#,
        );
        assert!(
            map.translate(Message::Control {
                channel: 0,
                controller: 99,
                value: 64
            })
            .is_empty()
        );
    }

    /// A device names itself differently on every platform, so an exact match
    /// would work on the machine the mapping was written on and nowhere else.
    #[test]
    fn a_device_is_recognised_by_part_of_its_name() {
        let map = mapping(
            r#"
            name = "Test"
            device = "MIDI Mix"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 play_pause"
            "#,
        );
        assert!(map.fits("MIDI Mix"));
        assert!(map.fits("MIDI Mix:MIDI Mix MIDI 1 24:0"));
        assert!(map.fits("midi mix"), "case should not matter");
        assert!(!map.fits("Launchpad"));
    }

    /// A mapping with no device name matches nothing rather than everything.
    /// The other way round, one generic file would claim every controller
    /// plugged in.
    #[test]
    fn a_mapping_with_no_device_claims_nothing() {
        let map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 play_pause"
            "#,
        );
        assert!(!map.fits("MIDI Mix"));
        assert!(!map.fits(""));
    }

    /// One control can drive several actions, which is how a single pad becomes
    /// "load and play".
    #[test]
    fn two_bindings_on_one_control_both_fire() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 cue"

            [[binding]]
            on = "note 1 36"
            press = "deck 1 sync"
            "#,
        );
        assert_eq!(
            map.translate(Message::NoteOn {
                channel: 0,
                note: 36,
                velocity: 100
            }),
            vec!["deck 1 cue", "deck 1 sync"]
        );
    }
}

#[cfg(test)]
mod platter_tests {
    use super::*;

    fn mapping(extra: &str) -> Result<Mapping, MappingError> {
        Mapping::parse(&format!(
            "name = \"Motorised\"\ndevice = \"Twelve\"\n\n[[binding]]\n{extra}\n"
        ))
    }

    /// **The whole point of the platter path.** A motorised platter's angle
    /// wraps, and a mapping has to turn that into movement rather than into a
    /// revolution of audio every time the record passes zero.
    #[test]
    fn a_platter_reports_movement_rather_than_position() {
        let mut map =
            mapping("on = \"bend 1\"\nplatter = \"deck 1 jog {value}\"\nresolution = 3600")
                .expect("a platter binding");

        // The first report is a starting point.
        assert!(
            map.translate(crate::Message::PitchBend {
                channel: 0,
                value: 3_598
            })
            .first()
            .is_some_and(|a| a.ends_with('0')),
            "the first report should be no movement"
        );

        // Crossing zero: 3598 -> 2 is four steps forwards, not a revolution
        // backwards.
        let out = map.translate(crate::Message::PitchBend {
            channel: 0,
            value: 2,
        });
        let action = out.first().expect("a platter always reports something");
        let turns: f32 = action
            .rsplit(' ')
            .next()
            .expect("the action ends in a number")
            .parse()
            .expect("a number");
        assert!(
            (turns - 4.0 / 3600.0).abs() < 1e-5,
            "crossing zero produced {turns} of a revolution"
        );
    }

    /// A platter is a different kind of control from a fader, and the file has
    /// to say which. Without the resolution there is no way to tell a wrap
    /// from a turn, so the file is refused when it loads rather than producing
    /// nonsense all night.
    #[test]
    fn a_platter_without_its_resolution_is_refused() {
        let error = mapping("on = \"bend 1\"\nplatter = \"deck 1 jog {value}\"")
            .expect_err("a platter needs its resolution");
        assert!(
            matches!(error, MappingError::NoResolution(_)),
            "got {error}"
        );
    }

    /// And a resolution on something that is not a platter is a mistake worth
    /// naming: it means the DJ thought they had written a platter binding.
    #[test]
    fn a_resolution_on_something_that_is_not_a_platter_is_refused() {
        let error =
            mapping("on = \"cc 1 0x10\"\nmove = \"deck 1 volume {value}\"\nresolution = 3600")
                .expect_err("a fader has no resolution");
        assert!(
            matches!(error, MappingError::ResolutionWithoutPlatter(_)),
            "got {error}"
        );
    }

    #[test]
    fn a_platter_too_coarse_to_follow_is_refused_when_the_file_loads() {
        let error = mapping("on = \"bend 1\"\nplatter = \"deck 1 jog {value}\"\nresolution = 2")
            .expect_err("two steps is not a platter");
        assert!(matches!(error, MappingError::BadPlatter(..)), "got {error}");
    }

    /// A platter binding is not silent, so it must not be refused as one.
    #[test]
    fn a_platter_counts_as_saying_something() {
        assert!(
            mapping("on = \"bend 1\"\nplatter = \"deck 1 jog {value}\"\nresolution = 3600").is_ok()
        );
    }

    /// The action is checked like every other, when the file loads.
    #[test]
    fn a_platters_action_is_checked_too() {
        let error = mapping("on = \"bend 1\"\nplatter = \"deck 1 jgo {value}\"\nresolution = 3600")
            .expect_err("that is not an action");
        assert!(matches!(error, MappingError::BadAction(..)), "got {error}");
    }

    /// Two platters on one mapping keep their own angles: a two-deck
    /// controller has two, and sharing an unwrapper between them would make
    /// each one's movement depend on the other's.
    #[test]
    fn two_platters_do_not_share_an_angle() {
        let mut map = Mapping::parse(
            r#"
            name = "Two platters"
            device = "Twelve"

            [[binding]]
            on = "bend 1"
            platter = "deck 1 jog {value}"
            resolution = 3600

            [[binding]]
            on = "bend 2"
            platter = "deck 2 jog {value}"
            resolution = 3600
            "#,
        )
        .expect("two platters");

        // Deck 1 starts at 0, deck 2 starts at 1800.
        map.translate(crate::Message::PitchBend {
            channel: 0,
            value: 0,
        });
        map.translate(crate::Message::PitchBend {
            channel: 1,
            value: 1_800,
        });

        // Deck 1 moves ten steps. If they shared an angle this would be read
        // against deck 2's 1800 and refused as a jump.
        let out = map.translate(crate::Message::PitchBend {
            channel: 0,
            value: 10,
        });
        let turns: f32 = out
            .first()
            .expect("deck 1 reports")
            .rsplit(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            turns > 0.0,
            "deck 1's movement was read against deck 2's angle"
        );
    }
}

#[cfg(test)]
mod fill_precision_tests {
    use super::*;

    /// **The bug this pins.** One step of a 3600-step platter is 0.000278 of a
    /// revolution. At three decimal places that is zero, so a platter turned
    /// slowly -- which is most of the time, since a record turns at 33 1/3 RPM
    /// -- would report no movement at all.
    #[test]
    fn one_step_of_a_platter_survives_being_written_down() {
        let step = 1.0 / 3600.0;
        let text = fill("deck 1 jog {value}", step);
        let written: f32 = text.rsplit(' ').next().unwrap().parse().unwrap();
        assert!(
            written > 0.0,
            "one platter step was written as {text:?} and rounds to nothing"
        );
        assert!(
            (written - step).abs() < step * 0.01,
            "one platter step became {written} instead of {step}"
        );
    }

    /// The bundled mapping takes the pitch fader on pitch bend specifically to
    /// get fourteen bits, because 128 steps across ±8% is audibly coarse when
    /// beatmatching. Rounding it back to three places threw that away.
    #[test]
    fn a_fourteen_bit_fader_keeps_its_fourteen_bits() {
        // Across -1..=1, one step of 16,384 is 0.000122.
        let step = 2.0 / 16_384.0;
        let a: f32 = fill("x {value}", step)
            .rsplit(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let b: f32 = fill("x {value}", step * 2.0)
            .rsplit(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            b > a,
            "two adjacent 14-bit positions wrote the same number: {a} and {b}"
        );
    }

    /// And the common case still reads like a number a person would write.
    #[test]
    fn a_round_number_is_still_written_roundly() {
        assert_eq!(fill("v {value}", 0.5), "v 0.5");
        assert_eq!(fill("v {value}", 1.0), "v 1");
        assert_eq!(fill("v {value}", 0.0), "v 0");
        assert_eq!(fill("v {value}", -1.0), "v -1");
    }

    /// Whatever is written has to parse back, or the mapping produces actions
    /// the engine refuses.
    #[test]
    fn everything_written_parses_as_an_action() {
        for value in [
            0.0,
            1.0,
            -1.0,
            0.5,
            1.0 / 3600.0,
            -1.0 / 3600.0,
            2.0 / 16_384.0,
            0.123_456_7,
            f32::MIN_POSITIVE,
        ] {
            let text = fill("deck 1 jog {value}", value);
            dj_core::Action::parse(&text)
                .unwrap_or_else(|e| panic!("{text:?} does not parse: {e}"));
        }
    }
}

#[cfg(test)]
mod audio_preset_tests {
    use super::*;

    fn mapping(audio: &str) -> Result<Mapping, MappingError> {
        Mapping::parse(&format!(
            "name = \"Preset\"\ndevice = \"DDJ\"\n\n\
             [[binding]]\non = \"note 1 0x0b\"\npress = \"deck 1 play_pause\"\n\n{audio}"
        ))
    }

    #[test]
    fn a_mapping_can_say_where_its_sockets_go() {
        let map = mapping("[audio]\ndevice = \"DDJ-400\"\nmaster = [0, 1]\ncue = [2, 3]")
            .expect("a normal controller");
        let preset = map.audio.expect("the preset was read");
        let routing = preset.routing().expect("and it is usable");

        assert_eq!(routing.master, (0, 1));
        assert_eq!(routing.cue, Some((2, 3)));
        assert!(preset.fits("PIONEER DDJ-400 Analog Stereo"));
    }

    /// **The check that matters.** A mapping from a stranger that routed the
    /// cue into the master would play the next track through the speakers, and
    /// the first anyone would know is the crowd hearing it. Refused when the
    /// file loads.
    #[test]
    fn a_mapping_whose_cue_overlaps_the_master_is_refused() {
        let error = mapping("[audio]\ndevice = \"X\"\nmaster = [0, 1]\ncue = [1, 2]")
            .expect_err("that would put the cue in the room");
        assert!(
            matches!(error, MappingError::BadAudioPreset(_)),
            "got {error}"
        );
        assert!(
            error.to_string().contains("room would hear the cue"),
            "the message should say what is wrong: {error}"
        );
    }

    /// Most mappings are for controllers with no soundcard of their own, and
    /// saying nothing about audio has to stay the normal case.
    #[test]
    fn a_mapping_with_no_audio_section_is_fine() {
        let map = mapping("").expect("a mapping need not mention audio");
        assert!(map.audio.is_none());
    }

    /// The preset survives being written back out by the editor, or a DJ who
    /// edits a controller mapping loses its routing.
    #[test]
    fn an_audio_preset_survives_the_editor() {
        let original = mapping("[audio]\ndevice = \"DDJ-400\"\nmaster = [0, 1]\ncue = [2, 3]")
            .expect("a normal controller");

        let mut draft = crate::editor::Draft::from_mapping(&original);
        draft
            .bind(
                "note 1 0x0c",
                &crate::editor::Role::Latching {
                    press: "deck 1 cue".to_owned(),
                },
            )
            .expect("adding a control");

        let saved = draft.to_toml().expect("writable");
        let reloaded = Mapping::parse(&saved).expect("readable");
        assert_eq!(
            reloaded.audio, original.audio,
            "editing the mapping lost its audio routing"
        );
    }
}
