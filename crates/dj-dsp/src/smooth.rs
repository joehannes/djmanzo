//! Parameter smoothing.

/// A value that ramps toward its target instead of jumping.
///
/// Faders and gains arrive from the UI or a controller as discrete steps. Applying
/// them directly multiplies the signal by a discontinuous envelope, which is
/// audible as a click on every move -- "zipper noise". A short ramp removes it.
///
/// This is a one-pole filter: cheap, allocation-free, and it never overshoots.
#[derive(Debug, Clone, Copy)]
pub struct SmoothedValue {
    current: f32,
    target: f32,
    /// Per-sample coefficient, derived from the ramp time and sample rate.
    coefficient: f32,
    /// Below this distance we snap, so the value actually *arrives* rather than
    /// approaching asymptotically forever and denormalising.
    epsilon: f32,
}

impl SmoothedValue {
    /// Ramp time long enough to remove clicks, short enough to feel immediate.
    /// Around 10 ms is the usual choice for fader moves.
    pub const DEFAULT_RAMP_MS: f32 = 10.0;

    #[must_use]
    pub fn new(initial: f32, sample_rate: f32) -> Self {
        Self::with_ramp(initial, sample_rate, Self::DEFAULT_RAMP_MS)
    }

    #[must_use]
    pub fn with_ramp(initial: f32, sample_rate: f32, ramp_ms: f32) -> Self {
        let samples = (sample_rate * ramp_ms / 1000.0).max(1.0);
        Self {
            current: initial,
            target: initial,
            // Reaches ~63% of the way in `ramp_ms`, effectively arriving in 3-4x that.
            coefficient: 1.0 - (-1.0 / samples).exp(),
            epsilon: 1e-6,
        }
    }

    /// Set the destination. Cheap enough to call every callback.
    pub fn set_target(&mut self, target: f32) {
        if target.is_finite() {
            self.target = target;
        }
    }

    /// Jump immediately, skipping the ramp. For load and reset, not for faders.
    pub fn snap_to(&mut self, value: f32) {
        if value.is_finite() {
            self.current = value;
            self.target = value;
        }
    }

    #[must_use]
    pub fn target(&self) -> f32 {
        self.target
    }

    #[must_use]
    pub fn current(&self) -> f32 {
        self.current
    }

    /// True once the ramp has finished. Lets callers take a cheaper code path
    /// for the common case of a parameter that is not moving.
    #[must_use]
    pub fn is_settled(&self) -> bool {
        (self.target - self.current).abs() <= self.epsilon
    }

    /// Advance one sample and return the new value.
    #[must_use]
    pub fn next_value(&mut self) -> f32 {
        let delta = self.target - self.current;
        if delta.abs() <= self.epsilon {
            self.current = self.target;
            return self.current;
        }

        let next = self.current + delta * self.coefficient;
        // A one-pole ramp approaches its target geometrically, so the step keeps
        // shrinking. Near a target of 1.0 the step falls below one f32 ULP while
        // `delta` is still larger than `epsilon` -- at which point `current`
        // stops moving and the value would never settle. Detect that directly:
        // if the step no longer changes the value, we have converged as far as
        // the format allows, so snap.
        self.current = if next == self.current {
            self.target
        } else {
            next
        };
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn starts_settled_at_its_initial_value() {
        let mut s = SmoothedValue::new(0.5, SR);
        assert!(s.is_settled());
        assert_eq!(s.next_value(), 0.5);
    }

    #[test]
    fn ramps_toward_the_target_without_overshooting() {
        let mut s = SmoothedValue::new(0.0, SR);
        s.set_target(1.0);
        let mut previous = 0.0;
        for _ in 0..2_000 {
            let v = s.next_value();
            assert!(v >= previous, "ramp went backwards: {previous} -> {v}");
            assert!(v <= 1.0, "ramp overshot: {v}");
            previous = v;
        }
    }

    #[test]
    fn arrives_exactly_and_settles() {
        let mut s = SmoothedValue::with_ramp(0.0, SR, 5.0);
        s.set_target(1.0);
        // Well beyond the ramp time: 5 ms at 48 kHz is 240 samples.
        for _ in 0..10_000 {
            let _ = s.next_value();
        }
        assert!(s.is_settled());
        assert_eq!(s.current(), 1.0, "must land exactly on target, not near it");
    }

    #[test]
    fn ramp_takes_a_plausible_amount_of_time() {
        let mut s = SmoothedValue::with_ramp(0.0, SR, 10.0);
        s.set_target(1.0);
        // One time constant is 10 ms = 480 samples; expect ~63% by then.
        for _ in 0..480 {
            let _ = s.next_value();
        }
        assert!(
            (0.55..0.70).contains(&s.current()),
            "after one time constant, expected ~0.63, got {}",
            s.current()
        );
    }

    #[test]
    fn snap_bypasses_the_ramp() {
        let mut s = SmoothedValue::new(0.0, SR);
        s.snap_to(1.0);
        assert!(s.is_settled());
        assert_eq!(s.next_value(), 1.0);
    }

    #[test]
    fn non_finite_targets_are_ignored() {
        // A NaN reaching the audio path would poison every sample downstream and
        // the source would be almost impossible to find, so reject it at entry.
        let mut s = SmoothedValue::new(0.5, SR);
        s.set_target(f32::NAN);
        assert_eq!(s.target(), 0.5);
        s.set_target(f32::INFINITY);
        assert_eq!(s.target(), 0.5);
        s.snap_to(f32::NAN);
        assert_eq!(s.current(), 0.5);
    }

    #[test]
    fn ramps_downward_too() {
        let mut s = SmoothedValue::new(1.0, SR);
        s.set_target(0.0);
        for _ in 0..10_000 {
            let _ = s.next_value();
        }
        assert_eq!(s.current(), 0.0);
    }
}
