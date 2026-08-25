//! Turning files on disk into audio the engine can play.
//!
//! [`decode_file`] reads a whole track into an [`AudioBuffer`]. The engine does
//! not depend on that: it is written against [`TrackSource`], so a streaming
//! reader can take over in M1 without the engine changing.

pub mod buffer;
pub mod decoder;
pub mod source;

pub use buffer::{AudioBuffer, CHANNELS, STEM_COUNT, StemBuffer, StemFrame};
pub use decoder::{DecodeError, DecodedTrack, decode_file};
pub use source::TrackSource;
