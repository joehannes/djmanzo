//! Harmonic/percussive separation, and the four-way split built on it.
//!
//! # The idea, in one picture
//!
//! Look at a spectrogram. A sustained note is a **horizontal** line: the same
//! frequency, frame after frame. A drum hit is a **vertical** line: every
//! frequency at once, for an instant. So:
//!
//! - median-filter each frequency bin **along time** and horizontal structure
//!   survives while transients are flattened away — that is the harmonic
//!   estimate;
//! - median-filter each frame **along frequency** and the reverse happens —
//!   that is the percussive estimate.
//!
//! Published by Fitzgerald in 2010, and it is arithmetic rather than a model:
//! no weights to download, no runtime to install, no licence to read.
//!
//! # Soft masks, not hard choices
//!
//! Each bin is not assigned to one side. Both estimates are turned into masks
//! that **sum to one**:
//!
//! ```text
//!   harmonic_mask = h² / (h² + p² + ε)
//!   percussive_mask = p² / (h² + p² + ε)
//! ```
//!
//! Applied to the *original complex spectrum* — phase and all — the two parts
//! reconstruct to exactly the input.
//!
//! To be precise about which property does what, because it is easy to credit
//! the wrong one: the crate's central invariant — the stems sum back to the
//! mix — comes from the masks **partitioning** each bin, not from their being
//! soft. A hard `h² >= p²` assignment partitions too, and the sum survives it;
//! that was measured, and `the_stems_sum_back_to_the_mix` says so.
//!
//! Soft masks are here for a different reason: a hard assignment flips a bin's
//! entire content from one stem to the other the instant the two estimates
//! cross, which is the musical noise that characterises a bad separator. That
//! is a perceptual claim, and nothing in this crate's tests demonstrates it —
//! it rests on the published method rather than on our own measurement.
//!
//! # From two parts to four
//!
//! HPSS gives drums and everything-else. The rest is split with the tools a
//! mixing engineer would use, and the same assumptions:
//!
//! - **bass** is the harmonic part below [`BASS_HZ`];
//! - **vocals** is what is *centred* in the harmonic part between [`BASS_HZ`]
//!   and [`VOCAL_TOP_HZ`] — a lead vocal is panned dead centre in almost every
//!   record ever mastered;
//! - **other** is everything left: the sides of that band, and the harmonic
//!   part above it.
//!
//! Where this is wrong is worth stating plainly. A centred synth lead lands in
//! "vocals". A percussive bassline lands in "drums". A mono recording has no
//! sides, so "other" is nearly empty and the voice takes the whole middle. It
//! is a real separator and it is not a good one; it is the one that works with
//! no download, and [`crate::Separator`] is the seam where a better one goes.

use crate::stems::{Separator, StemError, Stems};
use dj_core::Stem;
use rustfft::{FftPlanner, num_complex::Complex32};

/// Window length. About 46 ms at 48 kHz.
///
/// Long enough that a sustained note occupies several frames — which is what
/// makes it horizontal in the first place — and short enough that a kick is not
/// smeared across half a bar.
const WINDOW: usize = 2048;
/// Hop. A quarter of the window, so four frames overlap.
///
/// Overlap-add with a Hann window at 75% is exactly unity, which is why the
/// reconstruction is sample-accurate rather than approximate.
const HOP: usize = WINDOW / 4;

/// Median filter length along time, in frames. Odd, so there is a middle.
///
/// Seventeen frames is about 180 ms — longer than any transient and shorter
/// than a held note.
const TIME_SPAN: usize = 17;
/// Median filter length along frequency, in bins. Odd.
///
/// Seventeen bins is about 400 Hz at this window — wide enough to flatten a
/// harmonic peak, narrow enough to keep a broadband hit.
const FREQ_SPAN: usize = 17;

/// Where the bass ends and the vocal band begins.
pub const BASS_HZ: f32 = 200.0;
/// Where the vocal band ends.
pub const VOCAL_TOP_HZ: f32 = 8_000.0;

/// Guards the mask denominator. Below this a bin is silent and the split is
/// arbitrary, so it is shared evenly rather than divided by nearly zero.
const EPSILON: f32 = 1e-12;

/// Classical four-way separation. No model, no download, no GPU.
#[derive(Debug, Default, Clone, Copy)]
pub struct Hpss;

