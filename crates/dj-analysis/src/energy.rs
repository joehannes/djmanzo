//! How hard a record hits, which is not how loud it is.
//!
//! [§20](../../../docs/DIRECTIVE.md) asks the library for an **energy** column
//! and djmanzo has been showing loudness under that name. `dj_library::suggest`
//! says so in its own words and names what a real measure would be:
//!
//! > A real energy measure -- spectral flux over the track, percussive
//! > density, dynamic range -- belongs in `dj-analysis` and is not here yet.
//!
//! This is those three. It is not a better loudness meter; it is a different
//! question, and the whole reason for having it is that the two answers differ
//! for records a DJ would never confuse: a sparse, tense record can be quieter
//! than a wall-of-sound filler and carry a room better, and a mastered-flat
//! ambient piece can be as loud as a peak-time track and empty underneath it.
//!
//! # The three, and why each is in
//!
//! **Flux** is how much the spectrum is *changing*, averaged over the record.
//! It separates a track that moves from one that sits, and it is already
//! computed: the onset detector's envelope is exactly this curve, so this
//! costs one pass over numbers that exist rather than a second FFT.
//!
//! **Drive** is percussive density, counted in the low band and measured **per
//! beat** rather than per second. Per second would make 174 BPM drum and bass
//! arithmetically more energetic than 124 BPM house before either was
//! listened to, which is tempo wearing energy's name -- the exact mistake this
//! module exists to stop.
//!
//! **Punch** is dynamic range, inverted. A record whose loud moments sit close
//! to its quiet ones is relentless; one with a wide range breathes. Inverted
//! because "relentless" is the high-energy end, and measured against the
//! record's own distribution so mastering level does not enter twice.
//!
//! # What it deliberately is not
//!
//! Not a genre detector, not a mood, and not a judgement about whether the
//! record is any good. And not a single number a DJ has to take on trust: the
//! three parts are carried alongside the total, because a number whose reason
//! cannot be seen is one nobody can argue with -- the same posture §42 takes
//! about suggestions.

use serde::{Deserialize, Serialize};

use crate::loudness::Lufs;
use crate::onset::BandedOnset;

/// How hard a record hits, and the three readings behind it.
///
/// Every field is 0..=1, so the parts can be drawn as bars beside the total
/// without a scale nobody can read.
/// `Default` is all zeros, which is "nothing measured" rather than "a record
/// with no energy in it". Only a hand-built [`crate::Analysis`] has one -- a
/// real analysis always computes the three readings, even for silence.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Energy {
    /// The weighted total. See [`of`].
    pub value: f32,
    /// How much the spectrum changes, averaged over the record.
    pub flux: f32,
    /// Percussive events per beat, in the low band.
    pub drive: f32,
    /// Dynamic range, inverted: 1 is relentless, 0 breathes.
    pub punch: f32,
}

/// What each reading is worth in the total.
///
/// Drive first, because "does it hit" is the question a DJ is actually asking
/// when they look at this column, and because it is the reading that most
/// clearly is *not* loudness. Flux second. Punch is real and is the weakest of
/// the three on its own -- plenty of quiet records are compressed flat -- so it
/// trims rather than decides.
const WEIGHTS: (f32, f32, f32) = (0.45, 0.35, 0.20);

/// Half-width of the window a hit has to be the loudest hop in, in hops.
///
/// Seven, which at the onset detector's hop is about 75 ms either side. It was
/// three, and three is too short: a kick's decay produces a second local peak
/// in its own tail, so the same pattern measured 1.75 hits per beat at 120 BPM
/// and 1.00 at 174 -- the slower record looking busier because its tail had
/// more hops to put a bump in. Seven is long enough to swallow a kick's decay
/// and short enough to stay well under the gap between hits at any tempo a DJ
/// plays: at 174 BPM the beats are 345 ms apart.
const PEAK_WINDOW: usize = 7;

/// What counts as a percussive event, as a share of the record's own activity.
///
/// Measured against the mean flux across **all four bands** rather than
/// against the low band's own mean, and that is the whole of it. A relative
/// threshold inside the band looked right and measures noise: a pad has almost
/// nothing in its low band, so every wobble in it is several times that band's
/// own mean, and the reading came back at fourteen events per beat -- a pad
/// reading as the most driving record in the collection. Against the record's
/// overall activity the same pad produces nothing at all.
///
/// A floor rather than a threshold, now that the peak-picking above does the
/// selecting: it is here to keep a record with no low end from reporting its
/// noise as hits. At half the overall flux a four-to-the-floor kick measures
/// 1.01 hits per beat at 128 BPM and 1.00 at 174, and a pad measures 0.02.
const EVENT: f32 = 0.5;

