//! The shift register a control record's position is written with.
//!
//! # What a timecode record actually carries
//!
//! Not a position. A **bitstream** — the output of a linear feedback shift
//! register, amplitude-modulated onto a carrier tone. Position is recovered by
//! reading enough consecutive bits to identify *where in that stream* the
//! needle is, and looking the answer up.
//!
//! That indirection is the whole trick. A record cannot be told where the
//! needle was dropped, and a plain counter would need framing, sync words and
//! error handling. An LFSR of `n` bits visits every one of its `2^n - 1`
//! non-zero states exactly once before repeating, so **any** `n` consecutive
//! bits name a unique point in the sequence — drop the needle anywhere, read
//! twenty bits, and you know the position to within a millisecond.
//!
//! # On stepping backwards
//!
//! A record gets played backwards, so the register has to run backwards too.
//! An LFSR is reversible: the bit shifted out is recoverable from the state
//! that remains, which is what [`Lfsr::step_back`] does. Without it, scratching
//! would lose position the moment the platter turned the wrong way.

/// A Galois-configuration linear feedback shift register.
///
/// Galois rather than Fibonacci because stepping is one shift, one test and
/// one exclusive-or in each direction, with no parity to compute over the tap
/// mask — and the reverse step stays as cheap as the forward one, which matters
/// when a scratch flips direction hundreds of times a second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lfsr {
    state: u32,
    taps: u32,
    /// How many bits wide. 20 for the records in circulation.
    bits: u32,
}

impl Lfsr {
    /// A register of `bits` width, starting at `seed`, feeding back through
    /// `taps`.
    ///
    /// Returns `None` for a width outside 2..=32, or a zero seed. Zero is the
    /// one state an LFSR cannot leave — it would sit there for ever, reporting
    /// the same position however far the record turned.
    #[must_use]
    pub fn new(bits: u32, seed: u32, taps: u32) -> Option<Self> {
        if !(2..=32).contains(&bits) {
            return None;
        }
        let mask = Self::mask_for(bits);
        let state = seed & mask;
        if state == 0 {
            return None;
        }
        Some(Self {
            state,
            taps: taps & mask,
            bits,
        })
    }

    const fn mask_for(bits: u32) -> u32 {
        if bits >= 32 {
            u32::MAX
        } else {
            (1u32 << bits) - 1
        }
    }

    #[must_use]
    const fn mask(&self) -> u32 {
        Self::mask_for(self.bits)
    }

    #[must_use]
    pub const fn state(&self) -> u32 {
        self.state
    }

    #[must_use]
    pub const fn bits(&self) -> u32 {
        self.bits
    }

    /// Shift one bit out of the register, and return it.
    pub fn step(&mut self) -> u32 {
        let out = self.state & 1;
        self.state >>= 1;
        if out == 1 {
            self.state ^= self.taps;
        }
        self.state &= self.mask();
        out
    }

    /// Undo one step, returning the bit that had been shifted out.
    ///
    /// The high bit of the previous state is whatever makes the forward step
    /// reproduce the current one: if the tap mask was applied, the bit that
    /// came out was a one, and the register's top bit follows from the taps.
    pub fn step_back(&mut self) -> u32 {
        // The forward step ends with the fed-back bit in the top position when
        // the output was one, because the top tap of a maximal-length mask is
        // always set.
        let top = 1u32 << (self.bits - 1);
        let out = u32::from(self.state & top != 0);
        if out == 1 {
            self.state ^= self.taps;
        }
        self.state = ((self.state << 1) | out) & self.mask();
        out
    }

    /// The next `count` bits, oldest first, leaving the register advanced.
    pub fn take(&mut self, count: usize) -> u32 {
        let mut word = 0u32;
        for index in 0..count.min(32) {
            word |= self.step() << index;
        }
        word
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// djmanzo's own 20-bit parameters, verified maximal by
    /// `crate::format`'s tests and by `the_twenty_bit_register_is_maximal`
    /// below.
    ///
    /// Deliberately *not* the numbers that circulate for the Serato record:
    /// those cycle after 43,307 states here, and `TimecodeFormat::is_usable`
    /// refuses them for that reason.
    const BITS: u32 = 20;
    const SEED: u32 = 1;
    const TAPS: u32 = 0x0008_0004;

    #[test]
    fn a_zero_seed_is_refused() {
        assert!(Lfsr::new(20, 0, TAPS).is_none());
        // And a seed that masks down to zero is the same trap wearing a hat.
        assert!(Lfsr::new(4, 0xffff_0000, 0b1100).is_none());
    }

    #[test]
    fn an_impossible_width_is_refused() {
        assert!(Lfsr::new(0, 1, 1).is_none());
        assert!(Lfsr::new(1, 1, 1).is_none());
        assert!(Lfsr::new(33, 1, 1).is_none());
        assert!(Lfsr::new(32, 1, 1).is_some());
    }

    /// **Stepping back undoes stepping forward.** Without this a record played
    /// backwards loses its position, which is every scratch.
    #[test]
    fn stepping_back_undoes_stepping_forward() {
        let mut lfsr = Lfsr::new(BITS, SEED, TAPS).unwrap();
        let start = lfsr.state();

        let mut forward = Vec::new();
        for _ in 0..1000 {
            forward.push(lfsr.step());
        }
        for expected in forward.into_iter().rev() {
            assert_eq!(lfsr.step_back(), expected, "a bit came back wrong");
        }
        assert_eq!(lfsr.state(), start, "the register did not return home");
    }

    /// **Every position is a different state.** This is the property the whole
    /// scheme rests on: if two points on the record produced the same twenty
    /// bits, the needle could not be located from them.
    ///
    /// A maximal-length register visits all `2^n - 1` non-zero states before
    /// repeating. Checked exhaustively at a small width, where "exhaustively"
    /// is cheap; the same argument holds at twenty.
    #[test]
    fn a_maximal_register_visits_every_state_once() {
        // x^5 + x^3 + 1 in this Galois arrangement.
        let taps = 0b10100;
        let mut lfsr = Lfsr::new(5, 1, taps).unwrap();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..31 {
            assert!(seen.insert(lfsr.state()), "a state repeated early");
            lfsr.step();
        }
        assert_eq!(seen.len(), 31, "not every state was visited");
        assert_eq!(lfsr.state(), 1, "the sequence did not close the loop");
    }

    /// The twenty-bit register the records use is maximal too — 1,048,575
    /// states, which at a thousand bits a second is over seventeen minutes of
    /// record before any position could be mistaken for another.
    #[test]
    fn the_twenty_bit_register_is_maximal() {
        let mut lfsr = Lfsr::new(BITS, SEED, TAPS).unwrap();
        let start = lfsr.state();
        let period = (1u32 << BITS) - 1;

        for step in 1..period {
            lfsr.step();
            assert_ne!(
                lfsr.state(),
                start,
                "the sequence repeated after {step} steps, not {period}"
            );
        }
        lfsr.step();
        assert_eq!(
            lfsr.state(),
            start,
            "the sequence did not close after {period}"
        );
    }

    /// `take` is `step` in a loop, and must agree with it.
    #[test]
    fn taking_a_word_matches_stepping_bit_by_bit() {
        let mut a = Lfsr::new(BITS, SEED, TAPS).unwrap();
        let mut b = a;
        let word = a.take(20);
        for index in 0..20 {
            assert_eq!((word >> index) & 1, b.step(), "bit {index} disagreed");
        }
        assert_eq!(a.state(), b.state());
    }
}