impl Separator for Hpss {
    fn name(&self) -> &'static str {
        // Shown to a DJ next to the stem controls, so it says which of the two
        // separators is running rather than which algorithm it happens to use.
        "built-in (harmonic/percussive)"
    }

    fn separate(&self, mix: &[f32], sample_rate: u32) -> Result<Stems, StemError> {
        if sample_rate == 0 {
            return Err(StemError::NoSampleRate);
        }
        if !mix.len().is_multiple_of(2) {
            return Err(StemError::NotStereo(mix.len()));
        }
        Ok(separate_stereo(mix, sample_rate))
    }
}

fn separate_stereo(mix: &[f32], sample_rate: u32) -> Stems {
    let frames = mix.len() / 2;
    let mut parts: [Vec<f32>; Stem::COUNT] = [
        vec![0.0; mix.len()],
        vec![0.0; mix.len()],
        vec![0.0; mix.len()],
        vec![0.0; mix.len()],
    ];

    // Mid and side rather than left and right. The vocal test is "is it
    // centred", and that is a question about mid against side — asking it of
    // left and right separately would answer a different question in each ear.
    let mut mid = Vec::with_capacity(frames);
    let mut side = Vec::with_capacity(frames);
    for frame in mix.as_chunks::<2>().0 {
        mid.push((frame[0] + frame[1]) * 0.5);
        side.push((frame[0] - frame[1]) * 0.5);
    }

    let mid = split(&mid, sample_rate);
    let side = split(&side, sample_rate);

    // Back to left and right. Mid + side is left, mid − side is right, and
    // every stem is reconstructed the same way — so the four of them sum to the
    // input exactly as the mid/side pair did.
    //
    // The side channel's harmonic content is *all* "other": something present
    // only in the sides is by definition not centred, so it is not the lead
    // vocal. Its percussive content still belongs with the drums, because a
    // hi-hat panned left is a drum.
    for frame in 0..frames {
        let write = |parts: &mut [Vec<f32>; Stem::COUNT], stem: Stem, m: f32, s: f32| {
            parts[stem.index()][frame * 2] = m + s;
            parts[stem.index()][frame * 2 + 1] = m - s;
        };
        write(
            &mut parts,
            Stem::Drums,
            mid.percussive[frame],
            side.percussive[frame],
        );
        write(&mut parts, Stem::Bass, mid.bass[frame], side.bass[frame]);
        write(&mut parts, Stem::Vocal, mid.vocal[frame], 0.0);
        write(
            &mut parts,
            Stem::Other,
            mid.other[frame],
            side.vocal[frame] + side.other[frame],
        );
    }

    Stems::new(parts, sample_rate).expect("built from one mix, so never ragged")
}

/// One channel, taken apart.
struct Split {
    percussive: Vec<f32>,
    bass: Vec<f32>,
    /// Harmonic, inside the vocal band.
    vocal: Vec<f32>,
    /// Harmonic, above the vocal band.
    other: Vec<f32>,
}

