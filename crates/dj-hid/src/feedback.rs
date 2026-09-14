//! Lighting the controller: parameters going back out as MIDI.
//!
//! The inbound half of this crate turns a knob into an [`dj_core::Action`].
//! This is the return path -- a deck that starts playing lights its play
//! button, a loop that engages lights its pad, a filter knob moves a ring of
//! LEDs. Without it a controller is half dead: a DJ looks down mid-set and the
//! hardware disagrees with the screen about what is happening.
//!
//! # The same discipline as a binding
//!
//! A feedback line names a **parameter**, by the same stable name the network
//! API and the interface use -- `deck.1.playing`, `master.crossfader`. The name
//! is resolved [when the file loads](FeedbackMap::parse), so a typo is a
//! message at the moment a DJ chooses the mapping rather than an LED that
//! silently never lights. That is the same promise the inbound side makes
//! about actions, for the same reason.
//!
//! # Why only what changed is sent
//!
//! A MIDI DIN cable carries 3,125 bytes a second: about a thousand three-byte
//! messages, shared with everything else on the wire. A controller with sixty
//! lit controls, refreshed at the snapshot rate, would be 3,600 messages a
//! second -- more than the cable holds, and the ones that matter (a pad a DJ
//! just hit) would queue behind the ones that did not change. So
//! [`Feedback::poll`] emits a message only when a value has actually moved.

use crate::mapping::{MappingError, Trigger};
use dj_control::ParameterRegistry;
use dj_core::ParamId;
use serde::Deserialize;
use std::collections::HashMap;

/// The seven-bit range every MIDI data byte lives in.
const MAX_VALUE: u8 = 127;

/// What a parameter's value becomes on the wire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// On or off: below `threshold` sends `off`, at or above sends `on`.
    ///
    /// Most lit controls are this. The parameter registry holds `playing` as
    /// 0.0 or 1.0, and a pad is lit or it is not.
    Switch { threshold: f32, on: u8, off: u8 },
    /// A position across `min..=max`, sent as 0..=127.
    ///
    /// For LED rings and motorised faders, where the control shows *where* a
    /// value is rather than whether it is set.
    Range { min: f32, max: f32 },
}

/// One parameter, lit on one control.
#[derive(Debug, Clone, PartialEq)]
pub struct Light {
    /// The parameter that drives it.
    pub parameter: ParamId,
    /// Where to send it.
    pub target: Trigger,
    /// How its value becomes a byte.
    pub shape: Shape,
}

impl Light {
    /// The byte this parameter currently wants on the wire.
    #[must_use]
    pub fn value(&self, reading: f32) -> u8 {
        match self.shape {
            Shape::Switch { threshold, on, off } => {
                // A NaN reading is neither above nor below a threshold. Off is
                // the safe answer: a light that stays dark is a light that is
                // wrong once, where a light stuck on is wrong until the set
                // ends.
                if reading >= threshold { on } else { off }
            }
            Shape::Range { min, max } => {
                if !reading.is_finite() || (max - min).abs() < f32::EPSILON {
                    return 0;
                }
                let position = ((reading - min) / (max - min)).clamp(0.0, 1.0);
                (position * f32::from(MAX_VALUE)).round() as u8
            }
        }
    }

    /// The three bytes to send for `value`.
    ///
    /// `None` for a target that cannot carry a light. A pitch bend is a
    /// fourteen-bit *input*; controllers do not take one as feedback, and
    /// silently sending something else would be worse than saying so.
    #[must_use]
    pub fn message(&self, value: u8) -> Option<[u8; 3]> {
        // A file counts channels from one, the wire from zero -- the same
        // convention `Message::from_bytes` uses on the way in.
        match self.target {
            Trigger::Note { channel, note } => {
                Some([0x90 | channel.saturating_sub(1) & 0x0F, note & 0x7F, value])
            }
            Trigger::Control {
                channel,
                controller,
            } => Some([
                0xB0 | channel.saturating_sub(1) & 0x0F,
                controller & 0x7F,
                value,
            ]),
            // None of these can carry a MIDI light. A pitch bend and a
            // fourteen-bit control change are both high-resolution *inputs* --
            // a fader's position, not a lamp -- and a `[[feedback]]` line
            // carries one seven-bit value, so there is no honest way to
            // address either. Sending the high byte alone would look like it
            // worked and leave the low byte stale, which is the shape of thing
            // this project keeps refusing to ship. A HID field is a location
            // in an *inbound* report; lighting a HID device means writing an
            // output report of its own shape, which is a different thing
            // entirely. Saying so beats sending three bytes a device would
            // read as something else.
            Trigger::Bend { .. } | Trigger::Control14 { .. } | Trigger::Hid(_) => None,
        }
    }
}

