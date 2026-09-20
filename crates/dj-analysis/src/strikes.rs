//! §75's *transient density*: how **often** something is struck.
//!
//! The last of [§75](../../../docs/DIRECTIVE.md)'s nine, and the one whose
//! absence had a reason filed against it that turns out to be wrong. The reason
//! was *"a curve at a resolution the overview cannot show"*, and it confuses
//! transients with their density. Individual strikes are indeed invisible at a
//! zoom where a whole record is six hundred pixels wide — but a *density* is a
//! count over a window, which is exactly the kind of thing that survives being
//! drawn small. Four minutes over six hundred pixels is four hundred
//! milliseconds a pixel, and a busy record puts several strikes in each one.
//!
//! # It is not the percussive share, and that is the whole point
//!
//! §25's `stems` layer already draws how much of a moment is percussive, from
//! [`crate::presence`]. That is a different question and the two come apart
//! constantly:
//!
//! - A sparse kick-and-clap pattern is **high** percussive share and **low**
//!   transient density. Loud, and not much is happening.
//! - A shaker rolling under a pad is **low** share and **high** density. Quiet,
//!   and a great deal is happening.
//!
//! A DJ reading a record for where to mix wants both, and a layer that drew one
//! while claiming the other would be the confusion §75 ends on: *do not confuse
//! visualization with control; every interactive visual needs clear semantics*.
//!
//! # Built on the curve that already exists
//!
//! [`crate::onset`] produces the onset detection function for the tempo
//! estimator: a value per 512-sample hop, bumping wherever energy appears. What
//! is missing is not a measurement but a *reading* of one — picking the peaks
//! out of that curve and counting them.
//!
//! So this is peak-picking and nothing else. No second pass over the audio, no
//! second FFT: the expensive part was paid for by the tempo estimator, and
//! doing it again to get a number out of the same curve would be the waste
//! [`crate::onset::detect_all`] exists to avoid.
//!
//! # Absolute, like the stem shares and unlike the energy curve
//!
//! Strikes per second, not scaled against the record's own busiest window. The
//! reason is [`crate::energy::Section::parts`]'s: *where does this record go*
//! is a question about the record, and *how busy is this* is not. A sparse
//! record scaled against its own maximum would report a flurry where there is
//! only a slightly-less-sparse passage.

use crate::onset::OnsetEnvelope;

/// How many hops either side a peak has to be the largest of.
///
/// Three at a 512-sample hop is about 35 ms each way at 44.1 kHz. Wide enough
/// that the two or three hops one drum hit smears across are one strike, narrow
/// enough that sixteenths at 180 BPM (83 ms apart) stay separate.
const LOOK: usize = 3;

/// How many hops the local threshold is averaged over, either side.
///
/// About a fifth of a second each way. The threshold has to follow the record —
/// a quiet passage with clear hits in it is dense, and a fixed threshold would
/// call it empty — and a window this long is shorter than a phrase and longer
/// than a bar, so it tracks the arrangement without tracking the beat.
const AROUND: usize = 18;

/// How far above the local mean a peak has to stand to be a strike.
///
/// A multiplier rather than an absolute, because the onset curve's scale
/// depends on the record.
///
/// Calibrated against synthesised percussion with a known pattern — see
/// `a_synthesised_pattern_is_counted_at_the_rate_it_was_written_at` below,
/// which builds a kick-and-hat part at a stated tempo and checks the density
/// that comes back. Synthesised rather than measured off a record because the
/// only audio in this container is four seconds of test tone, and a constant
/// tuned until it looked right on that would be tuned to nothing.
///
/// That is a weaker calibration than real material and is worth saying so: the
/// pattern it was fitted to has clean transients and no reverb, and a record
/// with a long tail on every hit raises the local mean and needs a lower
/// figure. This is the second number to tune against real records, after
/// [`BUSY`].
const OVER: f32 = 1.5;

