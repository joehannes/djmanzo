//! §116: the rhythm, step by step.
//!
//! > use the bg of the display of a song control and waveform somehow to
//! > visualize rhythm and melody, intensity, amplitude, evolution of the song
//!
//! The beat grid says where the pulse is; it says nothing about what is
//! *playing* on it. A DJ reading a record for a mix wants the drum pattern:
//! where the kick drops out, where the hats double, whether the snare is on
//! two and four or rolling. That is a drum machine's grid — sixteen steps a
//! bar, a row a voice — and this reads one out of the record.
//!
//! # From the banded onset curve
//!
//! [`crate::onset::BandedOnset`] already splits the flux into four bands, and
//! three of them are where the three voices a DJ reads live:
//!
//! - **kick** — below 150 Hz (band 0);
//! - **snare and clap** — 800 Hz to 4 kHz, where a snare cracks (band 2);
//! - **hats and shakers** — above 4 kHz (band 3).
//!
//! Band 1, the body of the bass and the low voice, is not a voice: it moves
//! with the bassline, and a bassline is not a drum. It is what the kick is
//! read *against* — a bass note leaks below 150 Hz as it starts, and a kick
//! row that counted that would put a kick on every bass note.
//!
//! Each step takes the strongest onset within half a step of it, so a hit
//! played a little ahead of the grid or behind it — which is most hits — still
//! lands on its step. Each voice is scaled to its own strong hits, the 95th
//! percentile of its steps, so a quiet hat pattern shows as clearly as a loud
//! kick: the question is *where* each voice plays, not which is loudest.

use crate::onset::{BandedOnset, HOP};

/// Steps in a beat: sixteenths, a drum machine's resolution.
pub const STEPS_PER_BEAT: usize = 4;

/// The three voices, and the onset band each is read from.
pub const VOICES: [(&str, usize); 3] = [("kick", 0), ("snare", 2), ("hat", 3)];

/// The band a bassline's body sounds in, which the kick row is read against.
const BASS_BAND: usize = 1;

/// The percentile of a voice's steps that counts as a full-strength hit.
const STRONG: f64 = 0.95;

/// A record's rhythm on its grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Steps {
    /// The frame of the first step, at or after the start of the record.
    pub first_frame: f64,
    /// Frames between two steps.
    pub frames_per_step: f64,
    /// Per step, the kick, snare and hat, each 0 (nothing) to 255 (a hit as
    /// strong as that voice's strong hits).
    pub strength: Vec<[u8; 3]>,
}

