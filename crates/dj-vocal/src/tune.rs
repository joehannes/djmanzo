//! K9: pitch correction — a voice pulled toward the nearest note of the
//! song's key (`docs/KARAOKE.md` §12).
//!
//! Two halves, both on the audio thread and neither allocating after
//! [`Corrector::new`]:
//!
//! - **Hearing the note.** YIN on the corrector's own copy of the voice,
//!   box-averaged down to about 8 kHz, in a 40 ms window read every 5 ms. A
//!   reading YIN is not sure of — its dip above [`UNSURE`] — or a window too
//!   quiet to be a voice is no reading, and the pull lets go.
//! - **Moving it.** A delay line read by two taps half a window apart, each
//!   moving at the shift's rate. A tap is heard alone except near the point
//!   where it wraps, where the other takes over with an equal-power
//!   crossfade: two taps mixed *all* the time would comb-filter a voice that
//!   needs no shift at all. With nothing to correct, the taps drift — a few
//!   cents, inaudibly — back to where one is heard alone, so a corrector
//!   turned on and resting is a clean delay of half the window.

use serde::{Deserialize, Serialize};

/// The shifter's window, in seconds: twice the latency it adds at rest.
pub const WINDOW_SECONDS: f32 = 0.020;

/// K9: what the stage adds to a singer's voice at rest, in seconds — what
/// the strip's row says beside it, since a wedge hears it.
pub const LATENCY_SECONDS: f32 = WINDOW_SECONDS / 2.0;

/// The rate the note is heard at, in hertz — the listening tap's own.
const LISTEN_RATE: f32 = 8_000.0;
/// The window the note is heard in, in samples at [`LISTEN_RATE`]: 40 ms,
/// two periods of 80 Hz.
const LISTEN: usize = 320;
/// The longest period looked for, in samples at [`LISTEN_RATE`]: 50 Hz.
const LONGEST: usize = LISTEN / 2;
/// How often the note is read, in seconds.
const READ_EVERY: f32 = 0.005;
/// YIN's dip above this is a reading it is not sure of.
pub const UNSURE: f32 = 0.2;
/// Quieter than this, in RMS, a window is not a voice.
const QUIET: f32 = 0.003;
/// The share of the window at each end of a tap's sweep where it hands
/// over to the other.
const FADE: f32 = 0.15;
/// How fast the taps drift back to rest, as a rate of shift: about five
/// cents, which nobody hears.
const REST_PULL: f32 = 0.003;

/// How hard a voice is pulled, and how fast.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CorrectionSettings {
    /// 0 leaves the voice alone; 1 lands it on the note.
    pub strength: f32,
    /// How long the pull takes, in milliseconds: 10 is the robotic sound
    /// records made famous, 200 a nudge.
    pub speed_ms: f32,
}

impl Default for CorrectionSettings {
    fn default() -> Self {
        Self {
            strength: 0.8,
            speed_ms: 40.0,
        }
    }
}

impl CorrectionSettings {
    /// Held to what the screen offers: strength 0–1, speed 10–200 ms; a
    /// value that is not a number at all is what the stage starts at, as
    /// every other stage's is.
    #[must_use]
    pub fn held(self) -> Self {
        let start = Self::default();
        Self {
            strength: if self.strength.is_finite() {
                self.strength.clamp(0.0, 1.0)
            } else {
                start.strength
            },
            speed_ms: if self.speed_ms.is_finite() {
                self.speed_ms.clamp(10.0, 200.0)
            } else {
                start.speed_ms
            },
        }
    }
}

/// The notes of a key, as the corrector pulls to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale {
    /// Which of the twelve pitch classes, C first, are in it.
    notes: [bool; 12],
}

impl Scale {
    const MAJOR: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
    const MINOR: [u8; 7] = [0, 2, 3, 5, 7, 8, 10];

    /// The major scale on `tonic`, a pitch class with C as 0.
    #[must_use]
    pub fn major(tonic: u8) -> Self {
        Self::on(tonic, Self::MAJOR)
    }

    /// The natural minor scale on `tonic`.
    #[must_use]
    pub fn minor(tonic: u8) -> Self {
        Self::on(tonic, Self::MINOR)
    }

