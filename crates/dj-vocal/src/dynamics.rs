//! The chain's dynamics: a gate, a compressor and a de-esser.
//!
//! Textbook designs, written down rather than borrowed. Each works a sample
//! at a time on one channel — a microphone is one voice — and holds nothing
//! but a few numbers, so a strip of them costs the same whether it was made a
//! second ago or an hour ago.
//!
//! # Where the level is read
//!
//! All three follow a peak envelope in decibels and move their gain towards
//! where it should be at an attack or a release rate: the log-domain, smoothed
//! -gain arrangement, whose release is the same audible length at any level.

use dj_dsp::Biquad;

/// The quietest level worth reading, so silence is a number and not minus
/// infinity.
const FLOOR_DB: f32 = -120.0;

pub(crate) fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub(crate) fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        FLOOR_DB
    } else {
        (20.0 * linear.log10()).max(FLOOR_DB)
    }
}

/// A one-pole coefficient: how much of the old value survives each sample for
/// a 1/e change in `ms`.
pub(crate) fn coefficient(ms: f32, sample_rate: f32) -> f32 {
    let samples = (ms * 0.001 * sample_rate).max(1.0);
    (-1.0 / samples).exp()
}

// -- the gate ------------------------------------------------------------------

/// Shuts a microphone between phrases, so the room it hears — the PA, the
/// crowd, the next singer — is not sung through it.
///
/// Opens at the threshold and closes only below it by the hysteresis, and not
/// until it has held for a moment, so the end of a word does not chatter it.
/// Shut is a floor rather than silence: a gate that cut to nothing would make
/// a breath before a line sound clipped.
#[derive(Debug, Clone)]
pub struct Gate {
    open_at: f32,
    close_at: f32,
    floor: f32,
    attack: f32,
    release: f32,
    detector_release: f32,
    hold_frames: u32,
    held: u32,
    detector: f32,
    gain: f32,
    open: bool,
}

impl Gate {
    pub const DEFAULT_THRESHOLD_DB: f32 = -45.0;
    pub const HYSTERESIS_DB: f32 = 6.0;
    pub const DEFAULT_RANGE_DB: f32 = 40.0;
    const ATTACK_MS: f32 = 1.0;
    const HOLD_MS: f32 = 80.0;
    const RELEASE_MS: f32 = 150.0;
    const DETECTOR_MS: f32 = 5.0;

    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let mut gate = Self {
            open_at: 0.0,
            close_at: 0.0,
            floor: 0.0,
            attack: coefficient(Self::ATTACK_MS, sample_rate),
            release: coefficient(Self::RELEASE_MS, sample_rate),
            detector_release: coefficient(Self::DETECTOR_MS, sample_rate),
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            hold_frames: (Self::HOLD_MS * 0.001 * sample_rate) as u32,
            held: 0,
            detector: 0.0,
            gain: 0.0,
            open: false,
        };
        gate.set(Self::DEFAULT_THRESHOLD_DB, Self::DEFAULT_RANGE_DB);
        gate.gain = gate.floor;
        gate
    }

    /// Where it opens, in dBFS, and how far it turns the microphone down when
    /// shut, in dB.
    pub fn set(&mut self, threshold_db: f32, range_db: f32) {
        self.open_at = db_to_linear(threshold_db);
        self.close_at = db_to_linear(threshold_db - Self::HYSTERESIS_DB);
        self.floor = db_to_linear(-range_db.abs());
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let level = input.abs();
        self.detector = if level > self.detector {
            level
        } else {
            level + (self.detector - level) * self.detector_release
        };
        if self.detector >= self.open_at {
            self.open = true;
            self.held = self.hold_frames;
        } else if self.open && self.detector >= self.close_at {
            self.held = self.hold_frames;
        } else if self.held > 0 {
            self.held -= 1;
        } else {
            self.open = false;
        }
        let target = if self.open { 1.0 } else { self.floor };
        let rate = if target > self.gain {
            self.attack
        } else {
            self.release
        };
        self.gain = target + (self.gain - target) * rate;
        input * self.gain
    }

    /// Whether a voice is getting through.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether a sample is loud enough to open it — one comparison, which is
    /// all an idle strip spends listening for its next word.
    #[must_use]
    pub fn opens_at(&self, sample: f32) -> bool {
        sample.abs() >= self.open_at
    }
}

