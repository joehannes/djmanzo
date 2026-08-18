//! Hot cues and loops, as values.
//!
//! Both live here rather than inside the engine because three separate places
//! need to agree about them: the engine performs them, the interface draws
//! them, and — from M3 — the library persists them. A shared type is what stops
//! those three drifting into three slightly different ideas of what a loop is.

use crate::time::FramePos;
use serde::{Deserialize, Serialize};

/// Hot cues per deck.
///
/// Eight because that is what a pad row is, on every controller worth
/// supporting. More would be storage nobody can reach without a shift layer.
pub const HOT_CUE_SLOTS: usize = 8;

/// Shortest loop the engine will make.
///
/// Below about a sixteenth of a beat a loop stops being a loop and becomes a
/// pitched buzz — a real effect, but not this one, and reaching it by halving
/// four times in a row is almost always a mistake. At 128 BPM this is 29 ms.
pub const MIN_LOOP_BEATS: f64 = 1.0 / 16.0;

/// Longest loop, in beats. Thirty-two bars is already a section.
pub const MAX_LOOP_BEATS: f64 = 128.0;

/// A region of a track that repeats.
///
/// Held in frames rather than in beats because that is what the playhead is in,
/// and because a loop must survive a track having no beat grid at all — a
/// manual in/out loop on an unanalysed track is perfectly legitimate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LoopRegion {
    pub start: FramePos,
    pub end: FramePos,
}

impl LoopRegion {
    /// Build a region, or `None` if it is not a region.
    ///
    /// Rejects reversed and empty spans rather than repairing them: an end
    /// before its start means the caller's arithmetic is wrong, and silently
    /// swapping them would loop over something nobody asked for.
    ///
    /// Non-finite input needs no check of its own — `FramePos::new` clamps it
    /// to zero, so a NaN cannot reach here. The raw-`f64` methods below do
    /// check, because their arguments have not been through that gate.
    #[must_use]
    pub fn new(start: FramePos, end: FramePos) -> Option<Self> {
        if end.get() <= start.get() {
            return None;
        }
        Some(Self { start, end })
    }

    #[must_use]
    pub fn len_frames(&self) -> f64 {
        self.end.get() - self.start.get()
    }

    #[must_use]
    pub fn contains(&self, pos: FramePos) -> bool {
        pos.get() >= self.start.get() && pos.get() < self.end.get()
    }

    /// Fold a position into the loop.
    ///
    /// Modular rather than clamping, and that is the whole behaviour: a
    /// playhead that has run past the end by three frames must come back three
    /// frames after the start, not sit on the start. Clamping would stall the
    /// deck at one sample and produce a tone instead of a loop.
    ///
    /// Handles a position far outside in either direction, because a beat jump
    /// or a seek can put it there in one step.
    #[must_use]
    pub fn wrap(&self, pos: FramePos) -> FramePos {
        let len = self.len_frames();
        if len <= 0.0 || !pos.get().is_finite() {
            return self.start;
        }
        let offset = pos.get() - self.start.get();
        FramePos::new(self.start.get() + offset.rem_euclid(len))
    }

    /// The same loop, `factor` times as long, keeping its start.
    ///
    /// Start-anchored because that is what a DJ expects: halving a loop should
    /// tighten it onto the beat it began on, not creep the loop point forward.
    #[must_use]
    pub fn scaled(&self, factor: f64, limits: LoopLimits) -> Option<Self> {
        if !factor.is_finite() || factor <= 0.0 {
            return None;
        }
        let len = (self.len_frames() * factor).clamp(limits.min_frames, limits.max_frames);
        Self::new(self.start, FramePos::new(self.start.get() + len))
    }

    /// The same loop, moved by `frames`, keeping its length.
    #[must_use]
    pub fn moved(&self, frames: f64) -> Option<Self> {
        if !frames.is_finite() {
            return None;
        }
        Self::new(
            FramePos::new(self.start.get() + frames),
            FramePos::new(self.end.get() + frames),
        )
    }
}

/// How short and how long a loop may get, in frames.
///
/// Derived from the tempo where there is one, so "an eighth of a beat" means
/// the same musical thing at any tempo, and from the sample rate where there is
/// not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoopLimits {
    pub min_frames: f64,
    pub max_frames: f64,
}

impl LoopLimits {
    /// Limits for a deck with a beat grid.
    #[must_use]
    pub fn from_beat(beat_frames: f64) -> Self {
        Self {
            min_frames: beat_frames * MIN_LOOP_BEATS,
            max_frames: beat_frames * MAX_LOOP_BEATS,
        }
    }

