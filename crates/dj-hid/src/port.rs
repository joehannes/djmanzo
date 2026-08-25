//! Opening a MIDI port and pointing it at a mapping.
//!
//! # Where this sits
//!
//! Everything above this file is a pure function: a message goes in, action
//! text comes out, and none of it touches hardware. This is the thin layer
//! that does, and it is deliberately the only one — so the mapping engine can
//! be tested on a machine with nothing plugged in, which is most machines.
//!
//! # The callback thread
//!
//! The operating system delivers MIDI on a thread of its own. That thread is
//! *not* the audio thread, and the rule about never allocating does not apply
//! to it: a knob turn produces a `String`, which allocates, and that is fine
//! because nothing here feeds a buffer with a deadline. What matters instead is
//! that the callback never blocks — so it hands the action to a channel and
//! returns, and the application drains the channel on its own time.

use crate::mapping::Mapping;
use crate::message::Message;
use midir::{MidiInput, MidiInputConnection};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

/// What the application calls the client when it appears in a DJ's MIDI setup.
const CLIENT_NAME: &str = "djmanzo";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PortError {
    #[error("MIDI is not available on this machine: {0}")]
    Unavailable(String),
    #[error("no MIDI input called {0:?}")]
    NoSuchPort(String),
    #[error("could not open {0:?}: {1}")]
    Refused(String, String),
}

/// Every MIDI input the machine can see, by name.
///
/// # Errors
/// When the platform's MIDI service cannot be reached at all — no ALSA
/// sequencer in the container, no CoreMIDI on a stripped system.
pub fn inputs() -> Result<Vec<String>, PortError> {
    let midi = MidiInput::new(CLIENT_NAME).map_err(|e| PortError::Unavailable(e.to_string()))?;
    Ok(midi
        .ports()
        .iter()
        .filter_map(|port| midi.port_name(port).ok())
        .collect())
}

/// An open port. Dropping it closes the port and stops the callback.
///
/// Held by the application rather than detached, because a controller that
/// keeps sending after the DJ disconnected it is a controller nobody can turn
/// off.
pub struct Connection {
    /// Never read: it exists so the connection outlives this struct's owner
    /// and closes when they drop it. midir gives no other way to close one.
    _open: MidiInputConnection<Wiring>,
    port: String,
    mapping: String,
}

impl Connection {
    /// The port this is listening to, as the operating system names it.
    #[must_use]
    pub fn port(&self) -> &str {
        &self.port
    }

    /// The mapping in use.
    #[must_use]
    pub fn mapping(&self) -> &str {
        &self.mapping
    }
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("port", &self.port)
            .field("mapping", &self.mapping)
            .finish_non_exhaustive()
    }
}

/// What the callback owns and mutates.
struct Wiring {
    mapping: Mapping,
    /// The mapping's Lua, compiled here rather than carried in the `Mapping`.
    ///
    /// A `Mapping` is plain data -- cloned into the hub's list, serialised by
    /// the editor -- and a Lua state is neither cloneable nor serialisable. It
    /// belongs where it runs, which is this thread.
    script: Option<crate::script::Script>,
    out: Sender<String>,
    listener: Listener,
}

/// What the mapping editor watches the port for.
///
/// Shared with the callback rather than polled through a channel because the
/// question the editor asks -- "what did they just touch?" -- has one answer
/// at a time, and a queue of them would replay every pad a DJ brushed while
/// reaching for the right one.
#[derive(Clone, Debug, Default)]
pub struct Listener {
    /// While set, messages are described instead of translated.
    ///
    /// Suppressing the action matters: learning the play button by pressing
    /// the play button would otherwise start the deck, which is not what a DJ
    /// sitting down to map a controller wants to happen sixty times.
    learning: Arc<AtomicBool>,
    /// The last control seen while learning, as a mapping file would write it.
    seen: Arc<Mutex<Option<String>>>,
}

impl Listener {
    /// Start describing controls instead of acting on them.
    pub fn start(&self) {
        self.clear();
        self.learning.store(true, Ordering::Release);
    }

    /// Go back to acting on them.
    pub fn stop(&self) {
        self.learning.store(false, Ordering::Release);
    }

    #[must_use]
    pub fn is_learning(&self) -> bool {
        self.learning.load(Ordering::Acquire)
    }

    /// The last control touched since learning began, if any.
    #[must_use]
    pub fn seen(&self) -> Option<String> {
        self.seen.lock().ok()?.clone()
    }

    /// Forget it, so the next press is unambiguous.
    pub fn clear(&self) {
        if let Ok(mut slot) = self.seen.lock() {
            *slot = None;
        }
    }

