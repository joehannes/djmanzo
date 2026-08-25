//! Sending MIDI out: clock, transport and whatever else djmanzo has to say.
//!
//! The mirror of [`crate::port`]. Separate because they are separate devices
//! as far as the operating system is concerned -- a controller's input and
//! output ports are two entries in the same list and a DJ can want the clock
//! going somewhere else entirely, to a drum machine that sends nothing back.
//!
//! # Why a trait
//!
//! [`Sink`] exists so everything above it is testable on a machine with no
//! MIDI at all. That is the same four-layer arrangement the input side uses,
//! and it is what lets the clock's timing be proved in CI rather than by
//! plugging something in and listening.

use midir::{MidiOutput, MidiOutputConnection};

/// What djmanzo calls itself in a DJ's MIDI setup.
const CLIENT_NAME: &str = "djmanzo";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OutError {
    #[error("MIDI is not available on this machine: {0}")]
    Unavailable(String),
    #[error("no MIDI output called {0:?}")]
    NoSuchPort(String),
    #[error("could not open {0:?}: {1}")]
    Refused(String, String),
}

/// Somewhere bytes can be sent.
///
/// Failure is not reported. A clock tick that did not go out is gone -- there
/// is no retry that makes sense, because by the time anyone noticed the moment
/// it belonged to has passed, and a clock that stalls to complain is worse
/// than one that drops a byte.
pub trait Sink: Send {
    fn send(&mut self, message: &[u8]);
}

/// Every MIDI output the machine can see, by name.
///
/// # Errors
/// When the platform's MIDI service cannot be reached at all.
pub fn outputs() -> Result<Vec<String>, OutError> {
    let midi = MidiOutput::new(CLIENT_NAME).map_err(|e| OutError::Unavailable(e.to_string()))?;
    Ok(midi
        .ports()
        .iter()
        .filter_map(|port| midi.port_name(port).ok())
        .collect())
}

/// A real MIDI output. Dropping it closes the port.
pub struct Port {
    connection: MidiOutputConnection,
    name: String,
}

impl Port {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Debug for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Port").field("name", &self.name).finish()
    }
}

impl Sink for Port {
    fn send(&mut self, message: &[u8]) {
        let _ = self.connection.send(message);
    }
}

/// Open the output whose name contains `wanted`.
///
/// Loose matching, the same rule [`crate::Mapping::fits`] uses on the way in
/// and for the same reason: a device is spelled differently on every platform.
///
/// # Errors
/// When MIDI is unavailable, nothing matches, or the port refuses to open.
pub fn open(wanted: &str) -> Result<Port, OutError> {
    let midi = MidiOutput::new(CLIENT_NAME).map_err(|e| OutError::Unavailable(e.to_string()))?;
    let ports = midi.ports();
    let found = ports
        .iter()
        .find(|candidate| {
            midi.port_name(candidate).is_ok_and(|name| {
                name == wanted || name.to_lowercase().contains(&wanted.to_lowercase())
            })
        })
        .ok_or_else(|| OutError::NoSuchPort(wanted.to_owned()))?
        .clone();
    let name = midi.port_name(&found).unwrap_or_else(|_| wanted.to_owned());

    let connection = midi
        .connect(&found, CLIENT_NAME)
        .map_err(|e| OutError::Refused(name.clone(), e.to_string()))?;
    Ok(Port { connection, name })
}

/// The System Realtime bytes a clock master sends.
///
/// One byte each, and they may appear **between the bytes of another
/// message** -- which is why they are sent as their own tiny writes rather
/// than batched into whatever else is going out.
pub mod realtime {
    /// One pulse. Twenty-four to the quarter note, always.
    pub const CLOCK: u8 = 0xF8;
    /// Start from the top.
    pub const START: u8 = 0xFA;
    /// Carry on from where it stopped.
    pub const CONTINUE: u8 = 0xFB;
    /// Stop.
    pub const STOP: u8 = 0xFC;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Somewhere bytes can be sent that is not a device.
    #[derive(Debug, Default, Clone)]
    pub struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl Recorder {
        pub fn bytes(&self) -> Vec<u8> {
            self.0.lock().expect("not poisoned").clone()
        }
    }

    impl Sink for Recorder {
        fn send(&mut self, message: &[u8]) {
            self.0
                .lock()
                .expect("not poisoned")
                .extend_from_slice(message);
        }
    }

    #[test]
    fn listing_outputs_survives_a_machine_without_midi() {
        match outputs() {
            Ok(found) => {
                for name in found {
                    assert!(!name.is_empty());
                }
            }
            Err(OutError::Unavailable(why)) => assert!(!why.is_empty()),
            Err(other) => panic!("listing should not fail this way: {other}"),
        }
    }

    #[test]
    fn opening_an_output_that_is_not_there_says_which() {
        match open("not-a-real-midi-output-anywhere") {
            Err(OutError::NoSuchPort(name)) => {
                assert_eq!(name, "not-a-real-midi-output-anywhere");
            }
            Err(OutError::Unavailable(_)) => {}
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// The four bytes are fixed by the MIDI specification and getting one
    /// wrong means a drum machine that never starts. Worth pinning.
    #[test]
    fn the_realtime_bytes_are_what_the_specification_says() {
        assert_eq!(realtime::CLOCK, 0xF8);
        assert_eq!(realtime::START, 0xFA);
        assert_eq!(realtime::CONTINUE, 0xFB);
        assert_eq!(realtime::STOP, 0xFC);
    }

    #[test]
    fn a_recorder_collects_what_it_is_given() {
        let mut sink = Recorder::default();
        sink.send(&[realtime::START]);
        sink.send(&[realtime::CLOCK]);
        assert_eq!(sink.bytes(), vec![0xFA, 0xF8]);
    }
}
