//! The shape of a tune, and how to find it in a record.
//!
//! # What this is for
//!
//! Somebody hums eight seconds of a bassline they cannot name. djmanzo already
//! narrows the collection by the key and the tempo of that hum, which helps and
//! is not the same as recognising the tune. This module is the part that
//! actually compares *melodies*: a pitch contour for the hum, a pitch contour
//! for each record, and a distance between them that does not care what key it
//! was hummed in or how fast.
//!
//! # What it can and cannot do
//!
//! It matches a hum against **your own collection**. Recognising a recording
//! you do not own needs a licensed fingerprint service with tens of millions of
//! reference melodies, and djmanzo has none.
//!
//! Within the collection it is honest but not magic. Pulling the melody out of
//! a finished mix is a research problem, and what [`contour`] extracts is the
//! **strongest periodic thing in a melodic band**, which is the vocal much of
//! the time, the bassline some of the time, and a synth pad occasionally. A hum
//! of the vocal will not match a record whose loudest periodicity is the bass.
//! That is a real limit, it is why the result is a shortlist rather than an
//! answer, and it is why key and tempo stay in the ranking beside it.
//!
//! # Three choices that decide everything
//!
//! **YIN rather than plain autocorrelation.** Autocorrelation of a signal with
//! strong harmonics peaks at the octave below about as readily as at the
//! fundamental, and an octave error is a semitone error of twelve. YIN's
//! cumulative-mean-normalised difference function was designed for exactly that
//! failure and costs about the same.
//!
//! **Matching on the intervals, not on the pitches.** A tune hummed a fifth
//! higher is the same tune, so the match has to be blind to a constant offset.
//! Storing each contour relative to its own median does not achieve that, and
//! it took a failing test to see why: the hum's median is the median of eight
//! seconds and the record's is the median of five minutes, and for a
//! subsequence search those two are never the same number. The phrase sat nine
//! semitones away from where the hum thought it was, and the search found the
//! intro instead.
//!
//! So [`Contour::steps`] takes the difference between consecutive points --
//! the interval sequence, with held notes as runs of zero -- and any constant
//! offset cancels out of every difference exactly. No key detection is
//! involved and none can go wrong. Those differences are then folded into an
//! octave, which is what makes the search survive a voice that slips one; the
//! measurement behind that is in [`Contour::steps`].
//!
//! The median centring stays anyway, because a contour is also a thing to
//! draw, and a line about zero is one you can put on a screen beside another.
//!
//! **Subsequence DTW rather than whole-sequence.** The hum is eight seconds and
//! the record is five minutes, so the question is *where in this record*, not
//! *is this record the hum*. Dynamic time warping with a free start and end in
//! the reference answers that, and the warping itself is what makes the match
//! tempo-independent -- so there is no resampling step to get wrong.

/// Contour points per second.
///
/// Ten. A sung note is rarely shorter than 150 ms, so this holds the shape of
/// the tune and throws away the vibrato; and it is what makes the search
/// affordable, because dynamic time warping costs the product of the two
/// lengths. At this rate an eight-second hum against a five-minute record is
/// about 240,000 cells, which is under a millisecond.
pub const RATE: f64 = 10.0;

/// The rate the audio is resampled to before any of this.
///
/// Eight kilohertz is four times [`HIGHEST_HZ`] with room to spare, and
/// dropping to it first makes every lag in the search four to six times
/// cheaper than it would be at 44.1 or 48.
pub const WORK_RATE: u32 = 8_000;

/// The lowest fundamental worth looking for, in hertz.
///
/// Seventy. Below a low bass E, and below any voice; going lower buys longer
/// lags and octave errors rather than melodies.
pub const LOWEST_HZ: f32 = 70.0;

/// The highest fundamental worth looking for, in hertz.
///
/// Seven hundred, about F5. Above a soprano's working range and far above
/// anybody humming.
pub const HIGHEST_HZ: f32 = 700.0;

/// How unlike a period a frame may be and still count as pitched.
///
/// YIN's own threshold, on its normalised difference function: zero is a
/// perfect period and one is no periodicity at all. 0.15 is the value the
/// original paper uses and it holds up here -- a hummed note lands around
/// 0.02, a drum hit around 0.6.
pub const VOICED: f32 = 0.15;

/// The longest unvoiced gap that is filled in rather than left as a hole, in
/// contour points.
///
/// Five, so half a second. A breath between two notes of the same phrase is
/// part of the phrase and the melody either side of it is one line; a bar of
/// rest is not, and filling that would invent a held note nobody sang.
pub const LONGEST_GAP: usize = 5;

/// The shortest contour that can be a query.
///
/// Twenty points, two seconds. Below that almost anything matches almost
/// anything, which is worse than declining.
pub const SHORTEST_QUERY: usize = 20;