/// Analyse one channel and resynthesise its four parts.
fn split(channel: &[f32], sample_rate: u32) -> Split {
    let frames = channel.len();
    let mut out = Split {
        percussive: vec![0.0; frames],
        bass: vec![0.0; frames],
        vocal: vec![0.0; frames],
        other: vec![0.0; frames],
    };
    if frames < WINDOW {
        // Shorter than one window: there is no spectrogram to filter, so there
        // is nothing to separate. It all goes to `other`, which keeps the sum
        // exact — the alternative is to invent structure from a fragment.
        out.other.copy_from_slice(channel);
        return out;
    }

    let window = hann(WINDOW);
    let mut planner = FftPlanner::<f32>::new();
    let forward = planner.plan_fft_forward(WINDOW);
    let inverse = planner.plan_fft_inverse(WINDOW);

    // The whole spectrogram, because a median along time needs to see along
    // time. This is why separation is an offline job on a worker thread and not
    // something the audio thread could ever do.
    let count = (frames - WINDOW) / HOP + 1;
    let bins = WINDOW / 2 + 1;
    let mut spectra: Vec<Vec<Complex32>> = Vec::with_capacity(count);
    let mut magnitude: Vec<Vec<f32>> = Vec::with_capacity(count);
    let mut scratch = vec![Complex32::new(0.0, 0.0); WINDOW];

    for index in 0..count {
        let start = index * HOP;
        for (slot, (sample, coefficient)) in scratch
            .iter_mut()
            .zip(channel[start..start + WINDOW].iter().zip(&window))
        {
            *slot = Complex32::new(sample * coefficient, 0.0);
        }
        forward.process(&mut scratch);
        magnitude.push(scratch[..bins].iter().map(|bin| bin.norm()).collect());
        spectra.push(scratch[..bins].to_vec());
    }

    let harmonic = median_along_time(&magnitude, bins);
    let percussive = median_along_frequency(&magnitude);

    let hz_per_bin = sample_rate as f32 / WINDOW as f32;
    let bass_bin = (BASS_HZ / hz_per_bin).round() as usize;
    let vocal_top_bin = ((VOCAL_TOP_HZ / hz_per_bin).round() as usize).min(bins);

    // Resynthesise. Four overlap-add passes over the same frames, each with its
    // own mask — so every pass sees the original phase and the four of them
    // reconstruct the input.
    let mut sums = [
        vec![0.0f32; frames],
        vec![0.0f32; frames],
        vec![0.0f32; frames],
        vec![0.0f32; frames],
    ];
    let mut weight = vec![0.0f32; frames];

    for index in 0..count {
        let start = index * HOP;
        #[allow(
            clippy::needless_range_loop,
            reason = "the index selects both which mask to apply and which scratch to write"
        )]
        for part in 0..4 {
            for bin in 0..bins {
                let h = harmonic[index][bin];
                let p = percussive[index][bin];
                let (h2, p2) = (h * h, p * p);
                let total = h2 + p2 + EPSILON;
                let harmonic_share = h2 / total;
                let percussive_share = p2 / total;

                // Which of the four this bin's harmonic share belongs to.
                let share = match part {
                    0 => percussive_share,
                    1 if bin < bass_bin => harmonic_share,
                    2 if bin >= bass_bin && bin < vocal_top_bin => harmonic_share,
                    3 if bin >= vocal_top_bin => harmonic_share,
                    _ => 0.0,
                };
                scratch[bin] = spectra[index][bin] * share;
            }
            // Rebuild the negative frequencies: the signal is real, so the
            // upper half is the conjugate mirror of the lower. Leaving it zero
            // would halve the amplitude and put an imaginary part in the
            // output — and the stems would not sum.
            for bin in bins..WINDOW {
                scratch[bin] = scratch[WINDOW - bin].conj();
            }
            inverse.process(&mut scratch);
            let scale = 1.0 / WINDOW as f32;
            for (offset, coefficient) in window.iter().enumerate() {
                sums[part][start + offset] += scratch[offset].re * scale * coefficient;
            }
        }
        for (offset, coefficient) in window.iter().enumerate() {
            weight[start + offset] += coefficient * coefficient;
        }
    }

    // Divide out the window's overlap. Hann at 75% overlap sums to a constant
    // across the middle, but not at the two ends, where fewer windows land —
    // so this is a division rather than a constant, and the first and last
    // window of a track come out at the right level rather than fading in.
    for frame in 0..frames {
        let w = weight[frame];
        for (part, sum) in sums.iter().enumerate() {
            let value = if w > 1e-6 {
                sum[frame] / w
            } else {
                // Outside every window — the tail shorter than one hop. Nothing
                // was analysed there, so it is passed through on `other` to
                // keep the sum exact.
                if part == 3 { channel[frame] } else { 0.0 }
            };
            match part {
                0 => out.percussive[frame] = value,
                1 => out.bass[frame] = value,
                2 => out.vocal[frame] = value,
                _ => out.other[frame] = value,
            }
        }
    }
    out
}

/// Median of each frequency bin over a sliding window of frames.
fn median_along_time(magnitude: &[Vec<f32>], bins: usize) -> Vec<Vec<f32>> {
    let count = magnitude.len();
    let half = TIME_SPAN / 2;
    let mut out = vec![vec![0.0f32; bins]; count];
    let mut window = Vec::with_capacity(TIME_SPAN);
    for bin in 0..bins {
        #[allow(
            clippy::needless_range_loop,
            reason = "the index is the centre of the sliding window as well as the write target"
        )]
        for index in 0..count {
            window.clear();
            let from = index.saturating_sub(half);
            let to = (index + half + 1).min(count);
            window.extend(magnitude[from..to].iter().map(|frame| frame[bin]));
            out[index][bin] = median(&mut window);
        }
    }
    out
}