    /// Called from the MIDI callback.
    fn note(&self, message: Message) -> bool {
        if !self.is_learning() {
            return false;
        }
        if let Ok(mut slot) = self.seen.lock() {
            *slot = Some(crate::editor::describe(message));
        }
        true
    }
}

/// Listen to `port` for MIDI clock, and report the sender's tempo.
///
/// Separate from [`open`] because it is a different job on a different cable:
/// a clock usually arrives from the thing that is *not* the DJ's controller,
/// and `open` deliberately tells midir to ignore realtime bytes so a
/// controller's own clock does not wake its callback several hundred times a
/// second for nothing.
///
/// `on_tempo` is called from the MIDI thread with each new estimate, and with
/// `None` when the sender stops. It must not block.
///
/// # Errors
/// When MIDI is unavailable, no port matches, or the platform refuses it.
pub fn listen_to_clock(
    port: &str,
    mut on_tempo: impl FnMut(Option<dj_core::Bpm>) + Send + 'static,
) -> Result<ClockConnection, PortError> {
    let mut midi =
        MidiInput::new(CLIENT_NAME).map_err(|e| PortError::Unavailable(e.to_string()))?;
    // The opposite of `open`: realtime is the *only* thing wanted here.
    midi.ignore(midir::Ignore::SysexAndTime);

    let ports = midi.ports();
    let found = ports
        .iter()
        .find(|candidate| {
            midi.port_name(candidate).is_ok_and(|name| {
                name == port || name.to_lowercase().contains(&port.to_lowercase())
            })
        })
        .ok_or_else(|| PortError::NoSuchPort(port.to_owned()))?
        .clone();
    let name = midi.port_name(&found).unwrap_or_else(|_| port.to_owned());

    let open = midi
        .connect(
            &found,
            CLIENT_NAME,
            move |_timestamp, bytes, following: &mut dj_net::MidiClockIn| {
                match bytes.first() {
                    // 0xF8 -- one pulse of twenty-four.
                    Some(0xF8) => on_tempo(following.tick(std::time::Instant::now())),
                    // Stop or start: the estimate is discarded rather than
                    // averaged across the gap, and the follower is told the
                    // room has no tempo rather than left on a stale one.
                    Some(0xFA | 0xFC) => {
                        *following = dj_net::MidiClockIn::default();
                        on_tempo(None);
                    }
                    _ => {}
                }
            },
            dj_net::MidiClockIn::default(),
        )
        .map_err(|e| PortError::Refused(name.clone(), e.to_string()))?;

    Ok(ClockConnection {
        _open: open,
        port: name,
    })
}

/// An open clock input. Dropping it stops listening.
pub struct ClockConnection {
    _open: MidiInputConnection<dj_net::MidiClockIn>,
    port: String,
}

impl ClockConnection {
    #[must_use]
    pub fn port(&self) -> &str {
        &self.port
    }
}

impl std::fmt::Debug for ClockConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClockConnection")
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

/// Open `port` and send everything it says, translated, down `out`.
///
/// The port is matched by [`Mapping::fits`] rules — loosely, on a substring —
/// because a device announces itself with a different suffix on every platform
/// and an exact match would work on the machine it was written on and nowhere
/// else.
///
/// # Errors
/// When MIDI is unavailable, no port matches, or the platform refuses to open
/// the one that does.
pub fn open(
    port: &str,
    mapping: Mapping,
    out: Sender<String>,
    listener: Listener,
) -> Result<Connection, PortError> {
    open_with(port, mapping, out, listener, None)
}

