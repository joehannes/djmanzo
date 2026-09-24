//! Reducing a track to something drawable.
//!
//! A five-minute track at 48 kHz is fourteen million frames. A waveform lane is
//! perhaps two thousand pixels wide. Drawing needs a summary, not the samples --
//! and it needs that summary at several resolutions, because zooming changes how
//! many frames fall under each pixel by orders of magnitude.

use dj_core::SampleRate;
use dj_dsp::Biquad;
use serde::{Deserialize, Serialize};
use std::f32::consts::FRAC_1_SQRT_2;

/// Crossovers for spectral colouring. Deliberately the same as the mixer's
/// isolator EQ, so what you see matches what the LOW/MID/HIGH knobs act on.
const LOW_HZ: f32 = 300.0;
const HIGH_HZ: f32 = 4_000.0;

/// How many bands §110's spectrum is read in.
///
/// Eight, because the owner's ask — every pitch its own colour of the
/// spectrum, the lowest red and the highest violet — needs enough bands for
/// the colours between: three bands can only ever be three hues, which is the
/// EQ-matched colouring djmanzo already had.
pub const SPECTRUM_BANDS: usize = 8;

/// Where one of the eight bands ends and the next begins, in Hz.
///
/// Log-spaced from 20 Hz to 20 kHz, about one and a quarter octaves each, so a
/// signal with the same energy in every octave — pink noise, which is roughly
/// the shape of a finished mix — puts the same energy in every band, and every
/// band takes its share of the waveform. The eight read as a DJ would name
/// them: sub, kick, bass, low mids, mids, presence, brilliance, air.
pub const SPECTRUM_EDGES_HZ: [f32; SPECTRUM_BANDS - 1] =
    [47.0, 112.0, 266.0, 632.0, 1_500.0, 3_560.0, 8_450.0];

/// How long each band's level is followed over, in seconds. See
/// `analyse_with_base` for why the level is followed at all.
const SPECTRUM_FOLLOW_SECONDS: f32 = 0.025;

/// One drawable column of audio.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Bucket {
    /// Most negative sample in the bucket.
    pub min: f32,
    /// Most positive sample in the bucket.
    pub max: f32,
    /// Root-mean-square level -- perceived loudness, drawn inside the peaks.
    pub rms: f32,
    /// Band energies, for colouring. Normalised so the largest is 1.0.
    pub low: f32,
    pub mid: f32,
    pub high: f32,
    /// §110's eight bands, lowest first, normalised so the largest is 255.
    ///
    /// A byte each rather than a float: it is a colour's worth of precision,
    /// and a five-minute record carries over a hundred thousand buckets across
    /// its zoom levels. All zeros means "not measured" — a summary built before
    /// this existed — and is drawn with the three bands instead.
    #[serde(default)]
    pub spectrum: [u8; SPECTRUM_BANDS],
}

impl Bucket {
    /// Peak-to-peak extent, which is what sets the drawn height.
    #[must_use]
    pub fn amplitude(&self) -> f32 {
        (self.max - self.min) * 0.5
    }

    #[must_use]
    pub fn is_silent(&self) -> bool {
        self.amplitude() < 1e-5
    }

    /// Merge two adjacent buckets into their coarser parent.
    #[must_use]
    pub fn merged(&self, other: &Bucket) -> Bucket {
        Bucket {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
            // RMS of a union is the quadratic mean, not the arithmetic one.
            // Averaging the RMS values directly would understate a loud half
            // next to a quiet one.
            rms: (((self.rms * self.rms) + (other.rms * other.rms)) * 0.5).sqrt(),
            low: (self.low + other.low) * 0.5,
            mid: (self.mid + other.mid) * 0.5,
            high: (self.high + other.high) * 0.5,
            spectrum: merge_spectrum(&self.spectrum, &other.spectrum),
        }
    }
}

/// A track summarised at several resolutions.
///
/// Level 0 is finest; each subsequent level covers twice as many frames per
/// bucket. Zooming picks the level whose density is closest to one bucket per
/// pixel, so a fully zoomed-out overview never walks fourteen million samples
/// and a zoomed-in view never draws from a summary coarser than its pixels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveformSummary {
    levels: Vec<Vec<Bucket>>,
    base_frames_per_bucket: usize,
    sample_rate: SampleRate,
    total_frames: usize,
}