/// Median of each frame over a sliding window of bins.
fn median_along_frequency(magnitude: &[Vec<f32>]) -> Vec<Vec<f32>> {
    let half = FREQ_SPAN / 2;
    let mut window = Vec::with_capacity(FREQ_SPAN);
    magnitude
        .iter()
        .map(|frame| {
            (0..frame.len())
                .map(|bin| {
                    window.clear();
                    let from = bin.saturating_sub(half);
                    let to = (bin + half + 1).min(frame.len());
                    window.extend_from_slice(&frame[from..to]);
                    median(&mut window)
                })
                .collect()
        })
        .collect()
}

/// Median by partial sort. `select_nth_unstable` rather than a full sort: it is
/// linear, and this is called once per bin per frame.
fn median(values: &mut [f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let middle = values.len() / 2;
    values.select_nth_unstable_by(middle, |a, b| {
        a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
    });
    values[middle]
}

fn hann(length: usize) -> Vec<f32> {
    (0..length)
        .map(|i| {
            let phase = std::f32::consts::TAU * i as f32 / length as f32;
            0.5 - 0.5 * phase.cos()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    /// Interleaved stereo from one mono signal, centred.
    fn centred(mono: &[f32]) -> Vec<f32> {
        mono.iter().flat_map(|s| [*s, *s]).collect()
    }

    fn sine(hz: f32, frames: usize, amplitude: f32) -> Vec<f32> {
        (0..frames)
            .map(|i| (std::f32::consts::TAU * hz * i as f32 / RATE as f32).sin() * amplitude)
            .collect()
    }

    /// A click every `period` frames — broadband and instantaneous, which is
    /// what a drum looks like to a spectrogram.
    fn clicks(period: usize, frames: usize, amplitude: f32) -> Vec<f32> {
        (0..frames)
            .map(|i| if i % period == 0 { amplitude } else { 0.0 })
            .collect()
    }

    fn energy(samples: &[f32]) -> f32 {
        samples.iter().map(|s| s * s).sum::<f32>().sqrt()
    }

    /// **The invariant the whole crate rests on.** A DJ who has not touched a
    /// stem control must hear exactly the track — so merely switching a deck
    /// into stem mode cannot change the sound.
    ///
    /// # Why the signal has four ingredients
    ///
    /// The first version of this test used a 440 Hz sine and a click track,
    /// and it was too weak to be worth having: halving the harmonic share of
    /// the band above [`VOCAL_TOP_HZ`] left it green, because that signal has
    /// essentially no harmonic energy up there for the mutation to lose.
    ///
    /// The mix below deliberately puts sustained energy in *every* band the
    /// partition splits on — below [`BASS_HZ`], between the two edges, above
    /// [`VOCAL_TOP_HZ`] — plus something percussive. Each of the four `match`
    /// arms in `separate_stereo` then carries real signal, so dropping or
    /// scaling any one of them shows up here.
    ///
    /// What this does **not** test is soft masking. Measured: replacing the
    /// `h²/(h²+p²)` masks with a hard `h² >= p²` assignment leaves the sum
    /// intact, because a hard assignment still partitions each bin exactly
    /// once. Soft masks are here to avoid musical noise at the boundaries,
    /// which is a perceptual claim these tests do not make.
    #[test]
    fn the_stems_sum_back_to_the_mix() {
        let frames = 48_000;
        let low = sine(80.0, frames, 0.3); // under BASS_HZ
        let mid = sine(440.0, frames, 0.3); // between the edges
        let high = sine(11_000.0, frames, 0.3); // over VOCAL_TOP_HZ
        let hits = clicks(6_000, frames, 0.6); // percussive

        let mono: Vec<f32> = (0..frames)
            .map(|i| low[i] + mid[i] + high[i] + hits[i])
            .collect();
        let mix = centred(&mono);
        let stems = Hpss.separate(&mix, RATE).unwrap();

        // Every stem has to have received something, or the test would pass
        // by summing three real parts and one silence.
        for stem in Stem::ALL {
            assert!(
                energy(stems.get(stem)) > 0.0,
                "{stem} got nothing, so this signal does not exercise it"
            );
        }

        let error = stems.error_against(&mix);
        assert!(
            error < 1e-3,
            "the stems drifted from the mix by {error} at worst"
        );
    }

    /// And with a genuinely stereo signal, where mid and side are separate
    /// problems and the reconstruction has to put them back together.
    #[test]
    fn the_stems_sum_back_to_a_stereo_mix() {
        let left = sine(300.0, 32_768, 0.3);
        let right = sine(700.0, 32_768, 0.3);
        let mix: Vec<f32> = left
            .iter()
            .zip(&right)
            .flat_map(|(l, r)| [*l, *r])
            .collect();
        let stems = Hpss.separate(&mix, RATE).unwrap();
        let error = stems.error_against(&mix);
        assert!(error < 1e-3, "stereo drifted by {error}");
    }

    /// **The feature.** A click track is percussive and a sine is not, so they
    /// end up in different stems.
    #[test]
    fn drums_and_a_held_note_go_to_different_stems() {
        let mono: Vec<f32> = sine(1_000.0, 48_000, 0.4)
            .iter()
            .zip(clicks(4_800, 48_000, 0.8))
            .map(|(a, b)| a + b)
            .collect();
        let stems = Hpss.separate(&centred(&mono), RATE).unwrap();

        let drums = energy(stems.get(Stem::Drums));
        // The sine is at 1 kHz, centred, so it lands in the vocal band.
        let held = energy(stems.get(Stem::Vocal));
        assert!(drums > 0.0 && held > 0.0, "drums {drums}, held {held}");

        // Now the same click track with no sine, and the same sine with no
        // clicks. Each should move its own stem far more than the other's.
        let only_clicks = Hpss
            .separate(&centred(&clicks(4_800, 48_000, 0.8)), RATE)
            .unwrap();
        let only_sine = Hpss
            .separate(&centred(&sine(1_000.0, 48_000, 0.4)), RATE)
            .unwrap();

        assert!(
            energy(only_clicks.get(Stem::Drums)) > energy(only_clicks.get(Stem::Vocal)),
            "a click track was not mostly drums"
        );
        assert!(
            energy(only_sine.get(Stem::Vocal)) > energy(only_sine.get(Stem::Drums)),
            "a held note was not mostly harmonic"
        );
    }

    /// Bass is the harmonic part below 200 Hz, and it must not be the vocal.
    #[test]
    fn a_low_note_lands_in_the_bass() {
        let stems = Hpss
            .separate(&centred(&sine(60.0, 48_000, 0.5)), RATE)
            .unwrap();
        let bass = energy(stems.get(Stem::Bass));
        let vocals = energy(stems.get(Stem::Vocal));
        assert!(bass > vocals * 4.0, "bass {bass}, vocals {vocals}");
    }

    /// A lead vocal is centred; something only in the sides is not one, however
    /// much it looks like a voice.
    #[test]
    fn something_only_in_the_sides_is_never_the_vocal() {
        // Anti-phase: pure side, no mid at all.
        let mono = sine(1_000.0, 32_768, 0.4);
        let mix: Vec<f32> = mono.iter().flat_map(|s| [*s, -*s]).collect();
        let stems = Hpss.separate(&mix, RATE).unwrap();

        let vocals = energy(stems.get(Stem::Vocal));
        let other = energy(stems.get(Stem::Other));
        assert!(
            vocals < other * 0.01,
            "a side-only signal landed in the vocal: {vocals} against {other}"
        );
    }

    /// Silence in, silence out — and still four stems of the right length.
    #[test]
    fn silence_separates_into_silence() {
        let stems = Hpss.separate(&vec![0.0; 32_768], RATE).unwrap();
        for stem in Stem::ALL {
            assert!(stems.get(stem).iter().all(|s| s.abs() < 1e-9), "{stem}");
        }
        assert_eq!(stems.frames(), 16_384);
    }

    /// A fragment shorter than one window has no spectrogram to filter. It must
    /// still come back whole rather than as silence — a DJ loading a one-shot
    /// should not lose it to the separator.
    #[test]
    fn something_shorter_than_a_window_survives_intact() {
        let mix = centred(&sine(440.0, 500, 0.5));
        let stems = Hpss.separate(&mix, RATE).unwrap();
        assert!(stems.error_against(&mix) < 1e-6);
        assert_eq!(stems.frames(), 500);
    }

    #[test]
    fn a_mix_that_is_not_stereo_is_refused() {
        assert!(matches!(
            Hpss.separate(&[0.0; 7], RATE),
            Err(StemError::NotStereo(7))
        ));
        assert_eq!(Hpss.separate(&[0.0; 8], 0), Err(StemError::NoSampleRate));
    }
}

#[cfg(test)]
mod seam_tests {
    use super::*;
    use crate::stems::{SEPARATION_MARGIN, Separator};

    /// Four seconds of something with a sustained tone and a repeating tick, so
    /// the harmonic/percussive split has real work to do.
    fn material(sr: u32, seconds: usize) -> Vec<f32> {
        (0..sr as usize * seconds)
            .flat_map(|n| {
                let t = n as f32 / sr as f32;
                let bass = (2.0 * std::f32::consts::PI * 80.0 * t).sin() * 0.5;
                let hat = if n % 4800 < 60 { 0.4 } else { 0.0 };
                let s = bass + hat;
                [s, s]
            })
            .collect()
    }

    /// The deviation of one passage separated with `margin` frames of context
    /// against the same passage separated as part of the whole track.
    fn deviation_with_margin(margin: usize) -> f32 {
        let sr = 48_000u32;
        let mix = material(sr, 6);
        let frames = mix.len() / 2;
        let sep = Hpss;
        let whole = sep.separate(&mix, sr).unwrap().into_parts();

        let body_start = 96_000usize;
        let body_len = 48_000usize;
        let lead = margin.min(body_start);
        let from = (body_start - lead) * 2;
        let to = ((body_start + body_len + margin).min(frames)) * 2;
        let padded = sep.separate(&mix[from..to], sr).unwrap().into_parts();

        let mut worst = 0.0f32;
        for stem in 0..dj_core::Stem::COUNT {
            for f in 0..body_len {
                let got = padded[stem][(lead + f) * 2];
                let want = whole[stem][(body_start + f) * 2];
                worst = worst.max((got - want).abs());
            }
        }
        worst
    }

    /// **A chunk separated on its own is wrong at its edges**, and this is how
    /// wrong.
    ///
    /// Not a subtlety: the deviation is larger than the signal. Separation is a
    /// windowed transform, and the first and last windows of any buffer have no
    /// neighbours to overlap-add with. Butting independently separated chunks
    /// together therefore put a glitch at every seam — once every ten seconds,
    /// for the whole track, in every stem.
    ///
    /// Kept as a test rather than deleted once fixed, because it is the reason
    /// [`SEPARATION_MARGIN`] exists and the only thing that would notice if
    /// someone decided the margin was wasted work.
    #[test]
    fn a_chunk_separated_with_no_context_is_badly_wrong_at_its_edges() {
        let worst = deviation_with_margin(0);
        assert!(
            worst > 1.0,
            "separating with no context deviated by only {worst}, so the margin \
             may no longer be earning its cost -- re-measure before removing it"
        );
    }

    /// And with the margin it is not. Half a percent of full scale, which is
    /// the median filters' longer-range statistics rather than an edge
    /// artefact: it does not improve with more context.
    #[test]
    fn the_margin_makes_a_chunk_match_the_whole_track() {
        let worst = deviation_with_margin(SEPARATION_MARGIN);
        assert!(
            worst < 0.008,
            "a chunk separated with {SEPARATION_MARGIN} frames of context still \
             deviated by {worst} from the whole-track separation"
        );
    }

    /// The margin is where the curve flattens, and the threshold above is set
    /// **between** the two sides of that turn — 1024 frames measures about
    /// 0.014, the margin about 0.005.
    ///
    /// This test exists because the first version of it did not work. It
    /// compared `SEPARATION_MARGIN` against `SEPARATION_MARGIN / 8`, which is
    /// a claim about the *shape* of the curve and not about the constant: cut
    /// the constant to a quarter and the test still passed, because a quarter
    /// still beats an eighth of a quarter. A mutation caught it. The literal
    /// below is the point of the test — it pins the choice against a fixed
    /// alternative rather than against itself.
    #[test]
    fn a_thousand_frames_of_context_is_not_enough() {
        let short = deviation_with_margin(1024);
        assert!(
            short > 0.008,
            "1024 frames of context deviated by only {short}, so the threshold \
             in the test above no longer separates a good margin from a bad one"
        );
    }
}
