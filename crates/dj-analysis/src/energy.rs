//! Where the drums leave, and where they come back.
//!
//! §25 wants the waveform to answer *what is about to happen*, and in dance
//! music the answer is nearly always one of two things: a breakdown is coming,
//! or a drop is. Those are the two moments a mix is planned around — you do not
//! land a new record in the middle of a breakdown, and you do land one on a
//! drop.
//!
//! # What a breakdown is, operationally
//!
//! **The drums leave.** Not "it gets quieter": a breakdown is often the loudest
//! part of a record, all pad and vocal and reverb, and a filter sweep that
//! doubles the perceived level is still a breakdown if the kick is out. So the
//! signal is the onset flux in the lowest band — under 150 Hz, which is the
//! kick and nothing else — measured per beat against what this record sounds
//! like when it is running.
//!
//! Against *this* record, not against a fixed level. A minimal techno record
//! and a big-room house record differ by more between themselves than either
//! differs from its own breakdown, so an absolute threshold would call one of
//! them a continuous breakdown and never find one in the other.
//!
//! # Why hysteresis
//!
//! One threshold produces a breakdown that starts, stops and starts again on
//! every stray kick and every ride cymbal that leaks under 150 Hz — and a
//! marker that flickers is worse than none, for the same reason the density
//! bands are bands. So a breakdown *starts* below [`QUIET`] and *ends* above
//! [`LOUD`], which is well above it. The same shape `dj_app::context` uses to
//! decide the night is rising.
//!
//! # What this deliberately does not do
//!
//! It does not name the first quiet stretch an intro, though that is usually
//! what it is. An intro is a stretch with the drums out that ends on a drop,
//! which is the same fact and the same use — it is where you mix in. Splitting
//! them would need a rule about position that would be wrong on every record
//! that opens on a peak.
//!
//! It reports nothing at all rather than guessing when there is no grid to
//! count beats against, when the record never has drums, or when it never
//! loses them. All three are real answers about real records.

use crate::onset::BandedOnset;
use dj_core::{Beatgrid, SampleRate};

/// A stretch of beats with the drums out, as half-open grid beat indices.
///
/// Beat indices rather than frames because that is what the rest of this crate
/// speaks and what a phrase is measured in — and because a caller drawing this
/// has the grid anyway, while a caller *reasoning* about it (can I mix here?)
/// wants beats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    pub start: i64,
    /// Exclusive. Equal to the drop, where there is one.
    pub end: i64,
}

impl Section {
    #[must_use]
    pub const fn beats(&self) -> i64 {
        self.end - self.start
    }

    /// Whether `beat` is inside this stretch.
    #[must_use]
    pub const fn contains(&self, beat: i64) -> bool {
        beat >= self.start && beat < self.end
    }
}

/// What the record does with its drums.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnergyAnalysis {
    /// Stretches with the drums out, in the order they happen.
    pub breakdowns: Vec<Section>,
    /// The beat the drums come back on, one per breakdown that ends before the
    /// record does.
    ///
    /// Separate from the sections rather than an `Option` inside each, because
    /// a drop is a thing a DJ aims at and a breakdown is a thing they avoid:
    /// the two are used by different code and reading a list of drops should
    /// not mean filtering a list of something else.
    pub drops: Vec<i64>,
}

/// Below this fraction of the record's own drive, the drums are out.
const QUIET: f32 = 0.30;

/// Above this fraction, they are back. See the module docs for why the two
/// differ.
const LOUD: f32 = 0.60;

/// How many beats of no drums before it is a breakdown rather than a gap.
///
/// Sixteen beats is four bars. Two bars is a fill and one bar is a stutter,
/// and marking either would put a breakdown in the middle of most records'
/// choruses.
const MIN_BEATS: usize = 16;

/// The quantile of per-beat drive taken as "what this record sounds like".
///
/// Not the mean and not the median. A record that is a third breakdown drags
/// both of those down until the breakdown looks normal and the drums look
/// exceptional, which inverts the answer.
///
/// Nine tenths of the way up rather than three quarters, which was the first
/// choice and is wrong for a reason worth writing down: on a record that is
/// more than a quarter breakdown, the seventy-fifth percentile **is** the
/// breakdown, so the record is measured against its own quiet part and nothing
/// is found. A long ambient intro and outro is enough to do it. Ninety survives
/// up to nine tenths, and is still an average over tens of beats on any record
/// long enough to be measured at all.
const REFERENCE_QUANTILE: f32 = 0.90;