impl WaveformSummary {
    /// Frames per bucket at the finest level.
    ///
    /// 256 gives roughly 190 buckets a second, which is finer than any screen
    /// shows at normal zoom while keeping a five-minute track under 60k buckets.
    pub const DEFAULT_BASE: usize = 256;

    /// Build a summary from interleaved stereo samples.
    ///
    /// Slow and allocating -- an analysis-worker job, never an audio-thread one.
    #[must_use]
    pub fn analyse(samples: &[f32], sample_rate: SampleRate) -> Self {
        Self::analyse_with_base(samples, sample_rate, Self::DEFAULT_BASE)
    }

    #[must_use]
    pub fn analyse_with_base(
        samples: &[f32],
        sample_rate: SampleRate,
        base_frames_per_bucket: usize,
    ) -> Self {
        let base = base_frames_per_bucket.max(1);
        let total_frames = samples.len() / 2;

        // Band splitting for colour. Mono sum is enough: nobody reads stereo
        // position off a waveform, and it halves the filtering work.
        let sr = sample_rate.as_f64() as f32;
        let mut low_pass = Biquad::low_pass(sr, LOW_HZ, FRAC_1_SQRT_2);
        let mut high_pass = Biquad::high_pass(sr, HIGH_HZ, FRAC_1_SQRT_2);
        let bucket_count = total_frames.div_ceil(base);
        let mut finest = Vec::with_capacity(bucket_count);

        let mut accumulator = Accumulator::default();
        for frame in 0..total_frames {
            let left = samples[frame * 2];
            let right = samples[frame * 2 + 1];
            let mono = (left + right) * 0.5;

            let low = low_pass.process(mono);
            let high = high_pass.process(mono);
            // Mid is the residual, so the three bands reconstruct exactly and no
            // energy is invented or lost at the crossovers.
            let mid = mono - low - high;

            accumulator.push(left, right, low, mid, high);

            if accumulator.count >= base {
                finest.push(accumulator.finish());
                accumulator = Accumulator::default();
            }
        }
        if accumulator.count > 0 {
            finest.push(accumulator.finish());
        }

        let mut levels = vec![finest];
        // Halve until a whole level fits in a handful of pixels; beyond that
        // there is nothing left to summarise.
        while levels.last().map(Vec::len).unwrap_or(0) > 2 {
            let previous = levels.last().expect("just checked");
            let mut coarser = Vec::with_capacity(previous.len().div_ceil(2));
            for pair in previous.chunks(2) {
                coarser.push(match pair {
                    [a, b] => a.merged(b),
                    [a] => *a,
                    _ => unreachable!("chunks(2) yields at most two"),
                });
            }
            levels.push(coarser);
        }

        Self {
            levels,
            base_frames_per_bucket: base,
            sample_rate,
            total_frames,
        }
    }

    /// A summary with §110's spectrum measured in: [`Self::analyse`] and then
    /// [`Self::measure_spectrum`], for callers that can afford both at once.
    #[must_use]
    pub fn analyse_with_spectrum(samples: &[f32], sample_rate: SampleRate) -> Self {
        let mut summary = Self::analyse(samples, sample_rate);
        summary.measure_spectrum(samples);
        summary
    }

