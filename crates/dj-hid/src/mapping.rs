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
    /// A fader or knob sent as a **pair** of control changes, high byte then
    /// low. `cc14 <channel> <controller>`.
    ///
    /// The low byte is not written because MIDI fixes it: controllers 0..=31
    /// carry the high byte of a value and 32..=63 carry the matching low one,
    /// so the partner of `n` is always `n + 32`. Every Pioneer, Denon and
    /// Native Instruments table in circulation follows that, and letting a
    /// mapping name a different partner would only let it name a wrong one.
    ///
    /// Worth having a trigger of its own rather than binding the high byte and
    /// living with seven bits: 128 steps across a pitch fader's range is
    /// 0.125% a step, which is audibly coarse when beatmatching, and it is the
    /// same 128 steps across an EQ kill.
    Control14 { channel: u8, msb: u8 },
    /// A high-resolution control that uses the pitch wheel. `bend <channel>`.
    Bend { channel: u8 },
    /// A field in a raw HID report. `hid <report> byte|word|word-le|bit <n>`.
    ///
    /// Unlike the three above, this one carries the **layout**: a HID packet is
    /// opaque bytes and nothing in it says what a control is. See
    /// [`crate::report`] for why that is, and why the change from the previous
    /// report is what matters rather than the value itself.
    Hid(crate::report::Field),
}

