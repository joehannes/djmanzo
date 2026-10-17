//! §106: a controller, for a machine with none plugged in.
//!
//! The room surface's camera and microphone have stand-ins that go through
//! the browser's own capture path. A MIDI controller's path is the operating
//! system's MIDI service, then [`crate::port`]'s callback — and this machine
//! has no MIDI service at all (no ALSA sequencer, so not even a virtual port
//! can be opened). So the stand-in starts one step in: it hands raw MIDI bytes,
//! exactly as the service would deliver them, to **the same function** a real
//! port's callback runs (`port::deliver`) — the learning intercept, a
//! mapping's script, the table — and reads back the actions that come out of
//! the same channel.
//!
//! [`presses`] is the other half: every control of a mapping, pressed, turned
//! and swept the way a person checking a controller would, spelled as the
//! bytes the hardware sends — note-on, the note-on at velocity zero that many
//! controllers send instead of a note-off, a 14-bit fader's two control
//! changes, a relative encoder's step.
//!
//! # What this cannot tell you
//!
//! Whether a particular controller sends what its mapping says it does. The
//! bytes here are the mapping's own description played back, so a mapping
//! that names the wrong note passes; only the hardware can say otherwise. What
//! it does prove is that every binding a bundled mapping ships reaches an
//! action the vocabulary accepts, through the path a real port takes.

use crate::mapping::{Encoding, Mapping, Trigger};
use crate::port::{Listener, PortError, Wiring, compile, deliver};
use std::sync::mpsc::{Receiver, channel};

/// A controller that is not there, wired like one that is.
pub struct StandIn {
    wiring: Wiring,
    actions: Receiver<String>,
}

impl std::fmt::Debug for StandIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StandIn")
            .field("mapping", &self.wiring.mapping.name)
            .finish_non_exhaustive()
    }
}

impl StandIn {
    /// Wire `mapping` as [`crate::port::open`] would, without a port.
    ///
    /// # Errors
    /// As `open`: a mapping whose script does not compile.
    pub fn new(mapping: Mapping, listener: Listener) -> Result<Self, PortError> {
        let script = compile(&mapping, None)?;
        let (out, actions) = channel();
        Ok(Self {
            wiring: Wiring {
                mapping,
                script,
                out,
                listener,
            },
            actions,
        })
    }

    /// Deliver one MIDI message, as the operating system would, and return
    /// what it was translated into.
    pub fn send(&mut self, bytes: &[u8]) -> Vec<String> {
        deliver(bytes, &mut self.wiring);
        self.actions.try_iter().collect()
    }
}

/// One control, exercised: what was sent, and what each message is expected
/// to produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Press {
    /// The binding's `on`, e.g. `note 1 11`.
    pub on: String,
    /// Each message in order, with whether the mapping says it should act.
    pub messages: Vec<(Vec<u8>, bool)>,
}

/// The status byte for `kind` on a mapping's channel, which is written 1..=16.
fn status(kind: u8, channel: u8) -> u8 {
    kind | (channel.saturating_sub(1) & 0x0F)
}

