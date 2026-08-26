//! Writing a timecode signal, which is what makes reading one testable.
//!
//! # Why a synthesiser is not a test fixture
//!
//! It is the only way this crate can be verified at all. There is no turntable
//! here and no pressed record; without something that produces a known signal
//! there is nothing to decode, and a decoder tested against nothing is a
//! decoder that compiles.
//!
//! It also earns its place at runtime twice over: it is how djmanzo can burn a
//! control CD or generate a timecode file for someone who has a turntable but
//! no record, and it is how the calibration screen shows a DJ what a *correct*
//! signal looks like beside the one their cartridge is actually delivering.
//!
//! # The signal
//!
//! Two channels of the same tone, a quarter cycle apart. The quarter cycle is
//! the direction sense: with the record going forwards one channel's peak
//! arrives before the other's, and backwards it arrives after. Nothing else in
//! the signal knows which way the platter is turning.
//!
//! The bitstream rides on the amplitude — a full-height cycle is a one, a
//! half-height cycle is a zero — so every cycle of carrier carries exactly one
//! bit, and the record's resolution is its carrier frequency.

use crate::{Lfsr, TimecodeFormat};

/// Amplitude of a cycle carrying a zero, relative to one carrying a one.
///
/// Not zero. A zero bit still has to be a *cycle*, or the decoder would lose
/// the carrier — and with it the speed and the direction — every time the
/// bitstream happened to produce a run of zeros.
const ZERO_LEVEL: f32 = 0.5;

/// Generates the signal a control record carries.
#[derive(Debug, Clone)]
pub struct Synth {
    format: TimecodeFormat,
    sample_rate: f64,
    /// The record's whole bitstream, one byte per bit.
    ///
    /// A megabyte for a 20-bit register, built once. The alternative is
    /// walking the register from its seed for every sample, which is fine at
    /// the start of a record and takes half a million steps per sample by the
    /// end of one.
    bits: Vec<u8>,
}

impl Synth {
    /// # Errors
    /// When the format could not describe a working record, or the sample rate
    /// is not positive.
    pub fn new(format: TimecodeFormat, sample_rate: f64) -> Option<Self> {
        if !format.is_usable() || sample_rate <= 0.0 {
            return None;
        }
        let mut lfsr = Lfsr::new(format.bits, format.seed, format.taps)?;
        let bits = (0..format.period())
            .map(|_| u8::try_from(lfsr.step()).unwrap_or(0))
            .collect();
        Some(Self {
            format,
            sample_rate,
            bits,
        })
    }

    /// Interleaved stereo timecode for `frames`, starting `from_bit` into the
    /// record and running at `speed` times normal.
    ///
    /// A negative `speed` is the record going backwards, which is not an edge
    /// case — it is half of scratching.
    #[must_use]
    pub fn render(&self, from_bit: u32, speed: f64, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        self.render_into(&mut out, from_bit, speed);
        out
    }

    /// As [`Self::render`], into a buffer the caller owns.
    pub fn render_into(&self, out: &mut [f32], from_bit: u32, speed: f64) {
        let period = f64::from(self.format.period());
        let cycles_per_sample = self.format.carrier_hz * speed / self.sample_rate;

        for (index, frame) in out.chunks_exact_mut(2).enumerate() {
            // How far into the bitstream this sample sits. One cycle of
            // carrier is one bit, so the cycle count *is* the bit position.
            #[allow(clippy::cast_precision_loss)]
            let cycles = f64::from(from_bit) + cycles_per_sample * index as f64;
            frame[0] = self.sample_at(cycles, period);
            // **The whole waveform** a quarter cycle behind, envelope included
            // -- not just the carrier.
            //
            // Both channels are cut from one groove modulation, so the
            // amplitude that carries the bits is offset by the same quarter
            // cycle the tone is. Shifting only the carrier would leave the two
            // channels disagreeing about the bit across every transition,
            // which is a signal no record produces.
            frame[1] = self.sample_at(cycles - 0.25, period);
        }
    }

    /// One sample of the signal, `cycles` into the record.
    fn sample_at(&self, cycles: f64, period: f64) -> f32 {
        let bit_index = cycles.rem_euclid(period);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let level = if self.bit_at(bit_index as u32) == 1 {
            1.0
        } else {
            f64::from(ZERO_LEVEL)
        };
        #[allow(clippy::cast_possible_truncation)]
        {
            (level * (std::f64::consts::TAU * cycles).sin()) as f32
        }
    }

    /// The bit `index` places into the record's sequence.
    #[must_use]
    pub fn bit_at(&self, index: u32) -> u32 {
        self.bits
            .get(index as usize % self.bits.len().max(1))
            .map_or(0, |bit| u32::from(*bit))
    }

