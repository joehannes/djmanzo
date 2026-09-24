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
//!
//! # The grammar under it
//!
//! [`grammar`] reads which of `dj_core::genre`'s rhythmic grammars the steps
//! make, where they make one plainly: a kick on every beat is four on the
//! floor; that kick with the snare on the fourth and seventh sixteenths is
//! dembow; a snare on every other beat and not between is a backbeat, which
//! is boom-bap at a hip-hop tempo and a breakbeat at drum and bass's. Anything
//! else — clave, a swung log drum, a record without drums — is not read at
//! all, because a wrong answer here files a record in the wrong place.

use crate::onset::{BandedOnset, HOP};
use dj_core::genre::Grammar;

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

/// How much a voice rose in one hop.
fn rise(values: &[f32; crate::onset::BANDS], voice: usize) -> f32 {
    let rise = values[VOICES[voice].1];
    if voice == 0 {
        // A kick rises most below 150 Hz; a bass note rises more in the band
        // above, where its body is, and leaks into the kick's band on the way
        // — on a log scale a large rise when the last kick has died away.
        // Counting only what rises beyond the bass band's rise is what keeps
        // a bassline off the kick row.
        (rise - values[BASS_BAND]).max(0.0)
    } else {
        rise.max(0.0)
    }
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
            for (voice, strongest) in strongest.iter_mut().enumerate() {
                *strongest = strongest.max(rise(values, voice));
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

/// The steps a grammar repeats over: two beats, which is the whole of a
/// dembow and the shortest span a backbeat can be told in.
const CYCLE: usize = 2 * STEPS_PER_BEAT;

/// A step is a hit at half the strength of the voice's strong hits.
const HIT: u8 = 128;

/// A voice *plays* on a place when it hits there in this share of the
/// cycles that groove ...
const PLAYS: f64 = 0.7;

/// ... and *rests* there when it hits in this share or fewer.
const RESTS: f64 = 0.3;

/// Fewer grooving cycles than this — eight bars — is too little to read a
/// grammar from.
const FEWEST_CYCLES: usize = 16;

/// How often each voice hits at each place in a two-beat cycle, over the
/// cycles in which the kick plays at all, and how many of those there were.
///
/// Counting only those is what lets an intro with no drums, or a breakdown,
/// leave the answer alone: the question is what the groove does when there
/// is one, not how much of the record has one.
#[must_use]
pub fn cycle(steps: &Steps, anchor_frame: f64) -> ([[f64; CYCLE]; 3], usize) {
    let mut rates = [[0.0; CYCLE]; 3];
    if steps.strength.is_empty() || steps.frames_per_step <= 0.0 {
        return (rates, 0);
    }
    // Where in its cycle each step falls, counted from the anchor's beat.
    let place = |index: usize| -> usize {
        let frame = steps.first_frame + index as f64 * steps.frames_per_step;
        #[allow(clippy::cast_possible_truncation)]
        let from_anchor = ((frame - anchor_frame) / steps.frames_per_step).round() as i64;
        #[allow(clippy::cast_sign_loss)]
        let place = from_anchor.rem_euclid(CYCLE as i64) as usize;
        place
    };
    let start = (0..steps.strength.len().min(CYCLE))
        .find(|&index| place(index) == 0)
        .unwrap_or(0);
    let mut grooving = 0;
    for chunk in steps.strength[start..].as_chunks::<CYCLE>().0 {
        if !chunk.iter().any(|step| step[0] >= HIT) {
            continue;
        }
        grooving += 1;
        for (at, step) in chunk.iter().enumerate() {
            for (voice, strength) in step.iter().enumerate() {
                if *strength >= HIT {
                    rates[voice][at] += 1.0;
                }
            }
        }
    }
    if grooving > 0 {
        for voice in &mut rates {
            for rate in voice.iter_mut() {
                *rate /= grooving as f64;
            }
        }
    }
    (rates, grooving)
}

/// The rhythmic grammar a record's steps make, when they make one plainly;
/// `None` otherwise. `bpm` is the grid's, which decides whether a backbeat is
/// boom-bap or a breakbeat, and between the two tempos it is neither.
#[must_use]
pub fn grammar(steps: &Steps, anchor_frame: f64, bpm: f64) -> Option<Grammar> {
    let ([kick, snare, _], grooving) = cycle(steps, anchor_frame);
    if grooving < FEWEST_CYCLES {
        return None;
    }
    let beats = [0, STEPS_PER_BEAT];
    if beats.iter().all(|&beat| kick[beat] >= PLAYS) {
        // Dembow's snare: the fourth and seventh sixteenths of its two beats,
        // and not on either beat. Either beat can be the one the anchor is on.
        let dembow = beats.iter().any(|&from| {
            snare[(from + 3) % CYCLE] >= PLAYS
                && snare[(from + 6) % CYCLE] >= PLAYS
                && snare[from] <= RESTS
                && snare[(from + STEPS_PER_BEAT) % CYCLE] <= RESTS
        });
        return Some(if dembow {
            Grammar::Dembow
        } else {
            Grammar::FourOnFloor
        });
    }
    let backbeat = beats
        .iter()
        .any(|&on| snare[on] >= PLAYS && snare[(on + STEPS_PER_BEAT) % CYCLE] <= RESTS);
    match bpm {
        _ if !backbeat => None,
        bpm if (60.0..110.0).contains(&bpm) => Some(Grammar::Boombap),
        bpm if (160.0..=185.0).contains(&bpm) => Some(Grammar::Breakbeat),
        _ => None,
    }
}

/// How far either side of the tempo finder's answer [`lock`] looks. The
/// finder is held to two percent; the answers it gives on a clean record are
/// within a fraction of one.
const LOCK_SPAN: f64 = 0.015;

/// How finely [`lock`] steps through that span, as a fraction of the tempo:
/// over three hundred beats a step this fine drifts a sixth of a sixteenth.
const LOCK_STEP: f64 = 0.000_2;

/// The tempos [`lock`] tries, as ratios of the finder's. A snare on a
/// tresillo — three sixteenths, three, two, the spine of dembow — repeats
/// every three sixteenths, and a finder that hears it loudest answers four
/// thirds of the tempo; the other way round is the same mistake in reverse.
/// Octaves are left to the finder, whose prior is there to choose them.
const LOCK_RATIOS: [f64; 3] = [1.0, 0.75, 4.0 / 3.0];

/// Places in a beat [`lock`] folds the onsets into.
const LOCK_BINS: usize = 48;

/// A grid laid on the drums themselves: the tempo, and the frame of a beat
/// in the onset curve's own frames — which is what [`steps`] reads, and
/// half an analysis window after the audio.
///
/// The tempo finder's grid is laid for a DJ to see and to sync on, from every
/// onset the record has, and it is honest to within its tolerance — which is
/// a problem for reading steps, not for mixing: two tenths of a percent is
/// nothing over a few bars and two sixteenths over sixty-four, and a grid
/// that has drifted two sixteenths reads the kick where the hat is. So this
/// takes the finder's tempo only as a start, folds the kick's and snare's
/// onsets modulo a beat at every tempo near it, and keeps the tempo and phase
/// at which they pile up tightest. A drifting tempo smears the pile; the true
/// one stacks every hit of the record in one place. It tries the tempos a
/// tresillo misleads a finder into too (`LOCK_RATIOS`).
///
/// Where the drums sit is where the beats are. On a record whose kick is on
/// every beat, that is the kick; on a backbeat, the snare on two and four
/// outweighs the kick's offbeat on the and of three.
#[must_use]
pub fn lock(onset: &BandedOnset, rough_bpm: f64) -> Option<(f64, f64)> {
    let hop = HOP as f64;
    if onset.values.is_empty() || !(rough_bpm.is_finite() && rough_bpm > 0.0) {
        return None;
    }
    // The kick's and the snare's rise, per hop, each scaled to its own strong
    // hits — a snare mixed loud must not outweigh the kick it plays against.
    let strong = |voice: usize| {
        let mut rises: Vec<f32> = onset.values.iter().map(|v| rise(v, voice)).collect();
        rises.sort_by(f32::total_cmp);
        let at = ((rises.len() as f64 - 1.0) * 0.999).round() as usize;
        f64::from(rises.get(at).copied().unwrap_or(0.0)).max(f64::EPSILON)
    };
    let (kick, snare) = (strong(0), strong(1));
    let rise: Vec<f64> = onset
        .values
        .iter()
        .map(|v| f64::from(rise(v, 0)) / kick + f64::from(rise(v, 1)) / snare)
        .collect();
    let total: f64 = rise.iter().sum();
    if total <= f64::EPSILON {
        return None;
    }
    let candidates = (2.0 * LOCK_SPAN / LOCK_STEP).round() as i64;
    let tempos = LOCK_RATIOS.iter().flat_map(|ratio| {
        (0..=candidates)
            .map(move |step| rough_bpm * ratio * (1.0 - LOCK_SPAN + step as f64 * LOCK_STEP))
    });
    let mut best: Option<(f64, f64, f64)> = None;
    for bpm in tempos {
        let beat_hops = onset.rate * 60.0 / bpm;
        let mut bins = [0.0_f64; LOCK_BINS];
        for (i, value) in rise.iter().enumerate() {
            let phase = (i as f64 / beat_hops).fract();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let bin = ((phase * LOCK_BINS as f64) as usize).min(LOCK_BINS - 1);
            bins[bin] += value;
        }
        // A hit spreads over neighbouring bins; three together are its pile.
        let (at, pile) = (0..LOCK_BINS)
            .map(|b| {
                let pile =
                    bins[(b + LOCK_BINS - 1) % LOCK_BINS] + bins[b] + bins[(b + 1) % LOCK_BINS];
                (b, pile)
            })
            .fold((0, f64::NEG_INFINITY), |a, b| if b.1 > a.1 { b } else { a });
        if best.is_none_or(|(_, _, top)| pile > top) {
            let anchor_hops = (at as f64 + 0.5) / LOCK_BINS as f64 * beat_hops;
            best = Some((bpm, anchor_hops * hop, pile));
        }
    }
    best.map(|(bpm, anchor, _)| (bpm, anchor))
}

/// A second-order filter section, as the audio cookbook writes one.
#[derive(Debug, Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    /// A Butterworth low-pass (`high` false) or high-pass at `hz`.
    fn new(hz: f64, rate: f64, high: bool) -> Self {
        let w = 2.0 * std::f64::consts::PI * hz / rate;
        let alpha = w.sin() / std::f64::consts::SQRT_2;
        let cos = w.cos();
        let a0 = 1.0 + alpha;
        let b = if high {
            [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0]
        } else {
            [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0]
        };
        Self {
            b: b.map(|v| v / a0),
            a: [-2.0 * cos / a0, (1.0 - alpha) / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn run(&mut self, input: f64) -> f64 {
        let out = self.b[0] * input + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [input, self.x[0]];
        self.y = [out, self.y[0]];
        out
    }
}

/// Where each drum voice sits, as the filters that isolate it: the kick
/// below 100 Hz, where a bassline's notes are two octaves down by the time
/// they reach it; the snare between 1 and 4 kHz; the hats above 7 kHz. Each
/// is two sections, a 24 dB-per-octave slope.
const DRUM_FILTERS: [&[(f64, bool)]; 3] = [
    &[(100.0, false), (100.0, false)],
    &[
        (1_000.0, true),
        (1_000.0, true),
        (4_000.0, false),
        (4_000.0, false),
    ],
    &[(7_000.0, true), (7_000.0, true)],
];

/// A rise quieter than this, -60 dB below full scale, is not a hit. Each
/// voice is scaled to its own strong hits, so without a floor what leaks
/// through a filter from the other voices — a thousandth of them — would be
/// scaled up into a kick on every hat.
const QUIET: f32 = 0.001;

/// The drums' own curve: per hop, how far each voice's level rose over the
/// last two hops, in the same shape as the onset curve so [`steps`] and
/// [`lock`] read it as they read that one.
///
/// Why not the onset curve itself: it is a log spectral flux, which is
/// exactly right for *when* something starts and wrong for *how hard*. A log
/// rise measures against whatever was there before, so the same kick reads as
/// a large rise after silence and a small one after a bass note's tail, and
/// identical kicks rendered on every beat read from nothing to full strength.
/// A level through a filter is linear: the same hit is the same height.
#[must_use]
pub fn drum_curve(samples: &[f32], sample_rate: u32) -> BandedOnset {
    let rate = f64::from(sample_rate);
    let mut filters: Vec<Vec<Biquad>> = DRUM_FILTERS
        .iter()
        .map(|sections| {
            sections
                .iter()
                .map(|&(hz, high)| Biquad::new(hz, rate, high))
                .collect()
        })
        .collect();
    let mut levels: Vec<[f32; 3]> = Vec::with_capacity(samples.len() / 2 / HOP + 1);
    for hop in samples.chunks(2 * HOP) {
        let mut peak = [0.0_f64; 3];
        for frame in hop.as_chunks::<2>().0 {
            let mono = f64::from(frame[0] + frame[1]) / 2.0;
            for (voice, sections) in filters.iter_mut().enumerate() {
                let out = sections.iter_mut().fold(mono, |x, section| section.run(x));
                peak[voice] = peak[voice].max(out.abs());
            }
        }
        #[allow(clippy::cast_possible_truncation)]
        levels.push(peak.map(|p| p as f32));
    }
    let values = (0..levels.len())
        .map(|i| {
            let before = levels[i.saturating_sub(2)];
            let rise = |voice: usize| {
                let rise = levels[i][voice] - before[voice];
                if rise > QUIET { rise } else { 0.0 }
            };
            // Laid out as the onset curve's bands: nothing in the bass band,
            // so the kick is read as it is.
            [rise(0), 0.0, rise(1), rise(2)]
        })
        .collect();
    BandedOnset {
        values,
        rate: rate / HOP as f64,
    }
}

/// The grammar of a whole record, and the written tempo it was read at: its
/// tempo found, a grid [`lock`]ed on its drums, its steps read from
/// [`drum_curve`]. Interleaved stereo.
///
/// Worker-thread work, like [`crate::analyse`]: it reads the whole record.
#[must_use]
pub fn heard(samples: &[f32], sample_rate: dj_core::SampleRate) -> Option<(Grammar, f64)> {
    let envelope = crate::onset::detect(samples, sample_rate.get());
    let rough = crate::tempo::analyse(&envelope, sample_rate)?;
    let drums = drum_curve(samples, sample_rate.get());
    let (bpm, anchor) = lock(&drums, rough.grid.bpm.get())?;
    let frames = (samples.len() / 2) as f64;
    let frames_per_beat = f64::from(sample_rate.get()) * 60.0 / bpm;
    let steps = steps(&drums, anchor, frames_per_beat, frames);
    grammar(&steps, anchor, bpm).map(|grammar| (grammar, bpm))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;

    /// One voice of a drum machine: its pitch, how long it rings, how loud,
    /// and the sixteenths of a bar it plays on.
    struct Voice {
        hz: f64,
        seconds: f64,
        level: f32,
        on: &'static [usize],
    }

    const KICK: f64 = 55.0;
    const SNARE: f64 = 1_800.0;
    const HAT: f64 = 9_000.0;
    const BASS: f64 = 220.0;

    /// Bars of a drum machine, rendered: each voice a decaying tone in its
    /// own band, so a test knows exactly what is where.
    fn render(bpm: f64, bars: usize, offset_frames: usize, voices: &[Voice]) -> Vec<f32> {
        let step = f64::from(RATE) * 60.0 / bpm / 4.0;
        let total = offset_frames + (step * 16.0 * bars as f64) as usize;
        let mut mono = vec![0.0_f32; total];
        for bar in 0..bars {
            for voice in voices {
                for &at in voice.on {
                    let start = offset_frames + ((bar * 16 + at) as f64 * step) as usize;
                    let length = (voice.seconds * f64::from(RATE)) as usize;
                    for i in 0..length {
                        let Some(sample) = mono.get_mut(start + i) else {
                            break;
                        };
                        let t = i as f64 / f64::from(RATE);
                        // Two milliseconds in, as an instrument's attack is,
                        // rather than a click that sounds in every band.
                        let attack = (t / 0.002).min(1.0) as f32;
                        let envelope = attack * (-(t * 30.0)).exp() as f32;
                        *sample += voice.level
                            * envelope
                            * (2.0 * std::f64::consts::PI * voice.hz * t).sin() as f32;
                    }
                }
            }
        }
        mono.iter().flat_map(|s| [*s, *s]).collect()
    }

    /// A kick on every beat, a snare on two and four, a hat on every
    /// off-beat eighth, and a bassline on the last sixteenth of every beat, in
    /// the band between the kick and the snare that no voice is read from.
    /// Without it the kick is the only thing in that band too, and reading
    /// the wrong band gives the same answer.
    fn drums(bpm: f64, bars: usize, offset_frames: usize) -> Vec<f32> {
        render(bpm, bars, offset_frames, &HOUSE)
    }

    const HOUSE: [Voice; 4] = [
        Voice {
            hz: KICK,
            seconds: 0.18,
            level: 0.9,
            on: &[0, 4, 8, 12],
        },
        Voice {
            hz: SNARE,
            seconds: 0.12,
            level: 0.5,
            on: &[4, 12],
        },
        Voice {
            hz: HAT,
            seconds: 0.05,
            level: 0.3,
            on: &[2, 6, 10, 14],
        },
        Voice {
            hz: BASS,
            seconds: 0.15,
            level: 0.8,
            on: &[3, 7, 11, 15],
        },
    ];

    /// Reggaeton's: the kick on every beat, the snare on the fourth and
    /// seventh sixteenths of every two beats, and not on a beat.
    const DEMBOW: [Voice; 3] = [
        Voice {
            hz: KICK,
            seconds: 0.18,
            level: 0.9,
            on: &[0, 4, 8, 12],
        },
        Voice {
            hz: SNARE,
            seconds: 0.1,
            level: 0.5,
            on: &[3, 6, 11, 14],
        },
        Voice {
            hz: HAT,
            seconds: 0.05,
            level: 0.3,
            on: &[0, 2, 4, 6, 8, 10, 12, 14],
        },
    ];

    /// A backbeat: the kick on one and the and of three, the snare on two and
    /// four, as hip-hop and drum and bass both have it.
    const BACKBEAT: [Voice; 3] = [
        Voice {
            hz: KICK,
            seconds: 0.18,
            level: 0.9,
            on: &[0, 10],
        },
        Voice {
            hz: SNARE,
            seconds: 0.12,
            level: 0.5,
            on: &[4, 12],
        },
        Voice {
            hz: HAT,
            seconds: 0.04,
            level: 0.3,
            on: &[0, 2, 4, 6, 8, 10, 12, 14],
        },
    ];

    /// The grammar of a rendered pattern, read from its drum curve on the
    /// grid it was rendered on.
    fn grammar_of(bpm: f64, bars: usize, voices: &[Voice]) -> Option<Grammar> {
        let audio = render(bpm, bars, 0, voices);
        let onset = drum_curve(&audio, RATE);
        let beat = f64::from(RATE) * 60.0 / bpm;
        let steps = steps(&onset, 0.0, beat, (audio.len() / 2) as f64);
        grammar(&steps, 0.0, bpm)
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
    ///
    /// The same holds for the drum curve a grammar is read from, where it is
    /// the kick's filter that keeps the bass out.
    #[test]
    fn a_bassline_is_not_a_kick() {
        let bpm = 124.0;
        let audio = drums(bpm, 8, 0);
        let beat = f64::from(RATE) * 60.0 / bpm;
        for (curve, onset) in [
            ("onset", crate::onset::detect_bands(&audio, RATE)),
            ("drum", drum_curve(&audio, RATE)),
        ] {
            let steps = steps(&onset, 0.0, beat, (audio.len() / 2) as f64);
            let middle = &steps.strength[16..steps.strength.len() - 16];
            for (i, step) in middle.iter().enumerate() {
                if (i + 16) % 4 == 3 {
                    assert!(
                        step[0] < 48,
                        "{curve}: a bass note read as a kick at step {}: {}",
                        (i + 16) % 16,
                        step[0]
                    );
                }
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

    /// **Each grammar is read from the pattern that makes it**, and the tempo
    /// decides a backbeat: boom-bap at a hip-hop tempo, a breakbeat at drum
    /// and bass's, and nothing between, where trap, dubstep and garage all
    /// sit and a backbeat alone cannot tell them apart.
    #[test]
    fn the_grammar_is_read_from_the_pattern() {
        assert_eq!(grammar_of(124.0, 16, &HOUSE), Some(Grammar::FourOnFloor));
        assert_eq!(grammar_of(95.0, 16, &DEMBOW), Some(Grammar::Dembow));
        assert_eq!(grammar_of(90.0, 16, &BACKBEAT), Some(Grammar::Boombap));
        assert_eq!(grammar_of(174.0, 24, &BACKBEAT), Some(Grammar::Breakbeat));
        assert_eq!(grammar_of(140.0, 16, &BACKBEAT), None);
    }

    /// **What is not plain is not read.** Hats alone have no kick to groove
    /// on; a kick on every beat for four bars is too little to go on; and a
    /// backbeat read on a grid at double its tempo — the snare now on every
    /// fourth beat — is not a breakbeat, which is what a hip-hop record the
    /// tempo finder doubled would otherwise be filed as.
    #[test]
    fn what_is_not_plain_is_not_read() {
        let hats = [Voice {
            hz: HAT,
            seconds: 0.05,
            level: 0.3,
            on: &[0, 2, 4, 6, 8, 10, 12, 14],
        }];
        assert_eq!(grammar_of(124.0, 16, &hats), None);
        assert_eq!(grammar_of(124.0, 4, &HOUSE), None);

        // A snare on every beat is not a backbeat, whatever the kick does.
        let every_beat = [
            Voice {
                hz: KICK,
                seconds: 0.18,
                level: 0.9,
                on: &[0, 10],
            },
            Voice {
                hz: SNARE,
                seconds: 0.12,
                level: 0.5,
                on: &[0, 4, 8, 12],
            },
        ];
        assert_eq!(grammar_of(90.0, 16, &every_beat), None);

        let audio = render(88.0, 16, 0, &BACKBEAT);
        let onset = drum_curve(&audio, RATE);
        let doubled = f64::from(RATE) * 60.0 / 176.0;
        let steps = steps(&onset, 0.0, doubled, (audio.len() / 2) as f64);
        assert_eq!(grammar(&steps, 0.0, 176.0), None);
    }

    /// An intro with no drums and a breakdown leave the grammar alone: it is
    /// what the groove does when there is one.
    #[test]
    fn an_intro_and_a_breakdown_leave_the_grammar_alone() {
        let bpm = 124.0;
        let hats = [Voice {
            hz: HAT,
            seconds: 0.05,
            level: 0.3,
            on: &[2, 6, 10, 14],
        }];
        let mut audio = render(bpm, 8, 0, &hats);
        audio.extend(render(bpm, 8, 0, &HOUSE));
        audio.extend(render(bpm, 8, 0, &hats));
        audio.extend(render(bpm, 8, 0, &HOUSE));
        let onset = drum_curve(&audio, RATE);
        let beat = f64::from(RATE) * 60.0 / bpm;
        let steps = steps(&onset, 0.0, beat, (audio.len() / 2) as f64);
        assert_eq!(grammar(&steps, 0.0, bpm), Some(Grammar::FourOnFloor));
    }

    /// **A grid locked on the drums does not drift.** Over two minutes the
    /// tempo finder's answer, honest to its tolerance, drifts by sixteenths;
    /// the locked tempo is within a hundredth of a percent, and its beat is
    /// on the kick, wherever the record's first kick falls.
    #[test]
    fn a_locked_grid_sits_on_the_kick_and_stays_there() {
        let rate = dj_core::SampleRate::new(RATE).unwrap();
        for (bpm, offset) in [(124.0, 3_000), (128.0, 17_000), (95.0, 9_000)] {
            let audio = render(bpm, 64, offset, &HOUSE);
            let envelope = crate::onset::detect(&audio, RATE);
            let rough = crate::tempo::analyse(&envelope, rate)
                .unwrap()
                .grid
                .bpm
                .get();
            let (locked, anchor) = lock(&drum_curve(&audio, RATE), rough).unwrap();
            assert!(
                (locked - bpm).abs() / bpm < 0.000_1,
                "{bpm}: locked at {locked}, the finder said {rough}"
            );
            let kick = offset as f64;
            let beat = f64::from(RATE) * 60.0 / bpm;
            let off = (anchor - kick).rem_euclid(beat);
            let off = off.min(beat - off);
            assert!(
                off < beat / 32.0,
                "{bpm}: the beat is {off} frames off the kick"
            );
        }
    }

    /// **A whole record is heard through its own tempo**, not on a grid the
    /// test hands it — two minutes of each grammar, long enough that reading
    /// it on the tempo finder's grid alone would drift off the pattern.
    #[test]
    fn a_record_is_heard_through_its_own_tempo() {
        let rate = dj_core::SampleRate::new(RATE).unwrap();
        let heard_as =
            |bpm, offset, voices: &[Voice]| heard(&render(bpm, 64, offset, voices), rate);
        let (grammar, bpm) = heard_as(124.0, 3_000, &HOUSE).unwrap();
        assert_eq!(grammar, Grammar::FourOnFloor);
        assert!((bpm - 124.0).abs() < 0.05, "{bpm}");
        assert_eq!(
            heard_as(96.0, 1_000, &DEMBOW).map(|h| h.0),
            Some(Grammar::Dembow)
        );
        // A snare mixed four times louder than the kick still leaves the beat
        // on the kick: each voice weighs the same in the lock. The tempo
        // finder hears this one at four thirds of its tempo, from the snare's
        // tresillo, and the lock brings it back.
        let loud_snare = [
            Voice {
                hz: KICK,
                seconds: 0.18,
                level: 0.25,
                on: &[0, 4, 8, 12],
            },
            Voice {
                hz: SNARE,
                seconds: 0.1,
                level: 1.0,
                on: &[3, 6, 11, 14],
            },
        ];
        assert_eq!(
            heard_as(96.0, 7_000, &loud_snare).map(|h| h.0),
            Some(Grammar::Dembow)
        );
        assert_eq!(
            heard_as(88.0, 5_000, &BACKBEAT).map(|h| h.0),
            Some(Grammar::Boombap)
        );
        assert_eq!(heard(&vec![0.0; 44_100 * 20], rate), None);
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