    /// Limits for a deck with no grid, where a loop can still be set by hand.
    ///
    /// Ten milliseconds to a minute: wide enough never to get in the way,
    /// narrow enough that a mis-typed number cannot make a loop out of a whole
    /// track.
    #[must_use]
    pub fn from_rate(sample_rate: f64) -> Self {
        Self {
            min_frames: sample_rate * 0.01,
            max_frames: sample_rate * 60.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(start: f64, end: f64) -> LoopRegion {
        LoopRegion::new(FramePos::new(start), FramePos::new(end)).unwrap()
    }

    #[test]
    fn a_reversed_or_empty_region_is_not_a_region() {
        assert!(LoopRegion::new(FramePos::new(100.0), FramePos::new(50.0)).is_none());
        assert!(LoopRegion::new(FramePos::new(100.0), FramePos::new(100.0)).is_none());
    }

    /// A NaN cannot reach `LoopRegion` — `FramePos` sanitises first — so the
    /// invariant is pinned where it actually lives rather than guarded for
    /// redundantly here. A NaN *factor*, which has been through no such gate,
    /// is refused.
    #[test]
    fn non_finite_input_is_handled_where_it_can_actually_arrive() {
        assert_eq!(FramePos::new(f64::NAN).get(), 0.0);

        let r = region(1_000.0, 2_000.0);
        let limits = LoopLimits::from_rate(48_000.0);
        assert!(r.scaled(f64::NAN, limits).is_none());
        assert!(r.scaled(f64::INFINITY, limits).is_none());
        assert!(
            r.scaled(0.0, limits).is_none(),
            "a zero-length loop is not a loop"
        );
        assert!(r.moved(f64::NAN).is_none());
        // And wrapping a non-finite position falls back to the start rather
        // than propagating the NaN into the playhead.
        assert_eq!(r.wrap(FramePos::new(f64::NAN)).get(), 1_000.0);
    }

    /// **What makes a loop a loop.** Running three frames past the end must
    /// come back three frames after the start. Clamping instead would stall the
    /// playhead on one sample and produce a tone.
    #[test]
    fn wrapping_is_modular_rather_than_clamping() {
        let r = region(1_000.0, 2_000.0);
        assert_eq!(r.wrap(FramePos::new(2_003.0)).get(), 1_003.0);
        assert_eq!(r.wrap(FramePos::new(1_500.0)).get(), 1_500.0);
        assert_eq!(r.wrap(FramePos::new(1_000.0)).get(), 1_000.0);
    }

    /// A beat jump or a seek can land far outside in one step, including
    /// before the loop. `rem_euclid` is what makes the backward case land
    /// inside rather than negative.
    #[test]
    fn wrapping_survives_a_position_far_outside() {
        let r = region(1_000.0, 2_000.0);
        assert_eq!(r.wrap(FramePos::new(9_400.0)).get(), 1_400.0);
        assert_eq!(r.wrap(FramePos::new(-1_600.0)).get(), 1_400.0);
        assert!(r.contains(r.wrap(FramePos::new(-1_600.0))));
    }

    /// Halving and doubling keep the start, so a loop tightens onto the beat it
    /// began on instead of creeping forward.
    #[test]
    fn scaling_is_anchored_to_the_start() {
        let r = region(1_000.0, 3_000.0);
        let limits = LoopLimits::from_rate(48_000.0);

        let half = r.scaled(0.5, limits).unwrap();
        assert_eq!(half.start.get(), 1_000.0);
        assert_eq!(half.len_frames(), 1_000.0);

        let double = r.scaled(2.0, limits).unwrap();
        assert_eq!(double.start.get(), 1_000.0);
        assert_eq!(double.len_frames(), 4_000.0);
    }

    /// Halving four times in a row is nearly always a slip, and the result is a
    /// buzz rather than a loop. The floor stops it becoming one.
    #[test]
    fn a_loop_cannot_be_halved_into_a_buzz() {
        let limits = LoopLimits::from_beat(24_000.0);
        let mut r = region(0.0, 24_000.0);
        for _ in 0..10 {
            r = r.scaled(0.5, limits).unwrap();
        }
        assert!(
            r.len_frames() >= limits.min_frames - 1e-9,
            "halved down to {} frames, past the {} floor",
            r.len_frames(),
            limits.min_frames
        );
    }

    #[test]
    fn a_loop_cannot_be_doubled_past_its_ceiling() {
        let limits = LoopLimits::from_beat(24_000.0);
        let mut r = region(0.0, 24_000.0);
        for _ in 0..10 {
            r = r.scaled(2.0, limits).unwrap();
        }
        assert!(r.len_frames() <= limits.max_frames + 1e-9);
    }

    #[test]
    fn moving_keeps_the_length() {
        let r = region(1_000.0, 3_000.0);
        let moved = r.moved(500.0).unwrap();
        assert_eq!(moved.start.get(), 1_500.0);
        assert_eq!(moved.len_frames(), r.len_frames());
    }

    /// A loop's limits should mean the same musical thing at any tempo, which
    /// is why they are derived from the beat rather than fixed in frames.
    #[test]
    fn limits_scale_with_the_tempo() {
        let slow = LoopLimits::from_beat(48_000.0);
        let fast = LoopLimits::from_beat(24_000.0);
        assert!(slow.min_frames > fast.min_frames);
        assert_eq!(slow.min_frames / fast.min_frames, 2.0);
    }
}