/// Read the steps of a record from its banded onset curve, on the grid whose
/// beat falls at `anchor_frame` every `frames_per_beat`, for a record of
/// `total_frames`.
#[must_use]
pub fn steps(
    onset: &BandedOnset,
    anchor_frame: f64,
    frames_per_beat: f64,
    total_frames: f64,
) -> Steps {
    let frames_per_step = frames_per_beat / STEPS_PER_BEAT as f64;
    if !(frames_per_step.is_finite() && frames_per_step > HOP as f64 / 2.0)
        || onset.values.is_empty()
        || !anchor_frame.is_finite()
    {
        return Steps {
            first_frame: 0.0,
            frames_per_step: frames_per_step.max(0.0),
            strength: Vec::new(),
        };
    }
    // The earliest step at or after the start: the grid runs back from its
    // anchor as well as forward.
    let first_frame = anchor_frame.rem_euclid(frames_per_step);
    let count = ((total_frames - first_frame) / frames_per_step)
        .floor()
        .max(0.0) as usize
        + 1;
    let hop = HOP as f64;
    let last = onset.values.len() - 1;

    let mut raw: Vec<[f32; 3]> = Vec::with_capacity(count);
    for step in 0..count {
        let at = first_frame + step as f64 * frames_per_step;
        let from = (((at - frames_per_step / 2.0) / hop).floor().max(0.0) as usize).min(last);
        let to = (((at + frames_per_step / 2.0) / hop).ceil().max(0.0) as usize).min(last);
        let mut strongest = [0.0_f32; 3];
        for values in &onset.values[from..=to] {
            for (voice, (_, band)) in VOICES.iter().enumerate() {
                let mut rise = values[*band];
                if voice == 0 {
                    // A kick rises most below 150 Hz; a bass note rises more
                    // in the band above, where its body is, and leaks into
                    // the kick's band on the way — on a log scale a large
                    // rise when the last kick has died away. Counting only
                    // what rises beyond the bass band's rise is what keeps a
                    // bassline off the kick row.
                    rise = (rise - values[BASS_BAND]).max(0.0);
                }
                strongest[voice] = strongest[voice].max(rise);
            }
        }
        raw.push(strongest);
    }

    let references: [f32; 3] = std::array::from_fn(|voice| {
        let mut values: Vec<f32> = raw.iter().map(|step| step[voice]).collect();
        values.sort_by(f32::total_cmp);
        values
            .get(((values.len() as f64 - 1.0) * STRONG).round() as usize)
            .copied()
            .unwrap_or(0.0)
    });
    let strength = raw
        .iter()
        .map(|step| {
            std::array::from_fn(|voice| {
                let reference = references[voice];
                if reference <= f32::EPSILON {
                    0
                } else {
                    ((step[voice] / reference).clamp(0.0, 1.0) * 255.0).round() as u8
                }
            })
        })
        .collect();

    Steps {
        first_frame,
        frames_per_step,
        strength,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;

    /// A bar of a drum machine, rendered: a kick on every beat, a snare on two
    /// and four, a hat on every off-beat eighth — each a decaying tone in its
    /// own band, so the test knows exactly what is where — and a bassline on
    /// the last sixteenth of every beat, in the band between the kick and the
    /// snare that no voice is read from. Without it the kick is the only
    /// thing in that band too, and reading the wrong band gives the same
    /// answer.
    fn drums(bpm: f64, bars: usize, offset_frames: usize) -> Vec<f32> {
        let beat = f64::from(RATE) * 60.0 / bpm;
        let total = offset_frames + (beat * 4.0 * bars as f64) as usize;
        let mut mono = vec![0.0_f32; total];
        let mut hit = |start: f64, hz: f64, length: f64, level: f32| {
            let start = offset_frames + start as usize;
            let length = (length * f64::from(RATE)) as usize;
            for i in 0..length {
                let Some(sample) = mono.get_mut(start + i) else {
                    break;
                };
                let t = i as f64 / f64::from(RATE);
                // Two milliseconds in, as an instrument's attack is, rather
                // than a click that sounds in every band at once.
                let attack = (t / 0.002).min(1.0) as f32;
                let envelope = attack * (-(t * 30.0)).exp() as f32;
                *sample += level * envelope * (2.0 * std::f64::consts::PI * hz * t).sin() as f32;
            }
        };
        for beat_index in 0..bars * 4 {
            let at = beat_index as f64 * beat;
            hit(at, 55.0, 0.18, 0.9);
            if beat_index % 2 == 1 {
                hit(at, 1_800.0, 0.12, 0.5);
            }
            hit(at + beat / 2.0, 9_000.0, 0.05, 0.3);
            hit(at + beat * 0.75, 220.0, 0.15, 0.8);
        }
        mono.iter().flat_map(|s| [*s, *s]).collect()
    }

    /// Which steps of the sixteen in a bar a voice is strong on, over the
    /// middle bars (the first and last bars have edges).
    fn strong_steps(steps: &Steps, voice: usize) -> Vec<usize> {
        let mut on: Vec<usize> = steps
            .strength
            .iter()
            .enumerate()
            .skip(16)
            .take(steps.strength.len().saturating_sub(32))
            .filter(|(_, s)| s[voice] >= 128)
            .map(|(i, _)| i % 16)
            .collect();
        on.sort_unstable();
        on.dedup();
        on
    }

    /// **The pattern a drum machine would show**: the kick on every beat, the
    /// snare on two and four, the hat on every off-beat — each voice on its
    /// own steps and nowhere else.
    #[test]
    fn a_drum_pattern_lands_on_its_steps() {
        let bpm = 124.0;
        let audio = drums(bpm, 8, 0);
        let onset = crate::onset::detect_bands(&audio, RATE);
        let beat = f64::from(RATE) * 60.0 / bpm;
        let steps = steps(&onset, 0.0, beat, (audio.len() / 2) as f64);
        assert_eq!(strong_steps(&steps, 0), vec![0, 4, 8, 12], "kick");
        assert_eq!(strong_steps(&steps, 1), vec![4, 12], "snare");
        assert_eq!(strong_steps(&steps, 2), vec![2, 6, 10, 14], "hat");
    }

    /// **A bassline is not a kick.** A bass note leaks below 150 Hz as it
    /// starts, and once the last kick has died away that is a large rise on
    /// a log scale; read naively, every bass note was a kick at half strength.
    /// On the bass's steps the kick row stays below what the lane draws.
    #[test]
    fn a_bassline_is_not_a_kick() {
        let bpm = 124.0;
        let audio = drums(bpm, 8, 0);
        let onset = crate::onset::detect_bands(&audio, RATE);
        let beat = f64::from(RATE) * 60.0 / bpm;
        let steps = steps(&onset, 0.0, beat, (audio.len() / 2) as f64);
        let middle = &steps.strength[16..steps.strength.len() - 16];
        for (i, step) in middle.iter().enumerate() {
            if (i + 16) % 4 == 3 {
                assert!(
                    step[0] < 48,
                    "a bass note read as a kick at step {}: {}",
                    (i + 16) % 16,
                    step[0]
                );
            }
        }
    }

    /// The grid runs back from its anchor: a record whose first beat is half a
    /// second in still has its steps from the start, and the kick is on the
    /// steps the anchor says, not the ones counted from frame zero.
    #[test]
    fn the_steps_follow_the_anchor() {
        let bpm = 120.0;
        let offset = 22_050; // half a second, a beat's worth at 120
        let audio = drums(bpm, 6, offset + 5_512); // an eighth late besides
        let onset = crate::onset::detect_bands(&audio, RATE);
        let beat = f64::from(RATE) * 60.0 / bpm;
        let anchor = (offset + 5_512) as f64;
        let steps = steps(&onset, anchor, beat, (audio.len() / 2) as f64);
        assert!(steps.first_frame < steps.frames_per_step);
        assert!((steps.first_frame - anchor.rem_euclid(beat / 4.0)).abs() < 1e-6);
        // The kick is on the steps that are beats of this grid.
        let on_beats: Vec<usize> = steps
            .strength
            .iter()
            .enumerate()
            .filter(|(_, s)| s[0] >= 128)
            .map(|(i, _)| {
                let frame = steps.first_frame + i as f64 * steps.frames_per_step;
                (((frame - anchor) / steps.frames_per_step).round() as i64).rem_euclid(4) as usize
            })
            .collect();
        assert!(!on_beats.is_empty());
        assert!(on_beats.iter().all(|&s| s == 0), "{on_beats:?}");
    }

    /// Silence has no rhythm, and a nonsense grid has no steps — neither is a
    /// panic.
    #[test]
    fn silence_and_nonsense_are_empty() {
        let silent = vec![0.0_f32; 44_100 * 4];
        let onset = crate::onset::detect_bands(&silent, RATE);
        let quiet = steps(&onset, 0.0, 22_050.0, 88_200.0);
        assert!(quiet.strength.iter().all(|s| *s == [0, 0, 0]));
        assert!(steps(&onset, 0.0, 0.0, 88_200.0).strength.is_empty());
        assert!(
            steps(&onset, f64::NAN, 22_050.0, 88_200.0)
                .strength
                .is_empty()
        );
    }
}
