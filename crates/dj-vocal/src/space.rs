//! The chain's space: an echo and a reverb, cheap enough to run one of each on
//! every microphone.
//!
//! The effect rack's reverb (`dj_dsp::fx`) is built for the music: stereo in,
//! a slot that owns its memory and changes what it is. A singer's is simpler
//! and runs N times over, so it is its own: mono in, stereo out, its buffers
//! sized once for the longest setting it allows, so turning a knob never
//! allocates.
//!
//! The reverb is a feedback delay network — four delays at lengths that share
//! no factor, mixed through an orthogonal matrix and fed back, each through a
//! gentle low-pass so the tail darkens as a room's does. Its feedback is
//! worked out from the time the DJ asks for, so "two seconds" means the tail is
//! sixty decibels down in two seconds.

/// The longest echo, in seconds, and so the size of the echo's buffer.
pub const LONGEST_ECHO_SECONDS: f32 = 1.5;

/// The longest reverb, in seconds.
pub const LONGEST_REVERB_SECONDS: f32 = 6.0;

/// The level a tail is counted as gone at: sixty decibels down, the
/// definition of a reverberation time.
const GONE: f32 = 1e-3;

/// A repeating echo on one voice.
#[derive(Debug, Clone)]
pub struct Echo {
    buffer: Vec<f32>,
    write: usize,
    delay: usize,
    feedback: f32,
    damping: f32,
    damped: f32,
    sample_rate: f32,
}

impl Echo {
    pub const DEFAULT_DELAY_MS: f32 = 320.0;
    pub const DEFAULT_FEEDBACK: f32 = 0.35;
    /// How much of each repeat's treble survives into the next.
    const DAMPING: f32 = 0.35;

    /// Sized for [`LONGEST_ECHO_SECONDS`], once.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames = (LONGEST_ECHO_SECONDS * sample_rate) as usize + 1;
        let mut echo = Self {
            buffer: vec![0.0; frames],
            write: 0,
            delay: 1,
            feedback: Self::DEFAULT_FEEDBACK,
            damping: Self::DAMPING,
            damped: 0.0,
            sample_rate,
        };
        echo.set(Self::DEFAULT_DELAY_MS, Self::DEFAULT_FEEDBACK);
        echo
    }

    /// How long between repeats, in milliseconds, and how much of each comes
    /// back, 0 to 0.9.
    pub fn set(&mut self, delay_ms: f32, feedback: f32) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let delay = (delay_ms.max(1.0) * 0.001 * self.sample_rate) as usize;
        self.delay = delay.clamp(1, self.buffer.len() - 1);
        self.feedback = feedback.clamp(0.0, 0.9);
    }

    /// Without the damping, for a test that counts repeats exactly.
    #[cfg(test)]
    fn undamped(mut self) -> Self {
        self.damping = 0.0;
        self
    }

    /// One sample in; the echo of it out, without the dry voice.
    pub fn process(&mut self, input: f32) -> f32 {
        let len = self.buffer.len();
        let read = (self.write + len - self.delay) % len;
        let repeat = self.buffer[read];
        self.damped = repeat + (self.damped - repeat) * self.damping;
        self.buffer[self.write] = input + self.damped * self.feedback;
        self.write = (self.write + 1) % len;
        repeat
    }

    /// How long it goes on sounding after the voice stops, in frames.
    #[must_use]
    pub fn tail_frames(&self) -> usize {
        let repeats = if self.feedback <= 0.0 {
            1.0
        } else {
            (GONE.ln() / self.feedback.ln()).ceil() + 1.0
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames = repeats as usize * self.delay;
        frames
    }
}

/// The four delays' lengths at the default size, in milliseconds. Chosen to
/// share no factor, so their repeats never line up into a flutter.
const LINES_MS: [f32; 4] = [29.7, 37.1, 41.1, 43.7];

/// The largest room, as a multiple of those lengths.
const LARGEST: f32 = 1.5;

/// One of the reverb's four delays: its buffer, how much of it is in use,
/// where it is written, what it keeps each pass, and its damping's memory.
#[derive(Debug, Clone)]
struct Line {
    buffer: Vec<f32>,
    length: usize,
    write: usize,
    gain: f32,
    damped: f32,
}

impl Line {
    /// Sized for the largest room.
    fn new(ms: f32, sample_rate: f32) -> Self {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames = (ms * LARGEST * 0.001 * sample_rate) as usize + 1;
        Self {
            buffer: vec![0.0; frames],
            length: 1,
            write: 0,
            gain: 0.0,
            damped: 0.0,
        }
    }

    /// What comes out of the line now, through its damping.
    fn read(&mut self, damping: f32) -> f32 {
        let sample = self.buffer[self.write];
        self.damped = sample + (self.damped - sample) * damping;
        self.damped
    }

    fn write(&mut self, sample: f32) {
        self.buffer[self.write] = sample;
        self.write = (self.write + 1) % self.length;
    }
}

/// A small room, or a hall, around one voice.
#[derive(Debug, Clone)]
pub struct Reverb {
    lines: [Line; 4],
    damping: f32,
    seconds: f32,
    sample_rate: f32,
}

impl Reverb {
    pub const DEFAULT_SECONDS: f32 = 1.6;
    const DAMPING: f32 = 0.25;