/// The least of a frame's energy that has to survive the band limit for the
/// frame to be worth reading a pitch from.
///
/// A twentieth. Everything a melody is made of lives below four kilohertz, so a
/// frame with less than five per cent of its energy down there is cymbals, hiss
/// or something out of band folding in, and whatever period it appears to have
/// is an artefact of the folding. White noise sits at about a sixth and passes
/// this, which is right -- it is then refused for having no period, which is
/// the reason that actually applies to it.
pub const IN_BAND: f32 = 0.05;

/// A pitch contour: what the strongest periodic thing was doing, over time.
#[derive(Debug, Clone, PartialEq)]
pub struct Contour {
    /// Semitones relative to this contour's own median pitch. `None` where
    /// nothing periodic was found.
    pub semitones: Vec<Option<f32>>,
    /// Points per second. Always [`RATE`]; carried so a reader does not have
    /// to know that.
    pub rate: f64,
}

impl Contour {
    #[must_use]
    pub fn len(&self) -> usize {
        self.semitones.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.semitones.is_empty()
    }

    /// How much of it was pitched at all, from zero to one.
    ///
    /// A hum of a tune is mostly pitched. A recording of a room, or of a drum
    /// loop, is not -- and a caller can decline before searching rather than
    /// returning the least bad of a thousand meaningless answers.
    #[must_use]
    pub fn voiced(&self) -> f32 {
        if self.semitones.is_empty() {
            return 0.0;
        }
        let voiced = self.semitones.iter().filter(|s| s.is_some()).count();
        voiced as f32 / self.semitones.len() as f32
    }

    /// The contour as a plain line, with short gaps bridged.
    ///
    /// Gaps up to [`LONGEST_GAP`] hold the pitch either side of them, because a
    /// breath in the middle of a phrase is not a change of note. Longer gaps,
    /// and the ends, sit at zero -- the contour's own median, which is where a
    /// tune with nothing happening in it belongs.
    #[must_use]
    pub fn line(&self) -> Vec<f32> {
        let mut out = vec![0.0f32; self.semitones.len()];
        let mut last: Option<(usize, f32)> = None;
        for (index, value) in self.semitones.iter().enumerate() {
            let Some(pitch) = value else { continue };
            if let Some((was, before)) = last {
                let gap = index - was - 1;
                if gap > 0 && gap <= LONGEST_GAP {
                    // Straight across, rather than a step at one end: a slide
                    // between two notes reads as a slide either way, and a step
                    // would put an edge where the singer put a breath.
                    for step in 1..=gap {
                        let fraction = step as f32 / (gap + 1) as f32;
                        out[was + step] = before + (pitch - before) * fraction;
                    }
                }
            }
            out[index] = *pitch;
            last = Some((index, *pitch));
        }
        out
    }

    /// What the tune does between one point and the next.
    ///
    /// The interval sequence: zero while a note is held, the size of the jump
    /// where it changes. This is what [`find`] compares, and it is why a match
    /// does not depend on what key anything was in -- a constant offset is in
    /// both halves of every difference and cancels.
    ///
    /// **Folded into an octave**, which is the one thing that makes this
    /// survive a real voice.
    ///
    /// Reading a note an octave below the one that was sung is the classic
    /// pitch-tracking failure, and it does not happen once: a voice that slips
    /// slips repeatedly. Each slip puts *two* twelve-semitone steps into the
    /// sequence, one down and one back, and measured on a thirty-two point
    /// hum, **three slips were enough to make a different tune score better
    /// than the right one** -- the search returned the wrong record, with
    /// confidence.
    ///
    /// Clamping the step at an octave was the first attempt and it barely
    /// moved the number (a margin of 0.67 against 0.69 -- both lost). Folding
    /// removes it: `-12` and `+12` become `0`, so a slip costs nothing at all,
    /// and the same fixture goes from losing to a perfect score.
    ///
    /// The price is that intervals are now octave-equivalent, so a rising
    /// fifth and a falling fourth read alike. That is a real loss of
    /// discrimination and it is much the smaller one: on the same fixture the
    /// wrong tune still scored 0.295 against the right tune's zero.
    #[must_use]
    pub fn steps(&self) -> Vec<f32> {
        self.line()
            .windows(2)
            .map(|pair| {
                let step = pair[1] - pair[0];
                step - OCTAVE * (step / OCTAVE).round()
            })
            .collect()
    }
}

/// Semitones in an octave.
///
/// Named because [`Contour::steps`] folds intervals into one, and a bare 12 in
/// that expression would look like a tuning knob rather than arithmetic.
pub const OCTAVE: f32 = 12.0;

/// Where a hum was found in a record, and how well.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Match {
    /// Mean semitone error per point of the query. Zero is identical.
    pub cost: f32,
    /// Seconds into the record where the matching passage starts.
    pub at_seconds: f64,
}