/// How far above its high byte a control change's low byte sits.
///
/// Fixed by MIDI, not by any one manufacturer: controllers 0..=31 are high
/// bytes and 32..=63 are their partners.
const LSB_OFFSET: u8 = 32;

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
            "cc14" => {
                let channel = number(words.next())?;
                let msb = number(words.next())?;
                // Refused here rather than at the first message, because a
                // high byte at 32 or above has no partner to pair with: MIDI
                // reserves only 0..=31 for them. A mapping that named one
                // would sit there looking bound and never move anything.
                if msb >= 32 {
                    return Err(MappingError::BadTrigger(text.to_owned()));
                }
                Ok(Trigger::Control14 { channel, msb })
            }
            "bend" => Ok(Trigger::Bend {
                channel: number(words.next())?,
            }),
            "hid" => Self::parse_hid(text, &mut words),
            _ => Err(MappingError::BadTrigger(text.to_owned())),
        }
    }

    /// The tail of a `hid ...` trigger: report number, width, offset.
    ///
    /// `hid 1 bit 3.2` is report 1, byte 3, bit 2. The dotted form is used
    /// because a bit belongs to a byte and writing them apart invites getting
    /// them the wrong way round.
    fn parse_hid<'a>(
        text: &str,
        words: &mut impl Iterator<Item = &'a str>,
    ) -> Result<Self, MappingError> {
        use crate::report::{Field, Width};
        let bad = || MappingError::BadTrigger(text.to_owned());

        let report: u8 = words.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
        let kind = words.next().ok_or_else(bad)?;
        let where_ = words.next().ok_or_else(bad)?;

        let (offset, width) = match kind {
            "bit" => {
                // `3.2` -- byte then bit, in that order, because that is the
                // order they are written in every device manual.
                let (byte, bit) = where_.split_once('.').ok_or_else(bad)?;
                let bit: u8 = bit.parse().map_err(|_| bad())?;
                if bit > 7 {
                    return Err(bad());
                }
                (byte.parse::<usize>().map_err(|_| bad())?, Width::Bit(bit))
            }
            "byte" => (where_.parse::<usize>().map_err(|_| bad())?, Width::Byte),
            "word" => (where_.parse::<usize>().map_err(|_| bad())?, Width::Word),
            "word-le" => (where_.parse::<usize>().map_err(|_| bad())?, Width::WordLe),
            _ => return Err(bad()),
        };
        Ok(Trigger::Hid(Field::new(report, offset, width)))
    }

    /// The HID field this trigger reads, if it is a HID trigger at all.
    #[must_use]
    pub fn field(self) -> Option<crate::report::Field> {
        match self {
            Trigger::Hid(field) => Some(field),
            _ => None,
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
            (
                Trigger::Control14 { channel, msb },
                crate::Message::Control {
                    controller: got, ..
                },
            ) => same_channel(channel) && (got == msb || got == msb + LSB_OFFSET),
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
    /// Hand this control to the mapping's script instead of acting on it here.
    ///
    /// Per binding rather than per file, so a mapping can script the eight
    /// pads that need a shift key and leave the crossfader as a table entry.
    #[serde(default)]
    pub script: bool,
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
    /// Which convention this control reports movement in, when it reports
    /// movement rather than a position.
    ///
    /// An `Option` and not a defaulted value, because the *presence* of this
    /// line is itself the fact: a fader has a position and no encoding, while
    /// a jog wheel or an endless knob has a convention and no position. Given a
    /// default, a jog wheel bound with `move` would be indistinguishable from a
    /// fader and would be read as one -- which puts its centre a hair off zero
    /// and creeps the deck. See [`centred`].
    #[serde(default)]
    pub encoding: Option<Encoding>,
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
    /// Lua, for the controls a table cannot describe.
    ///
    /// A shift button that changes what eight pads do is a *decision*, and a
    /// decision needs an `if`. Bindings still handle everything else: the
    /// script sees only the controls that name it with `script = true`, so a
    /// mapping is not all-or-nothing.
    ///
    /// Nothing a script returns skips the vocabulary -- see
    /// [`crate::script`], which is also where the sandbox is.
    #[serde(default)]
    pub script: Option<String>,

    /// Parsed triggers, built once when the file loads.
    #[serde(skip)]
    triggers: Vec<Trigger>,
    /// The last value seen for each control, so an encoder knows which way it
    /// turned and a fader can be asked what it is set to.
    #[serde(skip)]
    last: HashMap<Trigger, u8>,
    /// The last value seen for each HID field.
    ///
    /// Separate from `last` and wider because a HID field carries up to
    /// sixteen bits, and because it is doing a different job: MIDI's `last`
    /// tells an encoder which way it turned, while this one is what turns a
    /// stream of identical level reports into the handful of changes that
    /// actually happened. See [`crate::report`].
    #[serde(skip)]
    last_hid: HashMap<Trigger, u32>,
    /// One unwrapper per motorised platter, keyed by the control it is on.
    ///
    /// Per mapping rather than per message because a platter's angle only
    /// means anything against the previous one.
    #[serde(skip)]
    platters: HashMap<Trigger, crate::platter::AbsolutePlatter>,
    /// The two halves of each 14-bit control, most recent of each.
    ///
    /// Kept because the halves arrive as separate messages and neither is the
    /// value on its own. Acting on the high byte alone would quantise every
    /// Pioneer fader back to the seven bits `cc14` exists to escape; waiting
    /// for the low byte alone would leave a controller that sends only high
    /// bytes -- some do, at rest -- entirely dead.
    ///
    /// The low byte is an `Option` and not a zero, because "not sent yet" and
    /// "sent as zero" are different faders. Treated as zero, a controller that
    /// sends only high bytes would top out at 16256/16383 and never quite
    /// reach the end of its own travel -- a fader that cannot be pushed all
    /// the way up. Absent, the control is read as the seven bits it actually
    /// is, and it reaches both ends; the moment a low byte does arrive it
    /// becomes fourteen.
    #[serde(skip)]
    highres: HashMap<Trigger, (u8, Option<u8>)>,
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
    #[error("{0:?} is marked `script` but the mapping has no script")]
    ScriptWithoutOne(String),
    #[error("the platter on {0:?} cannot be followed: {1}")]
    BadPlatter(String, String),
    #[error("the audio preset is not usable: {0}")]
    BadAudioPreset(String),
    #[error(
        "the encoder on {0:?} is a HID field; HID reports a level, not a turn, \
         so `turn_up`/`turn_down` cannot be read from one"
    )]
    EncoderOnHid(String),
    #[error("the platter on {0:?} needs {1} steps and the field it reads holds only {2}")]
    PlatterTooFineForField(String, u32, u32),
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
        script: Option<String>,
    ) -> Self {
        Self {
            name,
            device,
            bindings,
            audio,
            script,
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
            // A scripted control says what it does in Lua, so it is allowed to
            // say nothing here -- but only if there is a script to say it in.
            // A binding marked `script` in a file with no `script` block is a
            // dead control, which is what this check exists to prevent.
            if binding.script && self.script.is_none() {
                return Err(MappingError::ScriptWithoutOne(binding.on.clone()));
            }
            if !says_something && !binding.script {
                return Err(MappingError::Silent(binding.on.clone()));
            }

            // A platter needs its resolution, and the resolution has to be one
            // a wrap can be told from a movement in. Both checked here, when
            // the file loads, so a platter that could never work says so
            // before a DJ puts a hand on it.
            // A HID field is a level, not an event, and two of the binding
            // shapes have no meaning against one. Both are refused here rather
            // than ignored at run time, so a mapping that could never work
            // says so when it is chosen.
            let field = Trigger::parse(&binding.on)?.field();
            if field.is_some() && (binding.turn_up.is_some() || binding.turn_down.is_some()) {
                return Err(MappingError::EncoderOnHid(binding.on.clone()));
            }

            if binding.platter.is_some() {
                let steps = binding
                    .resolution
                    .ok_or_else(|| MappingError::NoResolution(binding.on.clone()))?;
                // A platter of 3,600 steps read out of one byte would wrap
                // fourteen times a revolution and be unusable. The field has
                // to be wide enough for the resolution the manual gives.
                if let Some(field) = field {
                    let holds = field.width.max() + 1;
                    if steps > holds {
                        return Err(MappingError::PlatterTooFineForField(
                            binding.on.clone(),
                            steps,
                            holds,
                        ));
                    }
                }
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
            // Scripted controls belong to the script. Handling them here as
            // well would fire both, which is the one thing a shift key must
            // not do.
            if binding.script {
                continue;
            }

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
                crate::Message::Control {
                    value, controller, ..
                } => {
                    if let Trigger::Control14 { msb, .. } = trigger {
                        let halves = self.highres.entry(trigger).or_insert((0, None));
                        if controller == msb {
                            halves.0 = value;
                        } else {
                            halves.1 = Some(value);
                        }
                        let fraction = match halves.1 {
                            Some(low) => {
                                let combined = (u16::from(halves.0) << 7) | u16::from(low & 0x7F);
                                f32::from(combined) / 16_383.0
                            }
                            None => f32::from(halves.0) / 127.0,
                        };
                        if let Some(template) = &binding.moved {
                            out.push(fill(template, scale(binding, fraction)));
                        }
                        continue;
                    }
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
                        // A control that reports *movement* is centred, not a
                        // position; see `centred`. Which of the two it is, is
                        // the same fact `encoding` already states for a
                        // stepping encoder, so it is not asked twice.
                        let number = if let Some(encoding) = binding.encoding {
                            centred(binding, value, encoding)
                        } else {
                            scale(binding, f32::from(value) / 127.0)
                        };
                        out.push(fill(template, number));
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

    /// What a raw HID input report means, as action text.
    ///
    /// # Level in, edges out
    ///
    /// A HID device sends the state of **every** control it has, in one
    /// packet, as often as a thousand times a second. Nothing in the packet
    /// says what changed. So each field is compared with the last value seen
    /// and only a change becomes an action -- otherwise holding the play
    /// button would send "play" a thousand times a second, and a fader
    /// standing still would flood the bus with the value it already had.
    ///
    /// # The first report
    ///
    /// A switch with nothing remembered is treated as **off**, so the first
    /// packet does not fire a release for every button that is merely not
    /// being pressed. A button that genuinely *is* held when the device is
    /// plugged in reads as a press, and its release arrives when the DJ lets
    /// go -- a matched pair, where suppressing the press would leave an
    /// unmatched release.
    ///
    /// A range has no such default: its first report is a change, because a
    /// fader's position is a fact worth knowing the moment the device
    /// appears.
    pub fn translate_report(&mut self, report: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        for (index, trigger) in self.triggers.clone().into_iter().enumerate() {
            let Some(field) = trigger.field() else {
                continue;
            };
            // Not this field's report, or a packet too short to hold it.
            // Ordinary traffic on a device that sends more than one kind.
            let Some(value) = field.read(report) else {
                continue;
            };
            let Some(binding) = self.bindings.get(index) else {
                continue;
            };
            let previous = self.last_hid.insert(trigger, value);

            if field.width.is_switch() {
                if previous.unwrap_or(0) == value {
                    continue;
                }
                let action = if value == 0 {
                    &binding.release
                } else {
                    &binding.press
                };
                if let Some(action) = action {
                    out.push(action.clone());
                }
                continue;
            }

            if previous == Some(value) {
                continue;
            }

            if let Some(template) = binding.platter.clone() {
                if let Some(turns) = self
                    .platters
                    .get_mut(&trigger)
                    .map(|platter| platter.advance(value))
                {
                    out.push(fill(&template, turns));
                }
                continue;
            }
            if let Some(template) = &binding.moved {
                // The field's own full scale, so sixteen bits are worth
                // sixteen bits rather than being squeezed through 127.
                let span = field.width.max();
                #[allow(clippy::cast_precision_loss)]
                let position = value as f32 / span as f32;
                out.push(fill(template, scale(binding, position)));
            }
        }
        out
    }

    /// Whether `message` belongs to the script rather than to the table.
    ///
    /// The script sees only the controls that asked for it, so a mapping can
    /// script eight pads and leave the crossfader as a table entry.
    #[must_use]
    pub fn is_scripted(&self, message: crate::Message) -> bool {
        self.triggers
            .iter()
            .zip(&self.bindings)
            .any(|(trigger, binding)| binding.script && trigger.matches(message))
    }

    /// The `on = "..."` text and scaled value a script should be handed.
    ///
    /// `None` when nothing scripted matches. The value is scaled by the
    /// binding's own `min`/`max` exactly as a `move` action's would be, so a
    /// script sees the number the table would have produced.
    #[must_use]
    pub fn script_event(
        &self,
        message: crate::Message,
    ) -> Option<(String, crate::script::Event, f32)> {
        let index = self
            .triggers
            .iter()
            .zip(&self.bindings)
            .position(|(trigger, binding)| binding.script && trigger.matches(message))?;
        let binding = self.bindings.get(index)?;
        let (event, value) = match message {
            crate::Message::NoteOn { velocity: 0, .. } | crate::Message::NoteOff { .. } => {
                (crate::script::Event::Release, 0.0)
            }
            crate::Message::NoteOn { .. } => (crate::script::Event::Press, 1.0),
            crate::Message::Control { value, .. } => (
                crate::script::Event::Move,
                scale(binding, f32::from(value) / 127.0),
            ),
            crate::Message::PitchBend { value, .. } => (
                crate::script::Event::Move,
                scale(binding, f32::from(value) / 16_383.0),
            ),
        };
        Some((binding.on.clone(), event, value))
    }

    /// Every HID field this mapping reads.
    ///
    /// For the device layer, which needs to know whether a mapping is a HID
    /// mapping at all before it goes looking for a HID device.
    #[must_use]
    pub fn hid_fields(&self) -> Vec<crate::report::Field> {
        self.triggers.iter().filter_map(|t| t.field()).collect()
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
    // A stepping encoder that did not say which convention it uses gets the
    // common one, which is what it had before `encoding` became optional.
    let up = match binding.encoding.unwrap_or_default() {
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

/// The centre of a control that reports movement rather than position.
///
/// Named because it is the number the whole convention turns on: 64 is
/// standing still, and a jog wheel that thinks standing still is anything else
/// creeps.
const CENTRED: f32 = 64.0;

/// Full deflection either side of [`CENTRED`].
const DEFLECTION: f32 = 63.0;

/// A jog wheel's reading as a signed fraction of full deflection.
///
/// A platter does not send a position; it sends how far, or how fast, it just
/// moved, centred on a value that means "not moving". Feeding that through
/// [`scale`] as if it were a fader position gets the ends right and the middle
/// **wrong**: 64 out of 127 is 0.504 of the way up, which lands a hair off zero
/// and drives the deck forwards while the DJ's hand is nowhere near the
/// platter. Over a set that is a track sliding out of time on its own.
///
/// So the centre is anchored exactly and each half is scaled to its own end.
/// That also lets a mapping give the two directions different weights, which
/// asymmetric `min`/`max` on a fader could never mean.
fn centred(binding: &Binding, value: u8, encoding: Encoding) -> f32 {
    let raw = f32::from(value);
    let offset = match encoding {
        // 1..=63 one way, 127..=65 the other, with 0 and 64 both standing
        // still. Two's complement in seven bits.
        Encoding::Signed => {
            if value == 0 || value == 64 {
                0.0
            } else if value < 64 {
                raw
            } else {
                raw - 128.0
            }
        }
        // Counting up from 65 and down from 63.
        Encoding::Offset | Encoding::Absolute => raw - CENTRED,
    };
    let fraction = (offset / DEFLECTION).clamp(-1.0, 1.0);
    if fraction >= 0.0 {
        fraction * binding.max.unwrap_or(1.0)
    } else {
        -fraction * binding.min.unwrap_or(0.0)
    }
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

    /// **A 14-bit fader resolves what a 7-bit one cannot.**
    ///
    /// The point of `cc14`: every Pioneer, Denon and Native Instruments fader
    /// arrives as a high byte and a low one, and reading only the high byte
    /// puts a pitch fader back on 128 steps -- 0.125% each, audibly coarse
    /// when beatmatching. Two positions one low-byte step apart have to give
    /// two different numbers.
    #[test]
    fn a_fourteen_bit_pair_resolves_a_step_a_seven_bit_control_cannot() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "cc14 1 0"
            move = "deck 1 pitch {value}"
            min = -1.0
            max = 1.0
            "#,
        );
        let mut at = |msb: u8, lsb: u8| {
            map.translate(Message::Control {
                channel: 0,
                controller: 0,
                value: msb,
            });
            map.translate(Message::Control {
                channel: 0,
                controller: 32,
                value: lsb,
            })
            .join("")
        };
        let a = at(64, 0);
        let b = at(64, 1);
        assert_ne!(
            a, b,
            "two positions one low-byte step apart wrote the same number, so the fader is \
             still seven bits"
        );
    }

    /// **The pair spans its whole range.** Both bytes at zero is the bottom of
    /// the fader and both at their maximum is the top; anything else means the
    /// two halves are being combined wrongly and the fader would never reach
    /// one end.
    #[test]
    fn a_fourteen_bit_pair_reaches_both_ends() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "cc14 1 0x13"
            move = "deck 1 volume {value}"
            "#,
        );
        let mut at = |msb: u8, lsb: u8| {
            map.translate(Message::Control {
                channel: 0,
                controller: 0x13,
                value: msb,
            });
            map.translate(Message::Control {
                channel: 0,
                controller: 0x33,
                value: lsb,
            })
        };
        assert_eq!(at(0, 0), vec!["deck 1 volume 0"]);
        assert_eq!(at(127, 127), vec!["deck 1 volume 1"]);
        // And which byte is which. Both ends are symmetric under swapping the
        // halves, so without an asymmetric point a mapping that read the low
        // byte as the high one would pass everything above while putting every
        // fader in the wrong place across its whole travel.
        assert_eq!(
            at(127, 0),
            vec!["deck 1 volume 0.992248"],
            "a full high byte with an empty low one is the top of the fader, not the bottom"
        );
        assert_eq!(
            at(0, 127),
            vec!["deck 1 volume 0.007752"],
            "an empty high byte with a full low one is the bottom of the fader"
        );
    }

    /// **A controller that sends only high bytes still works.**
    ///
    /// Some send the low byte only while a fader is actually moving and drop
    /// it at rest, and some never send one at all. A mapping that waited for
    /// both would leave those faders dead, which is worse than the coarseness
    /// it was avoiding -- and one that assumed a missing low byte was zero
    /// would leave the fader unable to reach its own top.
    #[test]
    fn a_high_byte_on_its_own_still_moves_the_control() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "cc14 1 7"
            move = "deck 1 volume {value}"
            "#,
        );
        assert_eq!(
            map.translate(Message::Control {
                channel: 0,
                controller: 7,
                value: 127
            }),
            vec!["deck 1 volume 1"],
            "a high byte alone did not reach the top of its own fader"
        );
    }

    /// **A high byte with no partner is refused when the file loads.**
    ///
    /// MIDI reserves 0..=31 for high bytes and 32..=63 for their partners, so
    /// `cc14` on 40 names a pair that cannot exist. Refusing at load means a
    /// DJ finds out while editing the file rather than by pushing a fader that
    /// does nothing.
    #[test]
    fn a_fourteen_bit_control_outside_the_high_byte_range_is_refused() {
        assert!(Trigger::parse("cc14 1 40").is_err());
        assert!(Trigger::parse("cc14 1 32").is_err());
        assert!(Trigger::parse("cc14 1 31").is_ok());
    }

    /// The two halves of one fader must not be confused with a different
    /// fader's. Deck 1's EQ high byte and deck 2's are the same controller on
    /// different channels.
    #[test]
    fn two_channels_keep_their_own_halves() {
        let mut map = mapping(
            r#"
            name = "Test"
            [[binding]]
            on = "cc14 1 7"
            move = "deck 1 volume {value}"
            [[binding]]
            on = "cc14 2 7"
            move = "deck 2 volume {value}"
            "#,
        );
        map.translate(Message::Control {
            channel: 0,
            controller: 7,
            value: 127,
        });
        map.translate(Message::Control {
            channel: 0,
            controller: 39,
            value: 127,
        });
        // Deck 2 has been told nothing, so it must still read zero.
        assert_eq!(
            map.translate(Message::Control {
                channel: 1,
                controller: 39,
                value: 0
            }),
            vec!["deck 2 volume 0"],
            "deck 1's fader moved deck 2's"
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

/// Lua, where a table cannot express what a control does.
#[cfg(test)]
mod script_binding_tests {
    use super::*;

    const SHIFTED: &str = r#"
name = "Scripted"
device = "Test"

script = """
local shifted = false
function on_control(control, event, value)
  if control == "note 1 0x3f" then
    shifted = (event == "press")
    return nil
  end
  if event ~= "press" then return nil end
  if shifted then return "deck 1 hotcue_set 1" end
  return "deck 1 hotcue 1"
end
"""

[[binding]]
on = "note 1 0x3f"
script = true

# Deliberately carries a `press` as well. A DJ converting a table binding to a
# scripted one leaves the old line behind, and if the table still handled it
# the pad would fire twice.
[[binding]]
on = "note 1 0x01"
script = true
press = "deck 2 play"

[[binding]]
on = "cc 1 0x08"
move = "crossfader {value}"
min = -1.0
max = 1.0
"#;

    fn note(note: u8, down: bool) -> crate::Message {
        if down {
            crate::Message::NoteOn {
                channel: 0,
                note,
                velocity: 127,
            }
        } else {
            crate::Message::NoteOff { channel: 0, note }
        }
    }

    /// A mapping may be part table and part script. The crossfader stays a
    /// table entry; the pads that need a shift key go to Lua.
    #[test]
    fn a_mapping_is_part_table_and_part_script() {
        let mut map = Mapping::parse(SHIFTED).expect("it parses");
        assert!(map.script.is_some());

        assert!(map.is_scripted(note(0x01, true)), "the pad is not scripted");
        assert!(
            !map.is_scripted(crate::Message::Control {
                channel: 0,
                controller: 0x08,
                value: 127,
            }),
            "the crossfader was taken by the script"
        );

        // The table half still works.
        assert_eq!(
            map.translate(crate::Message::Control {
                channel: 0,
                controller: 0x08,
                value: 127,
            }),
            vec!["crossfader 1"]
        );
    }

    /// **A scripted control must not fire twice.**
    ///
    /// The binding in `SHIFTED` keeps a `press` action on purpose: a DJ
    /// converting a table binding to a scripted one leaves the old line
    /// behind, and if the table still handled it the pad would do the scripted
    /// thing *and* the old thing. A binding with nothing but `script = true`
    /// would pass this test whether or not the rule held.
    #[test]
    fn a_scripted_control_is_not_also_handled_by_the_table() {
        let mut map = Mapping::parse(SHIFTED).expect("it parses");
        let binding = map
            .bindings
            .iter()
            .find(|b| b.on == "note 1 0x01")
            .expect("the pad is in the file");
        assert!(
            binding.press.is_some(),
            "this test only means something while the binding has a table action too"
        );

        assert!(
            map.translate(note(0x01, true)).is_empty(),
            "the table acted on a control the script owns, so the pad fires twice"
        );
    }

    /// What the script is handed: the control's own `on` text, so it can tell
    /// its pads apart, and the value the table would have produced.
    #[test]
    fn the_script_is_handed_the_control_and_its_scaled_value() {
        let map = Mapping::parse(
            "name = \"x\"\ndevice = \"y\"\n\nscript = \"\"\"\nfunction on_control(c,e,v) return nil end\n\"\"\"\n\n             [[binding]]\non = \"cc 1 0x08\"\nscript = true\nmin = -1.0\nmax = 1.0\n",
        )
        .expect("it parses");

        let (control, event, value) = map
            .script_event(crate::Message::Control {
                channel: 0,
                controller: 0x08,
                value: 127,
            })
            .expect("the control is scripted");
        assert_eq!(control, "cc 1 0x08");
        assert_eq!(event, crate::script::Event::Move);
        assert!(
            (value - 1.0).abs() < 1e-6,
            "the binding's own min/max were not applied: {value}"
        );
    }

    /// A press and a release are told apart, which is the whole basis of a
    /// held shift key.
    #[test]
    fn a_press_and_a_release_are_told_apart() {
        let map = Mapping::parse(SHIFTED).expect("it parses");
        assert_eq!(
            map.script_event(note(0x3f, true)).map(|e| e.1),
            Some(crate::script::Event::Press)
        );
        assert_eq!(
            map.script_event(note(0x3f, false)).map(|e| e.1),
            Some(crate::script::Event::Release)
        );
        // Note-on with velocity 0 is a release, which half the controllers in
        // the world send instead of a note-off.
        assert_eq!(
            map.script_event(crate::Message::NoteOn {
                channel: 0,
                note: 0x3f,
                velocity: 0,
            })
            .map(|e| e.1),
            Some(crate::script::Event::Release)
        );
    }

    /// **A dead control is refused when the file loads.** A binding marked
    /// `script` in a mapping with no script does nothing at all, which is
    /// exactly the failure the whole crate checks for at load time.
    #[test]
    fn a_scripted_binding_with_no_script_is_refused() {
        let why = Mapping::parse(
            "name = \"x\"\ndevice = \"y\"\n\n[[binding]]\non = \"note 1 0x01\"\nscript = true\n",
        )
        .expect_err("a scripted binding with no script should be refused");
        assert!(
            matches!(why, MappingError::ScriptWithoutOne(ref on) if on == "note 1 0x01"),
            "wrong error: {why}"
        );
    }
}

/// HID: turning a stream of level reports into the changes that happened.
///
/// The property everything here rests on is that a device sending the same
/// state a thousand times a second must produce **nothing** a thousand times,
/// and exactly one action when a finger lands.
#[cfg(test)]
mod hid_tests {
    use super::*;

    fn mapping(bindings: &str) -> Mapping {
        Mapping::parse(&format!(
            "name = \"HID test\"\ndevice = \"Test\"\n\n{bindings}"
        ))
        .expect("the test mapping parses")
    }

    fn pad() -> Mapping {
        mapping(
            "[[binding]]\non = \"hid 1 bit 0.2\"\n\
             press = \"deck 1 play_pause\"\nrelease = \"deck 1 cue\"\n",
        )
    }

    #[test]
    fn a_hid_trigger_parses_every_width_it_offers() {
        use crate::report::{Field, Width};
        for (text, want) in [
            ("hid 1 bit 3.2", Field::new(1, 3, Width::Bit(2))),
            ("hid 1 byte 5", Field::new(1, 5, Width::Byte)),
            ("hid 2 word 6", Field::new(2, 6, Width::Word)),
            ("hid 2 word-le 6", Field::new(2, 6, Width::WordLe)),
        ] {
            assert_eq!(Trigger::parse(text), Ok(Trigger::Hid(want)), "{text}");
        }
    }

    /// A bit index past the end of a byte is a typo, and a typo in a mapping
    /// is a message when the file loads -- the same promise the action text
    /// makes.
    #[test]
    fn a_bit_index_past_the_end_of_a_byte_is_refused() {
        assert!(Trigger::parse("hid 1 bit 3.8").is_err());
        assert!(Trigger::parse("hid 1 bit 3").is_err());
        assert!(Trigger::parse("hid 1 nibble 3").is_err());
        assert!(Trigger::parse("hid 1 byte").is_err());
        assert!(Trigger::parse("hid").is_err());
    }

    /// **The load-bearing one.** A HID device repeats itself; djmanzo must
    /// not. Holding a pad through a hundred reports is one press.
    ///
    /// Both kinds of field are checked here on purpose. A switch and a range
    /// take different paths through [`Mapping::translate_report`], and a test
    /// that only pressed a pad would leave the fader path -- the one that
    /// would flood the action bus at a thousand values a second -- uncovered.
    #[test]
    fn a_repeated_report_says_nothing_after_the_first_change() {
        let mut map = mapping(
            "[[binding]]\non = \"hid 1 bit 0.2\"\n\
             press = \"deck 1 play_pause\"\nrelease = \"deck 1 cue\"\n\n\
             [[binding]]\non = \"hid 1 byte 1\"\nmove = \"deck 1 volume {value}\"\n",
        );
        let held = [1u8, 0b0000_0100, 0x40];

        // The first packet: the pad went down and the fader said where it is.
        assert_eq!(
            map.translate_report(&held),
            vec!["deck 1 play_pause", "deck 1 volume 0.25098"]
        );
        // A thousand identical packets a second must produce nothing at all.
        for _ in 0..100 {
            assert!(
                map.translate_report(&held).is_empty(),
                "a report that changed nothing produced an action"
            );
        }
        // And a real change still gets through, from either field alone.
        assert_eq!(
            map.translate_report(&[1u8, 0, 0x40]),
            vec!["deck 1 cue"],
            "letting go of the pad was lost"
        );
        assert_eq!(
            map.translate_report(&[1u8, 0, 0x41]),
            vec!["deck 1 volume 0.254902"],
            "moving the fader was lost"
        );
    }

    /// The first packet must not fire a release for every button that simply
    /// is not being pressed. A mapping with sixteen pads would otherwise
    /// deliver sixteen actions the moment a controller is plugged in.
    #[test]
    fn the_first_report_does_not_release_buttons_nobody_touched() {
        let mut map = pad();
        assert!(
            map.translate_report(&[1u8, 0]).is_empty(),
            "an untouched button reported as up produced an action"
        );
    }

    /// A button genuinely held when the device appears reads as a press, so
    /// the release that follows has something to match. Suppressing the press
    /// instead would leave a momentary binding switched on with no way back.
    #[test]
    fn a_button_already_held_when_the_device_appears_is_a_matched_pair() {
        let mut map = pad();
        assert_eq!(
            map.translate_report(&[1u8, 0b0000_0100]),
            vec!["deck 1 play_pause"]
        );
        assert_eq!(map.translate_report(&[1u8, 0]), vec!["deck 1 cue"]);
    }

    /// A fader's position is a fact worth knowing the moment the device
    /// appears, so unlike a switch its first report *is* a change.
    #[test]
    fn a_fader_reports_where_it_is_on_the_first_packet() {
        let mut map =
            mapping("[[binding]]\non = \"hid 1 byte 1\"\nmove = \"deck 1 volume {value}\"\n");
        assert_eq!(
            map.translate_report(&[1u8, 0, 0xFF]),
            vec!["deck 1 volume 1"]
        );
        assert!(map.translate_report(&[1u8, 0, 0xFF]).is_empty());
    }

    /// The reason to use HID at all. Two adjacent sixteen-bit values are two
    /// distinguishable positions; squeezed through seven bits they would be
    /// the same number, and a jog wheel would step where it should glide.
    #[test]
    fn sixteen_bits_of_travel_survive_the_whole_way_through() {
        let mut map =
            mapping("[[binding]]\non = \"hid 1 word 1\"\nmove = \"deck 1 volume {value}\"\n");
        let low = map.translate_report(&[1u8, 0, 0x80, 0x00]);
        let high = map.translate_report(&[1u8, 0, 0x80, 0x01]);
        assert_eq!(low.len(), 1);
        assert_eq!(high.len(), 1);
        assert_ne!(
            low[0], high[0],
            "one step of a 16-bit control vanished: {low:?} == {high:?}"
        );

        // And the same two values through a 7-bit MIDI control would not be
        // distinguishable at all -- 0x8000 and 0x8001 are both 128/65536 of
        // the way up, which is one 127th step.
        let step = 1.0 / 65_535.0;
        assert!(
            step < 1.0 / 127.0,
            "a 16-bit step is finer than a 7-bit one"
        );
    }

    /// A HID field reports a level. `turn_up` and `turn_down` describe an
    /// encoder's *event*, and there is no honest way to read one from the
    /// other -- so it is refused when the file loads rather than silently
    /// doing nothing.
    #[test]
    fn an_encoder_on_a_hid_field_is_refused_when_the_file_loads() {
        let why = Mapping::parse(
            "name = \"x\"\ndevice = \"y\"\n\n[[binding]]\non = \"hid 1 byte 2\"\n\
             turn_up = \"deck 1 beat_jump 1\"\nturn_down = \"deck 1 beat_jump -1\"\n",
        )
        .expect_err("an encoder on a HID field should be refused");
        assert!(
            matches!(why, MappingError::EncoderOnHid(_)),
            "wrong error: {why}"
        );
    }

    /// A 3,600-step platter read out of one byte would wrap fourteen times a
    /// revolution. The field has to be wide enough for the resolution, and
    /// that is arithmetic djmanzo can do when the file loads.
    #[test]
    fn a_platter_finer_than_its_field_is_refused() {
        let text = |on: &str| {
            format!(
                "name = \"x\"\ndevice = \"y\"\n\n[[binding]]\non = \"{on}\"\n\
                 platter = \"deck 1 jog {{value}}\"\nresolution = 3600\n"
            )
        };
        let why =
            Mapping::parse(&text("hid 1 byte 2")).expect_err("3600 steps do not fit in a byte");
        assert!(
            matches!(why, MappingError::PlatterTooFineForField(_, 3600, 256)),
            "wrong error: {why}"
        );
        // The same platter in a sixteen-bit field is fine, which is the whole
        // reason a mapping would reach for HID.
        assert!(Mapping::parse(&text("hid 1 word 2")).is_ok());
    }

    /// A device that sends several kinds of report sends them down one pipe.
    /// A field on report 1 reading report 2 would be a button pressing itself.
    #[test]
    fn a_report_for_another_field_is_ignored_entirely() {
        let mut map = pad();
        assert!(map.translate_report(&[2u8, 0b0000_0100]).is_empty());
        assert!(map.translate_report(&[1u8]).is_empty());
        assert!(map.translate_report(&[]).is_empty());
        // And the state it did not see must not have been remembered: the real
        // press still has to arrive.
        assert_eq!(
            map.translate_report(&[1u8, 0b0000_0100]),
            vec!["deck 1 play_pause"]
        );
    }

    /// A HID platter turns like any other: what reaches the deck is movement,
    /// not the angle, and a wrap through zero is a small step rather than a
    /// revolution backwards.
    #[test]
    fn a_hid_platter_reports_movement_through_the_wrap() {
        let mut map = mapping(
            "[[binding]]\non = \"hid 1 word 1\"\n\
             platter = \"deck 1 jog {value}\"\nresolution = 3600\n",
        );
        let at = |angle: u16| {
            let [hi, lo] = angle.to_be_bytes();
            [1u8, 0, hi, lo]
        };
        // The first report is a starting point: it establishes where the
        // platter *is*, so what it reports is a movement of zero -- not
        // nothing, and emphatically not 3,590 steps of jog.
        let first = map.translate_report(&at(3_590));
        assert_eq!(
            first,
            vec!["deck 1 jog 0"],
            "the first angle was read as movement"
        );
        let over = map.translate_report(&at(10));
        assert_eq!(over.len(), 1, "a wrap produced nothing");
        let turns: f64 = over[0]
            .rsplit(' ')
            .next()
            .and_then(|n| n.parse().ok())
            .expect("the action ends in a number");
        // Twenty steps forward of 3,600 is a small step, not 0.997 backwards.
        assert!(
            (turns - 20.0 / 3600.0).abs() < 1e-4,
            "a wrap was read as a revolution: {turns}"
        );
    }

    /// MIDI and HID triggers live in one mapping and must not disturb each
    /// other: a controller with a HID jog and MIDI pads is exactly the device
    /// this is for.
    #[test]
    fn hid_and_midi_bindings_coexist_in_one_mapping() {
        let mut map = mapping(
            "[[binding]]\non = \"hid 1 bit 0.2\"\npress = \"deck 1 play_pause\"\n\n\
             [[binding]]\non = \"note 1 0x0B\"\npress = \"deck 2 play_pause\"\n",
        );
        assert_eq!(map.hid_fields().len(), 1);
        assert_eq!(
            map.translate_report(&[1u8, 0b0000_0100]),
            vec!["deck 1 play_pause"]
        );
        assert_eq!(
            map.translate(crate::Message::NoteOn {
                channel: 0,
                note: 0x0B,
                velocity: 127,
            }),
            vec!["deck 2 play_pause"]
        );
        // A MIDI message must not be mistaken for a HID field, or every note
        // on a busy controller would run the HID path.
        assert!(Trigger::parse("note 1 0x0B").unwrap().field().is_none());
    }
}
