//! The second buffer a slot owns: a reverberation tank.
//!
//! The delay line handles everything with *one* echo path — echo, delay,
//! flanger. A reverb is different in kind: it needs several short delays
//! running at once, at lengths chosen to be mutually prime so their repeats do
//! not pile up into a flutter.
//!
//! So a slot owns two buffers rather than one. That is the same principle as
//! before — the *slot* owns the memory, not the effect — widened by one word:
//! rack memory still scales with how many effects can run at once rather than
//! with the size of the catalogue, and switching to reverb still costs an
//! assignment rather than an allocation.
//!
//! The structure is the classic Schroeder arrangement, which is textbook rather
//! than borrowed: four comb filters in parallel for the tail, two allpass
//! filters in series to smear their echoes into something without a pitch.

use crate::CHANNELS;

/// A comb filter: one delay with feedback and a damping low-pass.
///
/// The damping is what makes a tail sound like a room instead of a pipe. Real
/// rooms lose their high frequencies fastest — air and soft surfaces both
/// absorb treble — so a tail whose repeats keep their brightness reads as
/// artificial however long it is.
#[derive(Debug)]
struct Comb {
    buffer: Vec<f32>,
    frames: usize,
    write: usize,
    /// One-pole state per channel, for the damping.
    damped: [f32; CHANNELS],
}

impl Comb {
    fn new(frames: usize) -> Self {
        Self {
            buffer: vec![0.0; frames * CHANNELS],
            frames,
            write: 0,
            damped: [0.0; CHANNELS],
        }
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.damped = [0.0; CHANNELS];
        self.write = 0;
    }

    #[inline]
    fn process(&mut self, left: f32, right: f32, feedback: f32, damping: f32) -> (f32, f32) {
        let at = self.write * CHANNELS;
        let (out_l, out_r) = (self.buffer[at], self.buffer[at + 1]);

        // One-pole low-pass in the feedback path, not on the output: damping
        // has to compound with every pass round the loop, which is what makes
        // the tail get darker as it decays rather than merely being dark.
        self.damped[0] = out_l * (1.0 - damping) + self.damped[0] * damping;
        self.damped[1] = out_r * (1.0 - damping) + self.damped[1] * damping;

        self.buffer[at] = left + self.damped[0] * feedback;
        self.buffer[at + 1] = right + self.damped[1] * feedback;
        self.write = (self.write + 1) % self.frames;
        (out_l, out_r)
    }
}

/// An allpass filter: passes every frequency at the same level, and scrambles
/// their phases.
///
/// It adds no colour of its own, which is exactly why it is the right thing to
/// put after the combs — it multiplies their sparse echoes into a dense wash
/// without adding resonances a listener would hear as a note.
#[derive(Debug)]
struct Allpass {
    buffer: Vec<f32>,
    frames: usize,
    write: usize,
}

impl Allpass {
    fn new(frames: usize) -> Self {
        Self {
            buffer: vec![0.0; frames * CHANNELS],
            frames,
            write: 0,
        }
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.write = 0;
    }

    #[inline]
    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Fixed at the textbook value. It sets how dense the scattering is, and
        // there is no musical reason to expose it: a DJ wants "more reverb",
        // not "more diffusion coefficient".
        const GAIN: f32 = 0.5;

        let at = self.write * CHANNELS;
        let (stored_l, stored_r) = (self.buffer[at], self.buffer[at + 1]);
        let out_l = -left + stored_l;
        let out_r = -right + stored_r;
        self.buffer[at] = left + stored_l * GAIN;
        self.buffer[at + 1] = right + stored_r * GAIN;
        self.write = (self.write + 1) % self.frames;
        (out_l, out_r)
    }
}

/// Four combs and two allpasses: enough room to be a room.
#[derive(Debug)]
pub struct Tank {
    combs: [Comb; 4],
    allpasses: [Allpass; 2],
}

impl Tank {
    /// Roughly where each comb should sit, in milliseconds.
    ///
    /// Only roughly: what actually matters is that the lengths in *frames* are
    /// mutually prime, and choosing pretty millisecond values does not give you
    /// that — at 44.1 kHz these round to 1310 and 1928, which share a factor of
    /// two. Lengths sharing a factor line their repeats up periodically, and a
    /// periodic repeat is a flutter: the ringing you hear in a stairwell, which
    /// is a fault in a reverb rather than a feature.
    ///
    /// So the millisecond figures are a target and [`next_prime`] does the
    /// deciding. Every length is then prime, so any two are mutually prime at
    /// every sample rate, without a table per rate.
    const COMB_MS: [f32; 4] = [29.7, 37.1, 41.1, 43.7];
    /// Allpass lengths. Short, because their job is density rather than length.
    const ALLPASS_MS: [f32; 2] = [5.0, 1.7];

    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let frames = |ms: f32| next_prime(((sample_rate * ms / 1000.0).ceil() as usize).max(2));
        Self {
            combs: std::array::from_fn(|i| Comb::new(frames(Self::COMB_MS[i]))),
            allpasses: std::array::from_fn(|i| Allpass::new(frames(Self::ALLPASS_MS[i]))),
        }
    }

    pub fn clear(&mut self) {
        for comb in &mut self.combs {
            comb.clear();
        }
        for allpass in &mut self.allpasses {
            allpass.clear();
        }
    }

    /// One frame of reverberation.
    ///
    /// `size` is how big the room is, 0..=1, and it drives both the feedback
    /// and the damping — a bigger room rings longer *and* absorbs more treble,
    /// so one knob moves both the way the world does.
    #[inline]
    #[must_use]
    pub fn process(&mut self, left: f32, right: f32, size: f32) -> (f32, f32) {
        // Capped below the point of self-oscillation. An infinite reverb is a
        // synthesiser feature and a PA speaker hazard.
        let size = size.clamp(0.0, 1.0);
        let feedback = 0.7 + size * 0.27;
        // A gentle range, and the reason is a mistake worth recording: damping
        // shortens the tail, so a damping that climbs as fast as the feedback
        // cancels it out and the size knob does nothing. A big room does absorb
        // more treble than a small one — but it still rings longer, and the
        // control has to make that true.
        let damping = 0.15 + size * 0.2;

        // Combs in parallel, summed. Scaled by the count so a four-comb tank
        // and a longer one would sit at the same level.
        let mut wet = (0.0, 0.0);
        for comb in &mut self.combs {
            let (l, r) = comb.process(left, right, feedback, damping);
            wet.0 += l;
            wet.1 += r;
        }
        let scale = 1.0 / self.combs.len() as f32;
        wet = (wet.0 * scale, wet.1 * scale);

        // Allpasses in series, to smear what the combs produced.
        for allpass in &mut self.allpasses {
            wet = allpass.process(wet.0, wet.1);
        }
        wet
    }
}