/// Fold interleaved channels down to one.
///
/// Its own function because the alternative is a bug that does not look like
/// one. A decoded track is interleaved stereo, and handing that straight to
/// [`contour`] does not fail or panic -- it produces a contour that is *twice
/// as long as the record*, because two channels of a five-minute track are ten
/// minutes of numbers. Everything downstream then quietly halves: a passage
/// seventy-four seconds in is reported at thirty-seven. The match itself
/// survives, since the intervals are unchanged and the warping absorbs the
/// rate, which is exactly why nothing shouts.
///
/// It also moves the pitch. With the two channels near enough alike, the
/// interleaved stream is the mono signal with every sample doubled, so a
/// period of `p` becomes `2p` and YIN reports an octave below the truth --
/// harmless for a search on octave-folded intervals, and not harmless for the
/// [`LOWEST_HZ`] gate, which then drops any bass line under 140 Hz.
///
/// `channels` of zero returns nothing rather than dividing by it.
#[must_use]
pub fn mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }
    if channels == 1 {
        return interleaved.to_vec();
    }
    let scale = 1.0 / channels as f32;
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() * scale)
        .collect()
}

/// The pitch contour of some audio.
///
/// **Mono**, resampled, and read frame by frame with YIN -- see [`mono`], which
/// is not optional for anything that came out of a decoder. Everything above
/// about [`HIGHEST_HZ`] is thrown away by the resampling, which is the point:
/// the question is what the melody is doing, and a cymbal is not a melody.
#[must_use]
pub fn contour(samples: &[f32], sample_rate: u32) -> Contour {
    let work = resample(samples, sample_rate);
    let hop = (WORK_RATE as f64 / RATE).round() as usize;
    // Long enough for two of the lowest period, which is what YIN needs to see
    // a fundamental at all, plus room for the difference function's tail.
    let window = (2.5 * WORK_RATE as f32 / LOWEST_HZ) as usize;
    let longest = (WORK_RATE as f32 / LOWEST_HZ) as usize;
    let shortest = (WORK_RATE as f32 / HIGHEST_HZ) as usize;

    let mut hertz: Vec<Option<f32>> = Vec::new();
    let mut at = 0usize;
    while at + window <= work.samples.len() {
        let frame = &work.samples[at..at + window];
        hertz.push(
            in_band(frame, &work.energy[at..at + window])
                .then(|| yin(frame, shortest, longest))
                .flatten(),
        );
        at += hop;
    }

    Contour {
        semitones: centre(&hertz),
        rate: RATE,
    }
}

/// Turn frequencies into semitones about their own median.
///
/// The median rather than the mean: a handful of octave errors would drag a
/// mean and leave every other note wrong by the amount they dragged it, and the
/// median does not move for them at all.
fn centre(hertz: &[Option<f32>]) -> Vec<Option<f32>> {
    let mut voiced: Vec<f32> = hertz.iter().flatten().copied().collect();
    if voiced.is_empty() {
        return vec![None; hertz.len()];
    }
    voiced.sort_by(f32::total_cmp);
    let middle = voiced[voiced.len() / 2];
    hertz
        .iter()
        .map(|value| value.map(|hz| 12.0 * (hz / middle).log2()))
        .collect()
}

/// The signal the pitch search reads, and how much of the original it is.
#[derive(Debug)]
struct Work {
    /// Band-limited to [`WORK_RATE`] and decimated to it.
    samples: Vec<f32>,
    /// Mean square of the *original* over the same spans, at the same points.
    energy: Vec<f32>,
}

/// Down to [`WORK_RATE`], through a filter that is actually low-pass -- and
/// carrying the evidence of what the filter threw away.
///
/// Taking every nth sample folds everything above the new Nyquist back into
/// the band this is about to search, and a folded cymbal looks exactly like a
/// note. So the signal is smoothed first, twice: one pass of a box the width
/// of the decimation ratio is a sinc whose sidelobes are only about 19 dB
/// down, and running it twice squares that.
///
/// **Attenuation alone does not fix it**, which is the part that took
/// measuring. A 9 kHz tone came through the double filter at about a hundredth
/// of its amplitude, aliased to a perfectly periodic 1 kHz -- and YIN is
/// scale-invariant, so it read that as a confident note at full confidence. A
/// quiet perfect period is still a perfect period.
///
/// So the energy before and after is carried alongside, and [`contour`] refuses
/// a frame whose in-band share is negligible. That is the honest test: not "is
/// this loud enough" but "was any of this ever in the band a melody lives in".
fn resample(samples: &[f32], sample_rate: u32) -> Work {
    if samples.is_empty() || sample_rate == 0 {
        return Work {
            samples: Vec::new(),
            energy: Vec::new(),
        };
    }
    let ratio = f64::from(sample_rate) / f64::from(WORK_RATE);
    if ratio <= 1.0 {
        return Work {
            energy: samples.iter().map(|s| s * s).collect(),
            samples: samples.to_vec(),
        };
    }
    let span = ratio.ceil() as usize;
    let smoothed = smooth(&smooth(samples, span), span);
    let squared: Vec<f32> = samples.iter().map(|s| s * s).collect();
    let power = smooth(&squared, span);
    let out_len = (smoothed.len() as f64 / ratio) as usize;
    let at = |index: usize| (index as f64 * ratio) as usize;
    Work {
        samples: (0..out_len).map(|index| smoothed[at(index)]).collect(),
        energy: (0..out_len).map(|index| power[at(index)]).collect(),
    }
}

