//! Where somebody is singing: [§25](../../../docs/DIRECTIVE.md)'s `vocal`
//! layer, and the *vocal density* [§75](../../../docs/DIRECTIVE.md) asks for.
//!
//! # The thing djmanzo already had
//!
//! Both rows have carried the same reason for as long as they have existed —
//! *the analysis does not exist* — and for as long as they have, djmanzo has
//! shipped a separator that pulls a vocal stem out of a record with no model,
//! no download and no runtime: [`dj_stems::hpss`]. A DJ can solo the voice.
//! The waveform could not draw where it was.
//!
//! # What is measured, and what it is not
//!
//! The separator's definition of a lead vocal is *the harmonic part of the
//! centre channel between [`VOICE_BOTTOM_HZ`] and [`VOICE_TOP_HZ`]*. This
//! measures how much of a record's energy is that, moment to moment, and it
//! does it **without reconstructing anything**: a presence curve is a question
//! about the masks, not about the audio, so there is no inverse transform, no
//! overlap-add and no second copy of the record in memory.
//!
//! The band edges come from `dj_core` rather than from either module, because
//! they are one fact asked twice and a fact defined twice drifts.
//!
//! # Where this asks a sharper question than the separator does
//!
//! On one point it deliberately differs. The separator has to produce audio
//! that sums back to the mix, so its centre is the mid channel as it finds it
//! -- and a hard-panned guitar appears in mid at half amplitude whether anyone
//! wanted it there or not. Written as a share, that guitar reads as half a
//! voice, which was the first thing the tests below caught.
//!
//! A *measurement* is under no such constraint. It can ask how centred a bin
//! actually is -- mid against side, where a centred source has no side at all
//! and a hard-panned one has as much side as mid -- and weight the bin by the
//! answer. So the curve is sharper than the stem a DJ would hear if they
//! soloed the voice, and that is the right way round: the stem has a job the
//! reading does not.
//!
//! Where it is still wrong is the separator's own list, and it is worth
//! repeating rather than burying: **a centred synth lead reads as a voice**, a
//! mono recording has no sides so everything centred takes the whole middle,
//! and a record mixed with the vocal off-centre reads as having less of one
//! than it has. It is a real measurement of a real thing, and the thing is "a
//! lead sitting where a singer sits" rather than "a singer".
//!
//! # The one number that is not measured
//!
//! Everything above is arithmetic over the audio. [`STRONG`] is not: it is the
//! share at which djmanzo will say a lead is *carrying* the record, and that
//! can only be set by listening to real records with real voices on them. The
//! machine this was written on has none — only synthesised test signals — so
//! it is a stated guess rather than a measurement, and it is **one** constant
//! rather than one per consumer so that whoever does the listening has a
//! single number to change.
//!
//! The curve itself needs no such number and does not use one: it is a share,
//! and the overview draws **how much** rather than **yes or no**.

use crate::onset::{HOP, WINDOW};
use dj_core::{VOICE_BOTTOM_HZ, VOICE_TOP_HZ};
use rustfft::{FftPlanner, num_complex::Complex32};

/// Median filter length along time, in frames. Odd, so there is a middle.
///
/// Seventeen frames is about 180 ms at this hop — longer than any transient
/// and shorter than a held note, which is the whole distinction being drawn.
/// The same span the separator uses, for the same reason.
const TIME_SPAN: usize = 17;

/// Median filter length along frequency, in bins. Odd.
const FREQ_SPAN: usize = 17;

/// The share at which a lead is unmistakably carrying the record.
///
/// **A stated guess, not a measurement** — see the module docs. Two things
/// read it and they are deliberately the same number: the overview saturates
/// its vocal strip here, and [`Presence::enters`] calls this the point at
/// which a voice has arrived. Two constants would be two answers to *is
/// somebody singing*, drawn a few pixels apart on one screen.
pub const STRONG: f32 = 0.25;

/// Guards a denominator. Below this there is nothing playing and the share of
/// nothing is nothing, rather than a division by very nearly zero.
const EPSILON: f32 = 1e-12;

