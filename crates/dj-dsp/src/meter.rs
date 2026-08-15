//! Level metering.

/// Peak meter with instant attack and exponential release.
///
/// Instant attack means a transient is never missed -- a meter that smooths its
/// rise will under-report exactly the peaks a DJ needs to see. The slow release
/// is purely so the reading is legible; the UI samples this at 60 Hz, far slower
/// than the audio, and without a hold the needle would be unreadable.
#[derive(Debug, Clone, Copy)]
pub struct PeakMeter {
    peak: f32,
    release_coefficient: f32,
}

impl PeakMeter {
    /// Time to fall by roughly 63%. 300 ms reads well without hiding anything.
    pub const DEFAULT_RELEASE_MS: f32 = 300.0;

    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self::with_release(sample_rate, Self::DEFAULT_RELEASE_MS)
    }

    #[must_use]
    pub fn with_release(sample_rate: f32, release_ms: f32) -> Self {
        let samples = (sample_rate * release_ms / 1000.0).max(1.0);
        Self {
            peak: 0.0,
            release_coefficient: (-1.0 / samples).exp(),
        }
    }

    /// Feed one block. Returns the current peak reading.
    ///
    /// Release is applied once per block rather than per sample: at a 60 Hz UI
    /// refresh the difference is invisible, and it keeps the inner loop to a
    /// compare-and-branch.
    pub fn process(&mut self, samples: &[f32]) -> f32 {
        let mut block_peak = 0.0f32;
        for &sample in samples {
            let magnitude = sample.abs();
            if magnitude > block_peak {
                block_peak = magnitude;
            }
        }

        // NaN in the signal must not become NaN in the meter, or the level
        // display latches forever and never recovers.
        if !block_peak.is_finite() {
            block_peak = 0.0;
        }

        if block_peak >= self.peak {
            self.peak = block_peak;
        } else {
            let decayed = self.peak * self.release_coefficient.powi(samples.len() as i32);
            self.peak = decayed.max(block_peak);
        }
        self.peak
    }

    #[must_use]
    pub fn peak(&self) -> f32 {
        self.peak
    }

    /// True when the signal reached or exceeded full scale.
    #[must_use]
    pub fn is_clipping(&self) -> bool {
        self.peak >= 1.0
    }

    pub fn reset(&mut self) {
        self.peak = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn silence_reads_zero() {
        let mut m = PeakMeter::new(SR);
        assert_eq!(m.process(&[0.0; 64]), 0.0);
    }

    #[test]
    fn attack_is_instant() {
        let mut m = PeakMeter::new(SR);
        let mut block = [0.0f32; 64];
        block[63] = 0.8;
        // Even a peak in the last sample of the block is reported immediately.
        assert_eq!(m.process(&block), 0.8);
    }

    #[test]
    fn negative_peaks_count() {
        let mut m = PeakMeter::new(SR);
        assert_eq!(m.process(&[-0.7, 0.1, 0.2]), 0.7);
    }

    #[test]
    fn release_decays_but_does_not_reach_zero_instantly() {
        let mut m = PeakMeter::with_release(SR, 300.0);
        m.process(&[1.0; 16]);
        assert_eq!(m.peak(), 1.0);
        // One block of silence: 64 samples is ~1.3 ms, far shorter than the
        // 300 ms release, so the reading should barely move.
        let after = m.process(&[0.0; 64]);
        assert!(after < 1.0, "peak should decay");
        assert!(
            after > 0.99,
            "300ms release should barely move in 1.3ms, got {after}"
        );
    }

    #[test]
    fn release_eventually_falls_to_near_zero() {
        let mut m = PeakMeter::with_release(SR, 50.0);
        m.process(&[1.0; 16]);
        for _ in 0..200 {
            m.process(&[0.0; 256]);
        }
        assert!(
            m.peak() < 0.01,
            "expected decay to near zero, got {}",
            m.peak()
        );
    }

    #[test]
    fn clipping_is_detected_at_full_scale() {
        let mut m = PeakMeter::new(SR);
        m.process(&[0.99; 8]);
        assert!(!m.is_clipping());
        m.process(&[1.0; 8]);
        assert!(m.is_clipping());
    }

    #[test]
    fn nan_does_not_latch_the_meter() {
        let mut m = PeakMeter::new(SR);
        let reading = m.process(&[f32::NAN, 0.5]);
        assert!(reading.is_finite(), "meter latched on NaN: {reading}");
    }

    #[test]
    fn reset_clears_the_reading() {
        let mut m = PeakMeter::new(SR);
        m.process(&[1.0; 8]);
        m.reset();
        assert_eq!(m.peak(), 0.0);
    }
}
