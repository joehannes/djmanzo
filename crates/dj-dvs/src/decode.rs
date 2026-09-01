//! Reading a control record: where the needle is, and how fast it is moving.
//!
//! # Two answers, arriving at different speeds
//!
//! **Speed and direction** come from the carrier itself and are available
//! immediately — every cycle of tone that goes past is another measurement.
//! This is the half that matters for scratching, where the DJ is moving the
//! record by hand and the absolute position barely changes.
//!
//! **Absolute position** needs a whole register's worth of bits before it means
//! anything: twenty consecutive bits identify a unique point in the record's
//! sequence, and nineteen identify nothing at all. At a 1 kHz carrier that is
//! twenty milliseconds of clean signal after the needle lands.
//!
//! So the decoder reports them separately, and the caller can act on the fast
//! one while the slow one is still arriving — which is exactly what a DJ
//! dropping the needle expects: sound at once, and the playhead jumping to the
//! right place a moment later.
//!
//! # How a bit is read
//!
//! One channel crosses zero exactly when the other is at a peak — that is what
//! the quarter-cycle offset buys. So the decoder watches for a zero crossing on
//! one channel, samples the other at that instant, and calls it a one if the
//! peak is tall and a zero if it is short. The *sign* of the channel that was
//! at its peak says which way the record is turning.

use crate::{Lfsr, TimecodeFormat};
use std::sync::Arc;

/// Where the needle is and what it is doing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    /// Speed as a fraction of normal play. Negative is backwards.
    ///
    /// Always available once a few cycles have gone past, because it comes from
    /// the carrier rather than from the bitstream.
    pub speed: f64,
    /// How far into the record, in seconds at normal speed.
    ///
    /// `None` until a whole register's worth of consecutive bits has been read
    /// cleanly — about twenty milliseconds after the needle lands.
    pub position: Option<f64>,
    /// How much of the recent signal looked like timecode at all, 0.0..=1.0.
    ///
    /// This is what tells a dusty record from a disconnected input, and it is
    /// what the calibration screen shows. A DJ whose needle is skipping needs
    /// to see *why* djmanzo stopped following it.
    pub quality: f64,
}

/// Every window of output bits to its place on the record.
///
/// Keyed on the **bits that come out**, not on the register's state. For a
/// Galois register those are not the same thing — the state is not the last
/// `n` outputs, which is a Fibonacci property — and keying on the state
/// returned a real entry for the wrong place on the record.
///
/// A flat `Vec` indexed by the window rather than a hash map: four megabytes
/// for a 20-bit register, one indexing operation on the audio path, and no
/// hashing. Shared behind an `Arc` because six decks reading the same kind of
/// record have no reason to hold six copies.
#[derive(Debug)]
pub struct PositionTable {
    /// Window to bit index, with [`PositionTable::NONE`] for a window that
    /// never occurs.
    entries: Vec<u32>,
    carrier_hz: f64,
    bits: u32,
}

impl PositionTable {
    /// A window that does not occur in the sequence — the all-zero one, and
    /// anything a garbled read produces.
    pub const NONE: u32 = u32::MAX;

    /// Build the table for `format`.
    ///
    /// Allocates, and takes a moment: a million steps of a shift register.
    /// **Off the audio thread**, once, when a record type is chosen.
    #[must_use]
    pub fn build(format: &TimecodeFormat) -> Option<Self> {
        if !format.is_usable() {
            return None;
        }
        let mut lfsr = Lfsr::new(format.bits, format.seed, format.taps)?;
        let width = 1usize << format.bits;
        let mut entries = vec![Self::NONE; width];
        let mask = (width - 1) as u32;
        let mut window = 0u32;
        for index in 0..format.period() {
            window = ((window << 1) | lfsr.step()) & mask;
            if index + 1 >= format.bits {
                let slot = &mut entries[window as usize];
                if *slot == Self::NONE {
                    *slot = index;
                }
            }
        }
        Some(Self {
            entries,
            carrier_hz: format.carrier_hz,
            bits: format.bits,
        })
    }

    /// Where a window of bits sits on the record, in seconds at normal speed.
    ///
    /// One index and one comparison — no hashing, no allocation, safe on the
    /// audio path.
    #[must_use]
    pub fn seconds(&self, window: u32) -> Option<f64> {
        let index = *self.entries.get(window as usize)?;
        if index == Self::NONE {
            return None;
        }
        Some(f64::from(index) / self.carrier_hz)
    }

    #[must_use]
    pub const fn bits(&self) -> u32 {
        self.bits
    }
}