/// How much of a record is a centred, sustained voice-range signal, over time.
///
/// One value per hop of the onset curve, so an index here and an index there
/// name the same moment. Each is a share of that moment's total energy, in
/// `0.0..=1.0`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Presence {
    /// One value per hop, `0.0..=1.0`.
    pub values: Vec<f32>,
    /// Hops per second — the sample rate of `values`.
    pub rate: f64,
}

impl Presence {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The mean share across a stretch of the record, in frames.
    ///
    /// `None` when nothing was measured there — an empty curve, or a span that
    /// falls off the end of one. Absence rather than zero, because "nothing
    /// was measured" and "nobody was singing" are different answers and the
    /// interface draws them differently.
    #[must_use]
    pub fn mean_between(&self, from: f64, to: f64) -> Option<f32> {
        if self.values.is_empty() || to <= from {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let index = |frame: f64| -> usize { (frame.max(0.0) / HOP as f64) as usize };
        let start = index(from);
        // At least one hop wide: a window shorter than 11 ms still happened.
        let end = index(to).max(start + 1).min(self.values.len());
        if start >= self.values.len() {
            return None;
        }
        let slice = &self.values[start..end];
        #[allow(clippy::cast_precision_loss)]
        Some(slice.iter().sum::<f32>() / slice.len() as f32)
    }
}

/// Measure where the voice is. See [`Presence`].
///
/// Interleaved stereo, and worker-thread work like everything else in this
/// crate: one transform pair per hop over the whole record.
#[must_use]
pub fn presence(samples: &[f32], sample_rate: u32) -> Presence {
    let frames = samples.len() / 2;
    let rate = f64::from(sample_rate) / HOP as f64;
    if sample_rate == 0 || frames < WINDOW {
        return Presence {
            values: Vec::new(),
            rate,
        };
    }

    // Mid and side rather than left and right, because the question is "is
    // this centred" and that is a question about the pair. Asking it of each
    // ear separately answers a different question twice.
    let mut mid = Vec::with_capacity(frames);
    let mut side = Vec::with_capacity(frames);
    for frame in samples.as_chunks::<2>().0 {
        mid.push((frame[0] + frame[1]) * 0.5);
        side.push((frame[0] - frame[1]) * 0.5);
    }

    let window = hann(WINDOW);
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(WINDOW);
    let bins = WINDOW / 2 + 1;

    let hz_per_bin = sample_rate as f32 / WINDOW as f32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let bottom = ((VOICE_BOTTOM_HZ / hz_per_bin).round() as usize).min(bins);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let top = ((VOICE_TOP_HZ / hz_per_bin).round() as usize).min(bins);
    if bottom >= top {
        // The band does not fit below Nyquist: an 8 kHz sample rate has no
        // room for a voice's harmonics, so nothing is claimed about one.
        return Presence {
            values: Vec::new(),
            rate,
        };
    }

    // A ring of the last `TIME_SPAN` magnitude frames, so the median along
    // time costs a fixed 70 kB rather than a copy of the whole spectrogram —
    // which for a twenty-minute set would be a hundred megabytes to answer a
    // question about seventeen frames at a time.
    let mut ring = vec![vec![0.0f32; bins]; TIME_SPAN];
    let mut sides = vec![vec![0.0f32; bins]; TIME_SPAN];
    let mut totals = [0.0f32; TIME_SPAN];
    let mut filled = 0usize;
    let mut scratch = vec![Complex32::new(0.0, 0.0); WINDOW];
    let mut medians = Medians::new();
    let mut values: Vec<f32> = Vec::with_capacity(frames / HOP);

    let mut start = 0;
    while start + WINDOW <= frames {
        let slot = filled % TIME_SPAN;

        for (i, cell) in scratch.iter_mut().enumerate() {
            *cell = Complex32::new(mid[start + i] * window[i], 0.0);
        }
        fft.process(&mut scratch);
        let mut total = 0.0f32;
        for bin in 0..bins {
            let magnitude = scratch[bin].norm();
            ring[slot][bin] = magnitude;
            total += magnitude * magnitude;
        }

        // The side channel is transformed rather than summed in the time
        // domain, so both halves of the share are the same kind of quantity
        // summed the same way. Halving the cost by invoking Parseval would put
        // a normalisation constant between the numerator and the denominator,
        // and a wrong constant there is a curve that is quietly scaled.
        for (i, cell) in scratch.iter_mut().enumerate() {
            *cell = Complex32::new(side[start + i] * window[i], 0.0);
        }
        fft.process(&mut scratch);
        for bin in 0..bins {
            let magnitude = scratch[bin].norm();
            sides[slot][bin] = magnitude;
            total += magnitude * magnitude;
        }
        totals[slot] = total;

        filled += 1;
        if filled >= TIME_SPAN {
            // The middle of the ring: a median along time is centred on the
            // frame it describes, so the answer trails the audio by half a
            // span.
            let centre = (filled - TIME_SPAN / 2 - 1) % TIME_SPAN;
            values.push(share(
                &Moment {
                    mid: &ring,
                    side: &sides,
                    centre,
                    total: totals[centre],
                },
                &mut medians,
                bottom..top,
            ));
        }
        start += HOP;
    }

    // The half-span at each end has no full ring, so it takes the nearest
    // value that does. Eight hops is 85 ms: shorter than a syllable, and the
    // alternative is a curve whose index no longer names the same moment as
    // the onset curve's.
    pad_edges(&mut values, (frames - WINDOW) / HOP + 1);

    Presence { values, rate }
}

/// One moment of the spectrogram, with the frames either side of it in time.
///
/// A struct rather than six arguments: the four of them are one thing --
/// *which frame, and what is around it* -- and a caller that passed the mid
/// ring with the side ring's centre index would be asking a question nobody
/// meant.
struct Moment<'a> {
    /// The mid channel's magnitudes, `TIME_SPAN` frames of them in a ring.
    mid: &'a [Vec<f32>],
    /// The side channel's, over the same frames.
    side: &'a [Vec<f32>],
    /// Which frame of the ring is the one being described.
    centre: usize,
    /// The centre frame's total energy, mid and side together.
    total: f32,
}

