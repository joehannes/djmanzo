//! Timecode vinyl: turning a control record into a position and a speed.
//!
//! # What this is
//!
//! A DJ puts a control record on a real turntable and djmanzo plays the track.
//! The record carries no music — it carries a **timecode signal**, and the
//! turntable's output arrives here as ordinary stereo audio. What comes back
//! out is where the needle is and how fast it is moving.
//!
//! # Licensing
//!
//! **No timecode code was read to write this.** The established open
//! implementation, `xwax`, is GPL-2.0, and ADR-0002 forbids linking or copying
//! it — including reading it closely enough to reproduce. This decoder is
//! written from published *prose* descriptions of how the format works, which
//! `docs/RESEARCH.md` records as safe input.
//!
//! The numbers describing a particular record — carrier frequency, register
//! width, seed and taps — are facts about a pressed disc rather than anybody's
//! expression, and they are held as **configuration** in [`format`] rather than
//! baked into the decoder, so a new pressing is a table entry and not a
//! release.
//!
//! # What is proven here, and what is not
//!
//! Everything in this crate is verified against a signal this crate generates:
//! [`Synth`] writes a timecode and [`Decoder`] reads it back, so the encoding,
//! the register, the quadrature and the position lookup are all pinned by
//! round-trip tests over synthetic audio.
//!
//! **That is not the same as being verified against a real record.** The bit
//! ordering, the register's direction of travel and the sense of the quadrature
//! are conventions, and a convention agreed with oneself is agreed with nobody.
//! Until this has been run against an actual Serato or Traktor pressing on a
//! real turntable, treat compatibility as unproven — the interface says so, and
//! so does the roadmap.

pub mod decode;
pub mod format;
pub mod lfsr;
pub mod synth;

pub use decode::{Decoder, Reading};
pub use format::TimecodeFormat;
pub use lfsr::Lfsr;
pub use synth::Synth;