/// A moving average of `span` samples, by prefix sums.
///
/// Prefix sums rather than a running total that is added to and subtracted
/// from: the same arithmetic, and it cannot drift or get its bookkeeping wrong
/// at the ends, which a running total in a filter nobody hears is exactly the
/// kind of thing that would go unnoticed.
fn smooth(samples: &[f32], span: usize) -> Vec<f32> {
    if span <= 1 || samples.len() < span {
        return samples.to_vec();
    }
    let mut sums = Vec::with_capacity(samples.len() + 1);
    sums.push(0.0f64);
    for value in samples {
        sums.push(sums[sums.len() - 1] + f64::from(*value));
    }
    let half = span / 2;
    (0..samples.len())
        .map(|index| {
            let start = index.saturating_sub(half);
            let end = (start + span).min(samples.len());
            let start = end - span.min(end);
            ((sums[end] - sums[start]) / (end - start) as f64) as f32
        })
        .collect()
}

/// Whether enough of this frame was ever in the band a melody lives in.
fn in_band(frame: &[f32], energy: &[f32]) -> bool {
    let kept: f32 = frame.iter().map(|s| s * s).sum();
    let had: f32 = energy.iter().sum();
    // Nothing there at all is not out of band; it is silence, and silence is
    // refused a line later for having no period.
    had <= f32::EPSILON || kept / had >= IN_BAND
}

/// One frame's fundamental, by YIN, or `None` if the frame is not pitched.
///
/// The three steps that matter, in order: the squared difference function; the
/// cumulative mean normalisation that stops it preferring the octave below;
/// and parabolic interpolation around the chosen lag, without which the answer
/// is quantised to whole samples and a 220 Hz note at 8 kHz can only be read as
/// 216 or 222.
fn yin(frame: &[f32], shortest: usize, longest: usize) -> Option<f32> {
    let longest = longest.min(frame.len() / 2);
    if longest <= shortest {
        return None;
    }

    let mut difference = vec![0.0f32; longest + 1];
    for (lag, slot) in difference.iter_mut().enumerate().skip(1) {
        let mut sum = 0.0f32;
        for index in 0..frame.len() - lag {
            let delta = frame[index] - frame[index + lag];
            sum += delta * delta;
        }
        *slot = sum;
    }

    // Cumulative mean normalisation. `difference[0]` is 1 by definition, which
    // is what keeps the search from choosing lag zero -- a signal is always
    // perfectly similar to itself.
    let mut normalised = vec![1.0f32; longest + 1];
    let mut running = 0.0f32;
    for lag in 1..=longest {
        running += difference[lag];
        normalised[lag] = if running > 0.0 {
            difference[lag] * lag as f32 / running
        } else {
            1.0
        };
    }

    // The *first* dip below the threshold, not the deepest: the deepest is
    // usually an octave or two down, and the first is the fundamental. Walks to
    // the bottom of the dip it finds, because the threshold is crossed on the
    // way down.
    let mut chosen = None;
    let mut lag = shortest;
    while lag <= longest {
        if normalised[lag] < VOICED {
            while lag < longest && normalised[lag + 1] < normalised[lag] {
                lag += 1;
            }
            chosen = Some(lag);
            break;
        }
        lag += 1;
    }
    let lag = chosen?;

    // Parabola through the chosen lag and its neighbours.
    let period = if lag > shortest && lag < longest {
        let before = normalised[lag - 1];
        let here = normalised[lag];
        let after = normalised[lag + 1];
        let curve = before + after - 2.0 * here;
        if curve.abs() > f32::EPSILON {
            lag as f32 + 0.5 * (before - after) / curve
        } else {
            lag as f32
        }
    } else {
        lag as f32
    };

    (period > 0.0).then(|| WORK_RATE as f32 / period)
}