/// Where the medians are taken, so a whole record's worth of them allocates
/// nothing.
struct Medians {
    column: [f32; TIME_SPAN],
    neighbours: [f32; FREQ_SPAN],
}

impl Medians {
    const fn new() -> Self {
        Self {
            column: [0.0; TIME_SPAN],
            neighbours: [0.0; FREQ_SPAN],
        }
    }
}

/// How much of one frame is centred, sustained and in the band.
fn share(moment: &Moment, medians: &mut Medians, band: std::ops::Range<usize>) -> f32 {
    if moment.total <= EPSILON {
        return 0.0;
    }
    let frame = &moment.mid[moment.centre];
    let side = &moment.side[moment.centre];
    let bins = frame.len();
    let mut voiced = 0.0f32;
    for bin in band {
        // Sustained: what survives a median along time. A held note is the
        // same bin frame after frame; a drum hit is one frame of everything.
        for (i, cell) in medians.column.iter_mut().enumerate() {
            *cell = moment.mid[i][bin];
        }
        let harmonic = median(&mut medians.column);

        // Percussive: what survives a median along frequency, in this frame.
        // A broadband hit is flat across bins and a harmonic peak is not.
        let half = FREQ_SPAN / 2;
        for (i, cell) in medians.neighbours.iter_mut().enumerate() {
            let at = (bin + i).saturating_sub(half).min(bins - 1);
            *cell = frame[at];
        }
        let percussive = median(&mut medians.neighbours);

        // The separator's own mask, and deliberately the same one: a soft
        // split rather than a winner, so a bin that is both does not flip its
        // whole content between answers as the two estimates cross.
        let (h, p) = (harmonic * harmonic, percussive * percussive);
        let mask = h / (h + p + EPSILON);

        // Centred: no side at all is dead centre, as much side as mid is hard
        // over. Anything panned past the middle contributes less than its
        // energy rather than half of it -- see the module docs.
        let (m, s) = (frame[bin], side[bin]);
        let centred = ((m - s) / (m + s + EPSILON)).clamp(0.0, 1.0);

        voiced += mask * centred * m * m;
    }
    (voiced / moment.total).clamp(0.0, 1.0)
}

/// Median of a small slice, in place.
fn median(values: &mut [f32]) -> f32 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    values[values.len() / 2]
}