    fn on(tonic: u8, steps: [u8; 7]) -> Self {
        let mut notes = [false; 12];
        for step in steps {
            notes[usize::from((tonic + step) % 12)] = true;
        }
        Self { notes }
    }

    /// The note of the scale nearest `midi` (a MIDI note number, fractional),
    /// as a whole MIDI note.
    #[must_use]
    pub fn nearest(&self, midi: f32) -> f32 {
        let below = midi.floor();
        // A scale has a note at least every two semitones, so the nearest is
        // within two of the note below.
        let mut best = below;
        let mut distance = f32::INFINITY;
        for offset in -2_i32..=3 {
            #[allow(clippy::cast_precision_loss)]
            let note = below + offset as f32;
            #[allow(clippy::cast_possible_truncation)]
            let class = (note as i32).rem_euclid(12);
            #[allow(clippy::cast_sign_loss)]
            if self.notes[class as usize] && (note - midi).abs() < distance {
                distance = (note - midi).abs();
                best = note;
            }
        }
        best
    }
}

/// A voice's pitch correction.
#[derive(Debug)]
pub struct Corrector {
    // Hearing.
    step: usize,
    sum: f32,
    count: usize,
    heard: [f32; LISTEN],
    next: usize,
    filled: usize,
    difference: [f32; LONGEST + 1],
    listen_rate: f32,
    since: usize,
    every: usize,
    last_heard: Option<f32>,
    // Moving.
    delay: Vec<f32>,
    write: usize,
    window: f32,
    phase: f32,
    ratio: f32,
    target: f32,
    ease: f32,
    // What to pull to.
    rate: f32,
    scale: Option<Scale>,
    settings: CorrectionSettings,
}