    /// Measure §110's eight bands into a summary built without them.
    ///
    /// **Separate from [`Self::analyse`] on purpose.** It costs more than the
    /// whole of the rest of the summary — about half a second more for five
    /// minutes of audio — and the summary is built on the path that puts a
    /// record on a deck, which a DJ may be doing mid-mix. So loading waits for
    /// the outline and the three bands, and the light arrives a moment later
    /// from a background pass: the waveform appears at once and takes its
    /// colour when this finishes. `samples` must be the same interleaved
    /// stereo the summary was built from.
    pub fn measure_spectrum(&mut self, samples: &[f32]) {
        let sr = self.sample_rate.as_f64() as f32;
        let base = self.base_frames_per_bucket;
        let total_frames = (samples.len() / 2).min(self.total_frames);

        // §110's spectrum: each band its own filter — a high-pass at its lower
        // edge and a low-pass at its upper one, each applied twice, so the
        // skirts fall at 24 dB an octave. **The first version took the
        // difference of neighbouring low-passes instead**, which sums back to
        // the signal exactly and separates badly: two second-order low-passes
        // a little over an octave apart shift a tone's phase differently, and
        // their difference kept two thirds of a 30 Hz sine in the band above —
        // a sub-bass line drew orange. Nothing here needs the bands to add
        // back up; they are only measured. An edge past a low sample rate's
        // Nyquist is pulled under it rather than left to make a filter that
        // rings.
        let edge = |i: usize| SPECTRUM_EDGES_HZ[i].min(sr * 0.45);
        let mut bands: [Vec<Biquad>; SPECTRUM_BANDS] = std::array::from_fn(|band| {
            let mut stages = Vec::with_capacity(4);
            if band > 0 {
                stages.push(Biquad::high_pass(sr, edge(band - 1), FRAC_1_SQRT_2));
                stages.push(Biquad::high_pass(sr, edge(band - 1), FRAC_1_SQRT_2));
            }
            if band < SPECTRUM_BANDS - 1 {
                stages.push(Biquad::low_pass(sr, edge(band), FRAC_1_SQRT_2));
                stages.push(Biquad::low_pass(sr, edge(band), FRAC_1_SQRT_2));
            }
            stages
        });
        // Each band's level is followed, not sampled. A bucket is five
        // milliseconds and a bass wave is up to fifty, so how much of one lands
        // in a bucket depends on its phase: read raw, a steady bass line came
        // out a third as strong as it is in some buckets and full in the next,
        // and would have been drawn as stripes. A follower with a time constant
        // longer than a bass cycle and far shorter than a note holds the level
        // across the cycle and still lets the colour change on the beat.
        let follow = 1.0 - (-1.0 / (SPECTRUM_FOLLOW_SECONDS * sr)).exp();
        let mut levels = [0.0f32; SPECTRUM_BANDS];

        let mut finest = Vec::with_capacity(total_frames.div_ceil(base));
        let mut sums = [0.0f32; SPECTRUM_BANDS];
        let mut count = 0usize;
        for frame in 0..total_frames {
            let mono = (samples[frame * 2] + samples[frame * 2 + 1]) * 0.5;
            for ((stages, level), sum) in bands.iter_mut().zip(&mut levels).zip(&mut sums) {
                let filtered = stages.iter_mut().fold(mono, |x, stage| stage.process(x));
                *level += (filtered.abs() - *level) * follow;
                *sum += *level;
            }
            count += 1;
            if count >= base {
                finest.push(balance(&sums));
                sums = [0.0; SPECTRUM_BANDS];
                count = 0;
            }
        }
        if count > 0 {
            finest.push(balance(&sums));
        }

        let Some(first) = self.levels.first_mut() else {
            return;
        };
        for (bucket, spectrum) in first.iter_mut().zip(finest) {
            bucket.spectrum = spectrum;
        }
        // The coarser levels are pairs of the level below, exactly as they
        // were built, so their spectra are pairs too.
        for level in 1..self.levels.len() {
            let (finer, coarser) = self.levels.split_at_mut(level);
            let finer = &finer[level - 1];
            for (index, bucket) in coarser[0].iter_mut().enumerate() {
                bucket.spectrum = match (finer.get(index * 2), finer.get(index * 2 + 1)) {
                    (Some(a), Some(b)) => merge_spectrum(&a.spectrum, &b.spectrum),
                    (Some(a), None) => a.spectrum,
                    _ => bucket.spectrum,
                };
            }
        }
    }

    /// Whether [`Self::measure_spectrum`] has run on this summary.
    #[must_use]
    pub fn has_spectrum(&self) -> bool {
        self.levels.first().is_some_and(|finest| {
            finest
                .iter()
                .any(|bucket| bucket.spectrum.iter().any(|&band| band > 0))
        })
    }

