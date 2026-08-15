//! Biquad filters.
//!
//! Every tone control in the mixer is built from these: the EQ bands, the
//! filter sweep, and later the crossovers a stem engine needs. One second-order
//! section, four multiply-accumulates per sample, no allocation.

use std::f32::consts::PI;

/// Transposed Direct Form II biquad.
///
/// TDF-II is the standard choice for floating-point audio: it needs only two
/// state variables and it is numerically better behaved at low frequencies than
/// Direct Form I, which matters because a bass EQ at 70 Hz on a 48 kHz stream is
/// exactly the ill-conditioned case.
#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    // Feed-forward coefficients, already normalised by a0.
    b0: f32,
    b1: f32,
    b2: f32,
    // Feedback coefficients, negated and normalised.
    a1: f32,
    a2: f32,
    // Filter state.
    z1: f32,
    z2: f32,
}

impl Biquad {
    /// A filter that passes its input unchanged.
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Low-pass, RBJ cookbook.
    ///
    /// `q` of `FRAC_1_SQRT_2` is Butterworth -- maximally flat, no resonant peak.
    #[must_use]
    pub fn low_pass(sample_rate: f32, frequency: f32, q: f32) -> Self {
        let (sin_w, cos_w, alpha) = Self::intermediates(sample_rate, frequency, q);
        let b1 = 1.0 - cos_w;
        Self::normalised(
            b1 * 0.5,
            b1,
            b1 * 0.5,
            1.0 + alpha,
            -2.0 * cos_w,
            1.0 - alpha,
            sin_w,
        )
    }

    /// High-pass, RBJ cookbook.
    #[must_use]
    pub fn high_pass(sample_rate: f32, frequency: f32, q: f32) -> Self {
        let (sin_w, cos_w, alpha) = Self::intermediates(sample_rate, frequency, q);
        let b0 = (1.0 + cos_w) * 0.5;
        Self::normalised(
            b0,
            -(1.0 + cos_w),
            b0,
            1.0 + alpha,
            -2.0 * cos_w,
            1.0 - alpha,
            sin_w,
        )
    }

    /// Peaking EQ: boost or cut around `frequency` by `gain_db`.
    #[must_use]
    pub fn peaking(sample_rate: f32, frequency: f32, q: f32, gain_db: f32) -> Self {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let (sin_w, cos_w, alpha) = Self::intermediates(sample_rate, frequency, q);
        Self::normalised(
            1.0 + alpha * a,
            -2.0 * cos_w,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos_w,
            1.0 - alpha / a,
            sin_w,
        )
    }