impl Corrector {
    /// A corrector for a voice at `rate`. Its buffers are made here, never
    /// again.
    #[must_use]
    pub fn new(rate: f32) -> Self {
        let rate = rate.max(8_000.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let step = (rate / LISTEN_RATE).round().max(1.0) as usize;
        #[allow(clippy::cast_precision_loss)]
        let listen_rate = rate / step as f32;
        let window = rate * WINDOW_SECONDS;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let length = window.ceil() as usize + 4;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let every = (rate * READ_EVERY).round().max(1.0) as usize;
        let mut corrector = Self {
            step,
            sum: 0.0,
            count: 0,
            heard: [0.0; LISTEN],
            next: 0,
            filled: 0,
            difference: [0.0; LONGEST + 1],
            listen_rate,
            since: 0,
            every,
            last_heard: None,
            delay: vec![0.0; length],
            write: 0,
            window,
            phase: 0.5,
            ratio: 1.0,
            target: 1.0,
            ease: 0.0,
            rate,
            scale: None,
            settings: CorrectionSettings::default(),
        };
        corrector.set(CorrectionSettings::default());
        corrector
    }

    /// How hard and how fast.
    pub fn set(&mut self, settings: CorrectionSettings) {
        self.settings = settings.held();
        let seconds = self.settings.speed_ms / 1_000.0;
        self.ease = 1.0 - (-1.0 / (seconds * self.rate)).exp();
    }

    /// The key to pull to; `None` pulls to nothing.
    pub fn set_scale(&mut self, scale: Option<Scale>) {
        self.scale = scale;
    }

    /// The frames a voice is late by, at rest: half the window.
    #[must_use]
    pub fn latency_frames(&self) -> f32 {
        self.window * 0.5
    }

    /// The pitch last heard, in hertz, if the last reading was sure.
    #[must_use]
    pub fn heard(&self) -> Option<f32> {
        self.last_heard
    }

    /// How far the voice is being moved now, as a ratio of frequencies.
    #[must_use]
    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    /// One sample of voice in, one corrected out.
    pub fn process(&mut self, input: f32) -> f32 {
        self.listen(input);
        self.since += 1;
        if self.since >= self.every {
            self.since = 0;
            self.last_heard = self.read();
            self.target = self.pull();
        }
        self.ratio += (self.target - self.ratio) * self.ease;

        // At rest, the taps drift back to where one is heard alone.
        let mut moving = self.ratio;
        if (self.target - 1.0).abs() < 1e-6 && (self.ratio - 1.0).abs() < 1e-4 {
            let away = 0.5 - self.phase;
            if away.abs() > REST_PULL / self.window {
                moving = 1.0 - REST_PULL * away.signum();
            }
        }

        let length = self.delay.len();
        self.delay[self.write] = input;
        self.phase -= (moving - 1.0) / self.window;
        self.phase -= self.phase.floor();
        let other = (self.phase + 0.5).fract();
        let a = self.tap(self.phase * self.window);
        let b = self.tap(other * self.window);
        let reach = (self.phase.min(1.0 - self.phase) / FADE).clamp(0.0, 1.0);
        let (gain_a, gain_b) = (
            (reach * std::f32::consts::FRAC_PI_2).sin(),
            (reach * std::f32::consts::FRAC_PI_2).cos(),
        );
        self.write = (self.write + 1) % length;
        a * gain_a + b * gain_b
    }

    /// The delay line `delay` frames back from the sample just written,
    /// between samples by a straight line.
    fn tap(&self, delay: f32) -> f32 {
        let length = self.delay.len();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let whole = delay.floor() as usize;
        let part = delay - delay.floor();
        let at = |back: usize| self.delay[(self.write + length * 2 - back) % length];
        at(whole) * (1.0 - part) + at(whole + 1) * part
    }

    fn listen(&mut self, input: f32) {
        self.sum += input;
        self.count += 1;
        if self.count >= self.step {
            #[allow(clippy::cast_precision_loss)]
            let average = self.sum / self.count as f32;
            self.heard[self.next] = average;
            self.next = (self.next + 1) % LISTEN;
            self.filled = (self.filled + 1).min(LISTEN);
            self.sum = 0.0;
            self.count = 0;
        }
    }

    /// YIN over the window, oldest first: the pitch, or `None` when it is
    /// not sure or there is no voice.
    fn read(&mut self) -> Option<f32> {
        if self.filled < LISTEN {
            return None;
        }
        let at = |j: usize, heard: &[f32; LISTEN], next: usize| heard[(next + j) % LISTEN];
        let mut energy = 0.0;
        for j in 0..LISTEN {
            let s = at(j, &self.heard, self.next);
            energy += s * s;
        }
        #[allow(clippy::cast_precision_loss)]
        if (energy / LISTEN as f32).sqrt() < QUIET {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let shortest = (self.listen_rate / 1_000.0).floor().max(2.0) as usize;
        self.difference[0] = 1.0;
        let mut running = 0.0;
        for tau in 1..=LONGEST {
            let mut d = 0.0;
            for j in 0..LONGEST {
                let x = at(j, &self.heard, self.next) - at(j + tau, &self.heard, self.next);
                d += x * x;
            }
            running += d;
            #[allow(clippy::cast_precision_loss)]
            let normal = if running > 0.0 {
                d * tau as f32 / running
            } else {
                1.0
            };
            self.difference[tau] = normal;
        }
        let mut tau = shortest;
        while tau < LONGEST {
            if self.difference[tau] < UNSURE {
                while tau + 1 < LONGEST && self.difference[tau + 1] < self.difference[tau] {
                    tau += 1;
                }
                let (before, here, after) = (
                    self.difference[tau - 1],
                    self.difference[tau],
                    self.difference[tau + 1],
                );
                let bend = before - 2.0 * here + after;
                let shift = if bend.abs() > f32::EPSILON {
                    0.5 * (before - after) / bend
                } else {
                    0.0
                };
                #[allow(clippy::cast_precision_loss)]
                let period = tau as f32 + shift.clamp(-1.0, 1.0);
                return Some(self.listen_rate / period);
            }
            tau += 1;
        }
        None
    }

    /// The ratio the voice is pulled to: toward the nearest note of the
    /// scale by the strength, or none.
    fn pull(&self) -> f32 {
        let (Some(scale), Some(heard)) = (self.scale, self.last_heard) else {
            return 1.0;
        };
        let midi = 69.0 + 12.0 * (heard / 440.0).log2();
        let cents = (scale.nearest(midi) - midi) * 100.0 * self.settings.strength;
        2.0_f32.powf(cents / 1_200.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;
    const A: u8 = 9;

    /// A voice gliding from `from` to `to` hertz over `seconds`.
    fn glide(from: f32, to: f32, seconds: f32) -> Vec<f32> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = (seconds * RATE) as usize;
        let mut phase = 0.0_f32;
        (0..n)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let f = from + (to - from) * i as f32 / n as f32;
                phase = (phase + f / RATE).fract();
                0.3 * (std::f32::consts::TAU * phase).sin()
            })
            .collect()
    }

    fn corrected(corrector: &mut Corrector, voice: &[f32]) -> Vec<f32> {
        voice.iter().map(|&s| corrector.process(s)).collect()
    }

    /// The frequency of a voice over a stretch, from its rising zero
    /// crossings, placed between samples.
    fn frequency(voice: &[f32]) -> f32 {
        let mut crossings = Vec::new();
        for (i, pair) in voice.windows(2).enumerate() {
            if pair[0] < 0.0 && pair[1] >= 0.0 {
                #[allow(clippy::cast_precision_loss)]
                crossings.push(i as f32 + pair[0] / (pair[0] - pair[1]));
            }
        }
        let (first, last) = (crossings[0], crossings[crossings.len() - 1]);
        #[allow(clippy::cast_precision_loss)]
        let cycles = (crossings.len() - 1) as f32;
        RATE * cycles / (last - first)
    }

    fn cents(f: f32, from: f32) -> f32 {
        1_200.0 * (f / from).log2()
    }

    fn corrector(strength: f32, scale: Option<Scale>) -> Corrector {
        let mut corrector = Corrector::new(RATE);
        corrector.set(CorrectionSettings {
            strength,
            speed_ms: 20.0,
        });
        corrector.set_scale(scale);
        corrector
    }

    /// **A held note between two of the key's lands on the nearer, at full
    /// strength** — 225 Hz in A minor comes out at A3, 220 Hz — and is left
    /// where it is at none, or with no key to pull to.
    #[test]
    fn a_held_note_is_pulled_to_the_key() {
        let voice = glide(225.0, 225.0, 1.5);
        let tail = |out: &[f32]| frequency(&out[out.len() / 2..]);

        let out = corrected(&mut corrector(1.0, Some(Scale::minor(A))), &voice);
        let heard = tail(&out);
        assert!(cents(heard, 220.0).abs() < 10.0, "came out at {heard} Hz");

        let out = corrected(&mut corrector(0.0, Some(Scale::minor(A))), &voice);
        let heard = tail(&out);
        assert!(
            cents(heard, 225.0).abs() < 3.0,
            "moved at no strength: {heard} Hz"
        );

        let out = corrected(&mut corrector(1.0, None), &voice);
        let heard = tail(&out);
        assert!(
            cents(heard, 225.0).abs() < 3.0,
            "moved with no key: {heard} Hz"
        );
    }

    /// **Half strength goes half way**: 225 Hz is 39 cents above A, and comes
    /// out about 20 above it.
    #[test]
    fn half_strength_goes_half_way() {
        let voice = glide(225.0, 225.0, 1.5);
        let out = corrected(&mut corrector(0.5, Some(Scale::minor(A))), &voice);
        let heard = frequency(&out[out.len() / 2..]);
        let above = cents(heard, 220.0);
        assert!((above - 19.5).abs() < 6.0, "{above} cents above A");
    }

    /// **A slide is followed, not stepped**: a voice gliding from 200 to
    /// 240 Hz at half strength still rises through every tenth of a second —
    /// pulled toward the notes, never held on one.
    #[test]
    fn a_slide_is_followed_not_stepped() {
        let voice = glide(200.0, 240.0, 2.0);
        let out = corrected(&mut corrector(0.5, Some(Scale::minor(A))), &voice);
        let tenth = 4_800;
        let heard: Vec<f32> = out[tenth..]
            .chunks(tenth)
            .filter(|chunk| chunk.len() == tenth)
            .map(frequency)
            .collect();
        for pair in heard.windows(2) {
            assert!(pair[1] > pair[0], "held, not followed: {heard:?}");
        }
    }

    /// **Noise is not a note**, and silence is not either: no reading, and
    /// nothing moved.
    #[test]
    fn noise_and_silence_are_not_pulled() {
        let mut state = 0x1234_5678_u32;
        let noise: Vec<f32> = (0..24_000)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                #[allow(clippy::cast_precision_loss)]
                let unit = state as f32 / u32::MAX as f32;
                (unit - 0.5) * 0.4
            })
            .collect();
        let mut tuned = corrector(1.0, Some(Scale::major(0)));
        corrected(&mut tuned, &noise);
        assert_eq!(tuned.heard(), None, "heard a note in noise");
        assert!(
            (tuned.ratio() - 1.0).abs() < 1e-3,
            "moved noise by {}",
            tuned.ratio()
        );