/// Where drive saturates: one and a half low-band hits per beat.
///
/// A four-to-the-floor kick measures exactly one at any tempo, and a kick with
/// an off-beat is two.
/// Past that the difference stops being audible as "more driving" and starts
/// being a different genre.
const BUSY: f32 = 1.5;

/// How much of a record's own loudness range counts as "breathing", in dB.
///
/// Twelve. A club master typically sits inside six; twelve is where a record
/// has real dynamics rather than a mastering choice.
const BREATH: f32 = 12.0;

/// Measure a record's energy.
///
/// `bpm` is needed because drive is per beat -- see the module note. Without a
/// grid there is nothing to count against, so drive falls back to the flux
/// reading rather than to a guess: a record djmanzo could not find a tempo in
/// is usually one with no steady percussion, and reporting zero drive for it
/// would be a confident claim built on a missing measurement.
#[must_use]
pub fn of(banded: &BandedOnset, loudness: &[Lufs], bpm: Option<f64>) -> Energy {
    let flux = flux_of(banded);
    let drive = bpm.map_or(flux, |bpm| drive_of(banded, bpm));
    let punch = punch_of(loudness);

    let (a, b, c) = WEIGHTS;
    let value = a.mul_add(drive, b.mul_add(flux, c * punch)).clamp(0.0, 1.0);
    Energy {
        value,
        flux,
        drive,
        punch,
    }
}

/// Mean spectral flux, squashed into 0..=1.
///
/// The mean over the record rather than a peak: one crash cymbal is not a
/// property of the record, and a measure a single frame can move is one that
/// reports mastering accidents.
///
/// Read off the **banded** curve summed back up rather than off
/// [`OnsetEnvelope`], which looks like the obvious source and is not:
/// `onset::normalise` centres that one on zero and scales it to unit
/// deviation, because the tempo search autocorrelates it and a large mean
/// would bury the periodicity. Its mean is therefore approximately zero by
/// construction, and a flux reading taken from it is approximately zero for
/// every record ever made. The banded curve is deliberately left raw.
fn flux_of(banded: &BandedOnset) -> f32 {
    if banded.values.is_empty() {
        return 0.0;
    }
    let sum: f32 = banded
        .values
        .iter()
        .map(|bands| bands.iter().sum::<f32>())
        .sum();
    let mean = sum / banded.values.len() as f32;
    // A soft curve rather than a cliff: `x / (x + k)` has no threshold to be
    // wrong about and is flat at both ends, which is what a reading that feeds
    // a weighted sum should be. `k` is the flux of an ordinary record.
    squash(mean, 2.5)
}

/// Low-band onsets per beat, squashed into 0..=1.
fn drive_of(banded: &BandedOnset, bpm: f64) -> f32 {
    if banded.values.is_empty() || bpm <= 0.0 {
        return 0.0;
    }
    // The record's overall activity, which is what an event is measured
    // against. See `EVENT`.
    let overall: f32 = banded
        .values
        .iter()
        .map(|bands| bands.iter().sum::<f32>())
        .sum::<f32>()
        / banded.values.len() as f32;
    if overall <= f32::EPSILON {
        return 0.0;
    }
    // Local peaks, not hops above a line. Two earlier versions of this counted
    // hops and then rising edges, and both made the same pattern read as less
    // driving at 174 BPM than at 120 -- 0.86 against 0.32, and then 0.67
    // against 0.43. The mechanism is that a busier record raises `overall`,
    // which raises the bar each hit has to clear, so the threshold drifts with
    // the very thing being counted. A peak is a peak whatever else is going on.
    let half = PEAK_WINDOW;
    let mut events = 0usize;
    for index in 0..banded.values.len() {
        let value = banded.values[index][0];
        if value <= overall * EVENT {
            continue;
        }
        let from = index.saturating_sub(half);
        let to = (index + half + 1).min(banded.values.len());
        let peak = banded.values[from..to]
            .iter()
            .map(|bands| bands[0])
            .fold(f32::MIN, f32::max);
        if value >= peak {
            events += 1;
        }
    }

    let seconds = banded.values.len() as f64 / banded.rate;
    if seconds <= 0.0 {
        return 0.0;
    }
    let beats = seconds * bpm / 60.0;
    #[allow(clippy::cast_possible_truncation)]
    let per_beat = (events as f64 / beats) as f32;
    (per_beat / BUSY).clamp(0.0, 1.0)
}