/// Grow a curve back to one value per hop by repeating its ends.
fn pad_edges(values: &mut Vec<f32>, hops: usize) {
    if values.is_empty() || values.len() >= hops {
        values.truncate(hops);
        return;
    }
    let before = TIME_SPAN / 2;
    let first = values[0];
    let last = values[values.len() - 1];
    values.splice(
        0..0,
        std::iter::repeat_n(first, before.min(hops - values.len())),
    );
    while values.len() < hops {
        values.push(last);
    }
}

fn hann(len: usize) -> Vec<f32> {
    #[allow(clippy::cast_precision_loss)]
    (0..len)
        .map(|n| {
            let phase = std::f32::consts::TAU * n as f32 / len as f32;
            0.5 * (1.0 - phase.cos())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: u32 = 48_000;

    /// Interleaved stereo from a per-channel generator.
    fn stereo(frames: usize, mut f: impl FnMut(usize) -> (f32, f32)) -> Vec<f32> {
        let mut out = Vec::with_capacity(frames * 2);
        for n in 0..frames {
            let (l, r) = f(n);
            out.push(l);
            out.push(r);
        }
        out
    }

    /// A sustained tone, at a pan. `pan` 0.0 is centred, 1.0 is hard left.
    fn tone(hz: f32, frames: usize, pan: f32) -> Vec<f32> {
        stereo(frames, |n| {
            #[allow(clippy::cast_precision_loss)]
            let v = (TAU * hz * n as f32 / SR as f32).sin() * 0.5;
            (v * (1.0 + pan), v * (1.0 - pan))
        })
    }

    fn seconds(n: f32) -> usize {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            (SR as f32 * n) as usize
        }
    }

    fn mean(curve: &Presence) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        {
            curve.values.iter().sum::<f32>() / curve.values.len() as f32
        }
    }

    /// The load-bearing test: each of the four claims the measurement makes,
    /// against a signal that breaks exactly one of them.
    ///
    /// A centred, sustained, in-band tone is what the separator calls a lead
    /// vocal. Break the *centred*, the *sustained* or the *in-band* and the
    /// reading has to fall away — otherwise this is measuring "is anything
    /// playing", which every record answers yes to.
    #[test]
    fn only_a_centred_sustained_in_band_tone_reads_as_a_voice() {
        let frames = seconds(3.0);
        let voice = mean(&presence(&tone(440.0, frames, 0.0), SR));

        // Not centred: the same note, hard left. A thing in one ear is not a
        // lead vocal, and the separator sends it to `other`.
        let panned = mean(&presence(&tone(440.0, frames, 1.0), SR));

        // Not in band: below `VOICE_BOTTOM_HZ`, which is a bassline.
        let bass = mean(&presence(&tone(80.0, frames, 0.0), SR));

        // Not sustained: centred clicks, which are a drum.
        let clicks = mean(&presence(
            &stereo(frames, |n| {
                let v = if n % 4_800 < 24 { 0.5 } else { 0.0 };
                (v, v)
            }),
            SR,
        ));

        assert!(
            voice > 0.5,
            "a centred sung note should read high, got {voice}"
        );
        for (name, other) in [("panned", panned), ("bass", bass), ("clicks", clicks)] {
            assert!(
                other < voice / 2.0,
                "{name} reads {other} against a voice's {voice}; \
                 this is measuring whether anything is playing"
            );
        }
    }

    /// The claim the four-signal test above does *not* pin, which is why this
    /// exists: **sustain**, tested where it is decided rather than through
    /// three seconds of audio.
    ///
    /// Two versions of this test were written and thrown away first, and both
    /// are worth recording because both *passed*. A click train read low
    /// because a click's energy is mostly outside the voice band -- the band
    /// was doing the work. Chopped noise against held noise read low because
    /// the chopped signal is silent nine tenths of the time -- the duty cycle
    /// was doing the work. Deleting the harmonic mask outright left both of
    /// them green.
    ///
    /// The distinction is between a **horizontal** ridge in the spectrogram
    /// and a **vertical** one, and synthetic audio that is one and not the
    /// other while holding level, band and centre still is hard to write and
    /// easy to get wrong. A spectrogram is not: here is one of each, by hand.
    #[test]
    fn a_held_note_and_a_broadband_hit_are_told_apart_by_the_mask() {
        const BINS: usize = 32;
        let flat = vec![0.0f32; BINS];
        let mut medians = Medians::new();
        let centre = TIME_SPAN / 2;
        let band = 4..BINS - 4;
        let silent = vec![flat.clone(); TIME_SPAN];

        // Horizontal: one bin, every frame. A held note.
        let mut held = vec![flat.clone(); TIME_SPAN];
        for frame in &mut held {
            frame[10] = 1.0;
        }
        let note = share(
            &Moment {
                mid: &held,
                side: &silent,
                centre,
                total: 1.0,
            },
            &mut medians,
            band.clone(),
        );

        // Vertical: every bin, one frame. A hit.
        let mut hit = vec![flat; TIME_SPAN];
        hit[centre] = vec![1.0; BINS];
        #[allow(clippy::cast_precision_loss)]
        let drum = share(
            &Moment {
                mid: &hit,
                side: &silent,
                centre,
                total: BINS as f32,
            },
            &mut medians,
            band.clone(),
        );

        // Both at once: held *and* broadband. A wash of noise is sustained,
        // so the median along time calls it harmonic all on its own -- and it
        // is not a voice. Discounting it is the only thing the median along
        // frequency is for, and without this arm that median can be deleted
        // outright with every other test still green.
        let wash = vec![vec![1.0f32; BINS]; TIME_SPAN];
        #[allow(clippy::cast_precision_loss)]
        let pad = share(
            &Moment {
                mid: &wash,
                side: &silent,
                centre,
                total: BINS as f32,
            },
            &mut medians,
            band,
        );

        assert!(
            note > 0.9,
            "a held note should be nearly all voice, got {note}"
        );
        assert!(
            drum < 0.05,
            "a broadband hit should be nearly no voice, got {drum}"
        );
        assert!(
            pad < note / 2.0,
            "a held wash is not a voice: it reads {pad} against a note's {note}"
        );
    }

    /// A curve, not a verdict: the reading has to say *where*.
    #[test]
    fn the_reading_follows_where_the_voice_actually_is() {
        let half = seconds(3.0);
        let mut audio = stereo(half, |_| (0.05, -0.05));
        audio.extend(tone(440.0, half, 0.0));

        let curve = presence(&audio, SR);
        assert!(!curve.is_empty());

        #[allow(clippy::cast_precision_loss)]
        let boundary = half as f64;
        let before = curve.mean_between(0.0, boundary).expect("first half");
        let after = curve
            .mean_between(boundary, boundary * 2.0)
            .expect("second half");
        assert!(
            after > before * 4.0,
            "the voice enters halfway: {before} before, {after} after"
        );
    }

    /// One value per hop, so an index here names the same moment as an index
    /// in the onset curve. A curve half a span short would put every reading
    /// 85 ms early for the rest of the record.
    #[test]
    fn the_curve_is_one_value_per_hop_of_the_onset_curve() {
        let audio = tone(440.0, seconds(2.0), 0.0);
        let curve = presence(&audio, SR);
        let onset = crate::onset::detect(&audio, SR);
        assert_eq!(curve.values.len(), onset.values.len());
        assert!((curve.rate - onset.rate).abs() < f64::EPSILON);
    }

    /// Nothing is claimed about what was not measured.
    #[test]
    fn too_short_too_silent_and_no_rate_all_say_nothing_rather_than_zero() {
        assert!(presence(&tone(440.0, 1_000, 0.0), SR).is_empty());
        assert!(presence(&[], SR).is_empty());
        assert!(presence(&tone(440.0, seconds(2.0), 0.0), 0).is_empty());

        // Silence is measured and is genuinely nothing, which is a different
        // answer from "not measured".
        let quiet = presence(&vec![0.0f32; seconds(2.0) * 2], SR);
        assert!(!quiet.is_empty());
        assert_eq!(mean(&quiet), 0.0);
    }

    /// A span nobody measured is absent, not zero.
    #[test]
    fn a_span_off_the_end_of_the_curve_is_absent() {
        let curve = presence(&tone(440.0, seconds(2.0), 0.0), SR);
        assert!(curve.mean_between(0.0, 1_000.0).is_some());
        assert!(curve.mean_between(1e12, 1e12 + 1_000.0).is_none());
        assert!(curve.mean_between(100.0, 100.0).is_none());
        assert!(Presence::default().mean_between(0.0, 1_000.0).is_none());
    }
}