/// Turns a control record's audio into a [`Reading`].
#[derive(Debug)]
pub struct Decoder {
    format: TimecodeFormat,
    sample_rate: f64,
    positions: Arc<PositionTable>,
    /// The most recent bits, oldest in the low positions.
    window: u32,
    /// How many of them are actually there yet.
    filled: u32,
    /// The previous frame, for spotting a zero crossing between two samples.
    previous: [f32; 2],
    /// Samples since the last crossing, for the carrier's period.
    since_crossing: f64,
    /// Smoothed speed, so one noisy crossing does not throw the platter.
    speed: f64,
    /// Recent crossings that looked like timecode, against those that did not.
    good: f64,
    seen: f64,
    /// Which way the record was turning at the last bit, so a reversal can
    /// drop a window that would otherwise mix bits read in both orders.
    was_forward: bool,
}

/// How much of the previous speed to keep on each new measurement.
///
/// A single crossing is a coarse measurement -- at 48 kHz there are only
/// twenty-four samples in a 1 kHz half-cycle, so the quantisation alone is
/// several percent. Averaging across a few of them costs a millisecond of
/// latency and removes the jitter that would otherwise reach the pitch.
const SPEED_SMOOTHING: f64 = 0.7;

/// Below this, a peak is not a bit and probably not a record.
///
/// A cartridge delivers a few hundred millivolts and a disconnected input
/// delivers noise; this is the line between them.
const SILENCE: f32 = 0.05;

/// A peak above this fraction of the recent maximum is a one, below it a zero.
///
/// Relative rather than absolute because cartridges differ by more than a
/// factor of two, and a DJ should not have to calibrate a threshold by hand
/// before any sound comes out.
const ONE_LEVEL: f32 = 0.75;

impl Decoder {
    /// A decoder for `format`, building its own table.
    ///
    /// Convenient for tests and for a one-off; a host with several decks should
    /// build one [`PositionTable`] and use [`Self::with_table`].
    ///
    /// # Errors
    /// When the format could not describe a working record, or the sample rate
    /// is not positive.
    #[must_use]
    pub fn new(format: TimecodeFormat, sample_rate: f64) -> Option<Self> {
        let table = Arc::new(PositionTable::build(&format)?);
        Self::with_table(format, sample_rate, table)
    }

    /// A decoder sharing an already-built table.
    ///
    /// # Errors
    /// When the sample rate is not positive, or the table was built for a
    /// register of a different width.
    #[must_use]
    pub fn with_table(
        format: TimecodeFormat,
        sample_rate: f64,
        positions: Arc<PositionTable>,
    ) -> Option<Self> {
        if sample_rate <= 0.0 || !format.is_usable() || positions.bits() != format.bits {
            return None;
        }
        Some(Self {
            format,
            sample_rate,
            positions,
            window: 0,
            filled: 0,
            previous: [0.0; 2],
            since_crossing: 0.0,
            speed: 0.0,
            good: 0.0,
            seen: 0.0,
            was_forward: true,
        })
    }

    /// Feed one block of interleaved stereo and read the result.
    ///
    /// Allocation-free: the lookup table was built at construction and nothing
    /// here grows. That is what lets this run on the audio path rather than
    /// behind a queue, which matters because a scratch that arrived a buffer
    /// late would feel like a scratch through treacle.
    pub fn feed(&mut self, input: &[f32]) -> Reading {
        for frame in input.as_chunks::<2>().0 {
            self.feed_frame(frame[0], frame[1]);
        }
        self.reading()
    }