/// Every MIDI control of `mapping`, exercised as a person checking it would.
///
/// HID fields are left out: a HID report is a whole device's state in one
/// packet, and `report`'s own tests drive those.
#[must_use]
pub fn presses(mapping: &Mapping) -> Vec<Press> {
    mapping
        .bindings
        .iter()
        .zip(mapping.triggers())
        .filter_map(|(binding, trigger)| {
            let messages = match *trigger {
                // A scripted control is the script's to answer, and a shift key
                // rightly answers nothing; only its actions are checked.
                Trigger::Note { channel, note } => {
                    vec![
                        (
                            vec![status(0x90, channel), note, 127],
                            !binding.script && binding.press.is_some(),
                        ),
                        // The note-off most controllers actually send.
                        (
                            vec![status(0x90, channel), note, 0],
                            !binding.script && binding.release.is_some(),
                        ),
                        (
                            vec![status(0x80, channel), note, 64],
                            !binding.script && binding.release.is_some(),
                        ),
                    ]
                }
                Trigger::Control {
                    channel,
                    controller,
                } => {
                    let cc = |value: u8| vec![status(0xB0, channel), controller, value];
                    if binding.turn_up.is_some() || binding.turn_down.is_some() {
                        let (up, down) = match binding.encoding.unwrap_or_default() {
                            Encoding::Signed => (1, 127),
                            Encoding::Offset => (65, 63),
                            Encoding::Absolute => (70, 60),
                        };
                        vec![
                            (cc(64), false),
                            (cc(up), binding.turn_up.is_some()),
                            (cc(down), binding.turn_down.is_some()),
                        ]
                    } else {
                        let acts = !binding.script
                            && (binding.moved.is_some() || binding.platter.is_some());
                        vec![(cc(0), acts), (cc(127), acts)]
                    }
                }
                Trigger::Control14 { channel, msb } => {
                    let acts = !binding.script && binding.moved.is_some();
                    vec![
                        (vec![status(0xB0, channel), msb, 127], acts),
                        (vec![status(0xB0, channel), msb + 32, 127], acts),
                    ]
                }
                Trigger::Bend { channel } => {
                    let acts =
                        !binding.script && (binding.moved.is_some() || binding.platter.is_some());
                    vec![
                        (vec![status(0xE0, channel), 0, 0], acts),
                        (vec![status(0xE0, channel), 127, 127], acts),
                    ]
                }
                Trigger::Hid(_) => return None,
            };
            Some(Press {
                on: binding.on.clone(),
                messages,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled() -> Vec<Mapping> {
        crate::bundled::controllers().expect("the bundled mappings load")
    }

    /// The number at the end of an action, for a control that moves.
    fn value_of(action: &str) -> f32 {
        action
            .rsplit(' ')
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| panic!("`{action}` ends in no number"))
    }

    /// **The load-bearing one: every control of every bundled controller
    /// reaches the vocabulary, through the path a real port takes.**
    ///
    /// Each binding is pressed, released (both ways a controller spells a
    /// release), turned or swept, as raw bytes delivered to the port's own
    /// callback body. A binding the mapping says acts must produce an action
    /// for it, and every action produced must be one `dj_core::Action::parse`
    /// accepts — the promise that a mapping can only say what the interface
    /// could.
    #[test]
    fn every_control_of_every_bundled_controller_reaches_the_vocabulary() {
        let mut pressed = 0;
        for mapping in bundled() {
            let name = mapping.name.clone();
            let presses = presses(&mapping);
            let mut standin = StandIn::new(mapping, Listener::default()).expect("wired");
            for press in presses {
                for (bytes, acts) in press.messages {
                    let actions = standin.send(&bytes);
                    if acts {
                        assert!(
                            !actions.is_empty(),
                            "{name}: `{}` sent {bytes:02x?} and nothing came out",
                            press.on
                        );
                    }
                    for action in &actions {
                        assert!(
                            dj_core::Action::parse(action).is_ok(),
                            "{name}: `{}` produced `{action}`, which the vocabulary refuses",
                            press.on
                        );
                    }
                    pressed += 1;
                }
            }
        }
        assert!(
            pressed > 100,
            "only {pressed} messages: the sweep found no controls"
        );
    }

    /// **A 14-bit fader moved one way reads one way.**
    ///
    /// A controller sends a high-resolution fader as two control changes,
    /// the high byte then the low. Swept slowly up and then down — as a
    /// pitch fader is when a DJ beatmatches — every value djmanzo acts on
    /// must be at least the last on the way up and at most it on the way
    /// down: a value that stepped back each time the high byte changed would
    /// nudge the tempo the wrong way, many times a sweep.
    #[test]
    fn a_fourteen_bit_fader_swept_one_way_reads_one_way() {
        let mut found = 0;
        for mapping in bundled() {
            let faders: Vec<(String, u8, u8)> = mapping
                .bindings
                .iter()
                .zip(mapping.triggers())
                .filter_map(|(binding, trigger)| match *trigger {
                    Trigger::Control14 { channel, msb }
                        if binding.moved.is_some() && !binding.script =>
                    {
                        Some((binding.on.clone(), channel, msb))
                    }
                    _ => None,
                })
                .collect();
            let name = mapping.name.clone();
            let mut standin = StandIn::new(mapping, Listener::default()).expect("wired");
            for (on, channel, msb) in faders {
                found += 1;
                let mut last: Option<f32> = None;
                // Up, then back down over exactly the same positions.
                let up: Vec<u16> = (2_000..=14_000).step_by(37).collect();
                let turn = up.len();
                let positions: Vec<u16> = up.iter().chain(up.iter().rev()).copied().collect();
                for (step, position) in positions.into_iter().enumerate() {
                    let high = u8::try_from(position >> 7).expect("seven bits");
                    let low = u8::try_from(position & 0x7F).expect("seven bits");
                    let up = step < turn;
                    for bytes in [
                        [status(0xB0, channel), msb, high],
                        [status(0xB0, channel), msb + 32, low],
                    ] {
                        for action in standin.send(&bytes) {
                            let value = value_of(&action);
                            if let Some(before) = last {
                                assert!(
                                    if up {
                                        value >= before - 1e-6
                                    } else {
                                        value <= before + 1e-6
                                    },
                                    "{name}: `{on}` swept {} read {value} after {before}",
                                    if up { "up" } else { "down" }
                                );
                            }
                            last = Some(value);
                        }
                    }
                }
            }
        }
        assert!(found > 0, "no bundled mapping has a 14-bit fader to sweep");
    }

    /// **A shift key changes what a pad does, through the mapping's script.**
    /// The one decision a table cannot describe, driven as bytes: the pad
    /// alone jumps to its hot cue; with shift held it sets one; shift let go,
    /// it jumps again.
    #[test]
    fn a_shift_key_changes_what_a_pad_does() {
        let mapping = bundled()
            .into_iter()
            .find(|m| m.script.is_some())
            .expect("a scripted mapping ships");
        let mut standin = StandIn::new(mapping, Listener::default()).expect("wired");
        let pad = [0x90, 0x01, 127];
        assert_eq!(standin.send(&pad), ["deck 1 hotcue 1"]);
        assert!(
            standin.send(&[0x90, 0x3f, 127]).is_empty(),
            "shift itself acted"
        );
        assert_eq!(standin.send(&pad), ["deck 1 hotcue_set 1"]);
        assert!(standin.send(&[0x90, 0x3f, 0]).is_empty());
        assert_eq!(standin.send(&pad), ["deck 1 hotcue 1"]);
    }

    /// **While the mapping editor is listening, a control says what it is and
    /// does nothing** — pressing play to learn it must not start the deck.
    #[test]
    fn a_control_touched_while_learning_is_described_not_acted_on() {
        let mapping = bundled()
            .into_iter()
            .find(|m| m.name.contains("2-deck") || m.name.contains("Generic"))
            .expect("a generic mapping ships");
        let press = presses(&mapping)
            .into_iter()
            .find(|p| p.messages.first().is_some_and(|(_, acts)| *acts))
            .expect("a control that acts");
        let listener = Listener::default();
        let mut standin = StandIn::new(mapping, listener.clone()).expect("wired");
        listener.start();
        let (bytes, _) = &press.messages[0];
        assert!(
            standin.send(bytes).is_empty(),
            "learning acted on `{}`",
            press.on
        );
        assert!(
            listener.seen().is_some(),
            "learning did not see `{}`",
            press.on
        );
        listener.stop();
        assert!(
            !standin.send(bytes).is_empty(),
            "`{}` did nothing after learning",
            press.on
        );
    }

    /// **What else a busy controller sends is ignored, never misread.**
    /// Clock, sysex, aftertouch, a program change, a message cut short and
    /// an empty one, into every bundled mapping: no action, no panic.
    #[test]
    fn what_else_a_controller_sends_is_ignored() {
        let noise: [&[u8]; 8] = [
            &[0xF8],
            &[0xFA],
            &[0xF0, 0x47, 0x7F, 0x10, 0xF7],
            &[0xA0, 36, 90],
            &[0xD0, 90],
            &[0xC0, 5],
            &[0x90],
            &[],
        ];
        for mapping in bundled() {
            let name = mapping.name.clone();
            let mut standin = StandIn::new(mapping, Listener::default()).expect("wired");
            for bytes in noise {
                assert!(
                    standin.send(bytes).is_empty(),
                    "{name} acted on {bytes:02x?}"
                );
            }
        }
    }
}