    /// Sized for the largest room, once.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let mut reverb = Self {
            lines: LINES_MS.map(|ms| Line::new(ms, sample_rate)),
            damping: Self::DAMPING,
            seconds: Self::DEFAULT_SECONDS,
            sample_rate,
        };
        reverb.set(Self::DEFAULT_SECONDS);
        reverb
    }

    /// How long the tail lasts, in seconds, to sixty decibels down. The room
    /// grows with it, from the lengths above to half as long again.
    pub fn set(&mut self, seconds: f32) {
        self.seconds = seconds.clamp(0.2, LONGEST_REVERB_SECONDS);
        let size = (0.7 + self.seconds / LONGEST_REVERB_SECONDS * 0.8).min(LARGEST);
        for (line, ms) in self.lines.iter_mut().zip(LINES_MS) {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let frames = (ms * size * 0.001 * self.sample_rate) as usize;
            line.length = frames.clamp(1, line.buffer.len());
            line.write %= line.length;
            #[allow(clippy::cast_precision_loss)]
            let each = line.length as f32 / self.sample_rate;
            // Each pass round a line loses its share of sixty decibels.
            line.gain = GONE.powf(each / self.seconds);
        }
    }

    /// Without the damping, for a test that measures the time exactly.
    #[cfg(test)]
    fn undamped(mut self) -> Self {
        self.damping = 0.0;
        self
    }

    /// One sample in; the room's answer out, left and right, without the dry
    /// voice.
    pub fn process(&mut self, input: f32) -> (f32, f32) {
        let damping = self.damping;
        let [a, b, c, d] = self.lines.each_mut().map(|line| line.read(damping));
        // A 4×4 Hadamard matrix, halved: orthogonal, so it moves energy
        // between the lines without adding any.
        let mixed = [
            (a + b + c + d) * 0.5,
            (a - b + c - d) * 0.5,
            (a + b - c - d) * 0.5,
            (a - b - c + d) * 0.5,
        ];
        for (line, back) in self.lines.iter_mut().zip(mixed) {
            let gain = line.gain;
            line.write(input * 0.5 + back * gain);
        }
        ((a + c) * 0.5, (b + d) * 0.5)
    }

    /// How long it goes on sounding after the voice stops, in frames.
    #[must_use]
    pub fn tail_frames(&self) -> usize {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames = (self.seconds * self.sample_rate) as usize;
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    /// **The echo repeats at its time, each repeat its feedback of the one
    /// before**, and says how long it will go on for.
    #[test]
    fn an_echo_repeats_at_its_time_and_fades_by_its_feedback() {
        let mut echo = Echo::new(RATE).undamped();
        echo.set(100.0, 0.5);
        let out: Vec<f32> = (0..20_000)
            .map(|n| echo.process(if n == 0 { 1.0 } else { 0.0 }))
            .collect();
        assert!((out[4_800] - 1.0).abs() < 1e-6, "the first repeat");
        assert!((out[9_600] - 0.5).abs() < 1e-6, "the second, at half");
        assert!((out[14_400] - 0.25).abs() < 1e-6);
        assert!(
            out[..4_800].iter().all(|s| *s == 0.0),
            "nothing before its time"
        );
        // 0.5 to the tenth is under a thousandth: eleven repeats.
        assert_eq!(echo.tail_frames(), 11 * 4_800);
    }

    /// **A reverb's time is the time the DJ asked for.** An impulse's tail is
    /// sixty decibels down in the reverberation time — measured as the energy
    /// in a tenth of a second at half that time, thirty down on the first —
    /// and left and right are not the same, which is what makes it a space.
    #[test]
    fn a_reverb_falls_sixty_decibels_in_its_time() {
        let mut reverb = Reverb::new(RATE).undamped();
        reverb.set(2.0);
        let frames = (RATE * 2.5) as usize;
        let mut left = Vec::with_capacity(frames);
        let mut right = Vec::with_capacity(frames);
        for n in 0..frames {
            let (l, r) = reverb.process(if n == 0 { 1.0 } else { 0.0 });
            left.push(l);
            right.push(r);
        }
        let energy = |from: f32| {
            let start = (from * RATE) as usize;
            let len = (0.1 * RATE) as usize;
            left[start..start + len]
                .iter()
                .chain(&right[start..start + len])
                .map(|s| s * s)
                .sum::<f32>()
        };
        let early = energy(0.1);
        let halfway = energy(1.0);
        let db = 10.0 * (halfway / early).log10();
        assert!((db + 27.0).abs() < 6.0, "{db} dB at half the time");
        assert!(left.iter().zip(&right).any(|(l, r)| (l - r).abs() > 1e-4));
        assert_eq!(reverb.tail_frames(), 96_000);
    }

    /// **Fed for a long time, it never runs away**: ten seconds of a loud
    /// voice into the longest room, and what comes out stays bounded.
    #[test]
    fn a_reverb_fed_for_a_long_time_stays_bounded() {
        let mut reverb = Reverb::new(RATE);
        reverb.set(LONGEST_REVERB_SECONDS);
        let mut seed = 1u32;
        let mut loudest = 0.0f32;
        for _ in 0..(RATE as usize * 10) {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            #[allow(clippy::cast_precision_loss)]
            let noise = (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
            let (l, r) = reverb.process(noise * 0.5);
            assert!(l.is_finite() && r.is_finite());
            loudest = loudest.max(l.abs()).max(r.abs());
        }
        assert!(loudest < 8.0, "{loudest}");
    }
}