/// The shortest gap between two strikes, in seconds.
///
/// Fifty milliseconds. Two hits closer together than that are one hit as far as
/// a DJ reading a waveform is concerned, and a flam counted as two would make
/// a live drummer read as twice as busy as a machine playing the same part.
const APART_SECONDS: f64 = 0.05;

/// Where things are struck, and how often.
#[derive(Debug, Clone, Default)]
pub struct Strikes {
    /// One frame position per strike, in order.
    pub at: Vec<f64>,
}

/// The density at which the interface draws this at full strength.
///
/// **A stated guess, not a measurement**, in the same way
/// [`crate::presence::STRONG`] is, and for the same reason: it is a decision
/// about drawing rather than a fact about audio. Twelve a second is a little
/// over sixteenths at 170 BPM, which is about as dense as programmed
/// percussion gets before it stops reading as separate hits at all.
///
/// Nothing in this container can check it against a room. It is the number to
/// tune first against real records.
pub const BUSY: f32 = 12.0;

impl Strikes {
    /// How many strikes a second between two frame positions.
    ///
    /// `None` when the span is empty or backwards, which is a real answer: a
    /// window with no time in it has no density, and returning zero would draw
    /// a silent passage where there is no passage at all.
    #[must_use]
    pub fn between(&self, from: f64, to: f64, rate: f64) -> Option<f32> {
        if !from.is_finite() || !to.is_finite() || to <= from || rate <= 0.0 {
            return None;
        }
        let seconds = (to - from) / rate;
        if seconds <= 0.0 {
            return None;
        }
        let found = self
            .at
            .iter()
            .filter(|at| **at >= from && **at < to)
            .count();
        #[allow(clippy::cast_precision_loss)]
        Some((found as f64 / seconds) as f32)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.at.is_empty()
    }
}