/// The `[[feedback]]` blocks of a mapping file, as written.
#[derive(Debug, Deserialize)]
pub struct FeedbackFile {
    #[serde(default)]
    pub feedback: Vec<FeedbackLine>,
}

/// One `[[feedback]]` block.
#[derive(Debug, Deserialize)]
pub struct FeedbackLine {
    /// The parameter, by its stable name: `deck.1.playing`.
    pub when: String,
    /// Where to send it: `note 1 0x0B`, `cc 1 0x20`.
    pub send: String,
    /// Value when the parameter is on. Presence of this selects a switch.
    pub on: Option<u8>,
    /// Value when it is off.
    pub off: Option<u8>,
    /// What counts as on.
    pub threshold: Option<f32>,
    /// Bottom of the range, for a continuous light.
    pub min: Option<f32>,
    /// Top of the range.
    pub max: Option<f32>,
}

/// Every light in a mapping.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FeedbackMap {
    pub lights: Vec<Light>,
}

impl FeedbackMap {
    /// Read the `[[feedback]]` blocks of a mapping file.
    ///
    /// # Errors
    /// If a parameter name is not one the engine has, if a target cannot be
    /// parsed, or if a light says it is both a switch and a range -- which
    /// would leave the file's meaning up to whichever field was read first.
    pub fn parse(text: &str) -> Result<Self, MappingError> {
        let file: FeedbackFile =
            toml::from_str(text).map_err(|e| MappingError::Unreadable(e.to_string()))?;

        let by_name = parameters_by_name();
        let mut lights = Vec::with_capacity(file.feedback.len());

        for line in file.feedback {
            let parameter = *by_name
                .get(line.when.as_str())
                .ok_or_else(|| MappingError::UnknownParameter(line.when.clone()))?;
            let target = Trigger::parse(&line.send)?;

            let is_switch = line.on.is_some() || line.off.is_some() || line.threshold.is_some();
            let is_range = line.min.is_some() || line.max.is_some();
            if is_switch && is_range {
                return Err(MappingError::AmbiguousFeedback(line.when.clone()));
            }

            let shape = if is_range {
                Shape::Range {
                    min: line.min.unwrap_or(0.0),
                    max: line.max.unwrap_or(1.0),
                }
            } else {
                Shape::Switch {
                    // Half way, so a parameter the engine writes as 0.0 or 1.0
                    // works with nothing declared.
                    threshold: line.threshold.unwrap_or(0.5),
                    on: line.on.unwrap_or(MAX_VALUE).min(MAX_VALUE),
                    off: line.off.unwrap_or(0).min(MAX_VALUE),
                }
            };

            lights.push(Light {
                parameter,
                target,
                shape,
            });
        }

        Ok(Self { lights })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lights.is_empty()
    }
}

/// Drives a mapping's lights from the parameter registry.
///
/// Holds the last byte sent for each light, which is what makes "only what
/// changed" possible.
#[derive(Debug)]
pub struct Feedback {
    map: FeedbackMap,
    last: Vec<Option<u8>>,
}

impl Feedback {
    #[must_use]
    pub fn new(map: FeedbackMap) -> Self {
        let last = vec![None; map.lights.len()];
        Self { map, last }
    }