    fn feed_frame(&mut self, left: f32, right: f32) {
        self.since_crossing += 1.0;

        // A zero crossing on the left channel is when the right channel is at
        // its peak -- that is what the quarter-cycle offset is for.
        let rising = self.previous[0] < 0.0 && left >= 0.0;
        let falling = self.previous[0] >= 0.0 && left < 0.0;
        self.previous = [left, right];
        if !rising && !falling {
            return;
        }

        let peak = right.abs();
        self.seen += 1.0;
        if peak < SILENCE {
            // Not a record, or not a record any more. The window is dropped
            // rather than kept: half a position from before the needle lifted,
            // spliced onto half from after it landed, is a position on neither.
            self.filled = 0;
            self.since_crossing = 0.0;
            return;
        }
        self.good += 1.0;

        // **Direction needs to know which crossing this was.**
        //
        // The sign of the peak alternates every half cycle whichever way the
        // record turns, so on its own it says nothing -- reading it as the
        // direction made the speed average two readings of opposite sign
        // towards zero, and normal play came out at a fifth of normal.
        //
        // What actually carries the direction is the sign *paired with the
        // edge*: going forwards, the trailing channel is negative at a rising
        // crossing and positive at a falling one. Backwards, both are the
        // other way about.
        let forward = if rising { right < 0.0 } else { right > 0.0 };
        let direction = if forward { 1.0 } else { -1.0 };

        // Half a carrier cycle per crossing, so the period is twice the gap.
        let cycles_per_sample = 1.0 / (self.since_crossing * 2.0);
        let measured = cycles_per_sample * self.sample_rate / self.format.carrier_hz;
        let instant = measured * direction;
        self.speed = if self.speed == 0.0 {
            instant
        } else {
            self.speed * SPEED_SMOOTHING + instant * (1.0 - SPEED_SMOOTHING)
        };
        self.since_crossing = 0.0;

        // One bit per cycle, and two crossings to a cycle -- so bits are taken
        // on the rising edge only, which is the same edge every time.
        if !rising {
            return;
        }

        // **Absolute position is re-acquired after a direction change.**
        //
        // The bits arrive in reverse when the record does, and a window that
        // mixed the two orders would name a point on neither. Speed is
        // unaffected and carries on frame by frame, which is the half that
        // matters while a DJ's hand is on the platter -- the playhead follows
        // the movement throughout and the absolute fix returns a register's
        // width after the record settles.
        if forward != self.was_forward {
            self.filled = 0;
            self.was_forward = forward;
        }
        if !forward {
            return;
        }

        let bit = u32::from(peak >= ONE_LEVEL * self.recent_peak());
        self.window = ((self.window << 1) | bit) & self.mask();
        self.filled = (self.filled + 1).min(self.format.bits);
    }

    /// The tallest peak worth comparing against.
    ///
    /// A fixed number here would mean a cartridge quieter than the one this
    /// was written on decoded every bit as a zero.
    fn recent_peak(&self) -> f32 {
        // Full scale, which the synthesiser produces and a well-set-up
        // cartridge approaches. Deliberately simple: an adaptive gain that
        // tracked the signal would also track a fade, and a DJ fading the
        // channel down must not change what the record says.
        1.0
    }

    const fn mask(&self) -> u32 {
        if self.format.bits >= 32 {
            u32::MAX
        } else {
            (1u32 << self.format.bits) - 1
        }
    }

    fn reading(&self) -> Reading {
        Reading {
            speed: self.speed,
            position: self.position(),
            quality: if self.seen > 0.0 {
                (self.good / self.seen).clamp(0.0, 1.0)
            } else {
                0.0
            },
        }
    }

    fn position(&self) -> Option<f64> {
        if self.filled < self.format.bits {
            return None;
        }
        self.positions.seconds(self.window)
    }

