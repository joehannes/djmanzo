//! Controllers, translated into the one action vocabulary.
//!
//! # What this crate is, and is not
//!
//! It turns a message from a piece of hardware into action text —
//! `deck 1 play_pause`, `deck 2 volume 0.61` — and nothing else. It does not
//! know what a deck is, cannot reach the engine, and holds no state about the
//! mix. [ADR-0003](../../docs/adr/0003-action-bus-and-parameter-registry.md)
//! says the interface, controllers, scripts and the assistant all speak one
//! language; this is the part that teaches a controller to speak it.
//!
//! The consequence worth stating: **a mapping cannot do anything the interface
//! cannot.** A file from a stranger can rebind every control on their
//! controller and cannot invent a capability, because everything it can say has
//! to survive [`dj_core::Action::parse`].
//!
//! # Why the translation is a pure function
//!
//! `Mapping::translate` takes a message and returns strings. No I/O, no
//! hardware, no clock. That is what lets the whole mapping layer — 7-bit and
//! 14-bit controls, encoders, toggles, shift layers — be tested on a machine
//! with nothing plugged into it, which is most machines most of the time.
//!
//! Opening a MIDI port is a separate, thin thing that sits on top.

pub mod bundled;
pub mod feedback;
pub mod keys;
pub mod mapping;
pub mod message;
pub mod port;

pub use keys::{Chord, KeyBinding, KeyError, KeyMap};
pub use mapping::{Binding, Mapping, MappingError, Trigger};
pub use message::Message;
pub use port::{Connection, PortError};