    /// Every message the controller needs to catch up with the engine.
    ///
    /// Empty when nothing moved, which is the common case: this is called at
    /// the snapshot rate and a mixer is mostly still.
    pub fn poll(&mut self, registry: &ParameterRegistry) -> Vec<[u8; 3]> {
        let mut out = Vec::new();
        for (index, light) in self.map.lights.iter().enumerate() {
            let value = light.value(registry.get(light.parameter));
            if self.last[index] == Some(value) {
                continue;
            }
            if let Some(message) = light.message(value) {
                out.push(message);
                self.last[index] = Some(value);
            }
        }
        out
    }

    /// Forget what the controller was last told.
    ///
    /// Called when a device is plugged in or a mapping is chosen: the hardware
    /// comes up dark and knows nothing, so the next poll has to send
    /// everything rather than only what has changed since it was unplugged.
    pub fn resend_everything(&mut self) {
        self.last.iter_mut().for_each(|slot| *slot = None);
    }

    /// Every light turned off, for letting go of a device.
    ///
    /// A controller keeps its LEDs lit after the application quits -- they are
    /// the device's state, not ours -- so a DJ closing djmanzo would be left
    /// with a board still showing the last set.
    #[must_use]
    pub fn blackout(&self) -> Vec<[u8; 3]> {
        self.map
            .lights
            .iter()
            .filter_map(|light| light.message(0))
            .collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.map.lights.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// How often the lights are brought up to date.
///
/// About thirty times a second. The module docs explain the ceiling: a MIDI
/// DIN cable carries roughly a thousand three-byte messages a second, shared
/// with everything else on the wire, so the snapshot rate would be twice the
/// budget for a busy board. Thirty is half of it and still under a frame of
/// lag on an LED, which nobody has ever seen.
const EVERY: std::time::Duration = std::time::Duration::from_millis(33);

/// A mapping's lights, driven on a thread of their own.
///
/// [`Feedback`] is the arithmetic — which byte each parameter wants, and which
/// of them changed. This is the thing that actually sends them, and until it
/// existed the whole return path was **parsed and consulted by nothing**: a
/// mapping declared its `[[feedback]]` blocks, `FeedbackMap::parse` resolved
/// every parameter name in them, and no byte ever left the machine. A DJ
/// looked down mid-set and the hardware disagreed with the screen about what
/// was playing.
///
/// # Its own thread, not the snapshot pump
///
/// A blocked MIDI write must not be able to stall the interface, and the
/// interface running slowly must not be able to leave a play button lit on a
/// deck that stopped. They are separate answers to separate questions and they
/// belong on separate threads. The registry is the shared truth between them
/// and it is lock-free by construction, so this thread costs one atomic read
/// per light per tick.
///
/// # It goes dark when it is dropped
///
/// A controller keeps its LEDs lit after the process that set them has gone —
/// they are the device's state, not djmanzo's — so letting go of a port
/// without a blackout leaves a DJ with a board still showing the last set.
/// That happens in [`Drop`], because the cases where it matters most are the
/// ones nobody wrote a close path for: a different mapping chosen, a device
/// unplugged, the application quitting.
#[derive(Debug)]
pub struct Lights {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    lit: usize,
    port: String,
}

impl Lights {
    /// Start driving `map`'s lights into `sink` from `registry`.
    ///
    /// `port` is only carried so the interface can say which output is being
    /// lit; nothing here reads it.
    #[must_use]
    pub fn start(
        map: FeedbackMap,
        mut sink: Box<dyn crate::out::Sink>,
        registry: std::sync::Arc<ParameterRegistry>,
        port: String,
    ) -> Self {
        use std::sync::atomic::{AtomicBool, Ordering};

        let lit = map.lights.len();
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&stop);
        // A named thread, so a stall shows up as `djmanzo-lights` in a
        // backtrace rather than as thread 7.
        let thread = std::thread::Builder::new()
            .name("djmanzo-lights".to_owned())
            .spawn(move || {
                // Nothing has been sent yet, so the first poll sends every
                // light — which is what a device that just came up dark needs.
                let mut feedback = Feedback::new(map);
                while !flag.load(Ordering::Relaxed) {
                    for message in feedback.poll(&registry) {
                        sink.send(&message);
                    }
                    std::thread::sleep(EVERY);
                }
                for message in feedback.blackout() {
                    sink.send(&message);
                }
            })
            .ok();

        Self {
            stop,
            thread,
            lit,
            port,
        }
    }

    /// How many lights this mapping declares.
    #[must_use]
    pub const fn lit(&self) -> usize {
        self.lit
    }

    /// The output being lit, by name.
    #[must_use]
    pub fn port(&self) -> &str {
        &self.port
    }
}

impl Drop for Lights {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            // Joined rather than detached: the blackout is the last thing the
            // thread does, and a detached one racing a newly opened port would
            // put the old mapping's darkness over the new mapping's lights.
            let _ = thread.join();
        }
    }
}