/// Dynamic range, inverted, from a record's own short-term loudness.
///
/// The spread between its loud moments and its quiet ones, with the extremes
/// trimmed: the loudest single window of a record is usually a transient and
/// the quietest is usually the run-out, and neither is what "this record
/// breathes" means.
fn punch_of(loudness: &[Lufs]) -> f32 {
    let mut values: Vec<f64> = loudness
        .iter()
        .map(|l| l.get())
        .filter(|v| v.is_finite())
        .collect();
    if values.len() < 4 {
        // Not enough of a record to have a range. Neutral rather than zero: a
        // missing measurement should not push the total down as if it had been
        // taken and come back low.
        return 0.5;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let at = |fraction: f64| -> f64 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let index = ((values.len() - 1) as f64 * fraction).round() as usize;
        values[index]
    };
    #[allow(clippy::cast_possible_truncation)]
    let range = (at(0.95) - at(0.10)) as f32;
    (1.0 - (range / BREATH)).clamp(0.0, 1.0)
}

/// `x / (x + k)`, which is 0 at 0, 0.5 at `k`, and approaches 1.
fn squash(value: f32, half: f32) -> f32 {
    if value <= 0.0 || !value.is_finite() {
        return 0.0;
    }
    value / (value + half)
}

// -- §75's energy trajectory, and the breakdowns and drops in it -------------

/// One window of a record, and how much is going on in it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Section {
    /// Where the window starts, in frames.
    pub at: f64,
    /// How much is going on, against this record's own busiest window.
    ///
    /// Relative to the record rather than absolute, which is what a
    /// *trajectory* is: the question a DJ asks of this curve is "where does
    /// this one go", not "is this louder than the last one". The whole-record
    /// [`Energy`] above is the comparison between records.
    pub energy: f32,
    /// The low band's share of it, on the same scale. A breakdown is this
    /// falling away while the rest of the record carries on.
    pub low: f32,
    /// What this window is made of: the four currents' shares of it, adding
    /// to one. [`crate::presence`], and §25's `vocal` and `stems` layers.
    ///
    /// Indexed by [`dj_core::Stem::index`], which is the one order in the
    /// project.
    ///
    /// **Absolute, unlike the two above, and the difference is the point.**
    /// "Where does this record go" is a question about the record's own
    /// busiest window, so energy is scaled against it. "Is somebody singing"
    /// is not: an instrumental scaled against its own maximum would report a
    /// vocal at whichever window had the most synth in it.
    ///
    /// `None` where nothing was measured -- a record too short for the
    /// measurement, or a sample rate with no room for a voice's harmonics
    /// under Nyquist. Absent rather than four zeroes, because "not measured"
    /// and "a window with nothing in it" are different answers and the
    /// overview draws them differently.
    pub parts: Option<[f32; dj_core::Stem::COUNT]>,
    /// How often something is struck in this window, in strikes a second.
    /// [`crate::strikes`], and §75's *transient density*.
    ///
    /// **Absolute, like `parts` and for the same reason**, and deliberately
    /// *not* the same question as the percussive share beside it. A sparse
    /// kick-and-clap pattern is high share and low density; a shaker under a
    /// pad is the other way round. A DJ reading a record for where to mix
    /// wants both, and one drawn while claiming the other is the confusion
    /// §75 ends on.
    ///
    /// `None` where nothing was measured, which is a different answer from a
    /// window with nothing struck in it -- the overview draws them
    /// differently.
    pub strikes: Option<f32>,
}

impl Section {
    /// This window's vocal share. §25's `vocal` layer.
    #[must_use]
    pub fn voice(&self) -> Option<f32> {
        self.parts.map(|parts| parts[dj_core::Stem::Vocal.index()])
    }
}

/// A stretch of a record where it thins out.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub from: f64,
    pub to: f64,
}