// -- the compressor ------------------------------------------------------------

/// Evens a voice out: a singer who leans into the microphone on the chorus and
/// away on the verse comes out at one level the room can hear.
///
/// Feed-forward, soft-kneed, with make-up gain after.
#[derive(Debug, Clone)]
pub struct Compressor {
    threshold_db: f32,
    ratio: f32,
    knee_db: f32,
    makeup_db: f32,
    attack: f32,
    release: f32,
    /// The voice's peak level, held between the peaks of its waveform so the
    /// level is the voice's and not the instant's.
    detector: f32,
    detector_release: f32,
    /// The gain reduction being applied, in dB, never above zero.
    reduction_db: f32,
}

impl Compressor {
    pub const DEFAULT_THRESHOLD_DB: f32 = -20.0;
    pub const DEFAULT_RATIO: f32 = 3.0;
    const KNEE_DB: f32 = 6.0;
    const ATTACK_MS: f32 = 5.0;
    const RELEASE_MS: f32 = 120.0;
    /// Long enough to hold across a low voice's cycle, short beside the
    /// release.
    const DETECTOR_MS: f32 = 30.0;

    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self {
            threshold_db: Self::DEFAULT_THRESHOLD_DB,
            ratio: Self::DEFAULT_RATIO,
            knee_db: Self::KNEE_DB,
            makeup_db: 0.0,
            attack: coefficient(Self::ATTACK_MS, sample_rate),
            release: coefficient(Self::RELEASE_MS, sample_rate),
            detector: 0.0,
            detector_release: coefficient(Self::DETECTOR_MS, sample_rate),
            reduction_db: 0.0,
        }
    }

    /// Where it starts, in dBFS; how hard, as a ratio (1 is off); and how
    /// much it brings the level back up after, in dB.
    pub fn set(&mut self, threshold_db: f32, ratio: f32, makeup_db: f32) {
        self.threshold_db = threshold_db;
        self.ratio = ratio.max(1.0);
        self.makeup_db = makeup_db;
    }

    /// With no knee, for a test to reason about exactly.
    #[cfg(test)]
    fn hard_knee(mut self) -> Self {
        self.knee_db = 0.0;
        self
    }

    /// How far a level is turned down, in dB (zero or less).
    fn computed(&self, level_db: f32) -> f32 {
        let over = level_db - self.threshold_db;
        let slope = 1.0 / self.ratio - 1.0;
        let half = self.knee_db / 2.0;
        if over <= -half {
            0.0
        } else if over < half {
            slope * (over + half) * (over + half) / (2.0 * self.knee_db)
        } else {
            slope * over
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let level = input.abs();
        self.detector = if level > self.detector {
            level
        } else {
            level + (self.detector - level) * self.detector_release
        };
        let wanted = self.computed(linear_to_db(self.detector));
        let rate = if wanted < self.reduction_db {
            self.attack
        } else {
            self.release
        };
        self.reduction_db = wanted + (self.reduction_db - wanted) * rate;
        input * db_to_linear(self.reduction_db + self.makeup_db)
    }

    /// How far it is turning the voice down now, in dB, as a positive number.
    #[must_use]
    pub fn reduction_db(&self) -> f32 {
        -self.reduction_db
    }
}

// -- the de-esser -----------------------------------------------------------------

