//! Mixer tone controls: the three-band EQ and the filter sweep.

use crate::biquad::Biquad;
use crate::smooth::SmoothedValue;
use std::f32::consts::FRAC_1_SQRT_2;

/// Crossover between low and mid.
pub const LOW_MID_HZ: f32 = 300.0;
/// Crossover between mid and high.
pub const MID_HIGH_HZ: f32 = 4_000.0;

/// A three-band isolator EQ, one audio channel.
///
/// # Why an isolator rather than shelf controls
///
/// A DJ EQ has to *kill* a band -- take the bass out entirely under a transition
/// and put it back on the beat. Tone-control shelves cannot do that: a shelf at
/// its minimum still leaves a shadow of the band behind, which sounds like mud
/// rather than silence when two tracks are running.
///
/// So the signal is split by Linkwitz-Riley crossovers into three bands, each
/// band gets its own gain, and the bands are summed. Gain zero means the band is
/// genuinely gone. LR4 sections (two cascaded Butterworth biquads) are used
/// because their outputs sum without the notch a single-order pair leaves at the
/// crossover.
///
/// Stereo is two of these; the type stays mono so nothing here has to reason
/// about channel layout.
#[derive(Debug, Clone)]
pub struct ThreeBandEq {
    low_pass: [Biquad; 2],
    mid_high_pass: [Biquad; 2],
    mid_low_pass: [Biquad; 2],
    high_pass: [Biquad; 2],
    low_gain: SmoothedValue,
    mid_gain: SmoothedValue,
    high_gain: SmoothedValue,
}

impl ThreeBandEq {
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let lp_low = Biquad::low_pass(sample_rate, LOW_MID_HZ, FRAC_1_SQRT_2);
        let hp_low = Biquad::high_pass(sample_rate, LOW_MID_HZ, FRAC_1_SQRT_2);
        let lp_high = Biquad::low_pass(sample_rate, MID_HIGH_HZ, FRAC_1_SQRT_2);
        let hp_high = Biquad::high_pass(sample_rate, MID_HIGH_HZ, FRAC_1_SQRT_2);

        Self {
            low_pass: [lp_low, lp_low],
            mid_high_pass: [hp_low, hp_low],
            mid_low_pass: [lp_high, lp_high],
            high_pass: [hp_high, hp_high],
            low_gain: SmoothedValue::new(1.0, sample_rate),
            mid_gain: SmoothedValue::new(1.0, sample_rate),
            high_gain: SmoothedValue::new(1.0, sample_rate),
        }
    }

    /// Set band gains as linear multipliers. `0.0` kills a band.
    pub fn set_gains(&mut self, low: f32, mid: f32, high: f32) {
        self.low_gain.set_target(sane(low));
        self.mid_gain.set_target(sane(mid));
        self.high_gain.set_target(sane(high));
    }

    pub fn set_low(&mut self, gain: f32) {
        self.low_gain.set_target(sane(gain));
    }

    pub fn set_mid(&mut self, gain: f32) {
        self.mid_gain.set_target(sane(gain));
    }

    pub fn set_high(&mut self, gain: f32) {
        self.high_gain.set_target(sane(gain));
    }

    /// Clear filter memory. On load or seek, not on knob movements.
    pub fn reset(&mut self) {
        for filter in self
            .low_pass
            .iter_mut()
            .chain(self.mid_high_pass.iter_mut())
            .chain(self.mid_low_pass.iter_mut())
            .chain(self.high_pass.iter_mut())
        {
            filter.reset();
        }
    }

    /// True when all three bands are at unity and settled, so the caller can
    /// skip the filtering entirely.
    #[must_use]
    pub fn is_neutral(&self) -> bool {
        self.low_gain.is_settled()
            && self.mid_gain.is_settled()
            && self.high_gain.is_settled()
            && (self.low_gain.target() - 1.0).abs() < 1e-6
            && (self.mid_gain.target() - 1.0).abs() < 1e-6
            && (self.high_gain.target() - 1.0).abs() < 1e-6
    }

    /// Process one sample.
    #[must_use]
    pub fn process(&mut self, input: f32) -> f32 {
        let mut low = input;
        for filter in &mut self.low_pass {
            low = filter.process(low);
        }

        let mut mid = input;
        for filter in &mut self.mid_high_pass {
            mid = filter.process(mid);
        }
        for filter in &mut self.mid_low_pass {
            mid = filter.process(mid);
        }

        let mut high = input;
        for filter in &mut self.high_pass {
            high = filter.process(high);
        }

        low * self.low_gain.next_value()
            + mid * self.mid_gain.next_value()
            + high * self.high_gain.next_value()
    }
}

