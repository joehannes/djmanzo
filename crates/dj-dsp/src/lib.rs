//! DSP primitives for the realtime path.
//!
//! Everything in this crate obeys the audio-thread rules: no allocation, no
//! locking, no I/O, no panics. Types are constructed with their buffers already
//! sized, and `process` only does arithmetic.

pub mod biquad;
pub mod eq;
pub mod fx;
pub mod karaoke;
pub mod keylock;
pub mod limiter;
pub mod meter;
pub mod mixer;
pub mod smooth;

pub use biquad::Biquad;
pub use eq::{SweepFilter, ThreeBandEq};
pub use fx::{DelayLine, FxContext, Slot};
pub use karaoke::CentreCancel;
pub use keylock::Keylock;
pub use limiter::Limiter;
pub use meter::PeakMeter;
pub use mixer::{CrossfaderCurve, crossfader_gains};
pub use smooth::SmoothedValue;

/// Interleaved stereo audio, borrowed for the length of one callback.
///
/// The engine works in stereo throughout. Mono sources are duplicated at load
/// time rather than special-cased in every processor.
pub const CHANNELS: usize = 2;