    #[must_use]
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    #[must_use]
    pub fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    #[must_use]
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    #[must_use]
    pub fn level(&self, index: usize) -> &[Bucket] {
        self.levels.get(index).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Frames covered by one bucket at `level`.
    #[must_use]
    pub fn frames_per_bucket(&self, level: usize) -> usize {
        self.base_frames_per_bucket << level.min(32)
    }

    /// Pick the level closest to one bucket per pixel.
    ///
    /// Choosing too fine a level means averaging many buckets per pixel for
    /// nothing; too coarse means a visibly blocky waveform. Rounding toward the
    /// finer option keeps detail, since over-sampling is cheap and blockiness is
    /// not recoverable.
    #[must_use]
    pub fn level_for(&self, frames_per_pixel: f64) -> usize {
        if frames_per_pixel <= 0.0 || !frames_per_pixel.is_finite() {
            return 0;
        }
        let ratio = frames_per_pixel / self.base_frames_per_bucket as f64;
        if ratio <= 1.0 {
            return 0;
        }
        let level = ratio.log2().floor() as usize;
        level.min(self.levels.len().saturating_sub(1))
    }

    /// The bucket covering `frame` at `level`, or silence past the end.
    #[must_use]
    pub fn bucket_at(&self, level: usize, frame: f64) -> Bucket {
        if frame < 0.0 {
            return Bucket::default();
        }
        let buckets = self.level(level);
        let index = (frame / self.frames_per_bucket(level) as f64) as usize;
        buckets.get(index).copied().unwrap_or_default()
    }
}

/// Two neighbouring spectra as their parent's: the mean of each band.
fn merge_spectrum(a: &[u8; SPECTRUM_BANDS], b: &[u8; SPECTRUM_BANDS]) -> [u8; SPECTRUM_BANDS] {
    std::array::from_fn(|band| {
        let sum = u16::from(a[band]) + u16::from(b[band]);
        u8::try_from(sum / 2).unwrap_or(u8::MAX)
    })
}

/// A bucket's band totals as a balance: the strongest band is 255.
///
/// Balance rather than level, like the three bands: colour says which part of
/// the spectrum, and the waveform's height already says how loud.
fn balance(sums: &[f32; SPECTRUM_BANDS]) -> [u8; SPECTRUM_BANDS] {
    let strongest = sums.iter().copied().fold(0.0f32, f32::max);
    if strongest <= 1e-9 {
        return [0; SPECTRUM_BANDS];
    }
    sums.map(|sum| (sum / strongest * 255.0).round() as u8)
}

/// Running totals for one bucket while scanning.
#[derive(Debug, Default)]
struct Accumulator {
    count: usize,
    min: f32,
    max: f32,
    sum_squares: f32,
    low: f32,
    mid: f32,
    high: f32,
}

impl Accumulator {
    fn push(&mut self, left: f32, right: f32, low: f32, mid: f32, high: f32) {
        for sample in [left, right] {
            if sample < self.min {
                self.min = sample;
            }
            if sample > self.max {
                self.max = sample;
            }
            self.sum_squares += sample * sample;
        }
        self.low += low.abs();
        self.mid += mid.abs();
        self.high += high.abs();
        self.count += 1;
    }

