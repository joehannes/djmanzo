//! The realtime mixing engine.
//!
//! # The rule
//!
//! Code in this crate runs inside the audio callback. It must not allocate,
//! lock, perform I/O, log, or panic. A single violation is a dropout, and a
//! dropout in front of an audience is the worst bug this application can have.
//!
//! Two consequences shape the API:
//!
//! - Everything is sized in [`Engine::new`], before audio flows.
//! - The engine **never drops an `Arc`**. Displaced track buffers go out through
//!   the retirement queue for the host thread to free. See
//!   [`command::Retired`].
//!
//! The dependency surface is kept deliberately small -- `dj-core`, `dj-dsp`,
//! `dj-decode`, `dj-control`, `rtrb` -- so this crate stays auditable.

pub mod bus;
pub mod command;
pub mod deck;
pub mod engine;
pub mod jog;
pub mod mic;
pub mod rack;
pub mod record;
pub mod sampler;

pub use bus::{BusLayout, BusRouting, STEM_OUT_CHANNELS};
pub use command::{Command, Retired};
pub use deck::{Deck, DeckLevels};
pub use engine::Engine;
pub use record::{Capture, Recorder};