/// Read the breakdowns and drops out of a record.
///
/// `None` when there is nothing to say: no grid, too short to have structure,
/// or a record with no low end at all.
#[must_use]
pub fn read(
    onset: &BandedOnset,
    grid: &Beatgrid,
    rate: SampleRate,
    frames: u64,
) -> Option<EnergyAnalysis> {
    let features = crate::structure::beat_features(onset, grid, rate, frames)?;
    // Fewer beats than two of the shortest breakdown cannot show one leaving
    // and coming back, and a "breakdown" covering most of a fragment is an
    // artefact of the fragment.
    if features.len() < MIN_BEATS * 2 {
        return None;
    }
    // Band 0 is under 150 Hz. See `onset::EDGES`.
    let drive: Vec<f32> = features.iter().map(|bands| bands[0]).collect();
    let reference = quantile(&drive, REFERENCE_QUANTILE);
    if reference <= f32::EPSILON {
        return None;
    }

    let first = grid.beat_index_at(dj_core::FramePos::new(0.0), rate);
    let mut breakdowns = Vec::new();
    let mut drops = Vec::new();
    let mut started: Option<usize> = None;
    for (index, level) in drive.iter().enumerate() {
        match started {
            None if *level < QUIET * reference => started = Some(index),
            Some(begin) if *level > LOUD * reference => {
                if index - begin >= MIN_BEATS {
                    breakdowns.push(Section {
                        start: first + begin as i64,
                        end: first + index as i64,
                    });
                    drops.push(first + index as i64);
                }
                started = None;
            }
            _ => {}
        }
    }
    // A record that fades out ends inside a breakdown. That is a real stretch
    // with the drums out and it is worth drawing; what it does not have is a
    // drop, and inventing one at the last beat would put a marker on the end
    // of every record that fades.
    if let Some(begin) = started
        && drive.len() - begin >= MIN_BEATS
    {
        breakdowns.push(Section {
            start: first + begin as i64,
            end: first + drive.len() as i64,
        });
    }

    (!breakdowns.is_empty()).then_some(EnergyAnalysis { breakdowns, drops })
}

