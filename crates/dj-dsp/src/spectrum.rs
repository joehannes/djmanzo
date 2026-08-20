//! Band energies from the master bus, for the interface to move to.
//!
//! # What this is for
//!
//! Nothing in the audio path listens to this. It exists so the interface can
//! react to the music — a control that swells on the kick, a glow that follows
//! the hats. That makes it a *meter*, next to [`crate::meter::PeakMeter`],
//! rather than a processor: it reads the signal and reports, and the signal
//! goes past it unchanged.
//!
//! # Why it does not run every block
//!
//! The window is 1024 frames; a block is typically 256. Running the transform
//! every block would analyse each sample four times over and publish at 190 Hz
//! into an interface that redraws at 60. So it runs on a hop — enough to stay
//! ahead of the display and no more. Measured at 9.5 µs a transform on this
//! machine, which is 0.2% of a 256-frame budget; the hop takes that to 0.05%.
//! Cheap either way, but there is no reason to pay four times for one answer.

use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

/// How many bands are reported.
pub const BANDS: usize = 4;

/// The upper edge of each band in Hz. The last is open-ended in practice —
/// anything above it is above hearing.
///
/// Four bands rather than a full spectrum because the interface is not a
/// spectrum analyser: it wants to know "is there a kick", "is there a hat", and
/// a hundred bins is a hundred numbers nobody reads.
const EDGES_HZ: [f32; BANDS] = [250.0, 2_000.0, 6_000.0, 20_000.0];

/// A Hann window's RMS gain.
///
/// `sqrt(3/8)`. Divided out so a band reads the amplitude that was actually
/// there rather than the amplitude after the window took a bite out of it.
const HANN_RMS: f32 = 0.612_372_44;

/// Band energies over a sliding window of the master bus.
///
/// Deliberately not `Clone`: every field is a heap buffer, so a clone is four
/// allocations, and the only place this type lives is the audio thread.
pub struct Spectrum {
    fft: Arc<dyn Fft<f32>>,
    scratch: Vec<Complex<f32>>,
    bins: Vec<Complex<f32>>,
    /// The sliding window, oldest sample at `cursor`.
    history: Vec<f32>,
    cursor: usize,
    /// Frames written since the last transform.
    since: usize,
    /// Frames between transforms.
    hop: usize,
    window: Vec<f32>,
    /// Which band each usable bin falls in, worked out once.
    ///
    /// A lookup rather than three comparisons per bin per transform: the edges
    /// cannot change, so deciding them 512 times a hop is 512 decisions already
    /// made.
    band_of: Vec<u8>,
    bands: [f32; BANDS],
    /// Scales a band's summed power into an amplitude. See [`Self::compute`].
    norm: f32,
}

impl std::fmt::Debug for Spectrum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spectrum")
            .field("size", &self.history.len())
            .field("hop", &self.hop)
            .field("bands", &self.bands)
            .finish()
    }
}

impl Spectrum {
    /// `size` is the window in frames and must be a power of two; `hop` is how
    /// many new frames are wanted before another transform runs.
    #[must_use]
    pub fn new(size: usize, hop: usize, sample_rate: f32) -> Self {
        let size = size.max(2);
        let fft = FftPlanner::new().plan_fft_forward(size);
        let scratch_len = fft.get_inplace_scratch_len();
        let bin_width = sample_rate / size as f32;

        let window = (0..size)
            .map(|n| 0.5 * (1.0 - (std::f32::consts::TAU * n as f32 / (size - 1) as f32).cos()))
            .collect();

        // Bin 0 is DC and carries no pitch, so the table starts at bin 1 and is
        // indexed by `bin - 1`. Bins at or past Nyquist are the mirror image of
        // the ones below it and would double every reading.
        let usable = size / 2;
        let band_of = (1..usable)
            .map(|bin| {
                let hz = bin as f32 * bin_width;
                EDGES_HZ.iter().position(|edge| hz < *edge).unwrap_or(BANDS) as u8
            })
            .collect();

        Self {
            fft,
            scratch: vec![Complex::default(); scratch_len],
            bins: vec![Complex::default(); size],
            history: vec![0.0; size],
            cursor: 0,
            since: 0,
            hop: hop.max(1),
            window,
            band_of,
            bands: [0.0; BANDS],
            // Parseval, one-sided: `sqrt(2 * sum|X|^2) / size` is the RMS
            // amplitude the band carried. Dividing by the window's own RMS
            // undoes the window. A full-scale sine alone in a band therefore
            // reads 0.707 — the RMS of a sine — rather than some number that
            // depends on how many bins the band happens to span.
            norm: std::f32::consts::SQRT_2 / (size as f32 * HANN_RMS),
        }
    }

    /// Feed one mono frame.
    #[inline]
    pub fn push(&mut self, sample: f32) {
        self.history[self.cursor] = sample;
        self.cursor += 1;
        if self.cursor == self.history.len() {
            self.cursor = 0;
        }
        self.since += 1;
    }

    /// The band amplitudes, 0..=1, recomputed only when a hop's worth of new
    /// audio has arrived.
    ///
    /// Between hops this repeats the last answer, which is what an interface
    /// wants: a number that changes on its own schedule rather than one that
    /// flickers with the block size.
    pub fn bands(&mut self) -> [f32; BANDS] {
        if self.since >= self.hop {
            self.since = 0;
            self.compute();
        }
        self.bands
    }