    #[must_use]
    pub const fn format(&self) -> &TimecodeFormat {
        &self.format
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format() -> TimecodeFormat {
        TimecodeFormat::bundled()[0].clone()
    }

    #[test]
    fn a_broken_format_or_rate_is_refused() {
        assert!(Synth::new(format(), 0.0).is_none());
        assert!(Synth::new(format(), -48_000.0).is_none());
        let broken = TimecodeFormat {
            seed: 0,
            ..format()
        };
        assert!(Synth::new(broken, 48_000.0).is_none());
    }

    /// **The quarter cycle is the direction**, so the two channels must not be
    /// the same signal. A synthesiser that emitted one channel twice would
    /// produce audio that looked right and could never be told from a record
    /// spinning backwards.
    #[test]
    fn the_two_channels_are_a_quarter_cycle_apart() {
        let synth = Synth::new(format(), 48_000.0).unwrap();
        let audio = synth.render(0, 1.0, 480);

        let left: Vec<f32> = audio.chunks_exact(2).map(|f| f[0]).collect();
        let right: Vec<f32> = audio.chunks_exact(2).map(|f| f[1]).collect();
        assert_ne!(left, right, "both channels carried the same signal");

        // A quarter of a 1 kHz cycle at 48 kHz is twelve samples. Shifting the
        // right channel back by that should line it up with the left.
        let shift = 12;
        let mut worst = 0.0f32;
        for index in shift..left.len() {
            worst = worst.max((left[index - shift] - right[index]).abs());
        }
        assert!(
            worst < 0.05,
            "the channels are not a quarter cycle apart: worst {worst}"
        );
    }

    /// Backwards is not a special case; it is half of scratching. The channels
    /// swap which one leads.
    #[test]
    fn reversing_the_record_swaps_which_channel_leads() {
        let synth = Synth::new(format(), 48_000.0).unwrap();
        let forward = synth.render(5_000, 1.0, 480);
        let backward = synth.render(5_000, -1.0, 480);

        // With the direction reversed, the right channel now leads instead of
        // trailing -- so the *forward* alignment should no longer hold.
        let shift = 12;
        let lead = |audio: &[f32]| {
            let left: Vec<f32> = audio.chunks_exact(2).map(|f| f[0]).collect();
            let right: Vec<f32> = audio.chunks_exact(2).map(|f| f[1]).collect();
            let mut worst = 0.0f32;
            for index in shift..left.len() {
                worst = worst.max((left[index - shift] - right[index]).abs());
            }
            worst
        };
        assert!(lead(&forward) < 0.05, "forward should line up");
        assert!(
            lead(&backward) > 0.5,
            "backward lined up the same way as forward, so direction is not encoded"
        );
    }

    /// A zero bit is a quieter cycle, not silence. Silence would lose the
    /// carrier -- and with it the speed and the direction -- on any run of
    /// zeros the bitstream happened to produce.
    #[test]
    fn a_zero_bit_is_quieter_rather_than_silent() {
        let synth = Synth::new(format(), 48_000.0).unwrap();

        // The left channel over one cycle starting at `from` -- the right is a
        // quarter cycle behind and is still rendering the bit before, which is
        // exactly the confusion this measurement has to avoid.
        let left_peak = |from: u32| {
            synth
                .render(from, 1.0, 48)
                .chunks_exact(2)
                .map(|frame| frame[0].abs())
                .fold(0.0f32, f32::max)
        };

        let zero_at = (0..4000).find(|index| synth.bit_at(*index) == 0).unwrap();
        let one_at = (0..4000).find(|index| synth.bit_at(*index) == 1).unwrap();

        let quiet = left_peak(zero_at);
        let loud = left_peak(one_at);
        assert!(
            quiet > 0.2,
            "a zero bit came out silent, which loses the carrier: peak {quiet}"
        );
        assert!(
            quiet < loud * 0.8,
            "a zero bit ({quiet}) was not quieter than a one ({loud})"
        );
    }

    /// The bitstream is the record's own sequence, not something invented per
    /// call: asking twice gives the same answer.
    #[test]
    fn the_bitstream_is_stable() {
        let synth = Synth::new(format(), 48_000.0).unwrap();
        let first: Vec<u32> = (0..64).map(|index| synth.bit_at(index)).collect();
        let again: Vec<u32> = (0..64).map(|index| synth.bit_at(index)).collect();
        assert_eq!(first, again);
        assert!(first.contains(&1), "no ones at all");
        assert!(first.contains(&0), "no zeros at all");
    }
}