/// The next prime at or above `n`.
///
/// Trial division, at construction time only. A tank is built when a device is
/// opened, so this runs four times a session and never on the audio thread.
fn next_prime(mut n: usize) -> usize {
    fn is_prime(n: usize) -> bool {
        if n < 2 {
            return false;
        }
        if n.is_multiple_of(2) {
            return n == 2;
        }
        let mut divisor = 3;
        while divisor * divisor <= n {
            if n.is_multiple_of(divisor) {
                return false;
            }
            divisor += 2;
        }
        true
    }
    while !is_prime(n) {
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// A tail has to outlast the sound that made it, or it is not a reverb.
    #[test]
    fn an_impulse_rings_on_long_after_it_stopped() {
        let mut tank = Tank::new(SR);
        let _ = tank.process(1.0, 1.0, 0.5);

        // Half a second later there should still be something audible.
        let mut late = 0.0f32;
        for frame in 0..(SR as usize / 2) {
            let (left, _) = tank.process(0.0, 0.0, 0.5);
            if frame > SR as usize / 4 {
                late = late.max(left.abs());
            }
        }
        assert!(late > 1e-4, "the tail died at {late}");
    }

    /// And it has to decay. A tank that gains is a speaker on fire.
    #[test]
    fn the_tail_decays_even_at_the_largest_size() {
        let mut tank = Tank::new(SR);
        let _ = tank.process(1.0, 1.0, 1.0);

        let mut early = 0.0f32;
        let mut late = 0.0f32;
        for frame in 0..(SR as usize * 8) {
            let (left, _) = tank.process(0.0, 0.0, 1.0);
            if frame < SR as usize {
                early = early.max(left.abs());
            } else if frame > SR as usize * 6 {
                late = late.max(left.abs());
            }
        }
        assert!(late < early, "it grew: {early} then {late}");
        assert!(early <= 2.0, "the tank rang up to {early}");
    }

    /// A bigger room rings longer. The one knob has to actually do something.
    #[test]
    fn a_bigger_room_rings_longer() {
        fn tail(size: f32) -> f32 {
            let mut tank = Tank::new(SR);
            let _ = tank.process(1.0, 1.0, size);
            let mut energy = 0.0;
            for _ in 0..(SR as usize * 2) {
                let (left, _) = tank.process(0.0, 0.0, size);
                energy += f64::from(left * left);
            }
            energy as f32
        }
        assert!(
            tail(1.0) > tail(0.0) * 1.5,
            "size made little difference: {} against {}",
            tail(1.0),
            tail(0.0)
        );
    }

    /// The tail is what a DJ throws and lets go of, but a switch of effect has
    /// to take it with them.
    #[test]
    fn clearing_forgets_the_tail() {
        let mut tank = Tank::new(SR);
        for _ in 0..1_000 {
            let _ = tank.process(1.0, 1.0, 0.5);
        }
        tank.clear();
        let mut peak = 0.0f32;
        for _ in 0..(SR as usize / 2) {
            let (left, _) = tank.process(0.0, 0.0, 0.5);
            peak = peak.max(left.abs());
        }
        assert!(peak < 1e-9, "a tail survived the clear at {peak}");
    }

    #[test]
    fn next_prime_finds_the_next_prime() {
        assert_eq!(next_prime(2), 2);
        assert_eq!(next_prime(4), 5);
        assert_eq!(next_prime(1310), 1319);
        assert_eq!(next_prime(1928), 1931);
    }

    /// The comb lengths must be mutually prime in *frames*, not only in
    /// milliseconds. Rounding to a sample rate is where that quietly breaks —
    /// this test caught 1310 and 1928 at 44.1 kHz, both even — and a shared
    /// factor is an audible flutter.
    #[test]
    fn the_comb_lengths_share_no_common_factor() {
        fn gcd(a: usize, b: usize) -> usize {
            if b == 0 { a } else { gcd(b, a % b) }
        }
        for rate in [44_100.0, 48_000.0, 96_000.0] {
            let tank = Tank::new(rate);
            let lengths: Vec<usize> = tank.combs.iter().map(|c| c.frames).collect();
            for (i, a) in lengths.iter().enumerate() {
                for b in &lengths[i + 1..] {
                    assert_eq!(gcd(*a, *b), 1, "at {rate} Hz, {a} and {b} share a factor");
                }
            }
        }
    }

    /// Silence in, silence out. A tank that idles into noise would hiss under
    /// a paused deck all night.
    #[test]
    fn a_quiet_tank_stays_quiet() {
        let mut tank = Tank::new(SR);
        for _ in 0..(SR as usize) {
            let (left, right) = tank.process(0.0, 0.0, 1.0);
            assert_eq!((left, right), (0.0, 0.0));
        }
    }
}