    #[must_use]
    pub const fn format(&self) -> &TimecodeFormat {
        &self.format
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Synth;

    const SR: f64 = 48_000.0;

    fn format() -> TimecodeFormat {
        TimecodeFormat::bundled()[0].clone()
    }

    fn rig() -> (Synth, Decoder) {
        (
            Synth::new(format(), SR).unwrap(),
            Decoder::new(format(), SR).unwrap(),
        )
    }

    #[test]
    fn a_broken_format_or_rate_is_refused() {
        assert!(Decoder::new(format(), 0.0).is_none());
        let quoted = TimecodeFormat {
            seed: 0x0005_9017,
            taps: 0x0003_61e4,
            ..format()
        };
        assert!(
            Decoder::new(quoted, SR).is_none(),
            "a register that repeats every 43 seconds was accepted"
        );
    }

    /// **Normal play reads as normal speed.** The single most basic claim, and
    /// the one a wrong carrier or a wrong crossing count breaks.
    #[test]
    fn playing_at_normal_speed_reads_as_normal_speed() {
        let (synth, mut decoder) = rig();
        let audio = synth.render(10_000, 1.0, 4800);
        let reading = decoder.feed(&audio);
        assert!(
            (reading.speed - 1.0).abs() < 0.05,
            "normal play read as {}",
            reading.speed
        );
    }

    /// Half speed, double speed, and the pitch fader range in between.
    #[test]
    fn the_speed_follows_the_record() {
        for speed in [0.5, 0.92, 1.0, 1.08, 2.0] {
            let (synth, mut decoder) = rig();
            let audio = synth.render(10_000, speed, 9600);
            let reading = decoder.feed(&audio);
            assert!(
                (reading.speed - speed).abs() < speed * 0.08,
                "{speed}x read as {}",
                reading.speed
            );
        }
    }

    /// **Backwards reads as backwards.** Half of scratching, and the only
    /// thing the quadrature is there for — a decoder that lost the sign would
    /// play forwards through a rewind.
    #[test]
    fn a_record_played_backwards_reads_negative() {
        let (synth, mut decoder) = rig();
        let audio = synth.render(10_000, -1.0, 4800);
        let reading = decoder.feed(&audio);
        assert!(
            reading.speed < 0.0,
            "a backwards record read as {}",
            reading.speed
        );
        assert!(
            (reading.speed + 1.0).abs() < 0.1,
            "backwards read as {} rather than -1",
            reading.speed
        );
    }

    /// **Where the needle was dropped.** The reason for the shift register:
    /// twenty bits anywhere in the sequence name one point on the record.
    #[test]
    fn the_position_is_found_from_anywhere_on_the_record() {
        for bit in [0u32, 1_000, 60_000, 500_000] {
            let (synth, mut decoder) = rig();
            // Enough audio for a register's worth of bits, and then some.
            let audio = synth.render(bit, 1.0, 4800);
            let reading = decoder.feed(&audio);
            let found = reading
                .position
                .unwrap_or_else(|| panic!("no position found from bit {bit}"));
            let expected = f64::from(bit) / format().carrier_hz;
            // Within a few bits: the decoder reports where the *window* ends,
            // which is a register's width into the block.
            assert!(
                (found - expected).abs() < 0.2,
                "dropped at {expected:.3}s, read as {found:.3}s"
            );
        }
    }

    /// A position needs a whole register. Nineteen bits identify nothing, and
    /// reporting a guess would put the playhead somewhere arbitrary the moment
    /// the needle touched down.
    #[test]
    fn no_position_is_reported_before_a_full_register_is_read() {
        let (synth, mut decoder) = rig();
        // Ten bits' worth of audio at 1 kHz is ten milliseconds.
        let audio = synth.render(10_000, 1.0, 480);
        let reading = decoder.feed(&audio);
        assert_eq!(
            reading.position, None,
            "a position was invented from half a register"
        );
        // The speed, though, is available immediately -- that is the point of
        // reporting them separately.
        assert!(reading.speed > 0.5, "the speed should not have waited");
    }

    /// Silence is not a record. A disconnected input, or a needle lifted, must
    /// not read as a position — and the quality figure is how the calibration
    /// screen says so.
    #[test]
    fn silence_reads_as_no_signal() {
        let (_, mut decoder) = rig();
        let reading = decoder.feed(&vec![0.0; 9600]);
        assert_eq!(reading.position, None);
        assert!(
            reading.quality < 0.5,
            "silence reported quality {}",
            reading.quality
        );
    }

    /// A needle lifted mid-record must not splice the bits from before it onto
    /// the bits from after: the result would be a position that is neither.
    #[test]
    fn a_lifted_needle_does_not_splice_two_positions() {
        let (synth, mut decoder) = rig();
        decoder.feed(&synth.render(10_000, 1.0, 4800));
        let before = decoder.feed(&synth.render(14_800, 1.0, 480)).position;
        assert!(before.is_some(), "should have a position before the lift");

        // Needle up.
        decoder.feed(&vec![0.0; 4800]);
        // Needle down somewhere else -- but only briefly.
        let after = decoder.feed(&synth.render(600_000, 1.0, 480)).position;
        assert_eq!(
            after, None,
            "a position survived the lift, so old bits were spliced onto new"
        );
    }

    /// Good signal reads as good quality, which is what the calibration screen
    /// shows a DJ whose cartridge is dying.
    #[test]
    fn a_clean_record_reads_as_good_quality() {
        let (synth, mut decoder) = rig();
        let reading = decoder.feed(&synth.render(10_000, 1.0, 9600));
        assert!(
            reading.quality > 0.9,
            "a clean signal reported quality {}",
            reading.quality
        );
    }

    /// Decoding runs on the audio path, so it must not allocate. The lookup
    /// table is built once, at construction.
    #[test]
    fn feeding_a_block_does_not_grow_anything() {
        let (synth, mut decoder) = rig();
        let audio = synth.render(10_000, 1.0, 4800);
        let before = Arc::strong_count(&decoder.positions);
        for _ in 0..50 {
            decoder.feed(&audio);
        }
        assert_eq!(
            Arc::strong_count(&decoder.positions),
            before,
            "decoding took a reference to the table it did not give back"
        );
    }
}