/// Every parameter, by the name a mapping file uses.
fn parameters_by_name() -> HashMap<String, ParamId> {
    ParamId::all().map(|id| (id.name(), id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::param::DeckParam;
    use dj_core::{Action, DeckId};

    /// Somewhere bytes can be sent that is not a device.
    #[derive(Debug, Default, Clone)]
    struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl Recorder {
        fn bytes(&self) -> Vec<u8> {
            self.0.lock().expect("not poisoned").clone()
        }
    }

    impl crate::out::Sink for Recorder {
        fn send(&mut self, message: &[u8]) {
            self.0
                .lock()
                .expect("not poisoned")
                .extend_from_slice(message);
        }
    }

    /// Wait until `check` is true, or give up. Returns whether it happened.
    ///
    /// The pump is a real thread on a real clock, so a test that slept a fixed
    /// amount would be either slow or flaky, and on a loaded CI machine both.
    fn until(check: impl Fn() -> bool) -> bool {
        for _ in 0..200 {
            if check() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        false
    }

    fn lit_map() -> FeedbackMap {
        FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "deck.1.playing"
            send = "note 1 0x0B"
            "#,
        )
        .expect("a mapping with one light")
    }

    /// **A light that nothing sends is not a light.**
    ///
    /// The whole reason this type exists. `FeedbackMap` resolved every
    /// parameter name in a mapping's `[[feedback]]` blocks and `Feedback`
    /// worked out the bytes, and no byte ever left the machine — the fourth
    /// table in this codebase found parsed, validated and consulted by
    /// nothing. A DJ looked down mid-set and the hardware disagreed with the
    /// screen.
    #[test]
    fn a_parameter_that_moves_reaches_the_wire() {
        let registry = std::sync::Arc::new(ParameterRegistry::new());
        let sink = Recorder::default();
        let lights = Lights::start(
            lit_map(),
            Box::new(sink.clone()),
            std::sync::Arc::clone(&registry),
            "Test Out".to_owned(),
        );
        assert_eq!(lights.lit(), 1);
        assert_eq!(lights.port(), "Test Out");

        // Off first, because a device comes up dark knowing nothing and the
        // first pass tells it everything.
        assert!(
            until(|| sink.bytes() == vec![0x90, 0x0B, 0x00]),
            "the first pass did not tell the device the deck was stopped: {:?}",
            sink.bytes()
        );

        registry.set(
            ParamId::Deck(DeckId::from_human(1).expect("deck 1"), DeckParam::Playing),
            1.0,
        );
        assert!(
            until(|| sink.bytes().len() >= 6),
            "the deck started and the light did not: {:?}",
            sink.bytes()
        );
        assert_eq!(&sink.bytes()[3..6], &[0x90, 0x0B, 0x7F]);
        drop(lights);
    }

    /// **Letting go of a device leaves it dark.**
    ///
    /// A controller keeps its LEDs after the process that set them has gone;
    /// they are the device's state, not djmanzo's. The blackout is in `Drop`
    /// because the cases that matter are the ones nobody writes a close path
    /// for — a different mapping chosen, a device unplugged, a quit.
    #[test]
    fn dropping_the_pump_turns_every_light_off() {
        let registry = std::sync::Arc::new(ParameterRegistry::new());
        registry.set(
            ParamId::Deck(DeckId::from_human(1).expect("deck 1"), DeckParam::Playing),
            1.0,
        );
        let sink = Recorder::default();
        let lights = Lights::start(
            lit_map(),
            Box::new(sink.clone()),
            std::sync::Arc::clone(&registry),
            "Test Out".to_owned(),
        );
        assert!(
            until(|| sink.bytes() == vec![0x90, 0x0B, 0x7F]),
            "the lit deck never lit: {:?}",
            sink.bytes()
        );

        drop(lights);
        // `Drop` joins the thread, so by here the blackout has been sent.
        assert_eq!(
            sink.bytes(),
            vec![0x90, 0x0B, 0x7F, 0x90, 0x0B, 0x00],
            "the device was left lit"
        );
    }

    /// **A board that is not moving costs nothing on the wire.**
    ///
    /// The module's own budget: a DIN cable carries about a thousand messages
    /// a second and a mixer is mostly still. A pump that resent every light on
    /// every tick would spend the whole cable on saying nothing changed, and
    /// the pad a DJ just hit would queue behind it.
    #[test]
    fn a_still_mixer_sends_nothing_after_the_first_pass() {
        let registry = std::sync::Arc::new(ParameterRegistry::new());
        let sink = Recorder::default();
        let lights = Lights::start(
            lit_map(),
            Box::new(sink.clone()),
            std::sync::Arc::clone(&registry),
            "Test Out".to_owned(),
        );
        assert!(until(|| !sink.bytes().is_empty()), "nothing was ever sent");
        let after_first = sink.bytes().len();
        // Several ticks' worth of a mixer nobody is touching.
        std::thread::sleep(EVERY * 6);
        assert_eq!(
            sink.bytes().len(),
            after_first,
            "a still mixer kept talking: {:?}",
            sink.bytes()
        );
        drop(lights);
    }

    fn registry() -> ParameterRegistry {
        ParameterRegistry::new()
    }

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    #[test]
    fn a_switch_lights_when_the_parameter_is_set() {
        let map = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "deck.1.playing"
            send = "note 1 0x0B"
            "#,
        )
        .expect("a valid mapping");

        let light = &map.lights[0];
        assert_eq!(light.value(0.0), 0);
        assert_eq!(light.value(1.0), 127);
        assert_eq!(light.message(127), Some([0x90, 0x0B, 127]));
    }

    #[test]
    fn on_and_off_values_can_be_chosen_for_coloured_pads() {
        let map = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "deck.2.playing"
            send = "note 2 0x14"
            on = 5
            off = 2
            "#,
        )
        .unwrap();

        let light = &map.lights[0];
        assert_eq!(light.value(1.0), 5);
        assert_eq!(light.value(0.0), 2);
        // Channel 2 in the file is channel 1 on the wire.
        assert_eq!(light.message(5), Some([0x91, 0x14, 5]));
    }

    #[test]
    fn a_range_maps_across_the_seven_bit_span() {
        let map = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "master.crossfader"
            send = "cc 1 0x20"
            min = -1.0
            max = 1.0
            "#,
        )
        .unwrap();

        let light = &map.lights[0];
        assert_eq!(light.value(-1.0), 0);
        assert_eq!(light.value(1.0), 127);
        assert_eq!(light.value(0.0), 64, "the centre should be mid-scale");
        assert_eq!(light.message(64), Some([0xB0, 0x20, 64]));
    }

    #[test]
    fn a_value_outside_the_range_is_clamped_rather_than_wrapped() {
        let map = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "master.crossfader"
            send = "cc 1 0x20"
            min = 0.0
            max = 1.0
            "#,
        )
        .unwrap();
        let light = &map.lights[0];
        assert_eq!(light.value(-5.0), 0);
        assert_eq!(light.value(5.0), 127);
    }

    /// A byte over 127 has its top bit set, which on the wire is a *status*
    /// byte -- the device would read it as the start of another message and
    /// everything after it would be garbage.
    #[test]
    fn no_light_can_emit_a_status_byte() {
        let map = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "deck.1.playing"
            send = "note 1 0x7F"
            on = 255
            "#,
        )
        .unwrap();

        let light = &map.lights[0];
        let message = light.message(light.value(1.0)).unwrap();
        for byte in &message[1..] {
            assert!(*byte <= 0x7F, "data byte {byte:#x} has its top bit set");
        }
    }

    /// **The reason this is not just a refresh loop.** A DIN cable carries
    /// about a thousand messages a second, shared with everything else. Sixty
    /// lit controls at the snapshot rate would be 3,600 -- so the pad a DJ
    /// just hit would queue behind fifty-nine that had not changed.
    #[test]
    fn nothing_is_sent_while_nothing_changes() {
        let map = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "deck.1.playing"
            send = "note 1 0x0B"

            [[feedback]]
            when = "deck.2.playing"
            send = "note 2 0x0B"
            "#,
        )
        .unwrap();
        let mut feedback = Feedback::new(map);
        let registry = registry();

        assert_eq!(
            feedback.poll(&registry).len(),
            2,
            "the first poll catches up"
        );
        assert!(feedback.poll(&registry).is_empty(), "nothing moved");
        assert!(feedback.poll(&registry).is_empty());
    }

    #[test]
    fn a_change_is_sent_once() {
        let map = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "deck.1.playing"
            send = "note 1 0x0B"
            "#,
        )
        .unwrap();
        let mut feedback = Feedback::new(map);
        let registry = registry();
        let _ = feedback.poll(&registry);

        registry.set(ParamId::Deck(deck(1), DeckParam::Playing), 1.0);
        assert_eq!(feedback.poll(&registry), vec![[0x90, 0x0B, 127]]);
        assert!(feedback.poll(&registry).is_empty(), "sent twice");

        registry.set(ParamId::Deck(deck(1), DeckParam::Playing), 0.0);
        assert_eq!(feedback.poll(&registry), vec![[0x90, 0x0B, 0]]);
    }

    /// A device plugged in mid-set comes up dark and knows nothing. Without
    /// this it would stay dark until each value happened to change.
    #[test]
    fn a_reconnected_device_is_told_everything_again() {
        let map = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "deck.1.playing"
            send = "note 1 0x0B"
            "#,
        )
        .unwrap();
        let mut feedback = Feedback::new(map);
        let registry = registry();

        assert_eq!(feedback.poll(&registry).len(), 1);
        assert!(feedback.poll(&registry).is_empty());

        feedback.resend_everything();
        assert_eq!(feedback.poll(&registry).len(), 1, "the device stays dark");
    }

    /// A controller's LEDs are its own state and survive the application
    /// quitting. Leaving them lit means a board still showing last night's set.
    #[test]
    fn letting_go_of_a_device_turns_its_lights_off() {
        let map = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "deck.1.playing"
            send = "note 1 0x0B"
            on = 5

            [[feedback]]
            when = "master.crossfader"
            send = "cc 1 0x20"
            min = -1.0
            max = 1.0
            "#,
        )
        .unwrap();
        let feedback = Feedback::new(map);

        let dark = feedback.blackout();
        assert_eq!(dark.len(), 2);
        assert!(dark.iter().all(|m| m[2] == 0), "something stayed lit");
    }

    /// **The promise the inbound side already makes.** A typo has to be a
    /// message when the file is chosen, not a light that never comes on an
    /// hour into a set.
    #[test]
    fn an_unknown_parameter_is_refused_when_the_file_loads() {
        let error = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "deck.1.playng"
            send = "note 1 0x0B"
            "#,
        )
        .expect_err("that parameter does not exist");
        assert!(
            error.to_string().contains("deck.1.playng"),
            "the message should name the typo: {error}"
        );
    }

    #[test]
    fn a_target_that_cannot_be_parsed_is_refused() {
        assert!(
            FeedbackMap::parse(
                r#"
                [[feedback]]
                when = "deck.1.playing"
                send = "sysex 1 2 3"
                "#,
            )
            .is_err()
        );
    }

    /// A line that is both a switch and a range has no single meaning, and
    /// picking one silently would light the wrong thing.
    #[test]
    fn a_light_cannot_be_both_a_switch_and_a_range() {
        assert!(
            FeedbackMap::parse(
                r#"
                [[feedback]]
                when = "deck.1.volume"
                send = "cc 1 0x20"
                on = 127
                min = 0.0
                max = 1.0
                "#,
            )
            .is_err()
        );
    }

    /// Pitch bend is a fourteen-bit input. Controllers do not take one as
    /// feedback, and quietly sending something else would light the wrong
    /// control.
    #[test]
    fn a_bend_target_sends_nothing_rather_than_something_wrong() {
        let map = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "deck.1.pitch"
            send = "bend 1"
            min = -1.0
            max = 1.0
            "#,
        )
        .unwrap();
        assert_eq!(map.lights[0].message(64), None);

        let mut feedback = Feedback::new(map);
        assert!(
            feedback.poll(&registry()).is_empty(),
            "a bend target must not put bytes on the wire"
        );
    }

    /// A NaN is neither above nor below a threshold. Dark is the safe answer:
    /// a light that is wrong once beats one stuck on until the set ends.
    #[test]
    fn a_nonsense_reading_leaves_the_light_off() {
        let map = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "deck.1.playing"
            send = "note 1 0x0B"
            "#,
        )
        .unwrap();
        assert_eq!(map.lights[0].value(f32::NAN), 0);

        let range = FeedbackMap::parse(
            r#"
            [[feedback]]
            when = "deck.1.volume"
            send = "cc 1 0x20"
            min = 0.0
            max = 1.0
            "#,
        )
        .unwrap();
        assert_eq!(range.lights[0].value(f32::NAN), 0);
    }

    /// A mapping with no `[[feedback]]` blocks must parse rather than error:
    /// most controllers are input only, and a file that says nothing about
    /// lights is not a broken file.
    #[test]
    fn a_mapping_with_no_lights_is_fine() {
        let map = FeedbackMap::parse("name = \"nothing lit\"\ndevice = \"MIDI\"\n")
            .expect("a mapping may have no lights");
        assert!(map.is_empty());
    }

    /// The bundled MIDI mappings ship with lights, and every one of them has
    /// to name a parameter that exists and a control that can carry it --
    /// checked here rather than discovered on somebody's first launch.
    ///
    /// A HID mapping is exempt, and the exemption is real rather than an
    /// oversight: a `[[feedback]]` line is three MIDI bytes, and lighting a
    /// HID device means writing an output report of that device's own shape.
    /// There is no honest way to express one as the other, so a HID mapping
    /// with no lights is correct and this asserts that it also has none of the
    /// broken kind.
    #[test]
    fn the_bundled_controller_mapping_lights_up() {
        for (name, text) in crate::bundled::CONTROLLERS {
            let map = FeedbackMap::parse(text)
                .unwrap_or_else(|e| panic!("bundled mapping {name} has a broken light: {e}"));
            // The generic controller mapping is the one a DJ starts from, so
            // it has to demonstrate lights. The others are examples of one
            // thing each -- a motorised platter, a HID device, a script -- and
            // a light would be noise in them. What every mapping must do is
            // have no *broken* lights, which is the loop below.
            if *name == "generic-2-deck" {
                assert!(!map.is_empty(), "the mapping DJs start from lights nothing");
            }
            for light in &map.lights {
                assert!(
                    light.message(0).is_some(),
                    "{name} points a light at a control that cannot carry one"
                );
            }
        }
    }

    /// Every parameter the engine has must be nameable from a file, or a
    /// mapping could not light something the interface can show.
    #[test]
    fn every_parameter_can_be_named_in_a_mapping() {
        let by_name = parameters_by_name();
        assert_eq!(
            by_name.len(),
            ParamId::all().count(),
            "two parameters share a name, so one of them cannot be addressed"
        );
        for id in ParamId::all() {
            assert_eq!(by_name.get(&id.name()), Some(&id));
        }
    }

    /// The action grammar and the feedback names have to agree about which
    /// decks exist, or a mapping could light deck 5 and never drive it.
    #[test]
    fn the_decks_a_light_can_name_are_the_decks_an_action_can_name() {
        for n in 1..=dj_core::MAX_DECKS as u8 {
            let name = format!("deck.{n}.playing");
            assert!(
                parameters_by_name().contains_key(&name),
                "{name} is not addressable"
            );
            assert!(
                Action::parse(&format!("deck {n} play")).is_ok(),
                "deck {n} has a light but no action"
            );
        }
    }
}