    fn finish(self) -> Bucket {
        let samples = (self.count * 2).max(1) as f32;
        let count = self.count.max(1) as f32;

        // Normalise the bands against each other rather than against full
        // scale: colour should show spectral *balance*, so a quiet passage keeps
        // its character instead of fading to black.
        let (low, mid, high) = (self.low / count, self.mid / count, self.high / count);
        let peak = low.max(mid).max(high);
        let scale = if peak > 1e-9 { 1.0 / peak } else { 0.0 };

        Bucket {
            min: self.min,
            max: self.max,
            rms: (self.sum_squares / samples).sqrt(),
            low: low * scale,
            mid: mid * scale,
            high: high * scale,
            spectrum: [0; SPECTRUM_BANDS],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const SR: SampleRate = SampleRate::DEFAULT;

    fn sine(frames: usize, frequency: f32, amplitude: f32) -> Vec<f32> {
        (0..frames)
            .flat_map(|n| {
                let v = (2.0 * PI * frequency * n as f32 / 48_000.0).sin() * amplitude;
                [v, v]
            })
            .collect()
    }

    #[test]
    fn silence_summarises_to_nothing() {
        let summary = WaveformSummary::analyse(&vec![0.0; 48_000 * 2], SR);
        assert!(summary.level(0).iter().all(Bucket::is_silent));
    }

    #[test]
    fn bucket_count_follows_the_base_resolution() {
        let samples = vec![0.0; 10_000 * 2];
        let summary = WaveformSummary::analyse_with_base(&samples, SR, 100);
        assert_eq!(summary.level(0).len(), 100);
        assert_eq!(summary.total_frames(), 10_000);
    }

    #[test]
    fn a_partial_final_bucket_is_kept() {
        // Losing the tail would truncate the end of every track that is not an
        // exact multiple of the bucket size, which is nearly all of them.
        let samples = vec![0.5; 250 * 2];
        let summary = WaveformSummary::analyse_with_base(&samples, SR, 100);
        assert_eq!(summary.level(0).len(), 3);
    }

    #[test]
    fn peaks_reach_the_input_amplitude() {
        let summary = WaveformSummary::analyse(&sine(48_000, 440.0, 0.8), SR);
        let loudest = summary
            .level(0)
            .iter()
            .fold(0.0f32, |acc, b| acc.max(b.amplitude()));
        assert!(
            (loudest - 0.8).abs() < 0.02,
            "expected peaks near 0.8, got {loudest}"
        );
    }

    #[test]
    fn rms_is_below_peak_for_a_sine() {
        // A sine's RMS is peak/sqrt(2). If these came out equal, the RMS
        // calculation would be measuring peaks by mistake.
        let summary = WaveformSummary::analyse(&sine(48_000, 440.0, 1.0), SR);
        let bucket = summary.level(0)[50];
        assert!(
            (bucket.rms - 0.707).abs() < 0.05,
            "expected RMS near 0.707, got {}",
            bucket.rms
        );
    }

    #[test]
    fn each_pyramid_level_halves_the_previous() {
        let summary = WaveformSummary::analyse_with_base(&vec![0.1; 6_400 * 2], SR, 100);
        assert_eq!(summary.level(0).len(), 64);
        assert_eq!(summary.level(1).len(), 32);
        assert_eq!(summary.level(2).len(), 16);
        assert_eq!(summary.frames_per_bucket(0), 100);
        assert_eq!(summary.frames_per_bucket(1), 200);
    }

    /// Coarsening must not lose peaks: a transient that survives at full zoom
    /// has to survive zoomed out, or the overview lies about where the drops are.
    #[test]
    fn peaks_survive_coarsening() {
        let mut samples = vec![0.05f32; 10_000 * 2];
        samples[5_000] = 1.0;
        samples[5_001] = -1.0;

        let summary = WaveformSummary::analyse_with_base(&samples, SR, 100);
        for level in 0..summary.level_count() {
            let loudest = summary
                .level(level)
                .iter()
                .fold(0.0f32, |acc, b| acc.max(b.amplitude()));
            assert!(
                loudest > 0.9,
                "level {level} lost the transient, peak was {loudest}"
            );
        }
    }

    #[test]
    fn level_selection_tracks_zoom() {
        let summary = WaveformSummary::analyse_with_base(&vec![0.1; 100_000 * 2], SR, 100);
        // One frame per pixel: the finest level, nothing to gain from coarser.
        assert_eq!(summary.level_for(1.0), 0);
        assert_eq!(summary.level_for(100.0), 0);
        assert_eq!(summary.level_for(200.0), 1);
        assert_eq!(summary.level_for(400.0), 2);
    }

    #[test]
    fn level_selection_survives_nonsense_zoom() {
        let summary = WaveformSummary::analyse_with_base(&vec![0.1; 1_000 * 2], SR, 100);
        assert_eq!(summary.level_for(0.0), 0);
        assert_eq!(summary.level_for(-5.0), 0);
        assert_eq!(summary.level_for(f64::NAN), 0);
        // Absurd zoom clamps to the coarsest level rather than indexing past it.
        assert!(summary.level_for(1e12) < summary.level_count());
    }

    #[test]
    fn reading_past_the_end_gives_silence() {
        let summary = WaveformSummary::analyse_with_base(&vec![0.5; 1_000 * 2], SR, 100);
        assert!(summary.bucket_at(0, 1e9).is_silent());
        assert!(summary.bucket_at(0, -1.0).is_silent());
        assert!(summary.bucket_at(99, 0.0).is_silent());
    }

    /// Colour is the point of the band split: a bass tone and a treble tone must
    /// not look the same.
    #[test]
    fn bass_and_treble_colour_differently() {
        let bass = WaveformSummary::analyse(&sine(48_000, 60.0, 0.8), SR);
        let treble = WaveformSummary::analyse(&sine(48_000, 10_000.0, 0.8), SR);

        // Skip the first buckets: the filters are still settling on the step.
        let bass_bucket = bass.level(0)[100];
        let treble_bucket = treble.level(0)[100];

        assert!(
            bass_bucket.low > bass_bucket.high,
            "a 60 Hz tone should read as low-dominant: {bass_bucket:?}"
        );
        assert!(
            treble_bucket.high > treble_bucket.low,
            "a 10 kHz tone should read as high-dominant: {treble_bucket:?}"
        );
    }

    #[test]
    fn band_energy_is_normalised() {
        let summary = WaveformSummary::analyse(&sine(48_000, 440.0, 0.9), SR);
        for bucket in summary.level(0).iter().skip(100) {
            let peak = bucket.low.max(bucket.mid).max(bucket.high);
            assert!(
                (peak - 1.0).abs() < 1e-4,
                "band energies should normalise to 1.0, got {bucket:?}"
            );
        }
    }

    #[test]
    fn merging_takes_the_quadratic_mean_of_rms() {
        let loud = Bucket {
            min: -1.0,
            max: 1.0,
            rms: 1.0,
            ..Default::default()
        };
        let quiet = Bucket::default();
        let merged = loud.merged(&quiet);
        // Arithmetic mean would give 0.5; the correct answer is 1/sqrt(2).
        assert!((merged.rms - 0.707).abs() < 0.01, "got {}", merged.rms);
        assert_eq!(merged.max, 1.0);
        assert_eq!(merged.min, -1.0);
    }

    #[test]
    fn an_empty_track_does_not_panic() {
        let summary = WaveformSummary::analyse(&[], SR);
        assert_eq!(summary.total_frames(), 0);
        assert!(summary.bucket_at(0, 0.0).is_silent());
        assert_eq!(summary.level_for(100.0), 0);
    }

    /// **§110's eight bands put a tone where it belongs.** A 50 Hz sine is
    /// strongest in the lowest bands and a 14 kHz one in the highest.
    #[test]
    fn the_spectrum_puts_a_tone_in_its_band() {
        let strongest = |hz: f32| {
            let summary =
                WaveformSummary::analyse_with_spectrum(&sine(96_000, hz, 0.8), SampleRate::DEFAULT);
            let bucket = summary.level(0)[200];
            (0..SPECTRUM_BANDS)
                .max_by_key(|&band| bucket.spectrum[band])
                .unwrap()
        };
        assert!(
            strongest(50.0) <= 1,
            "50 Hz landed in band {}",
            strongest(50.0)
        );
        assert_eq!(strongest(14_000.0), SPECTRUM_BANDS - 1);
        assert_eq!(strongest(1_000.0), 4, "1 kHz is a mid");
    }

    /// **The same energy in every band reads as every band full**, which is
    /// what gives every band of a full mix its room. One tone at the middle of each
    /// band, all at the same level: no band may read less than half of the
    /// strongest.
    #[test]
    fn equal_energy_in_every_band_fills_every_band() {
        let centres = [30.0, 72.0, 172.0, 410.0, 975.0, 2_300.0, 5_500.0, 13_000.0];
        let frames = 96_000;
        let samples: Vec<f32> = (0..frames)
            .flat_map(|n| {
                let t = n as f32 / 48_000.0;
                let v: f32 = centres
                    .iter()
                    .map(|hz| (2.0 * std::f32::consts::PI * hz * t).sin())
                    .sum::<f32>()
                    * 0.1;
                [v, v]
            })
            .collect();
        let summary = WaveformSummary::analyse_with_spectrum(&samples, SampleRate::DEFAULT);
        let bucket = summary.level(0)[200];
        for (band, energy) in bucket.spectrum.iter().enumerate() {
            assert!(
                *energy >= 128,
                "band {band} read {energy}: {:?}",
                bucket.spectrum
            );
        }
    }

    /// **The load path does not pay for the spectrum, and every zoom level
    /// gets it when it lands.** `analyse` leaves it unmeasured — that is what
    /// keeps a load as fast as it was — and `measure_spectrum` fills the
    /// finest level and every coarser one, since the overview draws from the
    /// coarsest.
    #[test]
    fn the_spectrum_arrives_separately_and_reaches_every_zoom_level() {
        let samples = sine(96_000, 14_000.0, 0.8);
        let mut summary = WaveformSummary::analyse(&samples, SampleRate::DEFAULT);
        assert!(
            !summary.has_spectrum(),
            "the load path measured the spectrum"
        );
        summary.measure_spectrum(&samples);
        assert!(summary.has_spectrum());
        for level in 0..summary.level_count() {
            let bucket = summary.level(level)[summary.level(level).len() / 2];
            assert_eq!(
                bucket.spectrum[SPECTRUM_BANDS - 1],
                255,
                "level {level} did not get the spectrum: {:?}",
                bucket.spectrum
            );
        }
    }
}