/// Where a record goes over its own length. [§75](../../../docs/DIRECTIVE.md).
///
/// §25 has had `breakdowns`, `drops` and `energy` in its layer table as
/// `Drawn::Nowhere` since the table existed, with the same reason beside each:
/// the analysis does not exist. It does now, and it is the same banded onset
/// curve the phrase detector reads -- which is the point. A second pass over
/// the audio to answer a second question about the same beats would be two
/// answers that could disagree.
/// `Default` is the empty answer -- no grid, nothing measured -- which is what
/// a hand-built [`crate::Analysis`] in a test gets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Trajectory {
    /// In order, one per window. Empty when there is no grid to count against.
    pub sections: Vec<Section>,
    /// How long a window is, in beats.
    pub beats_per_section: u32,
    /// Where the record thins out.
    pub breakdowns: Vec<Span>,
    /// Where it comes back, in frames. One per breakdown that ends before the
    /// record does -- a record that fades out on a breakdown has no drop.
    pub drops: Vec<f64>,
    /// Where the voice arrives, in frames. [§27](../../../docs/DIRECTIVE.md)'s
    /// *where the vocal enters*, and the last of the seven it asks for.
    ///
    /// The first window whose voice reading reaches
    /// [`crate::presence::STRONG`]. `None` for an instrumental, for a record
    /// nobody measured, and for a record whose lead never gets above a
    /// murmur -- all three are records with no entry to mark, and a mark
    /// placed at the loudest murmur would be the confident guess this crate
    /// is written against.
    pub voice_enters: Option<f64>,
}

/// How many beats a window covers when the record has no phrase structure.
///
/// Eight. Long enough that one bar of a fill does not read as a breakdown, and
/// short enough that a sixteen-bar breakdown is four windows rather than one.
const DEFAULT_SECTION_BEATS: u32 = 8;

/// How far below a record's own middle the low band has to fall to be thin.
///
/// Six tenths. A breakdown is not "slightly less kick": it is the floor coming
/// out, and a threshold close to the median would mark every quiet bar in a
/// record that breathes.
const THIN: f32 = 0.6;

/// How many windows in a row are a breakdown rather than a gap.
///
/// Two, which at eight beats is four bars. One window is a fill.
const AT_LEAST: usize = 2;

/// How much the low band has to come back for the return to be a drop.
///
/// Half as much again as the breakdown was. A record that drifts back up over
/// a minute has no drop in it, and marking one would put an arrow on the
/// overview where nothing happens.
const RETURN: f32 = 1.5;

/// Measure where a record goes. See [`Trajectory`].
///
/// `phrase_beats` decides the window when the record has one, because a
/// breakdown in dance music starts on a phrase and a window that straddled two
/// would smear its edges by half its own width.
#[must_use]
pub fn trajectory(
    banded: &BandedOnset,
    parts: &crate::presence::Presence,
    strikes: &crate::strikes::Strikes,
    grid: &dj_core::Beatgrid,
    rate: dj_core::SampleRate,
    frames: u64,
    phrase_beats: Option<u32>,
) -> Trajectory {
    let per_section = phrase_beats
        .filter(|beats| *beats > 0)
        .unwrap_or(DEFAULT_SECTION_BEATS);
    let empty = Trajectory {
        sections: Vec::new(),
        beats_per_section: per_section,
        breakdowns: Vec::new(),
        drops: Vec::new(),
        voice_enters: None,
    };
    let Some(beats) = crate::structure::beat_features(banded, grid, rate, frames) else {
        return empty;
    };
    if beats.len() < per_section as usize * 2 {
        return empty;
    }

    let first = grid.beat_index_at(dj_core::FramePos::new(0.0), rate);
    let mut sections = Vec::new();
    for (index, window) in beats.chunks(per_section as usize).enumerate() {
        // A trailing part-window is not a section: its mean is taken over
        // fewer beats and would sit at a level the record never played.
        if window.len() < per_section as usize {
            break;
        }
        let span = window.len() as f32;
        let total: f32 = window
            .iter()
            .map(|bands| bands.iter().sum::<f32>())
            .sum::<f32>()
            / span;
        let low: f32 = window.iter().map(|bands| bands[0]).sum::<f32>() / span;
        let beat = first + (index * per_section as usize) as i64;
        let at = grid.beat_position(beat, rate).get();
        let until = grid
            .beat_position(beat + i64::from(per_section), rate)
            .get();
        sections.push(Section {
            at,
            energy: total,
            low,
            parts: parts.all_between(at, until),
            // The same window the shares are taken over, deliberately: two
            // window schemes over one record would draw two grids that nearly
            // line up, which is worse than one.
            strikes: strikes.between(at, until, rate.as_f64()),
        });
    }
    if sections.is_empty() {
        return empty;
    }

    // Scaled against this record's own busiest window -- see `Section::energy`.
    let loudest = sections
        .iter()
        .map(|section| section.energy)
        .fold(0.0f32, f32::max);
    let busiest_low = sections.iter().map(|s| s.low).fold(0.0f32, f32::max);
    let median_low = median(&sections.iter().map(|s| s.low).collect::<Vec<_>>());
    for section in &mut sections {
        if loudest > 0.0 {
            section.energy = (section.energy / loudest).clamp(0.0, 1.0);
        }
        if busiest_low > 0.0 {
            section.low = (section.low / busiest_low).clamp(0.0, 1.0);
        }
    }
    let thin_below = if busiest_low > 0.0 {
        median_low / busiest_low * THIN
    } else {
        0.0
    };

    let (breakdowns, drops) = thin_stretches(&sections, thin_below, per_section, grid, rate);
    // The first window the voice actually carries, rather than the loudest one
    // -- §27 asks where it *enters*, and the loudest window of a record is
    // usually its last chorus.
    let voice_enters = sections
        .iter()
        .find(|section| {
            section
                .voice()
                .is_some_and(|share| share >= crate::presence::STRONG)
        })
        .map(|section| section.at);
    Trajectory {
        sections,
        beats_per_section: per_section,
        breakdowns,
        drops,
        voice_enters,
    }
}

