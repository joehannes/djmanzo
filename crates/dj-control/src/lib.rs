//! The control layer: how intent reaches the engine, and how state comes back.
//!
//! Two pieces, described in `docs/adr/0003-action-bus-and-parameter-registry.md`:
//!
//! - [`ActionBus`] carries typed [`dj_core::Action`]s from every input source
//!   into the engine along one ordered path, recording them as it goes.
//! - [`ParameterRegistry`] holds every observable value in a flat table of
//!   atomics that the audio thread can touch without locking.
//!
//! Nothing here spawns a thread or opens a device; it is the wiring, not the
//! hardware.

pub mod bus;
pub mod registry;

pub use bus::{ActionBus, BusFull, SessionLog, TimedAction};
pub use registry::ParameterRegistry;
