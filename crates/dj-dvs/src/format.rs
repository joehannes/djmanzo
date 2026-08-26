//! What a particular control record is, as data.
//!
//! Held as configuration rather than as code because these are **facts about a
//! pressed disc** — a carrier frequency and a shift register's parameters — and
//! because new pressings appear. A DJ with a record djmanzo has not heard of
//! should be able to add a line, not wait for a release.

use serde::{Deserialize, Serialize};

/// The parameters of one kind of control record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimecodeFormat {
    /// What to call it in the interface.
    pub name: String,
    /// Carrier frequency at normal playback speed, in hertz.
    ///
    /// This sets the resolution: one cycle carries one bit, so a 1 kHz carrier
    /// locates the needle to a millisecond of record.
    pub carrier_hz: f64,
    /// Width of the shift register, in bits.
    ///
    /// Also how many consecutive bits must be read before a position is known:
    /// a register of `n` bits has `2^n - 1` states and every run of `n` bits is
    /// unique within the sequence.
    pub bits: u32,
    /// The register's state at the start of the record.
    pub seed: u32,
    /// Feedback taps.
    pub taps: u32,
}

impl TimecodeFormat {
    /// How many bits the record carries before the sequence would repeat.
    #[must_use]
    pub const fn period(&self) -> u32 {
        if self.bits >= 32 {
            u32::MAX
        } else {
            (1u32 << self.bits) - 1
        }
    }

    /// How long the record is before a position could be mistaken for another,
    /// in seconds at normal speed.
    #[must_use]
    pub fn unambiguous_seconds(&self) -> f64 {
        if self.carrier_hz <= 0.0 {
            return 0.0;
        }
        f64::from(self.period()) / self.carrier_hz
    }