    /// Low shelf: `gain_db` applied below `frequency`.
    #[must_use]
    pub fn low_shelf(sample_rate: f32, frequency: f32, gain_db: f32) -> Self {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let (sin_w, cos_w, _) =
            Self::intermediates(sample_rate, frequency, std::f32::consts::FRAC_1_SQRT_2);
        // Shelves use their own alpha derived from the shelf slope.
        let alpha = sin_w * 0.5 * ((a + 1.0 / a) * (1.0 / 0.707 - 1.0) + 2.0).max(0.0).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        Self::normalised(
            a * ((a + 1.0) - (a - 1.0) * cos_w + two_sqrt_a_alpha),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w),
            a * ((a + 1.0) - (a - 1.0) * cos_w - two_sqrt_a_alpha),
            (a + 1.0) + (a - 1.0) * cos_w + two_sqrt_a_alpha,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos_w),
            (a + 1.0) + (a - 1.0) * cos_w - two_sqrt_a_alpha,
            sin_w,
        )
    }

    /// High shelf: `gain_db` applied above `frequency`.
    #[must_use]
    pub fn high_shelf(sample_rate: f32, frequency: f32, gain_db: f32) -> Self {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let (sin_w, cos_w, _) =
            Self::intermediates(sample_rate, frequency, std::f32::consts::FRAC_1_SQRT_2);
        let alpha = sin_w * 0.5 * ((a + 1.0 / a) * (1.0 / 0.707 - 1.0) + 2.0).max(0.0).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        Self::normalised(
            a * ((a + 1.0) + (a - 1.0) * cos_w + two_sqrt_a_alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w),
            a * ((a + 1.0) + (a - 1.0) * cos_w - two_sqrt_a_alpha),
            (a + 1.0) - (a - 1.0) * cos_w + two_sqrt_a_alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * cos_w),
            (a + 1.0) - (a - 1.0) * cos_w - two_sqrt_a_alpha,
            sin_w,
        )
    }

    /// Shared cookbook intermediates, with the frequency clamped to a sane band.
    ///
    /// A frequency at or above Nyquist produces a divide-by-zero and a filter
    /// full of NaN, which then poisons every sample downstream forever. Clamping
    /// here means no caller can do that by accident.
    fn intermediates(sample_rate: f32, frequency: f32, q: f32) -> (f32, f32, f32) {
        let nyquist = sample_rate * 0.5;
        let frequency = if frequency.is_finite() {
            frequency.clamp(1.0, nyquist * 0.99)
        } else {
            1_000.0
        };
        let q = if q.is_finite() {
            q.clamp(0.05, 40.0)
        } else {
            0.707
        };

        let w = 2.0 * PI * frequency / sample_rate;
        let sin_w = w.sin();
        let cos_w = w.cos();
        (sin_w, cos_w, sin_w / (2.0 * q))
    }

    #[allow(clippy::too_many_arguments)]
    fn normalised(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32, _sin_w: f32) -> Self {
        // a0 of zero would be a degenerate filter; fall back to passthrough
        // rather than emitting infinities.
        if a0.abs() < f32::EPSILON {
            return Self::identity();
        }
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Copy coefficients from `other`, keeping our own state.
    ///
    /// This is how a filter is retuned mid-stream: replacing the whole struct
    /// would zero `z1`/`z2` and produce a click on every knob movement.
    pub fn set_coefficients_from(&mut self, other: &Biquad) {
        self.b0 = other.b0;
        self.b1 = other.b1;
        self.b2 = other.b2;
        self.a1 = other.a1;
        self.a2 = other.a2;
    }

    /// Clear filter memory. Use on load or seek, not on parameter changes.
    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    /// Process one sample.
    #[must_use]
    pub fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.z1;
        self.z1 = self.b1 * input - self.a1 * output + self.z2;
        self.z2 = self.b2 * input - self.a2 * output;
        output
    }

    /// Magnitude response at `frequency`, as a linear gain.
    ///
    /// Used by the tests to verify filters actually do what they claim, rather
    /// than merely producing plausible-looking numbers.
    #[must_use]
    pub fn magnitude_at(&self, sample_rate: f32, frequency: f32) -> f32 {
        let w = 2.0 * PI * frequency / sample_rate;
        let (sin_w, cos_w) = (w.sin(), w.cos());
        let (sin_2w, cos_2w) = ((2.0 * w).sin(), (2.0 * w).cos());

        let num_real = self.b0 + self.b1 * cos_w + self.b2 * cos_2w;
        let num_imag = -(self.b1 * sin_w + self.b2 * sin_2w);
        let den_real = 1.0 + self.a1 * cos_w + self.a2 * cos_2w;
        let den_imag = -(self.a1 * sin_w + self.a2 * sin_2w);

        let num = (num_real * num_real + num_imag * num_imag).sqrt();
        let den = (den_real * den_real + den_imag * den_imag).sqrt();
        if den > 0.0 { num / den } else { 0.0 }
    }
}