/// The single-knob filter every DJ mixer has.
///
/// Centre is off. Turning left sweeps a low-pass down; turning right sweeps a
/// high-pass up. One control, because that is how it is played -- and because a
/// dead zone at the centre matters: without it the filter is never truly out of
/// circuit and the sound is subtly coloured all night.
#[derive(Debug, Clone)]
pub struct SweepFilter {
    low_pass: Biquad,
    high_pass: Biquad,
    sample_rate: f32,
    /// -1.0 fully low-passed, 0.0 off, +1.0 fully high-passed.
    position: f32,
    resonance: f32,
}

impl SweepFilter {
    /// Positions within this of centre count as off.
    pub const DEAD_ZONE: f32 = 0.02;

    /// Sweep range. Below 20 Hz or above 20 kHz there is nothing left to hear.
    pub const MIN_HZ: f32 = 20.0;
    pub const MAX_HZ: f32 = 20_000.0;

    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self {
            low_pass: Biquad::low_pass(sample_rate, Self::MAX_HZ, FRAC_1_SQRT_2),
            high_pass: Biquad::high_pass(sample_rate, Self::MIN_HZ, FRAC_1_SQRT_2),
            sample_rate,
            position: 0.0,
            resonance: FRAC_1_SQRT_2,
        }
    }

    /// Set knob position, `-1.0..=1.0`.
    pub fn set_position(&mut self, position: f32) {
        let position = if position.is_finite() {
            position.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.position = position;

        if self.is_bypassed() {
            return;
        }

        // Sweep the corner exponentially: frequency perception is logarithmic,
        // so a linear sweep spends most of its travel doing nothing audible.
        if position < 0.0 {
            let amount = (-position - Self::DEAD_ZONE) / (1.0 - Self::DEAD_ZONE);
            let frequency = exp_sweep(Self::MAX_HZ, 40.0, amount);
            self.low_pass.set_coefficients_from(&Biquad::low_pass(
                self.sample_rate,
                frequency,
                self.resonance,
            ));
        } else {
            let amount = (position - Self::DEAD_ZONE) / (1.0 - Self::DEAD_ZONE);
            let frequency = exp_sweep(Self::MIN_HZ, 8_000.0, amount);
            self.high_pass.set_coefficients_from(&Biquad::high_pass(
                self.sample_rate,
                frequency,
                self.resonance,
            ));
        }
    }

    /// Filter resonance. Higher is a more pronounced peak at the corner.
    pub fn set_resonance(&mut self, q: f32) {
        if q.is_finite() {
            self.resonance = q.clamp(0.5, 8.0);
            // Re-apply so the change takes effect immediately.
            let position = self.position;
            self.set_position(position);
        }
    }

    #[must_use]
    pub fn position(&self) -> f32 {
        self.position
    }

    /// True when the knob is in the centre dead zone.
    #[must_use]
    pub fn is_bypassed(&self) -> bool {
        self.position.abs() <= Self::DEAD_ZONE
    }

    pub fn reset(&mut self) {
        self.low_pass.reset();
        self.high_pass.reset();
    }

    /// Process one sample.
    #[must_use]
    pub fn process(&mut self, input: f32) -> f32 {
        if self.is_bypassed() {
            return input;
        }
        if self.position < 0.0 {
            self.low_pass.process(input)
        } else {
            self.high_pass.process(input)
        }
    }
}

/// Interpolate exponentially from `from` to `to`.
fn exp_sweep(from: f32, to: f32, amount: f32) -> f32 {
    let amount = amount.clamp(0.0, 1.0);
    from * (to / from).powf(amount)
}