/// Takes the edge off an S.
///
/// Sibilance lives above about six kilohertz, and a compressor that reacts to
/// the whole voice cannot catch it without dulling everything else. So the
/// band above the frequency is split off, and only that band is turned down,
/// and only while it is louder than the threshold.
///
/// The split is the input less a low-pass of it. That makes it exact — with
/// nothing to reduce, the voice comes out untouched to the last bit — and,
/// above the frequency, in phase with the voice, so turning the band down
/// turns the S down. (Split the other way, by a high-pass, the band arrives
/// some sixty degrees late at nine kilohertz and taking it away takes almost
/// nothing: measured, one decibel of a wanted ten.)
#[derive(Debug, Clone)]
pub struct DeEsser {
    split: Biquad,
    sample_rate: f32,
    threshold_db: f32,
    range_db: f32,
    detector: f32,
    detector_release: f32,
    attack: f32,
    release: f32,
    reduction_db: f32,
}

impl DeEsser {
    pub const DEFAULT_FREQUENCY_HZ: f32 = 6_500.0;
    pub const DEFAULT_THRESHOLD_DB: f32 = -30.0;
    /// The most it takes off an S, in dB.
    pub const RANGE_DB: f32 = 10.0;
    const DETECTOR_MS: f32 = 10.0;
    const ATTACK_MS: f32 = 1.0;
    const RELEASE_MS: f32 = 60.0;

    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self {
            split: Biquad::low_pass(
                sample_rate,
                Self::DEFAULT_FREQUENCY_HZ,
                std::f32::consts::FRAC_1_SQRT_2,
            ),
            sample_rate,
            threshold_db: Self::DEFAULT_THRESHOLD_DB,
            range_db: Self::RANGE_DB,
            detector: 0.0,
            detector_release: coefficient(Self::DETECTOR_MS, sample_rate),
            attack: coefficient(Self::ATTACK_MS, sample_rate),
            release: coefficient(Self::RELEASE_MS, sample_rate),
            reduction_db: 0.0,
        }
    }

    /// Where sibilance starts, in Hz, and how loud it may be, in dBFS.
    pub fn set(&mut self, frequency_hz: f32, threshold_db: f32) {
        let frequency = frequency_hz.clamp(2_000.0, self.sample_rate * 0.45);
        self.split.set_coefficients_from(&Biquad::low_pass(
            self.sample_rate,
            frequency,
            std::f32::consts::FRAC_1_SQRT_2,
        ));
        self.threshold_db = threshold_db;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let high = input - self.split.process(input);
        let level = high.abs();
        self.detector = if level > self.detector {
            level
        } else {
            level + (self.detector - level) * self.detector_release
        };
        let over = linear_to_db(self.detector) - self.threshold_db;
        let wanted = over.clamp(0.0, self.range_db);
        let rate = if wanted > self.reduction_db {
            self.attack
        } else {
            self.release
        };
        self.reduction_db = wanted + (self.reduction_db - wanted) * rate;
        let kept = db_to_linear(-self.reduction_db);
        input - (1.0 - kept) * high
    }

    /// How far it is turning the S down now, in dB.
    #[must_use]
    pub fn reduction_db(&self) -> f32 {
        self.reduction_db
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn tone(hz: f32, db: f32, seconds: f32) -> Vec<f32> {
        let amplitude = db_to_linear(db);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames = (seconds * RATE) as usize;
        (0..frames)
            .map(|n| (std::f32::consts::TAU * hz * n as f32 / RATE).sin() * amplitude)
            .collect()
    }

    /// The peak of the last `seconds` of a signal, in dBFS.
    fn tail_peak_db(samples: &[f32], seconds: f32) -> f32 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let tail = (seconds * RATE) as usize;
        let peak = samples[samples.len() - tail..]
            .iter()
            .fold(0.0f32, |m, s| m.max(s.abs()));
        linear_to_db(peak)
    }

    /// **A voice opens the gate; the room behind it does not.** A line sung
    /// at −20 dBFS comes through whole; the hum of the room at −60 dBFS is
    /// held down by the range; and a voice falling just under the threshold
    /// keeps it open, because it only shuts the hysteresis lower.
    #[test]
    fn the_gate_passes_a_voice_and_holds_the_room_down() {
        let mut gate = Gate::new(RATE);
        let room: Vec<f32> = tone(200.0, -60.0, 0.5)
            .into_iter()
            .map(|s| gate.process(s))
            .collect();
        assert!(!gate.is_open());
        assert!(
            (tail_peak_db(&room, 0.1) - (-60.0 - Gate::DEFAULT_RANGE_DB)).abs() < 1.0,
            "the room at {} dB",
            tail_peak_db(&room, 0.1)
        );
        let voice: Vec<f32> = tone(300.0, -20.0, 0.5)
            .into_iter()
            .map(|s| gate.process(s))
            .collect();
        assert!(gate.is_open());
        assert!((tail_peak_db(&voice, 0.1) + 20.0).abs() < 0.1);
        // Three dB under where it opens, but above where it shuts.
        let softer: Vec<f32> = tone(300.0, Gate::DEFAULT_THRESHOLD_DB - 3.0, 0.5)
            .into_iter()
            .map(|s| gate.process(s))
            .collect();
        assert!(
            gate.is_open(),
            "a voice just under the threshold chattered it"
        );
        assert!((tail_peak_db(&softer, 0.1) - (Gate::DEFAULT_THRESHOLD_DB - 3.0)).abs() < 0.1);
    }

    /// **The compressor's arithmetic.** A tone 14 dB over a −20 dB threshold
    /// at 4:1 comes out 3.5 dB over it; one under the threshold is not
    /// touched; make-up gain lifts the result by exactly itself.
    #[test]
    fn the_compressor_turns_a_level_down_by_its_ratio() {
        let mut compressor = Compressor::new(RATE).hard_knee();
        compressor.set(-20.0, 4.0, 0.0);
        let loud: Vec<f32> = tone(1_000.0, -6.0, 1.0)
            .into_iter()
            .map(|s| compressor.process(s))
            .collect();
        assert!(
            (tail_peak_db(&loud, 0.2) + 16.5).abs() < 0.5,
            "came out at {} dB",
            tail_peak_db(&loud, 0.2)
        );
        assert!((compressor.reduction_db() - 10.5).abs() < 0.5);

        let mut quiet = Compressor::new(RATE).hard_knee();
        quiet.set(-20.0, 4.0, 0.0);
        let under: Vec<f32> = tone(1_000.0, -30.0, 0.5)
            .into_iter()
            .map(|s| quiet.process(s))
            .collect();
        assert!((tail_peak_db(&under, 0.2) + 30.0).abs() < 0.05);

        let mut lifted = Compressor::new(RATE).hard_knee();
        lifted.set(-20.0, 4.0, 6.0);
        let made_up: Vec<f32> = tone(1_000.0, -6.0, 1.0)
            .into_iter()
            .map(|s| lifted.process(s))
            .collect();
        assert!((tail_peak_db(&made_up, 0.2) - tail_peak_db(&loud, 0.2) - 6.0).abs() < 0.1);
    }

    /// **The S is turned down; the voice is not.** A loud tone in the
    /// sibilance band loses most of the range; a vowel's worth of tone below
    /// it loses nothing; and with nothing over the threshold the output is the
    /// input exactly, because the split puts back what it takes apart.
    #[test]
    fn the_de_esser_turns_down_only_the_s() {
        let mut de_esser = DeEsser::new(RATE);
        let s: Vec<f32> = tone(9_000.0, -10.0, 0.5)
            .into_iter()
            .map(|x| de_esser.process(x))
            .collect();
        let taken = -10.0 - tail_peak_db(&s, 0.1);
        assert!(taken > 6.0, "only {taken} dB off an S");

        let mut vowel = DeEsser::new(RATE);
        let a: Vec<f32> = tone(500.0, -10.0, 0.5)
            .into_iter()
            .map(|x| vowel.process(x))
            .collect();
        assert!(
            (tail_peak_db(&a, 0.1) + 10.0).abs() < 0.3,
            "the vowel came out at {}",
            tail_peak_db(&a, 0.1)
        );

        let mut idle = DeEsser::new(RATE);
        let soft = tone(9_000.0, -50.0, 0.2);
        for &x in &soft {
            let out = idle.process(x);
            assert!((out - x).abs() < 1e-6, "{out} for {x}");
        }
    }
}