/// Find `hum` somewhere in `track`.
///
/// Subsequence dynamic time warping: the query has to be matched end to end,
/// the reference may be entered and left anywhere, and the cost comes back
/// divided by the query's length so that costs from different hums are
/// comparable.
///
/// Returns `None` when either side is too short to say anything about.
///
/// # Cost
///
/// One pass over `hum.len() * track.len()` cells, two rows of state. A
/// five-minute record against an eight-second hum is about 240,000 cells.
#[must_use]
pub fn find(hum: &Contour, track: &Contour) -> Option<Match> {
    let query = hum.steps();
    let reference = track.steps();
    if hum.len() < SHORTEST_QUERY || query.is_empty() || reference.len() < query.len() {
        return None;
    }

    // Each cell holds the cost of the best path ending here, and where in the
    // reference that path started -- which is the answer the caller wants and
    // is free to carry along.
    let mut previous: Vec<(f32, usize)> = reference
        .iter()
        .map(|value| ((query[0] - value).abs(), 0))
        .collect();
    for (at, start) in previous.iter_mut().enumerate() {
        start.1 = at;
    }
    let mut current = vec![(0.0f32, 0usize); reference.len()];

    for step in query.iter().skip(1) {
        for (at, value) in reference.iter().enumerate() {
            let local = (step - value).abs();
            // Straight down is the only move at the left edge: a path cannot
            // enter the reference before its beginning.
            let mut best = previous[at];
            if at > 0 {
                for candidate in [previous[at - 1], current[at - 1]] {
                    if candidate.0 < best.0 {
                        best = candidate;
                    }
                }
            }
            current[at] = (best.0 + local, best.1);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    let (cost, start) = previous
        .iter()
        .copied()
        .min_by(|a, b| a.0.total_cmp(&b.0))?;
    Some(Match {
        cost: cost / query.len() as f32,
        at_seconds: start as f64 / track.rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: u32 = 48_000;

    /// A tone at `hz` for `seconds`, with a couple of harmonics so it is a
    /// hum rather than a test tone -- harmonics are what makes octave errors
    /// possible, so a test without them proves less.
    fn tone(hz: f32, seconds: f32) -> Vec<f32> {
        let frames = (seconds * SR as f32) as usize;
        (0..frames)
            .map(|n| {
                let t = n as f32 / SR as f32;
                0.6 * (TAU * hz * t).sin()
                    + 0.3 * (TAU * hz * 2.0 * t).sin()
                    + 0.15 * (TAU * hz * 3.0 * t).sin()
            })
            .collect()
    }

    /// Notes in semitones from A3, each `each` seconds long.
    fn melody(steps: &[i32], each: f32) -> Vec<f32> {
        let mut out = Vec::new();
        for step in steps {
            let hz = 220.0 * 2.0f32.powf(*step as f32 / 12.0);
            out.extend(tone(hz, each));
        }
        out
    }

    fn silence(seconds: f32) -> Vec<f32> {
        vec![0.0; (seconds * SR as f32) as usize]
    }

    /// Noise of a fixed shape, so a test that depends on it depends on the
    /// same noise every time.
    fn hiss(seconds: f32) -> Vec<f32> {
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        (0..(seconds * SR as f32) as usize)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                ((seed >> 40) as f32 / 8_388_608.0) - 1.0
            })
            .collect()
    }

    /// Interleave one mono signal into `channels` identical channels.
    fn spread(samples: &[f32], channels: usize) -> Vec<f32> {
        let mut out = Vec::with_capacity(samples.len() * channels);
        for sample in samples {
            out.extend(std::iter::repeat_n(*sample, channels));
        }
        out
    }

    /// **A contour is as long as the record, not as long as its samples.**
    ///
    /// This is the test that would have caught the wiring bug: the sweep
    /// handed [`contour`] an interleaved stereo buffer, which produced a
    /// contour of twice as many points covering the same twelve seconds. Every
    /// [`Match::at_seconds`] downstream was then half of the truth, and
    /// nothing failed -- the search still found the passage, because folding
    /// into intervals leaves them unchanged and the warping absorbs a constant
    /// rate. A number being quietly halved is exactly the kind of wrong that
    /// only a test on the *length* can see.
    #[test]
    fn a_stereo_buffer_folded_to_mono_gives_a_contour_of_the_right_length() {
        let seconds = 12.0;
        let signal = melody(&[0, 4, 7, 4], seconds / 4.0);
        let expected = (f64::from(seconds) * RATE) as usize;

        let straight = contour(&signal, SR);
        assert!(
            straight.semitones.len().abs_diff(expected) <= 2,
            "mono: {} points for {seconds}s, expected about {expected}",
            straight.semitones.len(),
        );

        let folded = contour(&mono(&spread(&signal, 2), 2), SR);
        assert!(
            folded.semitones.len().abs_diff(expected) <= 2,
            "stereo folded: {} points for {seconds}s, expected about {expected}",
            folded.semitones.len(),
        );

        // And the trap itself, stated as a fact rather than left implied: the
        // unfolded buffer is the failure this test exists to forbid.
        let unfolded = contour(&spread(&signal, 2), SR);
        assert!(
            unfolded.semitones.len() > expected * 3 / 2,
            "an interleaved buffer read as mono should have been about twice \
             as long, so this test no longer proves what it claims",
        );
    }

    /// Folding averages the channels rather than picking one or summing them.
    #[test]
    fn mono_averages_the_channels() {
        assert_eq!(mono(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
        assert_eq!(mono(&[1.0, 2.0, 3.0], 1), vec![1.0, 2.0, 3.0]);
        // A ragged tail is dropped rather than read as a short frame.
        assert_eq!(mono(&[1.0, 1.0, 9.0], 2), vec![1.0]);
        assert!(mono(&[1.0, 2.0], 0).is_empty());
    }

    /// **The fundamental, even when the fundamental is not there.**
    ///
    /// Harmonics 2, 3 and 4 of 150 Hz and nothing at 150. Plain
    /// autocorrelation reads this as 300; the cumulative mean normalisation is
    /// the step that does not, and this is the test that says so.
    #[test]
    fn a_missing_fundamental_is_still_the_fundamental() {
        let frames = SR as usize;
        let samples: Vec<f32> = (0..frames)
            .map(|n| {
                let t = n as f32 / SR as f32;
                0.5 * (TAU * 300.0 * t).sin()
                    + 0.4 * (TAU * 450.0 * t).sin()
                    + 0.3 * (TAU * 600.0 * t).sin()
            })
            .collect();
        let work = resample(&samples, SR);
        let longest = (WORK_RATE as f32 / LOWEST_HZ) as usize;
        let shortest = (WORK_RATE as f32 / HIGHEST_HZ) as usize;
        let window = (2.5 * WORK_RATE as f32 / LOWEST_HZ) as usize;
        let found = yin(&work.samples[..window], shortest, longest).expect("pitched");
        let cents = 1200.0 * (found / 150.0).log2();
        assert!(
            cents.abs() < 30.0,
            "read as {found} Hz, wanted 150 ({cents} cents)"
        );
    }

    /// **Noise has no melody, however loud it is.**
    #[test]
    fn hiss_is_not_a_tune() {
        let shape = contour(&hiss(3.0), SR);
        assert!(
            shape.voiced() < 0.05,
            "noise read as {} voiced",
            shape.voiced()
        );
    }

    /// **Something entirely above the working band does not fold into a note.**
    ///
    /// A 9 kHz tone, well above the 4 kHz this works at. Decimation alone
    /// aliases it to a perfectly periodic 1 kHz; filtering alone only makes
    /// that alias quiet, and YIN does not care how quiet a period is. Before
    /// the in-band test this read as **fully** voiced -- a confident melody
    /// where there is not even a sound a person can enjoy.
    #[test]
    fn a_tone_above_the_band_does_not_become_a_note() {
        let frames = 3 * SR as usize;
        let samples: Vec<f32> = (0..frames)
            .map(|n| (TAU * 9_000.0 * n as f32 / SR as f32).sin())
            .collect();
        let shape = contour(&samples, SR);
        assert!(
            shape.voiced() < 0.05,
            "a 9 kHz tone read as {} voiced",
            shape.voiced()
        );
    }

    /// **Just above the band is the hard case, and the second filter pass is
    /// what handles it.**
    ///
    /// Six kilohertz folds to two, which is inside the band, so the in-band
    /// test cannot see it -- the alias really is in the band. Only attenuating
    /// it before the fold helps, and one pass of the box does not attenuate it
    /// enough: measured, a 6 kHz tone reads as **fully** voiced through one
    /// pass and as nothing through two. That is what the second pass is for,
    /// and this is the only test that says so.
    #[test]
    fn a_tone_just_above_the_band_does_not_fold_into_a_note() {
        let frames = 3 * SR as usize;
        let samples: Vec<f32> = (0..frames)
            .map(|n| (TAU * 6_000.0 * n as f32 / SR as f32).sin())
            .collect();
        let shape = contour(&samples, SR);
        assert!(
            shape.voiced() < 0.05,
            "a 6 kHz tone read as {} voiced",
            shape.voiced()
        );
    }

    /// **A hum through a laptop microphone is still a hum.**
    ///
    /// Noise at three tenths of the melody's amplitude, which is far worse
    /// than a phone in a quiet room and about what a laptop in a busy one
    /// sounds like. Every point still reads as pitched.
    #[test]
    fn a_hum_survives_a_noisy_room() {
        let mut mix = melody(&[0, 4, 7, 12, 7, 4], 0.5);
        let noise = hiss(mix.len() as f32 / SR as f32);
        for (sample, grain) in mix.iter_mut().zip(&noise) {
            *sample += *grain * 0.3;
        }
        let shape = contour(&mix, SR);
        assert!(
            shape.voiced() > 0.9,
            "a hum under noise read as {} voiced",
            shape.voiced()
        );

        // And it is still the same tune underneath.
        let clean = contour(&melody(&[0, 4, 7, 12, 7, 4], 0.5), SR);
        let found = find(&clean, &shape).expect("a match");
        assert!(
            found.cost < 0.5,
            "noise moved the tune by {} a point",
            found.cost
        );
    }

    /// **The fundamental, not one of its harmonics and not the octave below.**
    #[test]
    fn a_hummed_note_reads_as_its_own_pitch() {
        for hz in [110.0f32, 220.0, 330.0, 440.0] {
            let work = resample(&tone(hz, 1.0), SR);
            let longest = (WORK_RATE as f32 / LOWEST_HZ) as usize;
            let shortest = (WORK_RATE as f32 / HIGHEST_HZ) as usize;
            let window = (2.5 * WORK_RATE as f32 / LOWEST_HZ) as usize;
            let found = yin(&work.samples[..window], shortest, longest).expect("a tone is pitched");
            let cents = 1200.0 * (found / hz).log2();
            assert!(
                cents.abs() < 20.0,
                "{hz} Hz read as {found} Hz ({cents} cents)"
            );
        }
    }

    /// **Silence is unpitched, rather than pitched at whatever the noise did.**
    #[test]
    fn silence_has_no_melody() {
        let shape = contour(&silence(3.0), SR);
        assert!(
            shape.voiced() < 0.05,
            "silence read as {} voiced",
            shape.voiced()
        );
    }

    /// **The intervals come back, in semitones.**
    #[test]
    fn a_scale_rises_by_the_steps_it_was_sung_in() {
        let shape = contour(&melody(&[0, 4, 7, 12], 0.6), SR);
        let line = shape.line();
        // The middle of each note, avoiding the frames straddling a change.
        let per_note = (0.6 * RATE as f32) as usize;
        let read = |note: usize| line[note * per_note + per_note / 2];
        let base = read(0);
        for (note, expected) in [(1usize, 4.0f32), (2, 7.0), (3, 12.0)] {
            let got = read(note) - base;
            assert!(
                (got - expected).abs() < 0.6,
                "note {note} read as {got} semitones above the first, wanted {expected}"
            );
        }
    }

    /// **The same tune in another key is the same contour.**
    ///
    /// This is the whole of the key normalisation, so it is worth asserting
    /// directly rather than only through a match score.
    #[test]
    fn transposing_a_tune_does_not_change_its_shape() {
        let steps = [0, 2, 4, 5, 4, 2, 0];
        let low = contour(&melody(&steps, 0.5), SR);
        let high = contour(&melody(&steps.map(|s| s + 7), 0.5), SR);
        let (a, b) = (low.line(), high.line());
        let n = a.len().min(b.len());
        let worst = (0..n).map(|i| (a[i] - b[i]).abs()).fold(0.0f32, f32::max);
        assert!(
            worst < 0.7,
            "a fifth up changed the shape by {worst} semitones"
        );
    }

    /// **The right record scores better than the wrong one.**
    #[test]
    fn the_tune_that_matches_wins() {
        let hum = contour(&melody(&[0, 2, 4, 5, 7, 5, 4, 2], 0.4), SR);
        let same = contour(&melody(&[0, 2, 4, 5, 7, 5, 4, 2], 0.4), SR);
        let other = contour(&melody(&[0, -3, 7, -5, 11, 1, -7, 9], 0.4), SR);

        let right = find(&hum, &same).expect("a match");
        let wrong = find(&hum, &other).expect("a match");
        assert!(
            right.cost < wrong.cost / 3.0,
            "the right tune scored {} and the wrong one {}",
            right.cost,
            wrong.cost
        );
    }

    /// **Tempo does not have to be guessed, because warping absorbs it.**
    ///
    /// The same tune at half again the speed. No resampling step is involved
    /// anywhere -- if this passes, dynamic time warping is doing the work it
    /// was chosen for.
    #[test]
    fn the_same_tune_hummed_faster_still_matches() {
        let steps = [0, 2, 4, 5, 7, 5, 4, 2];
        let fast = contour(&melody(&steps, 0.3), SR);
        let slow = contour(&melody(&steps, 0.45), SR);
        let other = contour(&melody(&[0, -3, 7, -5, 11, 1, -7, 9], 0.45), SR);

        let right = find(&fast, &slow).expect("a match");
        let wrong = find(&fast, &other).expect("a match");
        assert!(
            right.cost < wrong.cost / 2.0,
            "the same tune at another tempo scored {} against {} for a different tune",
            right.cost,
            wrong.cost
        );
    }

    /// **It says where in the record the passage is.**
    #[test]
    fn a_passage_is_found_where_it_actually_starts() {
        let phrase = [0, 4, 7, 12, 7, 4];
        let hum = contour(&melody(&phrase, 0.4), SR);

        let mut record = melody(&[-5, -5, -3, -3, -5, -5, -3, -3], 0.4);
        let before = record.len() as f64 / f64::from(SR);
        record.extend(melody(&phrase, 0.4));
        record.extend(melody(&[-7, -7, -7, -7], 0.4));
        let record = contour(&record, SR);

        let found = find(&hum, &record).expect("a match");
        assert!(
            (found.at_seconds - before).abs() < 1.0,
            "the phrase starts at {before:.1}s and was reported at {:.1}s",
            found.at_seconds
        );
    }

    /// **A query too short to mean anything is declined rather than answered.**
    #[test]
    fn something_shorter_than_a_phrase_is_refused() {
        let hum = contour(&melody(&[0, 4], 0.4), SR);
        let record = contour(&melody(&[0, 4, 7, 12, 7, 4, 0, 4, 7], 0.4), SR);
        assert!(
            hum.len() < SHORTEST_QUERY,
            "the fixture stopped being short"
        );
        assert!(find(&hum, &record).is_none());
    }

    /// **A few octave slips do not move the line everything else is drawn
    /// against.**
    ///
    /// The centring is what makes a contour a thing you can put on a screen
    /// beside another one, and it is a median rather than a mean for this: a
    /// handful of frames read an octave low would drag a mean down and leave
    /// every correct note reading high by the amount they dragged it. The
    /// median does not move for them at all.
    #[test]
    fn octave_slips_do_not_drag_the_centre() {
        // Nine notes at 220 Hz and three at 110: a mean sits about three
        // semitones below the note that was actually sung, a median on it.
        let mut hertz = vec![Some(220.0f32); 9];
        hertz.extend(vec![Some(110.0f32); 3]);
        let centred = centre(&hertz);
        let sung: Vec<f32> = centred[..9].iter().map(|v| v.expect("voiced")).collect();
        let worst = sung.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(
            worst < 0.01,
            "the notes that were sung right ended up {worst} semitones off centre"
        );
    }

    /// **A voice that slips an octave does not lose its own record.**
    ///
    /// Three frames of a thirty-two point hum read an octave low, which is
    /// what a real voice does. Before the intervals were folded this returned
    /// the *wrong* tune -- 2.13 against 1.42 -- because each slip put two
    /// twelve-semitone steps into a sequence whose real steps are twos and
    /// threes.
    #[test]
    fn a_voice_that_slips_an_octave_keeps_its_record() {
        let steps = [0, 2, 4, 5, 7, 5, 4, 2];
        let hum = contour(&melody(&steps, 0.4), SR);
        let record = contour(&melody(&steps, 0.4), SR);
        let other = contour(&melody(&[0, -3, 7, -5, 11, 1, -7, 9], 0.4), SR);

        let mut slipped = hum.clone();
        for at in [7usize, 15, 23] {
            slipped.semitones[at] = slipped.semitones[at].map(|value| value - OCTAVE);
        }

        let right = find(&slipped, &record).expect("a match");
        let wrong = find(&slipped, &other).expect("a match");
        assert!(
            right.cost < wrong.cost / 3.0,
            "with three octave slips the right record scored {} and the wrong one {}",
            right.cost,
            wrong.cost
        );
    }

    /// **A record shorter than the hum is declined, not padded.**
    #[test]
    fn a_record_shorter_than_the_hum_is_refused() {
        let hum = contour(&melody(&[0, 2, 4, 5, 7, 5, 4, 2, 0, 2], 0.4), SR);
        let short = contour(&melody(&[0, 2], 0.4), SR);
        assert!(find(&hum, &short).is_none());
    }

    /// **A breath in the middle of a phrase does not become a held note.**
    #[test]
    fn a_short_gap_is_bridged_and_a_long_one_is_not() {
        let mut semitones = vec![Some(0.0f32); 4];
        semitones.extend(std::iter::repeat_n(None, 3)); // 300 ms -- a breath
        semitones.extend(vec![Some(4.0f32); 4]);
        semitones.extend(std::iter::repeat_n(None, 12)); // 1.2 s -- a rest
        semitones.extend(vec![Some(7.0f32); 4]);
        let line = Contour {
            semitones,
            rate: RATE,
        }
        .line();

        assert!(
            line[4] > 0.0 && line[6] < 4.0 && line[4] < line[6],
            "the breath was not bridged: {:?}",
            &line[4..7]
        );
        // The rest runs from 11 to 22 inclusive; 23 is the next sung note.
        assert!(
            line[11..23].iter().all(|v| *v == 0.0),
            "the rest was filled in: {:?}",
            &line[11..23]
        );
    }
}
