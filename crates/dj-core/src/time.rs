//! Positions, rates and sample rates.
//!
//! Everything the engine does is expressed in **frames** (one frame = one sample
//! per channel). Positions are `f64` rather than integers because a deck under
//! scratch control sits between frames constantly, and rounding to the nearest
//! frame on every callback is audible as jitter.

use serde::{Deserialize, Serialize};

/// Samples per second. Non-zero by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SampleRate(u32);

impl SampleRate {
    /// The rate the engine mixes at unless a device forces otherwise.
    pub const DEFAULT: SampleRate = SampleRate(48_000);

    /// Returns `None` for a rate of zero, which would make every conversion
    /// below produce infinities.
    #[must_use]
    pub const fn new(hz: u32) -> Option<Self> {
        if hz == 0 { None } else { Some(Self(hz)) }
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn as_f64(self) -> f64 {
        f64::from(self.0)
    }

    /// How many frames fit in `seconds` at this rate.
    #[must_use]
    pub fn frames_in(self, seconds: f64) -> f64 {
        seconds * self.as_f64()
    }
}

impl Default for SampleRate {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A position within a track, in frames from the start.
///
/// Fractional by design: at a playback rate of 1.03 the playhead lands between
/// frames on almost every callback, and during a scratch it moves backwards.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
pub struct FramePos(f64);

impl FramePos {
    pub const ZERO: FramePos = FramePos(0.0);

    /// Non-finite input is clamped to zero rather than propagating NaN into the
    /// audio path, where it would silently poison every downstream sample.
    #[must_use]
    pub fn new(frames: f64) -> Self {
        if frames.is_finite() {
            Self(frames)
        } else {
            Self::ZERO
        }
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }

    #[must_use]
    pub fn seconds(self, rate: SampleRate) -> f64 {
        self.0 / rate.as_f64()
    }

    #[must_use]
    pub fn from_seconds(seconds: f64, rate: SampleRate) -> Self {
        Self::new(seconds * rate.as_f64())
    }

    /// Advance by `frames`, which may be negative (reverse, or a backspin).
    #[must_use]
    pub fn advanced_by(self, frames: f64) -> Self {
        Self::new(self.0 + frames)
    }

    /// Clamp into `[0, len]`. Used at buffer edges so a deck stops cleanly
    /// instead of reading out of bounds.
    #[must_use]
    pub fn clamped(self, len_frames: f64) -> Self {
        Self(self.0.clamp(0.0, len_frames.max(0.0)))
    }
}

/// Playback speed as a multiple of the track's natural rate.
///
/// `1.0` is normal, `0.0` is stopped, negatives play backwards. The upper bound
/// exists because a runaway rate would make the reader stride past the end of
/// its buffer in a single callback.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Rate(f64);

impl Rate {
    pub const STOPPED: Rate = Rate(0.0);
    pub const NORMAL: Rate = Rate(1.0);

    /// Fast enough for any scratch a human can perform; Serato timecode tops out
    /// around 22x, and we allow a little headroom above that.
    pub const MAX_ABS: f64 = 32.0;

    #[must_use]
    pub fn new(rate: f64) -> Self {
        if rate.is_finite() {
            Self(rate.clamp(-Self::MAX_ABS, Self::MAX_ABS))
        } else {
            Self::STOPPED
        }
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }

    #[must_use]
    pub fn is_stopped(self) -> bool {
        self.0 == 0.0
    }
}

impl Default for Rate {
    fn default() -> Self {
        Self::NORMAL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_rate_rejects_zero() {
        assert!(SampleRate::new(0).is_none());
        assert_eq!(SampleRate::new(44_100).unwrap().get(), 44_100);
    }

    #[test]
    fn frame_pos_rejects_non_finite() {
        assert_eq!(FramePos::new(f64::NAN), FramePos::ZERO);
        assert_eq!(FramePos::new(f64::INFINITY), FramePos::ZERO);
        assert_eq!(FramePos::new(-f64::INFINITY), FramePos::ZERO);
    }

    #[test]
    fn seconds_round_trip() {
        let rate = SampleRate::new(48_000).unwrap();
        let pos = FramePos::from_seconds(2.5, rate);
        assert_eq!(pos.get(), 120_000.0);
        assert!((pos.seconds(rate) - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn position_clamps_to_buffer() {
        assert_eq!(FramePos::new(-5.0).clamped(100.0).get(), 0.0);
        assert_eq!(FramePos::new(500.0).clamped(100.0).get(), 100.0);
        assert_eq!(FramePos::new(50.0).clamped(100.0).get(), 50.0);
    }

    #[test]
    fn rate_clamps_and_allows_reverse() {
        assert_eq!(Rate::new(-2.0).get(), -2.0);
        assert_eq!(Rate::new(1e9).get(), Rate::MAX_ABS);
        assert_eq!(Rate::new(-1e9).get(), -Rate::MAX_ABS);
        assert!(Rate::new(f64::NAN).is_stopped());
    }
}
