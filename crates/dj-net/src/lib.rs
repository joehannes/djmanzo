//! M7's transport-independent network boundary.
//!
//! UDP discovery, WebSocket and OSC adapters are deliberately thin shells over
//! this crate: untrusted text is parsed here, then becomes the exact same
//! [`dj_core::Action`] sent by every local input. No network parser may call the
//! engine directly.

pub mod announce;
pub mod control;
pub mod midi_clock;
pub mod osc;
pub mod page;
pub mod peer;
pub mod room;
pub mod server;
pub mod sticker;
pub mod tempo;
pub mod web;

pub use control::{ControlError, ControlRequest, ControlResponse, ControlService, ErrorCode};
pub use midi_clock::{MIDI_CLOCK_TICKS_PER_BEAT, MidiClockIn, MidiClockOut};
pub use osc::{OscError, OscServer};
pub use peer::{Announcement, PeerSync};
pub use server::{ControlServer, ServerError};
pub use tempo::PhaseFollower;
