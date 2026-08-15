//! Band-limited centre cancellation.
//!
//! The oldest karaoke trick: a lead vocal is normally panned dead centre, so
//! subtracting one channel from the other cancels it.
//!
//! Done naively it is bad, and it is worth being precise about why. Everything
//! centred cancels with the voice -- kick, snare, bass, usually the lead
//! instrument. The result is thin and hollow, and on a dance floor the missing
//! kick is fatal.
//!
//! So this does not do the naive version. The signal is split by frequency and
//! the centre is cancelled **only in the vocal band**, leaving centred low end
//! and top intact:
//!
//! ```text
//!   input ─┬─ below 200 Hz ────────────────────── centre kept  ─┐
//!          ├─ 200 Hz – 8 kHz ─→ mid/side ─→ drop mid ──────────┼─→ sum
//!          └─ above 8 kHz ───────────────────────  centre kept ─┘
//! ```
//!
//! Kick and bass survive, cymbals keep their air, and the voice mostly goes.
//! Still worse than neural stem separation -- anything else centred in that band
//! goes too -- but it costs almost nothing, needs no model, no GPU and no cache,
//! and works instantly on a track dragged in thirty seconds ago.
//!
//! See `docs/KARAOKE.md` for where this sits among the alternatives.

use crate::biquad::Biquad;
use crate::smooth::SmoothedValue;
use std::f32::consts::FRAC_1_SQRT_2;

/// Where the protected low end ends.
pub const DEFAULT_LOW_HZ: f32 = 200.0;
/// Where the protected top begins.
pub const DEFAULT_HIGH_HZ: f32 = 8_000.0;

/// Stereo centre cancellation, restricted to a frequency band.
#[derive(Debug, Clone)]
pub struct CentreCancel {
    /// Low band extraction, per channel.
    low: [Biquad; 2],
    /// High band extraction, per channel.
    high: [Biquad; 2],
    /// 0.0 is bypass, 1.0 removes the centre completely.
    depth: SmoothedValue,
    depth_target: f32,
    low_hz: f32,
    high_hz: f32,
    sample_rate: f32,
}