        let mut quiet = corrector(1.0, Some(Scale::major(0)));
        corrected(&mut quiet, &vec![0.0; 24_000]);
        assert_eq!(quiet.heard(), None);

        // A hum far under a voice — a room's, a cable's — is not chased.
        let hum: Vec<f32> = glide(225.0, 225.0, 0.5).iter().map(|s| s * 0.005).collect();
        let mut room = corrector(1.0, Some(Scale::minor(A)));
        corrected(&mut room, &hum);
        assert_eq!(room.heard(), None, "chased a hum");
    }

    /// **After a correction it comes back to rest**: a held note pulled, then
    /// quiet, and the corrector is a clean delay of half its window again.
    #[test]
    fn after_a_correction_it_comes_back_to_rest() {
        let mut tuned = corrector(1.0, Some(Scale::minor(A)));
        corrected(&mut tuned, &glide(225.0, 225.0, 1.0));
        corrected(&mut tuned, &vec![0.0; 4 * 48_000]);
        let mut impulse = vec![0.0; 4_000];
        impulse[100] = 1.0;
        let out = corrected(&mut tuned, &impulse);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let expected = 100 + tuned.latency_frames().round() as usize;
        let loud: Vec<usize> = (0..out.len()).filter(|&i| out[i].abs() > 0.01).collect();
        assert!(
            loud.iter().all(|&i| i.abs_diff(expected) <= 1),
            "heard at {loud:?}, not once at {expected}"
        );
    }

    /// **At rest it is a clean delay of half the window** — one tap, not two
    /// mixed — so a corrector turned on with nothing to do does not colour a
    /// voice.
    #[test]
    fn at_rest_it_is_a_clean_delay() {
        let mut tuned = corrector(1.0, None);
        let mut impulse = vec![0.0; 4_000];
        impulse[100] = 1.0;
        let out = corrected(&mut tuned, &impulse);
        let late = tuned.latency_frames();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let expected = 100 + late.round() as usize;
        let loud: Vec<usize> = (0..out.len()).filter(|&i| out[i].abs() > 0.01).collect();
        assert!(
            loud.iter().all(|&i| i.abs_diff(expected) <= 1),
            "heard at {loud:?}, not once at {expected}"
        );
        assert!((late / RATE - LATENCY_SECONDS).abs() < 1e-4);
    }

    /// **The key's notes are the key's**: A minor has G and not G#, and a
    /// note exactly on one is left there.
    #[test]
    fn the_nearest_note_is_in_the_key() {
        let a_minor = Scale::minor(A);
        assert_eq!(a_minor.nearest(67.4), 67.0, "G is in A minor");
        assert_eq!(a_minor.nearest(68.4), 69.0, "G# is not; A is nearer than G");
        assert_eq!(a_minor.nearest(57.0), 57.0);
        let c_major = Scale::major(0);
        assert_eq!(c_major.nearest(61.6), 62.0, "C# is not in C major");
        assert_eq!(c_major.nearest(-0.4), 0.0);
    }

    /// **What the screen sends is held to what it offers.**
    #[test]
    fn settings_are_held() {
        let held = CorrectionSettings {
            strength: 3.0,
            speed_ms: f32::NAN,
        }
        .held();
        assert_eq!((held.strength, held.speed_ms), (1.0, 40.0));
        let held = CorrectionSettings {
            strength: f32::NAN,
            speed_ms: 1.0,
        }
        .held();
        assert_eq!((held.strength, held.speed_ms), (0.8, 10.0));
    }
}