    /// Whether this describes a record that could actually work.
    ///
    /// **Maximality is checked, not assumed.** A tap set that is not maximal
    /// gives a register that repeats early, and a register that repeats early
    /// gives two points on the record the same position — silently, with no
    /// error anywhere, showing up only as the playhead jumping backwards
    /// mid-set.
    ///
    /// That is not hypothetical. The tap value circulating in prose for the
    /// Serato record cycles after 43,307 states in every convention tried
    /// here, which at a 1 kHz carrier is a position that repeats every
    /// forty-three seconds. Whether the number is wrong or the convention is,
    /// the check is what caught it.
    ///
    /// Walking a 20-bit register is a million steps — microseconds, once, when
    /// a format is loaded, and never on the audio path.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        if self.carrier_hz <= 0.0 || !(2..=24).contains(&self.bits) {
            return false;
        }
        let Some(mut lfsr) = crate::Lfsr::new(self.bits, self.seed, self.taps) else {
            return false;
        };
        let start = lfsr.state();
        let period = self.period();
        for step in 1..=period {
            lfsr.step();
            if lfsr.state() == start {
                return step == period;
            }
        }
        false
    }

    /// The formats djmanzo ships.
    ///
    /// # Why there is no Serato or Traktor entry here
    ///
    /// Because nobody here has put a needle on one. The parameters that
    /// circulate in prose could not be confirmed — the widely-quoted tap value
    /// for the Serato record is not maximal in any convention tried, and a
    /// non-maximal register repeats its positions every forty-three seconds.
    /// Shipping it would mean a DVS that looks like it works and puts the
    /// playhead in the wrong place twice a minute.
    ///
    /// Adding a vendor record is a **file**, not a release — see
    /// [`TimecodeFormat`] — and [`Self::is_usable`] refuses one whose numbers
    /// cannot work. When someone with a real record and a turntable confirms a
    /// set of parameters, they belong here.
    ///
    /// What is here is djmanzo's own timecode, whose parameters are verified
    /// maximal by the tests below. It is not a second-class option: with
    /// [`crate::Synth`] a DJ can generate the signal, put it on a CD or a
    /// phone, and control djmanzo from any turntable or CD deck without buying
    /// anything.
    #[must_use]
    pub fn bundled() -> Vec<TimecodeFormat> {
        vec![
            TimecodeFormat {
                name: "djmanzo 1 kHz".to_owned(),
                carrier_hz: 1000.0,
                bits: 20,
                seed: 1,
                taps: 0x0008_0004,
            },
            TimecodeFormat {
                name: "djmanzo 2 kHz (finer, shorter)".to_owned(),
                carrier_hz: 2000.0,
                bits: 23,
                seed: 1,
                taps: 0x0040_0010,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_format_could_actually_work() {
        for format in TimecodeFormat::bundled() {
            assert!(format.is_usable(), "{} is not a usable format", format.name);
        }
    }

    /// A bundled format must outlast the record it is pressed on. If it did
    /// not, two points on the disc would decode to the same position.
    #[test]
    fn a_bundled_format_outlasts_the_record_it_is_pressed_on() {
        for format in TimecodeFormat::bundled() {
            let minutes = format.unambiguous_seconds() / 60.0;
            assert!(
                minutes > 10.0,
                "{} repeats after {minutes:.1} minutes, which is inside a record side",
                format.name
            );
        }
    }

    /// **The check that caught a real mistake.**
    ///
    /// The tap value circulating in prose for the Serato record is not
    /// maximal: it cycles after 43,307 states, which at 1 kHz is a position
    /// that repeats every forty-three seconds. Whether the published number is
    /// wrong or the convention behind it is something this project has not
    /// reproduced, the effect of shipping it would have been the same — a DVS
    /// that looks like it works and puts the playhead in the wrong place twice
    /// a minute.
    ///
    /// Kept as a test so that nobody pastes it back in.
    #[test]
    fn the_widely_quoted_serato_taps_are_refused() {
        let quoted = TimecodeFormat {
            name: "quoted Serato parameters".to_owned(),
            carrier_hz: 1000.0,
            bits: 20,
            seed: 0x0005_9017,
            taps: 0x0003_61e4,
        };
        assert!(
            !quoted.is_usable(),
            "a register that repeats every 43 seconds was accepted as a record"
        );
    }

    /// A tap set that is merely *plausible* is not enough, and this is the
    /// difference between checking maximality and checking the seed is
    /// non-zero.
    #[test]
    fn a_non_maximal_tap_set_is_refused() {
        // 4-bit maximal is 0b1100; 0b1010 is not.
        let good = TimecodeFormat {
            name: "four bit".to_owned(),
            carrier_hz: 1000.0,
            bits: 4,
            seed: 1,
            taps: 0b1100,
        };
        assert!(
            good.is_usable(),
            "the known-maximal four-bit set was refused"
        );
        assert!(
            !TimecodeFormat {
                taps: 0b1010,
                ..good
            }
            .is_usable(),
            "a short-cycling four-bit set was accepted"
        );
    }

    /// The failures a hand-typed format actually has.
    #[test]
    fn a_broken_format_is_refused() {
        let good = TimecodeFormat {
            name: "test".to_owned(),
            carrier_hz: 1000.0,
            bits: 20,
            seed: 1,
            taps: 0x0008_0004,
        };
        assert!(good.is_usable());

        assert!(
            !TimecodeFormat {
                seed: 0,
                ..good.clone()
            }
            .is_usable(),
            "a zero seed is the one state a register cannot leave"
        );
        assert!(
            !TimecodeFormat {
                carrier_hz: 0.0,
                ..good.clone()
            }
            .is_usable(),
            "a carrier at zero is not a tone"
        );
        assert!(
            !TimecodeFormat {
                carrier_hz: -1000.0,
                ..good.clone()
            }
            .is_usable()
        );
        assert!(
            !TimecodeFormat {
                bits: 0,
                ..good.clone()
            }
            .is_usable()
        );
        assert!(!TimecodeFormat { bits: 33, ..good }.is_usable());
    }

    /// It is configuration, so it has to survive being written down and read
    /// back — that is the whole point of a DJ being able to add a pressing.
    #[test]
    fn a_format_survives_a_round_trip_through_its_file() {
        for format in TimecodeFormat::bundled() {
            let text = toml::to_string(&format).expect("a format should serialise");
            let back: TimecodeFormat = toml::from_str(&text).expect("a format should read back");
            assert_eq!(back, format, "{} did not survive its own file", format.name);
        }
    }
}