/// Pick the strikes out of an onset curve.
///
/// `sample_rate` is the audio's, so the positions come back in frames — the
/// unit everything else in this crate speaks.
#[must_use]
pub fn find(envelope: &OnsetEnvelope, sample_rate: u32) -> Strikes {
    let values = &envelope.values;
    if values.is_empty() || envelope.rate <= 0.0 || sample_rate == 0 {
        return Strikes::default();
    }

    let frames_per_hop = f64::from(sample_rate) / envelope.rate;
    let apart_hops = (APART_SECONDS * envelope.rate).max(1.0);
    let mut at = Vec::new();
    let mut last: Option<f64> = None;

    for index in 0..values.len() {
        let here = values[index];
        if here <= 0.0 {
            continue;
        }

        // The largest of its neighbourhood. `>` on one side and `>=` on the
        // other so a plateau of equal values yields one strike rather than
        // none: a perfectly flat top is rare in a real curve and certain in a
        // synthetic one, and a peak-picker that silently ignores it is a
        // peak-picker whose tests all pass on material it will never see.
        let low = index.saturating_sub(LOOK);
        let high = (index + LOOK + 1).min(values.len());
        let peak = values[low..index].iter().all(|v| here > *v)
            && values[index + 1..high].iter().all(|v| here >= *v);
        if !peak {
            continue;
        }

        let from = index.saturating_sub(AROUND);
        let until = (index + AROUND + 1).min(values.len());
        let window = &values[from..until];
        #[allow(clippy::cast_precision_loss)]
        let mean = window.iter().sum::<f32>() / window.len() as f32;
        if here < mean * OVER {
            continue;
        }

        #[allow(clippy::cast_precision_loss)]
        let hop = index as f64;
        if let Some(previous) = last
            && hop - previous < apart_hops
        {
            continue;
        }
        last = Some(hop);
        at.push(hop * frames_per_hop);
    }

    Strikes { at }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;
    /// The onset curve's own rate at a 512-sample hop.
    const HOPS: f64 = RATE as f64 / crate::onset::HOP as f64;

    fn envelope(values: Vec<f32>) -> OnsetEnvelope {
        OnsetEnvelope { values, rate: HOPS }
    }

    /// A curve with a bump every `every` hops, on a quiet floor.
    fn regular(hops: usize, every: usize) -> OnsetEnvelope {
        let values = (0..hops)
            .map(|n| if n % every == 0 { 1.0 } else { 0.05 })
            .collect();
        envelope(values)
    }

    /// **The load-bearing one: a steady pulse is counted once per hit.**
    ///
    /// Not once per hop it smears across, and not once per two. Everything
    /// this module claims rests on the count being the count.
    #[test]
    fn a_steady_pulse_is_counted_once_per_hit() {
        // Every 16 hops: about 5.4 a second at this hop rate.
        let found = find(&regular(16 * 50, 16), RATE);
        assert_eq!(
            found.at.len(),
            50,
            "expected one strike per bump, got {}",
            found.at.len()
        );
    }

    /// **Silence has no strikes**, rather than a floor of them.
    #[test]
    fn a_flat_curve_has_nothing_in_it() {
        let found = find(&envelope(vec![0.2; 400]), RATE);
        assert!(
            found.is_empty(),
            "a curve with no peaks produced {} strikes",
            found.at.len()
        );
    }

    /// **Two hits inside the refractory window are one hit.**
    ///
    /// A flam, or one drum smearing across two hops. Counting both would make
    /// a live drummer read as twice as busy as a machine playing the same part.
    #[test]
    fn a_flam_is_one_strike() {
        let mut values = vec![0.05f32; 400];
        values[100] = 1.0;
        // Two hops later, about 23 ms: inside the 50 ms refractory.
        values[102] = 1.0;
        let found = find(&envelope(values), RATE);
        assert_eq!(found.at.len(), 1, "a flam was counted twice");
    }

    /// **And two hits outside it are two.**
    ///
    /// The control for the test above. A refractory period long enough to
    /// swallow real sixteenths would make every busy record read as sparse,
    /// which is the failure that looks like the measurement working.
    #[test]
    fn sixteenths_at_a_fast_tempo_are_not_swallowed() {
        // Sixteenths at 180 BPM are 83 ms apart: about 7 hops.
        let found = find(&regular(7 * 40, 7), RATE);
        assert_eq!(found.at.len(), 40, "fast sixteenths were merged");
    }

    /// **A quiet passage with clear hits in it is dense.**
    ///
    /// The threshold follows the record, so a breakdown with a rimshot every
    /// beat is *busy* rather than empty. A fixed threshold would call it empty
    /// and the layer would then be a second, worse energy curve.
    #[test]
    fn a_quiet_passage_with_hits_in_it_is_still_dense() {
        let loud = find(&regular(16 * 30, 16), RATE).at.len();
        let quiet: Vec<f32> = (0..16 * 30)
            .map(|n| if n % 16 == 0 { 0.01 } else { 0.0005 })
            .collect();
        assert_eq!(
            find(&envelope(quiet), RATE).at.len(),
            loud,
            "the same pattern twenty decibels quieter was counted differently"
        );
    }

    /// **Density is per second, and does not depend on the window's length.**
    ///
    /// The property that makes this drawable at two zooms: a section twice as
    /// long with twice the strikes is the same density.
    #[test]
    fn density_is_a_rate_rather_than_a_count() {
        let found = find(&regular(16 * 100, 16), RATE);
        let rate = f64::from(RATE);
        let short = found.between(0.0, rate, rate).expect("a second of it");
        let long = found.between(0.0, rate * 4.0, rate).expect("four seconds");
        // The true figure is 5.38 a second -- a bump every sixteen hops at
        // 86.1 hops a second. Both readings sit within one strike of it, and
        // they differ from each other for a reason worth stating: a one-second
        // window either does or does not contain a given strike, so a short
        // window quantises. Over one second six bumps land inside; over four
        // the same pattern averages down to 5.5. A tolerance tight enough to
        // forbid that would be a test about arithmetic rather than about the
        // measurement.
        assert!(
            (short - long).abs() < 0.7,
            "one second read {short} and four read {long}, which is more than \
             one strike of quantisation apart"
        );
        for (span, read) in [("one second", short), ("four seconds", long)] {
            assert!(
                (read - 5.38).abs() < 0.7,
                "{span} read {read} where the pattern is 5.38 a second"
            );
        }
    }

    /// **A window with no time in it has no density**, rather than zero.
    ///
    /// Absent and *nothing was struck* are different answers, and the overview
    /// draws them differently -- the same distinction `Section::parts` makes.
    #[test]
    fn an_empty_span_is_absent_rather_than_zero() {
        let found = find(&regular(160, 16), RATE);
        assert!(found.between(100.0, 100.0, 44_100.0).is_none());
        assert!(found.between(200.0, 100.0, 44_100.0).is_none());
        assert!(found.between(0.0, 100.0, 0.0).is_none());
    }

    /// **The load-bearing end-to-end one: a pattern written at a known rate
    /// is counted at that rate.**
    ///
    /// Every other test here feeds the peak-picker a curve built by hand,
    /// which checks the picking and not the thing it will actually be handed.
    /// This synthesises audio — a kick on every beat and a hat on every eighth
    /// at 128 BPM, which is 6.4 strikes a second — runs the real onset
    /// detector over it, and asks what comes back.
    ///
    /// It is also what `OVER` is calibrated against, and the calibration is
    /// weaker than it looks: this material has clean transients and no reverb.
    /// A record with a long tail on every hit raises the local mean and would
    /// want a lower threshold. Said here rather than left for somebody to
    /// discover.
    #[test]
    fn a_synthesised_pattern_is_counted_at_the_rate_it_was_written_at() {
        const BPM: f64 = 128.0;
        const SECONDS: f64 = 12.0;
        let rate = f64::from(RATE);
        let beat = 60.0 / BPM * rate;
        let frames = (SECONDS * rate) as usize;

        // Interleaved stereo. A kick is a decaying 60 Hz thud, a hat a short
        // burst of noise -- enough of a transient for a flux detector, and
        // nothing like a record.
        let mut audio = vec![0.0f32; frames * 2];
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut noise = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            ((seed >> 40) as f32 / 8_388_608.0) - 1.0
        };
        let strike = |audio: &mut [f32], at: usize, kick: bool, noise: &mut dyn FnMut() -> f32| {
            let length = if kick { 4_000 } else { 900 };
            for n in 0..length {
                let index = at + n;
                if index >= frames {
                    break;
                }
                let decay = (-(n as f32) / (length as f32 * 0.3)).exp();
                let value = if kick {
                    (std::f32::consts::TAU * 60.0 * n as f32 / RATE as f32).sin() * decay * 0.9
                } else {
                    noise() * decay * 0.35
                };
                audio[index * 2] += value;
                audio[index * 2 + 1] += value;
            }
        };

        let mut n = 0.0;
        while (n * beat / 2.0) < frames as f64 {
            let at = (n * beat / 2.0) as usize;
            // Every half-beat is a hat; every whole beat is also a kick.
            strike(&mut audio, at, (n as u32).is_multiple_of(2), &mut noise);
            n += 1.0;
        }

        let envelope = crate::onset::detect(&audio, RATE);
        let found = find(&envelope, RATE);
        let density = found
            .between(0.0, frames as f64, rate)
            .expect("the whole record");

        // 128 BPM is 2.133 beats a second, and a strike every half-beat is
        // 4.267 a second. Within one strike a second, which is the honest
        // tolerance for a flux detector on a hat that is 20 ms of noise.
        assert!(
            (density - 4.267).abs() < 1.0,
            "a pattern written at 4.27 strikes a second read {density}"
        );
    }

    /// **An empty curve is not a panic.**
    #[test]
    fn nothing_to_read_is_nothing() {
        assert!(find(&envelope(Vec::new()), RATE).is_empty());
        assert!(find(&regular(160, 16), 0).is_empty());
    }
}