/// The runs of thin windows, and the returns that count as drops.
fn thin_stretches(
    sections: &[Section],
    thin_below: f32,
    per_section: u32,
    grid: &dj_core::Beatgrid,
    rate: dj_core::SampleRate,
) -> (Vec<Span>, Vec<f64>) {
    let mut breakdowns = Vec::new();
    let mut drops = Vec::new();
    let mut run: Option<usize> = None;
    for index in 0..=sections.len() {
        let thin = sections.get(index).is_some_and(|s| s.low < thin_below);
        match (thin, run) {
            (true, None) => run = Some(index),
            (false, Some(start)) => {
                if index - start >= AT_LEAST {
                    let ends_at = sections.get(index).map_or_else(
                        || {
                            // The record ran out. The span ends where the last
                            // window does rather than where the next would
                            // start, which is a beat that does not exist.
                            let last = sections[sections.len() - 1].at;
                            last + span_frames(grid, rate, per_section)
                        },
                        |section| section.at,
                    );
                    breakdowns.push(Span {
                        from: sections[start].at,
                        to: ends_at,
                    });
                    // A drop only where it really comes back. A record that
                    // drifts up has no drop in it.
                    if let Some(after) = sections.get(index) {
                        let during = sections[start..index]
                            .iter()
                            .map(|s| s.low)
                            .fold(f32::MAX, f32::min);
                        if after.low >= during.max(f32::EPSILON) * RETURN {
                            drops.push(after.at);
                        }
                    }
                }
                run = None;
            }
            _ => {}
        }
    }
    (breakdowns, drops)
}

/// How many frames a window covers.
fn span_frames(grid: &dj_core::Beatgrid, rate: dj_core::SampleRate, beats: u32) -> f64 {
    let one = grid.beat_position(1, rate).get() - grid.beat_position(0, rate).get();
    one * f64::from(beats)
}