/// The value `fraction` of the way up a sorted copy of `values`.
///
/// Nearest-rank rather than interpolated: the input is a few hundred noisy
/// measurements and the difference between ranks is far below the difference
/// this is compared against.
fn quantile(values: &[f32], fraction: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f32::total_cmp);
    #[allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    let rank = ((sorted.len() - 1) as f32 * fraction).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onset::BANDS;
    use dj_core::{Bpm, Confidence, FramePos};

    const RATE: SampleRate = SampleRate::DEFAULT;
    const BPM: f64 = 120.0;

    fn grid() -> Beatgrid {
        Beatgrid {
            bpm: Bpm::new(BPM).unwrap(),
            anchor: FramePos::new(0.0),
            confidence: Confidence::new(1.0),
            beats_per_bar: 4,
        }
    }

    fn hops_per_beat() -> f64 {
        (f64::from(RATE.get()) / crate::onset::HOP as f64) * 60.0 / BPM
    }

    /// A hop curve whose kick band follows `playing(beat)`.
    ///
    /// Built at the hop level rather than from audio, for the reason
    /// `structure`'s tests are: a detector has enough slack that a wrong answer
    /// on synthesised audio can be the synthesis rather than the detector.
    fn curve(beats: usize, playing: impl Fn(usize) -> f32) -> BandedOnset {
        let hop_rate = f64::from(RATE.get()) / crate::onset::HOP as f64;
        let per_beat = hops_per_beat();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let hops = (beats as f64 * per_beat) as usize;
        let mut values = vec![[0.0f32; BANDS]; hops];
        for (index, hop) in values.iter_mut().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let exact = index as f64 / per_beat;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let beat = exact.floor() as usize;
            let phase = exact - exact.floor();
            // A transient at the top of each beat, so the flux lands where a
            // kick's would.
            let shape = if phase < 0.25 { 1.0 } else { 0.0 };
            hop[0] = shape * playing(beat);
            // Something in the upper bands throughout, so the record is never
            // silent -- a breakdown is not a gap in the audio.
            hop[2] = shape * 1.5;
        }
        BandedOnset {
            values,
            rate: hop_rate,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn frames(beats: usize) -> u64 {
        (beats as f64 * f64::from(RATE.get()) * 60.0 / BPM) as u64
    }

    /// The shape this exists to find: drums, no drums, drums again.
    #[test]
    fn a_breakdown_and_the_drop_that_ends_it_are_found() {
        let onset = curve(128, |beat| if (48..80).contains(&beat) { 0.0 } else { 1.0 });
        let found = read(&onset, &grid(), RATE, frames(128)).expect("a 32-beat breakdown");

        assert_eq!(found.breakdowns.len(), 1, "{:?}", found.breakdowns);
        let section = found.breakdowns[0];
        assert!(
            (section.start - 48).abs() <= 1,
            "the breakdown starts at beat {}, not 48",
            section.start
        );
        assert!(
            (section.end - 80).abs() <= 1,
            "the breakdown ends at beat {}, not 80",
            section.end
        );
        assert_eq!(found.drops, vec![section.end], "the drop is where it ends");
    }

    /// **A fill is not a breakdown.** Two beats without a kick happen in every
    /// chorus, and marking them would put a breakdown every sixteen bars in
    /// every record.
    #[test]
    fn a_two_beat_gap_is_not_a_breakdown() {
        let onset = curve(128, |beat| if (48..50).contains(&beat) { 0.0 } else { 1.0 });
        assert_eq!(read(&onset, &grid(), RATE, frames(128)), None);
    }

    /// **A record that is loud all the way through has no breakdown**, and
    /// saying so is the answer rather than a failure to find one.
    #[test]
    fn a_record_that_never_drops_its_drums_reports_nothing() {
        let onset = curve(128, |_| 1.0);
        assert_eq!(read(&onset, &grid(), RATE, frames(128)), None);
    }

    /// A record with no low end at all -- an a cappella, a field recording --
    /// has no drums to lose. Reporting one long breakdown would be true and
    /// useless, and it would put a marker over the whole lane.
    #[test]
    fn a_record_with_no_low_end_reports_nothing() {
        let onset = curve(128, |_| 0.0);
        assert_eq!(read(&onset, &grid(), RATE, frames(128)), None);
    }

    /// **A fade-out is a breakdown with no drop.** Half the records ever
    /// pressed end this way, and a drop invented at the last beat would put a
    /// marker on the end of all of them.
    #[test]
    fn a_record_that_ends_quiet_has_a_breakdown_and_no_drop() {
        let onset = curve(128, |beat| if beat >= 96 { 0.0 } else { 1.0 });
        let found = read(&onset, &grid(), RATE, frames(128)).expect("the ending is a breakdown");

        assert_eq!(found.breakdowns.len(), 1);
        assert!(found.drops.is_empty(), "a fade-out is not a drop");
    }

    /// **One stray kick does not end a breakdown.** This is what the two
    /// thresholds are for: with one, this record has three breakdowns and two
    /// drops, and the lane flickers where a DJ is looking for one moment.
    #[test]
    fn a_single_kick_inside_a_breakdown_does_not_end_it() {
        let onset = curve(128, |beat| match beat {
            64 => 0.45, // above QUIET, below LOUD
            48..=80 => 0.0,
            _ => 1.0,
        });
        let found = read(&onset, &grid(), RATE, frames(128)).expect("still one breakdown");

        assert_eq!(
            found.breakdowns.len(),
            1,
            "the stray kick split the breakdown: {:?}",
            found.breakdowns
        );
        assert_eq!(found.drops.len(), 1);
    }

    /// **The threshold is relative, so the same shape reads the same at any
    /// level.** A minimal techno record and a big-room house record differ by
    /// more between themselves than either differs from its own breakdown.
    ///
    /// Note what this does *not* pin: the mean would pass it too, since both
    /// sides scale together. The quantile's own job is the test below.
    #[test]
    fn the_same_shape_at_two_levels_reads_the_same() {
        let quiet = curve(
            128,
            |beat| {
                if (48..80).contains(&beat) { 0.0 } else { 0.05 }
            },
        );
        let loud = curve(128, |beat| if (48..80).contains(&beat) { 0.0 } else { 9.0 });

        let a = read(&quiet, &grid(), RATE, frames(128)).expect("a quiet record still has a shape");
        let b = read(&loud, &grid(), RATE, frames(128)).expect("so does a loud one");
        assert_eq!(a, b, "the same shape at two levels read differently");
    }

    /// **A record that is mostly breakdown still has one**, which is what the
    /// reference quantile is actually for and what the test above does not
    /// measure. Three quarters of this record has the drums out — a long
    /// ambient intro and a long outro is enough — so the mean, the median and
    /// the seventy-fifth percentile all *are* the breakdown, and the record
    /// gets measured against its own quiet part.
    #[test]
    fn a_record_that_is_mostly_breakdown_still_finds_it() {
        let onset = curve(
            128,
            |beat| if (16..112).contains(&beat) { 0.15 } else { 1.0 },
        );
        let found = read(&onset, &grid(), RATE, frames(128))
            .expect("ninety-six beats with the drums out is a breakdown");

        assert_eq!(found.breakdowns.len(), 1, "{:?}", found.breakdowns);
        assert!(
            (found.breakdowns[0].start - 16).abs() <= 1,
            "the breakdown starts at beat {}, not 16",
            found.breakdowns[0].start
        );
    }

    #[test]
    fn a_fragment_too_short_to_have_structure_reports_nothing() {
        let onset = curve(20, |beat| if beat >= 4 { 0.0 } else { 1.0 });
        assert_eq!(read(&onset, &grid(), RATE, frames(20)), None);
    }

    #[test]
    fn the_quantile_is_the_nearest_rank() {
        assert_eq!(quantile(&[], 0.5), 0.0);
        assert_eq!(quantile(&[3.0, 1.0, 2.0, 4.0, 5.0], 0.75), 4.0);
        assert_eq!(quantile(&[1.0], 0.75), 1.0);
    }
}