/// Reject non-finite gains and clamp to a sane range.
fn sane(gain: f32) -> f32 {
    if gain.is_finite() {
        gain.clamp(0.0, 4.0)
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const SR: f32 = 48_000.0;

    /// Measure what a processor does to a steady sine, as a linear gain.
    ///
    /// Analytic response does not work here: the EQ is three parallel filter
    /// chains summed, so the only honest measurement is to push signal through
    /// it and look at what comes out.
    fn measure(process: &mut impl FnMut(f32) -> f32, frequency: f32) -> f32 {
        // Settle first -- filters have memory and the first cycles are transient.
        let settle = (SR / frequency * 20.0) as usize;
        for n in 0..settle {
            let _ = process((2.0 * PI * frequency * n as f32 / SR).sin());
        }
        let mut peak = 0.0f32;
        let window = (SR / frequency * 20.0) as usize;
        for n in settle..settle + window {
            peak = peak.max(process((2.0 * PI * frequency * n as f32 / SR).sin()).abs());
        }
        peak
    }

    fn settled_eq(low: f32, mid: f32, high: f32) -> ThreeBandEq {
        let mut eq = ThreeBandEq::new(SR);
        eq.set_gains(low, mid, high);
        // Run the gain ramps out so measurements are of the target, not the ramp.
        for _ in 0..10_000 {
            let _ = eq.process(0.0);
        }
        eq
    }

    #[test]
    fn unity_gains_are_roughly_transparent() {
        let mut eq = settled_eq(1.0, 1.0, 1.0);
        for frequency in [60.0, 150.0, 800.0, 2_000.0, 8_000.0] {
            let gain = measure(&mut |s| eq.process(s), frequency);
            let db = 20.0 * gain.log10();
            // Linkwitz-Riley sums allpass, not perfectly flat in magnitude, so
            // allow a little ripple around the crossovers.
            assert!(
                db.abs() < 1.5,
                "at {frequency} Hz the flat EQ was {db:.2} dB off unity"
            );
        }
    }

    /// The whole reason for an isolator: killing a band must remove it, not
    /// merely reduce it.
    #[test]
    fn killing_the_low_band_removes_the_bass() {
        let mut eq = settled_eq(0.0, 1.0, 1.0);
        let bass = measure(&mut |s| eq.process(s), 60.0);
        assert!(
            20.0 * bass.log10() < -30.0,
            "bass kill left {:.1} dB behind",
            20.0 * bass.log10()
        );

        let mut eq = settled_eq(0.0, 1.0, 1.0);
        let treble = measure(&mut |s| eq.process(s), 8_000.0);
        assert!(
            (20.0 * treble.log10()).abs() < 1.5,
            "killing bass must not touch treble"
        );
    }

    #[test]
    fn killing_the_high_band_removes_the_treble() {
        let mut eq = settled_eq(1.0, 1.0, 0.0);
        let treble = measure(&mut |s| eq.process(s), 10_000.0);
        assert!(20.0 * treble.log10() < -30.0);

        let mut eq = settled_eq(1.0, 1.0, 0.0);
        let bass = measure(&mut |s| eq.process(s), 60.0);
        assert!((20.0 * bass.log10()).abs() < 1.5);
    }

    #[test]
    fn killing_the_mid_band_removes_the_mids() {
        let mut eq = settled_eq(1.0, 0.0, 1.0);
        let mid = measure(&mut |s| eq.process(s), 1_000.0);
        assert!(
            20.0 * mid.log10() < -25.0,
            "mid kill left {:.1} dB",
            20.0 * mid.log10()
        );
    }

    #[test]
    fn killing_everything_is_silence() {
        let mut eq = settled_eq(0.0, 0.0, 0.0);
        let out = measure(&mut |s| eq.process(s), 1_000.0);
        assert!(out < 1e-3, "full kill left {out}");
    }

    #[test]
    fn boosting_a_band_lifts_it() {
        let mut eq = settled_eq(2.0, 1.0, 1.0);
        let bass = measure(&mut |s| eq.process(s), 60.0);
        assert!(
            (20.0 * bass.log10() - 6.0).abs() < 1.5,
            "expected ~+6 dB, got {:.1}",
            20.0 * bass.log10()
        );
    }

    #[test]
    fn gain_changes_are_smoothed() {
        // A step change must not produce a discontinuity in the output.
        let mut eq = ThreeBandEq::new(SR);
        for _ in 0..1_000 {
            let _ = eq.process(1.0);
        }
        eq.set_gains(0.0, 0.0, 0.0);
        let mut previous = eq.process(1.0);
        for _ in 0..500 {
            let current = eq.process(1.0);
            assert!(
                (current - previous).abs() < 0.1,
                "gain jumped from {previous} to {current}"
            );
            previous = current;
        }
    }

    #[test]
    fn non_finite_gains_are_rejected() {
        let mut eq = ThreeBandEq::new(SR);
        eq.set_gains(f32::NAN, f32::INFINITY, -5.0);
        for _ in 0..1_000 {
            assert!(eq.process(0.5).is_finite());
        }
    }

    #[test]
    fn neutral_eq_is_detectable() {
        let mut eq = ThreeBandEq::new(SR);
        assert!(eq.is_neutral(), "a fresh EQ should be neutral");
        eq.set_low(0.5);
        assert!(!eq.is_neutral());
    }

    #[test]
    fn filter_centre_is_a_true_bypass() {
        let mut filter = SweepFilter::new(SR);
        filter.set_position(0.0);
        assert!(filter.is_bypassed());
        for input in [0.1, -0.7, 0.9] {
            assert_eq!(filter.process(input), input, "bypass must be bit-exact");
        }
        // And just inside the dead zone.
        filter.set_position(0.01);
        assert_eq!(filter.process(0.5), 0.5);
    }

    #[test]
    fn turning_left_low_passes() {
        // Half travel: the corner sits around 1 kHz, so bass passes and treble
        // is cut. This is the position the control actually gets used at.
        let mut filter = SweepFilter::new(SR);
        filter.set_position(-0.5);
        let bass = measure(&mut |s| filter.process(s), 60.0);

        let mut filter = SweepFilter::new(SR);
        filter.set_position(-0.5);
        let treble = measure(&mut |s| filter.process(s), 10_000.0);

        assert!(
            20.0 * bass.log10() > -3.0,
            "bass should survive a half-closed low-pass, got {:.1} dB",
            20.0 * bass.log10()
        );
        assert!(
            20.0 * treble.log10() < -30.0,
            "treble should be gone, got {:.1} dB",
            20.0 * treble.log10()
        );
    }

    /// At the end stop the corner reaches 40 Hz, which should leave almost
    /// nothing -- that is what makes a full filter sweep a usable transition
    /// rather than a tone control.
    #[test]
    fn the_low_pass_end_stop_nearly_closes() {
        for frequency in [200.0, 1_000.0, 10_000.0] {
            let mut filter = SweepFilter::new(SR);
            filter.set_position(-1.0);
            let level = 20.0 * measure(&mut |s| filter.process(s), frequency).log10();
            assert!(
                level < -20.0,
                "at the end stop {frequency} Hz was only {level:.1} dB down"
            );
        }
    }

    #[test]
    fn turning_right_high_passes() {
        let mut filter = SweepFilter::new(SR);
        filter.set_position(1.0);
        let bass = measure(&mut |s| filter.process(s), 60.0);

        let mut filter = SweepFilter::new(SR);
        filter.set_position(1.0);
        let treble = measure(&mut |s| filter.process(s), 12_000.0);

        assert!(20.0 * bass.log10() < -30.0);
        assert!(20.0 * treble.log10() > -3.0);
    }

    #[test]
    fn the_sweep_is_monotonic() {
        // Further left must always mean less treble, or the knob feels wrong.
        let mut previous = f32::INFINITY;
        for step in 0..=10 {
            let position = -(step as f32) / 10.0;
            let mut filter = SweepFilter::new(SR);
            filter.set_position(position);
            let treble = measure(&mut |s| filter.process(s), 6_000.0);
            assert!(
                treble <= previous + 0.02,
                "treble rose from {previous} to {treble} at position {position}"
            );
            previous = treble;
        }
    }

    #[test]
    fn extreme_positions_and_resonance_stay_finite() {
        for position in [-5.0, 5.0, f32::NAN, f32::INFINITY] {
            let mut filter = SweepFilter::new(SR);
            filter.set_position(position);
            for _ in 0..1_000 {
                assert!(filter.process(0.5).is_finite(), "position {position}");
            }
        }
        for q in [0.0, 1e9, f32::NAN] {
            let mut filter = SweepFilter::new(SR);
            filter.set_position(-0.5);
            filter.set_resonance(q);
            for _ in 0..1_000 {
                assert!(filter.process(0.5).is_finite(), "q {q}");
            }
        }
    }

    #[test]
    fn exp_sweep_hits_its_endpoints() {
        assert!((exp_sweep(100.0, 1_000.0, 0.0) - 100.0).abs() < 0.01);
        assert!((exp_sweep(100.0, 1_000.0, 1.0) - 1_000.0).abs() < 0.1);
        // Halfway in exponential terms is the geometric mean.
        assert!((exp_sweep(100.0, 10_000.0, 0.5) - 1_000.0).abs() < 1.0);
    }
}