    fn compute(&mut self) {
        let size = self.history.len();

        // Silence is the common case between tracks and costs nothing to spot.
        // Skipping the transform for it also stops the window's own numerical
        // noise being published as a faint permanent shimmer.
        let loudest = self
            .history
            .iter()
            .fold(0.0f32, |peak, s| peak.max(s.abs()));
        if loudest < 1e-5 {
            self.bands = [0.0; BANDS];
            return;
        }

        for i in 0..size {
            let at = (self.cursor + i) % size;
            self.bins[i] = Complex {
                re: self.history[at] * self.window[i],
                im: 0.0,
            };
        }
        self.fft
            .process_with_scratch(&mut self.bins, &mut self.scratch);

        let mut power = [0.0f32; BANDS];
        for (index, bin) in self.bins.iter().enumerate().take(size / 2).skip(1) {
            // `band_of` is indexed from bin 1, and `BANDS` marks a bin above
            // the top edge — above hearing, so it belongs to nothing.
            let band = self.band_of[index - 1] as usize;
            if band < BANDS {
                power[band] += bin.norm_sqr();
            }
        }

        for (band, sum) in power.iter().enumerate() {
            self.bands[band] = (sum.sqrt() * self.norm).clamp(0.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;
    const SIZE: usize = 1024;

    fn fed(hz: f32, amplitude: f32) -> Spectrum {
        // A hop of 1 so a test gets an answer from the samples it just wrote
        // rather than from whenever the next hop happens to land.
        let mut spectrum = Spectrum::new(SIZE, 1, SR);
        for n in 0..SIZE {
            let t = n as f32 / SR;
            spectrum.push(amplitude * (std::f32::consts::TAU * hz * t).sin());
        }
        spectrum
    }

    #[test]
    fn a_tone_lands_in_the_band_it_belongs_to() {
        // One frequency per band, comfortably inside it rather than at an edge.
        for (band, hz) in [(0, 60.0), (1, 800.0), (2, 4_000.0), (3, 10_000.0)] {
            let bands = fed(hz, 1.0).bands();
            for other in 0..BANDS {
                if other == band {
                    assert!(
                        bands[other] > 0.5,
                        "{hz} Hz should fill band {band}, read {:?}",
                        bands
                    );
                } else {
                    assert!(
                        bands[other] < 0.1,
                        "{hz} Hz leaked into band {other}, read {:?}",
                        bands
                    );
                }
            }
        }
    }

    /// **The bug the first version had.** Bands were sums of bin magnitudes, so
    /// a band spanning three hundred bins read far higher than one spanning
    /// five for the same amount of sound — and anything broadband pinned the
    /// upper bands at 1.0 permanently. A band has to mean the same thing
    /// wherever it sits, or the interface is reacting to bin counts.
    #[test]
    fn every_band_reads_the_same_for_the_same_amount_of_sound() {
        let readings: Vec<f32> = [60.0, 800.0, 4_000.0, 10_000.0]
            .into_iter()
            .enumerate()
            .map(|(band, hz)| fed(hz, 1.0).bands()[band])
            .collect();

        let low = readings.iter().copied().fold(f32::MAX, f32::min);
        let high = readings.iter().copied().fold(0.0f32, f32::max);
        assert!(
            high - low < 0.1,
            "the same tone reads differently per band: {readings:?}"
        );
        // A full-scale sine is 0.707 RMS, and that is what it should say.
        assert!(
            (low - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.1,
            "a full-scale sine should read about 0.707, read {readings:?}"
        );
    }

    /// Broadband material must not saturate. White noise across the whole range
    /// is the case that made the old normalisation useless.
    #[test]
    fn noise_does_not_pin_the_upper_bands() {
        let mut spectrum = Spectrum::new(SIZE, 1, SR);
        // A cheap deterministic hash, so this test cannot flake.
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..SIZE {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            spectrum.push((seed >> 40) as f32 / 8_388_608.0 - 1.0);
        }
        let bands = spectrum.bands();
        for (band, level) in bands.iter().enumerate() {
            assert!(*level < 0.95, "band {band} saturated on noise: {bands:?}");
        }
    }

    #[test]
    fn silence_reads_as_silence() {
        let mut spectrum = Spectrum::new(SIZE, 1, SR);
        for _ in 0..SIZE {
            spectrum.push(0.0);
        }
        assert_eq!(spectrum.bands(), [0.0; BANDS]);
    }

    /// The hop is the whole reason this is affordable, so it has to actually
    /// hop: between transforms the answer must be the previous one rather than
    /// a fresh one.
    #[test]
    fn the_answer_only_changes_on_a_hop() {
        let mut spectrum = Spectrum::new(SIZE, 512, SR);
        for n in 0..SIZE {
            let t = n as f32 / SR;
            spectrum.push((std::f32::consts::TAU * 60.0 * t).sin());
        }
        let first = spectrum.bands();
        assert!(first[0] > 0.5, "the tone should be there: {first:?}");

        // Silence, but not a hop's worth of it.
        for _ in 0..100 {
            spectrum.push(0.0);
        }
        assert_eq!(spectrum.bands(), first, "it recomputed early");

        for _ in 0..500 {
            spectrum.push(0.0);
        }
        assert_ne!(spectrum.bands(), first, "it never recomputed");
    }

    /// A tone above the top edge belongs to no band, and must not be quietly
    /// folded into the treble — the mirror image above Nyquist would otherwise
    /// double every reading.
    #[test]
    fn nothing_above_the_top_edge_is_counted() {
        let bands = fed(22_000.0, 1.0).bands();
        assert!(
            bands.iter().all(|level| *level < 0.1),
            "something above 20 kHz was counted: {bands:?}"
        );
    }
}