impl Default for Biquad {
    fn default() -> Self {
        Self::identity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn db(linear: f32) -> f32 {
        20.0 * linear.log10()
    }

    #[test]
    fn identity_passes_signal_through() {
        let mut f = Biquad::identity();
        for input in [0.0, 0.5, -0.3, 1.0] {
            assert_eq!(f.process(input), input);
        }
    }

    #[test]
    fn low_pass_passes_low_and_stops_high() {
        let f = Biquad::low_pass(SR, 1_000.0, std::f32::consts::FRAC_1_SQRT_2);
        assert!(
            db(f.magnitude_at(SR, 100.0)).abs() < 0.5,
            "should pass 100Hz"
        );
        // Butterworth is -3 dB at the corner.
        assert!((db(f.magnitude_at(SR, 1_000.0)) + 3.0).abs() < 0.5);
        assert!(
            db(f.magnitude_at(SR, 10_000.0)) < -30.0,
            "should reject 10kHz, got {} dB",
            db(f.magnitude_at(SR, 10_000.0))
        );
    }

    #[test]
    fn high_pass_passes_high_and_stops_low() {
        let f = Biquad::high_pass(SR, 1_000.0, std::f32::consts::FRAC_1_SQRT_2);
        assert!(db(f.magnitude_at(SR, 10_000.0)).abs() < 0.5);
        assert!((db(f.magnitude_at(SR, 1_000.0)) + 3.0).abs() < 0.5);
        assert!(db(f.magnitude_at(SR, 100.0)) < -30.0);
    }

    #[test]
    fn peaking_boosts_and_cuts_at_its_centre() {
        let boost = Biquad::peaking(SR, 1_000.0, 1.0, 6.0);
        assert!(
            (db(boost.magnitude_at(SR, 1_000.0)) - 6.0).abs() < 0.2,
            "expected +6 dB, got {}",
            db(boost.magnitude_at(SR, 1_000.0))
        );
        // Far from centre it should be transparent.
        assert!(db(boost.magnitude_at(SR, 50.0)).abs() < 0.5);

        let cut = Biquad::peaking(SR, 1_000.0, 1.0, -6.0);
        assert!((db(cut.magnitude_at(SR, 1_000.0)) + 6.0).abs() < 0.2);
    }

    #[test]
    fn shelves_affect_the_correct_side() {
        let low = Biquad::low_shelf(SR, 300.0, 6.0);
        assert!(
            db(low.magnitude_at(SR, 50.0)) > 4.0,
            "low shelf should lift the bass, got {}",
            db(low.magnitude_at(SR, 50.0))
        );
        assert!(
            db(low.magnitude_at(SR, 10_000.0)).abs() < 0.5,
            "low shelf must leave treble alone"
        );

        let high = Biquad::high_shelf(SR, 4_000.0, 6.0);
        assert!(db(high.magnitude_at(SR, 15_000.0)) > 4.0);
        assert!(db(high.magnitude_at(SR, 100.0)).abs() < 0.5);
    }

    /// A frequency at or beyond Nyquist is the classic way to fill a filter with
    /// NaN and silently poison the whole mix.
    #[test]
    fn frequencies_beyond_nyquist_are_clamped_not_catastrophic() {
        for frequency in [24_000.0, 48_000.0, 1e9, f32::INFINITY, f32::NAN] {
            let mut f = Biquad::low_pass(SR, frequency, 0.707);
            let out = f.process(0.5);
            assert!(out.is_finite(), "frequency {frequency} produced {out}");
        }
    }

    #[test]
    fn extreme_q_stays_finite() {
        for q in [0.0, -1.0, 1e9, f32::NAN] {
            let mut f = Biquad::peaking(SR, 1_000.0, q, 6.0);
            for _ in 0..100 {
                assert!(f.process(0.5).is_finite(), "q {q} went non-finite");
            }
        }
    }

    #[test]
    fn filter_is_stable_over_a_long_run() {
        let mut f = Biquad::low_pass(SR, 200.0, 4.0);
        let mut max = 0.0f32;
        for n in 0..48_000 {
            // Full-scale sine at the resonant frequency: worst case for blow-up.
            let input = (2.0 * PI * 200.0 * n as f32 / SR).sin();
            max = max.max(f.process(input).abs());
        }
        assert!(max.is_finite() && max < 100.0, "filter blew up: peak {max}");
    }

    /// Retuning must not clear state, or every knob move clicks.
    #[test]
    fn retuning_preserves_filter_state() {
        let mut f = Biquad::low_pass(SR, 1_000.0, 0.707);
        for _ in 0..50 {
            let _ = f.process(0.5);
        }
        let before = f.z1;
        f.set_coefficients_from(&Biquad::low_pass(SR, 2_000.0, 0.707));
        assert_eq!(f.z1, before, "retuning cleared filter memory");
    }

    #[test]
    fn reset_clears_state() {
        let mut f = Biquad::low_pass(SR, 1_000.0, 0.707);
        for _ in 0..50 {
            let _ = f.process(1.0);
        }
        f.reset();
        assert_eq!(f.z1, 0.0);
        assert_eq!(f.z2, 0.0);
    }

    /// The measured response must match what the filter actually does to a
    /// signal, otherwise every other test here is checking arithmetic against
    /// itself.
    #[test]
    fn magnitude_response_matches_measured_output() {
        let frequency = 500.0;
        let mut f = Biquad::low_pass(SR, 1_000.0, 0.707);
        let predicted = f.magnitude_at(SR, frequency);

        // Settle, then measure peak over whole cycles.
        for n in 0..4_800 {
            let _ = f.process((2.0 * PI * frequency * n as f32 / SR).sin());
        }
        let mut peak = 0.0f32;
        for n in 4_800..9_600 {
            peak = peak.max(
                f.process((2.0 * PI * frequency * n as f32 / SR).sin())
                    .abs(),
            );
        }

        assert!(
            (peak - predicted).abs() < 0.05,
            "predicted {predicted}, measured {peak}"
        );
    }
}