impl CentreCancel {
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let mut cancel = Self {
            low: [Biquad::identity(); 2],
            high: [Biquad::identity(); 2],
            depth: SmoothedValue::new(0.0, sample_rate),
            depth_target: 0.0,
            low_hz: DEFAULT_LOW_HZ,
            high_hz: DEFAULT_HIGH_HZ,
            sample_rate,
        };
        cancel.retune();
        cancel
    }

    fn retune(&mut self) {
        let low = Biquad::low_pass(self.sample_rate, self.low_hz, FRAC_1_SQRT_2);
        let high = Biquad::high_pass(self.sample_rate, self.high_hz, FRAC_1_SQRT_2);
        for channel in 0..2 {
            self.low[channel].set_coefficients_from(&low);
            self.high[channel].set_coefficients_from(&high);
        }
    }

    /// How much of the centre to remove. `0.0` is bypass, `1.0` is full.
    ///
    /// Partial depths are genuinely useful: around `0.7` often removes enough
    /// voice to sing over while leaving the track sounding less gutted.
    pub fn set_depth(&mut self, depth: f32) {
        if depth.is_finite() {
            self.depth_target = depth.clamp(0.0, 1.0);
            self.depth.set_target(self.depth_target);
        }
    }

    /// Move the protected band edges.
    ///
    /// The ideal band depends on the singer and the arrangement: a baritone
    /// needs a lower bottom edge, a dense mix a narrower band.
    pub fn set_band(&mut self, low_hz: f32, high_hz: f32) {
        if !low_hz.is_finite() || !high_hz.is_finite() {
            return;
        }
        let nyquist = self.sample_rate * 0.5;
        self.low_hz = low_hz.clamp(20.0, nyquist * 0.9);
        // Keep the band non-degenerate: crossing the edges over would make the
        // "vocal band" empty or inverted.
        self.high_hz = high_hz.clamp(self.low_hz * 1.5, nyquist * 0.98);
        self.retune();
    }

    #[must_use]
    pub fn depth(&self) -> f32 {
        self.depth_target
    }

    #[must_use]
    pub fn band(&self) -> (f32, f32) {
        (self.low_hz, self.high_hz)
    }

    /// True when the effect is off and settled, so the caller can skip it.
    #[must_use]
    pub fn is_bypassed(&self) -> bool {
        self.depth_target == 0.0 && self.depth.is_settled()
    }

    pub fn reset(&mut self) {
        for filter in self.low.iter_mut().chain(self.high.iter_mut()) {
            filter.reset();
        }
    }

    /// Process one stereo frame.
    #[must_use]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let depth = self.depth.next_value();
        if depth <= 0.0 {
            return (left, right);
        }

        let low_l = self.low[0].process(left);
        let low_r = self.low[1].process(right);
        let high_l = self.high[0].process(left);
        let high_r = self.high[1].process(right);

        // The vocal band is defined as the *residual*, not as a third filter.
        // That makes reconstruction exact: low + high + band == input, always,
        // so at depth zero the effect is bit-transparent and there is no comb
        // filtering from three filter paths failing to sum flat.
        let band_l = left - low_l - high_l;
        let band_r = right - low_r - high_r;

        let mid = (band_l + band_r) * 0.5;
        let cancelled_l = band_l - mid * depth;
        let cancelled_r = band_r - mid * depth;

        (low_l + high_l + cancelled_l, low_r + high_r + cancelled_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const SR: f32 = 48_000.0;

    fn settled(depth: f32) -> CentreCancel {
        let mut cancel = CentreCancel::new(SR);
        cancel.set_depth(depth);
        // Run the depth ramp out.
        for _ in 0..10_000 {
            let _ = cancel.process(0.0, 0.0);
        }
        cancel
    }

    /// Centre signal: identical in both channels.
    fn measure_centre(cancel: &mut CentreCancel, frequency: f32) -> f32 {
        let mut peak = 0.0f32;
        for n in 0..16_000 {
            let sample = (2.0 * PI * frequency * n as f32 / SR).sin() * 0.5;
            let (l, r) = cancel.process(sample, sample);
            if n >= 8_000 {
                peak = peak.max(l.abs()).max(r.abs());
            }
        }
        peak
    }

    /// Hard-left signal: present in one channel only.
    fn measure_panned(cancel: &mut CentreCancel, frequency: f32) -> f32 {
        let mut peak = 0.0f32;
        for n in 0..16_000 {
            let sample = (2.0 * PI * frequency * n as f32 / SR).sin() * 0.5;
            let (l, r) = cancel.process(sample, 0.0);
            if n >= 8_000 {
                peak = peak.max(l.abs()).max(r.abs());
            }
        }
        peak
    }

    #[test]
    fn bypass_is_exactly_transparent() {
        let mut cancel = CentreCancel::new(SR);
        assert!(cancel.is_bypassed());
        for input in [0.1f32, -0.7, 0.9, 0.0] {
            let (l, r) = cancel.process(input, input * 0.5);
            assert_eq!(l, input, "bypass altered the left channel");
            assert_eq!(r, input * 0.5, "bypass altered the right channel");
        }
    }

    /// The point of the whole module: a centred vocal-band tone should go.
    #[test]
    fn centred_vocal_band_is_cancelled() {
        let mut cancel = settled(1.0);
        let level = measure_centre(&mut cancel, 1_000.0);
        assert!(
            level < 0.05,
            "a centred 1 kHz tone should be cancelled, peak was {level}"
        );
    }

    /// The improvement over the naive version, and the reason it is worth
    /// building: centred bass must survive, or the kick disappears.
    #[test]
    fn centred_bass_survives() {
        let mut cancel = settled(1.0);
        let level = measure_centre(&mut cancel, 60.0);
        assert!(
            level > 0.4,
            "centred 60 Hz must survive -- losing the kick is fatal on a floor. \
             Peak was {level}, expected near 0.5"
        );
    }

    #[test]
    fn centred_treble_survives() {
        let mut cancel = settled(1.0);
        let level = measure_centre(&mut cancel, 14_000.0);
        assert!(
            level > 0.35,
            "centred 14 kHz should mostly survive, peak was {level}"
        );
    }

    #[test]
    fn panned_material_survives() {
        let mut cancel = settled(1.0);
        let level = measure_panned(&mut cancel, 1_000.0);
        assert!(
            level > 0.2,
            "a hard-panned 1 kHz tone must survive cancellation, peak was {level}"
        );
    }

    #[test]
    fn partial_depth_reduces_rather_than_removes() {
        let mut full = settled(1.0);
        let full_level = measure_centre(&mut full, 1_000.0);

        let mut half = settled(0.5);
        let half_level = measure_centre(&mut half, 1_000.0);

        let mut off = settled(0.0);
        let off_level = measure_centre(&mut off, 1_000.0);

        assert!(
            full_level < half_level && half_level < off_level,
            "depth should be monotonic: full {full_level}, half {half_level}, off {off_level}"
        );
    }

    #[test]
    fn depth_changes_are_smoothed() {
        let mut cancel = CentreCancel::new(SR);
        // Establish a steady signal first.
        for _ in 0..1_000 {
            let _ = cancel.process(0.5, 0.5);
        }
        cancel.set_depth(1.0);
        let (mut previous, _) = cancel.process(0.5, 0.5);
        for _ in 0..2_000 {
            let (current, _) = cancel.process(0.5, 0.5);
            assert!(
                (current - previous).abs() < 0.05,
                "depth jumped from {previous} to {current}"
            );
            previous = current;
        }
    }

    #[test]
    fn band_edges_are_adjustable() {
        let mut cancel = CentreCancel::new(SR);
        cancel.set_band(120.0, 6_000.0);
        assert_eq!(cancel.band(), (120.0, 6_000.0));
    }

    /// Crossed-over edges would make the "vocal band" empty or inverted.
    #[test]
    fn band_edges_cannot_cross() {
        let mut cancel = CentreCancel::new(SR);
        cancel.set_band(5_000.0, 1_000.0);
        let (low, high) = cancel.band();
        assert!(high > low, "band collapsed: {low} .. {high}");
    }

    #[test]
    fn extreme_and_non_finite_input_is_handled() {
        let mut cancel = CentreCancel::new(SR);
        cancel.set_depth(f32::NAN);
        assert_eq!(cancel.depth(), 0.0, "NaN depth must be ignored");

        cancel.set_depth(5.0);
        assert_eq!(cancel.depth(), 1.0, "depth must clamp");

        cancel.set_band(f32::NAN, f32::INFINITY);
        let (low, high) = cancel.band();
        assert!(low.is_finite() && high.is_finite());

        for _ in 0..1_000 {
            let (l, r) = cancel.process(0.5, -0.5);
            assert!(l.is_finite() && r.is_finite());
        }
    }

    #[test]
    fn reset_clears_filter_memory() {
        let mut cancel = settled(1.0);
        for _ in 0..1_000 {
            let _ = cancel.process(1.0, -1.0);
        }
        cancel.reset();
        // Nothing should blow up, and the next samples start clean.
        let (l, r) = cancel.process(0.0, 0.0);
        assert!(l.abs() < 1e-3 && r.abs() < 1e-3, "state survived reset");
    }

    /// A realistic case: voice centred in the band, bass centred below it,
    /// guitar panned hard left.
    ///
    /// Measured against a bypassed run rather than an absolute threshold,
    /// because the residual legitimately contains the surviving bass plus the
    /// half of the panned guitar that centre cancellation always bleeds into
    /// the opposite channel. An absolute number would be asserting against
    /// those, not against the voice.
    #[test]
    fn a_mixed_signal_loses_the_voice_and_keeps_the_rest() {
        // Each probe needs its own instance: the filters carry state, so
        // interleaving two different signals through one of them would corrupt
        // both measurements.
        fn run_mix(depth: f32) -> f32 {
            let mut cancel = settled(depth);
            let mut peak = 0.0f32;
            for n in 0..24_000 {
                let t = n as f32 / SR;
                let bass = (2.0 * PI * 60.0 * t).sin() * 0.4; // centred
                let voice = (2.0 * PI * 900.0 * t).sin() * 0.4; // centred
                let guitar = (2.0 * PI * 2_000.0 * t).sin() * 0.3; // hard left

                let (_l, r) = cancel.process(bass + voice + guitar, bass + voice);
                if n >= 12_000 {
                    peak = peak.max(r.abs());
                }
            }
            peak
        }

        fn run_bass_only(depth: f32) -> f32 {
            let mut cancel = settled(depth);
            let mut peak = 0.0f32;
            for n in 0..24_000 {
                let bass = (2.0 * PI * 60.0 * n as f32 / SR).sin() * 0.4;
                let (_l, r) = cancel.process(bass, bass);
                if n >= 12_000 {
                    peak = peak.max(r.abs());
                }
            }
            peak
        }

        let bypassed = run_mix(0.0);
        let cancelled = run_mix(1.0);
        let bass_only = run_bass_only(1.0);

        assert!(
            cancelled < bypassed * 0.85,
            "cancellation should measurably reduce the mix: {bypassed} -> {cancelled}"
        );
        assert!(
            bass_only > 0.35,
            "centred bass must come through at close to its 0.4 input level, got {bass_only}"
        );
    }
}