/// The middle value, for a slice that may be any length.
fn median(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sorted[sorted.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onset;
    use dj_core::SampleRate;

    const SR: SampleRate = SampleRate::DEFAULT;

    /// A four-to-the-floor kick over a bass bed: a record that hits.
    ///
    /// The kick is low, because a kick is. An earlier version of this test
    /// signal put a 3 kHz click on every beat, which is a rimshot rather than
    /// a kick and which dumped so much flux into the upper bands that the
    /// low-band reading was measured against a total it did not belong to.
    /// Tuning a measurement on a signal that is not the thing it measures is
    /// how a threshold ends up right for nothing.
    fn driving(bpm: f64, seconds: f64, level: f32) -> Vec<f32> {
        use std::f32::consts::TAU;
        let rate = SR.get();
        let frames = (seconds * f64::from(rate)) as usize;
        let per_beat = f64::from(rate) * 60.0 / bpm;
        let mut audio = vec![0.0f32; frames * 2];
        for n in 0..frames {
            let t = n as f32 / rate as f32;
            let since = (n as f64 % per_beat) / f64::from(rate);
            // A bed, because a record is not silence between hits.
            let mut value = (TAU * 110.0 * t).sin() * 0.08 * level;
            if since < 0.12 {
                // 55 Hz with a fast decay, plus a little 120 Hz snap.
                let decay = (-since * 28.0).exp() as f32;
                value += ((TAU * 55.0 * t).sin() + (TAU * 120.0 * t).sin() * 0.4) * decay * level;
            }
            audio[n * 2] = value;
            audio[n * 2 + 1] = value;
        }
        audio
    }

    /// A pad: the same loudness, and nothing happening.
    fn sustained(seconds: f64, level: f32) -> Vec<f32> {
        use std::f32::consts::TAU;
        let rate = SR.get();
        let frames = (seconds * f64::from(rate)) as usize;
        let mut audio = vec![0.0f32; frames * 2];
        for n in 0..frames {
            let t = n as f32 / rate as f32;
            // A slow swell rather than a dead tone, which is what a pad is and
            // which keeps the measurement off a signal no record resembles.
            let swell = 0.85 + 0.15 * (TAU * 0.15 * t).sin();
            let value = ((TAU * 220.0 * t).sin() + (TAU * 330.0 * t).sin()) * 0.5 * level * swell;
            audio[n * 2] = value;
            audio[n * 2 + 1] = value;
        }
        audio
    }

    fn measure(audio: &[f32], bpm: Option<f64>) -> (Energy, f64) {
        let (_, banded) = onset::detect_all(audio, SR.get());
        let measured = crate::loudness::measured(audio, SR.get());
        (
            of(&banded, &measured.blocks, bpm),
            measured.integrated.get(),
        )
    }

    /// **The load-bearing one: energy is not loudness.**
    ///
    /// The entire reason this module exists. A pad and a kick pattern are
    /// levelled to within a decibel of each other and one of them is obviously
    /// the more energetic record; if the two numbers agreed, djmanzo would have
    /// spent a column and an analysis pass on a second name for LUFS.
    #[test]
    fn a_kick_pattern_beats_a_pad_that_is_just_as_loud() {
        let (kick, kick_lufs) = measure(&driving(128.0, 20.0, 0.9), Some(128.0));
        let (pad, pad_lufs) = measure(&sustained(20.0, 0.25), Some(128.0));

        assert!(
            (kick_lufs - pad_lufs).abs() < 3.0,
            "the two test signals are {:.1} dB apart ({kick_lufs:.1} against \
             {pad_lufs:.1}), so this would be measuring loudness after all",
            (kick_lufs - pad_lufs).abs()
        );
        assert!(
            kick.value > pad.value + 0.15,
            "a four-to-the-floor kick read {:.2} and a pad at the same loudness \
             read {:.2}. Energy that tracks loudness is loudness",
            kick.value,
            pad.value
        );
    }

    /// And the part that carries the difference says so.
    #[test]
    fn the_reading_that_separates_them_is_the_one_about_hitting() {
        let (kick, _) = measure(&driving(128.0, 20.0, 0.9), Some(128.0));
        let (pad, _) = measure(&sustained(20.0, 0.25), Some(128.0));

        assert!(
            kick.drive > pad.drive,
            "drive was {:.2} for a kick pattern and {:.2} for a pad",
            kick.drive,
            pad.drive
        );
        assert!(
            kick.flux > pad.flux,
            "flux was {:.2} for a kick pattern and {:.2} for a pad",
            kick.flux,
            pad.flux
        );
    }

    /// **Drive is per beat, so tempo alone does not raise it.**
    ///
    /// The same pattern at two tempos is the same record played faster, and a
    /// reading that called the faster one more energetic would be tempo wearing
    /// energy's name -- which is the mistake loudness-as-energy already made
    /// once.
    #[test]
    fn the_same_pattern_faster_is_not_more_driving() {
        let (slow, _) = measure(&driving(120.0, 20.0, 0.9), Some(120.0));
        let (fast, _) = measure(&driving(174.0, 20.0, 0.9), Some(174.0));

        assert!(
            (slow.drive - fast.drive).abs() < 0.2,
            "the same kick pattern read {:.2} driving at 120 and {:.2} at 174",
            slow.drive,
            fast.drive
        );
    }

    /// Every reading stays inside the range the interface draws it in.
    #[test]
    fn every_reading_is_a_fraction() {
        for audio in [
            driving(128.0, 10.0, 1.0),
            sustained(10.0, 0.9),
            vec![0.0; 48_000 * 2],
        ] {
            let (energy, _) = measure(&audio, Some(128.0));
            for (what, value) in [
                ("value", energy.value),
                ("flux", energy.flux),
                ("drive", energy.drive),
                ("punch", energy.punch),
            ] {
                assert!(
                    (0.0..=1.0).contains(&value),
                    "{what} came back {value}, and the interface draws these as bars"
                );
            }
        }
    }

    /// Silence is not energetic, and does not panic.
    #[test]
    fn silence_reads_as_nothing_happening() {
        let (energy, _) = measure(&vec![0.0; 48_000 * 2], None);
        assert!(
            energy.value < 0.35,
            "silence read {:.2}. The punch reading is 0.5 for a record with no \
             measurable range, which is deliberate, but the total must not let \
             that alone look like a peak-time record",
            energy.value
        );
    }

    /// A record with no tempo still gets an answer.
    #[test]
    fn a_record_with_no_grid_is_measured_rather_than_refused() {
        let (with, _) = measure(&sustained(10.0, 0.5), Some(120.0));
        let (without, _) = measure(&sustained(10.0, 0.5), None);
        assert!(without.value > 0.0);
        assert!(
            (with.value - without.value).abs() < 0.5,
            "losing the grid moved the answer from {:.2} to {:.2}, which is a \
             different record rather than the same one measured without a tempo",
            with.value,
            without.value
        );
    }
}

#[cfg(test)]
mod trajectory_tests {
    use super::*;
    use crate::onset;
    use dj_core::{Beatgrid, Bpm, Confidence, FramePos, SampleRate};

    const SR: SampleRate = SampleRate::DEFAULT;
    const BPM: f64 = 128.0;

    /// A record with a shape: kick, then a stretch without it, then kick again.
    ///
    /// The bass bed carries on through the breakdown, which is what a breakdown
    /// is -- the floor comes out and the record does not stop. A test signal
    /// that went silent would be measuring a gap rather than a breakdown.
    fn with_a_breakdown(bars_in: usize, bars_thin: usize, bars_out: usize) -> Vec<f32> {
        use std::f32::consts::TAU;
        let rate = SR.get();
        let per_beat = f64::from(rate) * 60.0 / BPM;
        let beats = (bars_in + bars_thin + bars_out) * 4;
        let frames = (per_beat * beats as f64) as usize;
        let thin_from = bars_in * 4;
        let thin_to = thin_from + bars_thin * 4;

        let mut audio = vec![0.0f32; frames * 2];
        for n in 0..frames {
            let t = n as f32 / rate as f32;
            let beat = (n as f64 / per_beat) as usize;
            let since = (n as f64 % per_beat) / f64::from(rate);
            // The bed, all the way through.
            let mut value = (TAU * 220.0 * t).sin() * 0.12;
            let thin = beat >= thin_from && beat < thin_to;
            if !thin && since < 0.12 {
                let decay = (-since * 28.0).exp() as f32;
                value += ((TAU * 55.0 * t).sin() + (TAU * 120.0 * t).sin() * 0.4) * decay * 0.9;
            }
            audio[n * 2] = value;
            audio[n * 2 + 1] = value;
        }
        audio
    }

    fn grid() -> Beatgrid {
        Beatgrid {
            anchor: FramePos::new(0.0),
            bpm: Bpm::new(BPM).expect("a tempo"),
            beats_per_bar: 4,
            confidence: Confidence::new(1.0),
        }
    }

    fn measure(audio: &[f32]) -> Trajectory {
        let (envelope, banded) = onset::detect_all(audio, SR.get());
        let frames = (audio.len() / 2) as u64;
        trajectory(
            &banded,
            &crate::presence::Presence::default(),
            &crate::strikes::find(&envelope, SR.get()),
            &grid(),
            SR,
            frames,
            Some(8),
        )
    }

    /// **The load-bearing one: a breakdown is found where it is, and so is the
    /// drop.**
    ///
    /// §25 has carried `breakdowns`, `drops` and `energy` as layers nothing
    /// draws, each with "the analysis does not exist" beside it. This is the
    /// analysis, and the only thing worth asserting about it is that the marks
    /// land on the right part of the record.
    #[test]
    fn the_breakdown_and_the_drop_land_where_the_record_puts_them() {
        let found = measure(&with_a_breakdown(8, 8, 8));
        let seconds = |frames: f64| frames / f64::from(SR.get());

        assert_eq!(
            found.breakdowns.len(),
            1,
            "found {} breakdowns in a record with one: {:?}",
            found.breakdowns.len(),
            found
                .breakdowns
                .iter()
                .map(|span| (seconds(span.from), seconds(span.to)))
                .collect::<Vec<_>>()
        );
        // Eight bars at 128 BPM is 15 seconds, so the breakdown runs from 15 to 30.
        let span = found.breakdowns[0];
        assert!(
            (seconds(span.from) - 15.0).abs() < 2.0,
            "the breakdown starts at {:.1}s and the record thins out at 15",
            seconds(span.from)
        );
        assert!(
            (seconds(span.to) - 30.0).abs() < 2.0,
            "the breakdown ends at {:.1}s and the kick comes back at 30",
            seconds(span.to)
        );

        assert_eq!(
            found.drops.len(),
            1,
            "a record that comes back has one drop"
        );
        assert!(
            (seconds(found.drops[0]) - 30.0).abs() < 2.0,
            "the drop is marked at {:.1}s and the kick returns at 30",
            seconds(found.drops[0])
        );
    }

    /// A record that never thins out has nothing to mark.
    ///
    /// The half that stops this from being a detector of its own threshold: a
    /// measure that found a breakdown in a steady record would put arrows all
    /// over every overview in the collection.
    #[test]
    fn a_record_that_never_thins_out_has_no_breakdown_in_it() {
        let found = measure(&with_a_breakdown(24, 0, 0));
        assert!(
            found.breakdowns.is_empty(),
            "found {:?} in a record that runs one pattern from end to end",
            found.breakdowns
        );
        assert!(found.drops.is_empty());
        assert!(
            found.sections.len() >= 4,
            "only {} sections, so the record was barely measured",
            found.sections.len()
        );
    }

    /// One bar of a fill is not a breakdown.
    #[test]
    fn a_single_quiet_window_is_a_fill_rather_than_a_breakdown() {
        // Two bars thin, which at eight beats a window is one window.
        let found = measure(&with_a_breakdown(8, 2, 8));
        assert!(
            found.breakdowns.is_empty(),
            "a two-bar gap was marked as a breakdown: {:?}",
            found.breakdowns
        );
    }

    /// The curve is a fraction of the record's own peak, from end to end.
    #[test]
    fn the_trajectory_is_a_curve_the_overview_can_draw() {
        let found = measure(&with_a_breakdown(8, 8, 8));
        assert!(!found.sections.is_empty());
        for section in &found.sections {
            assert!(
                (0.0..=1.0).contains(&section.energy) && (0.0..=1.0).contains(&section.low),
                "a section reads {:?}, and the overview draws these as a height",
                section
            );
        }
        assert!(
            found
                .sections
                .windows(2)
                .all(|pair| pair[1].at > pair[0].at),
            "the sections are not in order"
        );
        assert!(
            found.sections.iter().any(|section| section.energy > 0.9),
            "nothing reaches the record's own peak, so the scaling is wrong"
        );
    }

    /// A record with no grid gets an empty answer rather than a guess.
    #[test]
    fn no_grid_is_no_trajectory() {
        let audio = with_a_breakdown(8, 8, 8);
        let (_, banded) = onset::detect_all(&audio, SR.get());
        let flat = Beatgrid {
            anchor: FramePos::new(0.0),
            bpm: Bpm::new(BPM).expect("a tempo"),
            beats_per_bar: 4,
            confidence: Confidence::new(1.0),
        };
        // Nothing to count against: zero frames is a record of no length.
        let found = trajectory(
            &banded,
            &crate::presence::Presence::default(),
            &crate::strikes::Strikes::default(),
            &flat,
            SR,
            0,
            Some(8),
        );
        assert!(found.sections.is_empty());
        assert!(found.breakdowns.is_empty());
    }
}