/// Open `port`, with `registry` readable from the mapping's script.
///
/// The plain [`open`] is this with no registry, which leaves a script able to
/// decide but not to look — enough for a shift key, not enough for "do
/// something different while the deck is playing".
///
/// # Errors
/// As [`open`], plus a script that does not compile — reported when the
/// mapping is chosen rather than when a pad is pressed, the same promise the
/// action text makes.
pub fn open_with(
    port: &str,
    mapping: Mapping,
    out: Sender<String>,
    listener: Listener,
    registry: Option<std::sync::Arc<dj_control::ParameterRegistry>>,
) -> Result<Connection, PortError> {
    // Compiled before the port opens, so a broken script is a message at the
    // moment a DJ chooses the mapping rather than a control that does nothing
    // an hour into a set.
    let script = match &mapping.script {
        Some(source) => Some(
            crate::script::Script::load(
                &mapping.name,
                source,
                registry
                    .unwrap_or_else(|| std::sync::Arc::new(dj_control::ParameterRegistry::new())),
            )
            .map_err(|e| PortError::Refused(mapping.name.clone(), e.to_string()))?,
        ),
        None => None,
    };

    let mut midi =
        MidiInput::new(CLIENT_NAME).map_err(|e| PortError::Unavailable(e.to_string()))?;
    // Without this, a controller's own clock — twenty-four messages a beat,
    // for as long as it is switched on — wakes the callback thread several
    // hundred times a second to be dropped by `Message::from_bytes`.
    midi.ignore(midir::Ignore::All);

    let ports = midi.ports();
    let found = ports
        .iter()
        .find(|candidate| {
            midi.port_name(candidate).is_ok_and(|name| {
                name == port || name.to_lowercase().contains(&port.to_lowercase())
            })
        })
        .ok_or_else(|| PortError::NoSuchPort(port.to_owned()))?
        .clone();

    let name = midi.port_name(&found).unwrap_or_else(|_| port.to_owned());
    let mapping_name = mapping.name.clone();

    let open = midi
        .connect(
            &found,
            CLIENT_NAME,
            |_timestamp, bytes, wiring: &mut Wiring| {
                let Some(message) = Message::from_bytes(bytes) else {
                    return;
                };
                // While the editor is listening, a control says what it is
                // instead of doing what it does.
                if wiring.listener.note(message) {
                    return;
                }
                // A scripted control is the script's, and the script decides
                // what it means -- including that it means nothing.
                if wiring.mapping.is_scripted(message) {
                    let Some(script) = &wiring.script else {
                        return;
                    };
                    let Some((control, event, value)) = wiring.mapping.script_event(message) else {
                        return;
                    };
                    match script.on_control(&control, event, value) {
                        Ok(actions) => {
                            for action in actions {
                                let _ = wiring.out.send(action);
                            }
                        }
                        Err(why) => {
                            // One line, not a dialog: a script that fails on
                            // one pad should not take the controller down, and
                            // the next press is a fresh call.
                            tracing::warn!(%why, %control, "a mapping script failed");
                        }
                    }
                    return;
                }

                for action in wiring.mapping.translate(message) {
                    // A full or disconnected channel means the application has
                    // gone away. There is nothing useful to do about it here
                    // and nowhere to report it to, so the message is dropped
                    // rather than blocking the operating system's MIDI thread.
                    let _ = wiring.out.send(action);
                }
            },
            Wiring {
                mapping,
                script,
                out,
                listener,
            },
        )
        .map_err(|e| PortError::Refused(name.clone(), e.to_string()))?;

    Ok(Connection {
        _open: open,
        port: name,
        mapping: mapping_name,
    })
}

/// The bundled mapping that fits `port`, if one does.
///
/// Used to pick a mapping automatically when a controller is plugged in, so
/// the common case needs no configuration at all.
#[must_use]
pub fn mapping_for(port: &str, mappings: &[Mapping]) -> Option<usize> {
    mappings.iter().position(|mapping| mapping.fits(port))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Listing ports must not panic on a machine with no MIDI at all, which is
    /// every continuous integration runner and this container. Either answer is
    /// correct; taking the process down is not.
    #[test]
    fn listing_ports_survives_a_machine_without_midi() {
        match inputs() {
            Ok(found) => println!("{} MIDI inputs", found.len()),
            Err(PortError::Unavailable(why)) => println!("no MIDI: {why}"),
            Err(other) => panic!("listing should not fail this way: {other}"),
        }
    }

    /// Opening a port that is not there is an error naming the port, not a
    /// panic and not a silent success — a DJ who mistyped a device name needs
    /// to be told which name did not match.
    #[test]
    fn opening_a_port_that_is_not_there_says_which() {
        let mapping = Mapping::parse(
            r#"
            name = "Test"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 play_pause"
            "#,
        )
        .unwrap();
        let (out, _keep) = std::sync::mpsc::channel();
        match open("a device nobody owns", mapping, out, Listener::default()) {
            Err(PortError::NoSuchPort(name)) => assert_eq!(name, "a device nobody owns"),
            // A machine with no MIDI service at all fails earlier, which is
            // also correct.
            Err(PortError::Unavailable(_)) => {}
            other => panic!("expected a missing port, got {other:?}"),
        }
    }

    #[test]
    fn a_bundled_mapping_is_found_by_the_port_name_the_platform_gives() {
        let mappings = crate::bundled::controllers().unwrap();
        // What ALSA calls a device, suffix and all.
        assert!(mapping_for("MIDI Mix:MIDI Mix MIDI 1 24:0", &mappings).is_some());
        assert!(mapping_for("Built-in Output", &mappings).is_none());
    }
}
