//! §106: a turntable with a control record on it, for a machine with neither.
//!
//! [`crate::Synth`] writes the signal a control record carries at one steady
//! speed, which is what the round-trip tests in [`crate::decode`] need and
//! nothing like what a DJ does to a record. This is the stand-in of the kind
//! §106 asks for, beside the room surface's drawn camera and generated
//! microphone: what a real deck **puts out** when a hand is on it — playing,
//! stopping, spun back, scratched, lifted and dropped somewhere else — through
//! the imperfections a real deck adds (wow and flutter from the platter's
//! speed, rumble from the bearing, hiss and mains hum from the cable and the
//! preamp, a level that is never full scale) — with the truth beside every
//! frame, so a decoder fed it in audio-callback-sized blocks can be held to
//! what the hand actually did.
//!
//! # What this cannot tell you
//!
//! Whether a real pressing, a real cartridge and a real preamp produce
//! anything like this. The impairments are textbook figures for a working
//! deck, chosen here, not measured from one; a stylus skating across a
//! groove, a warped record, a cartridge's own resonance and the RIAA curve
//! of a phono stage are not modelled at all. A decoder that follows this
//! follows a deck as this module imagines one.

use crate::Synth;

/// What the hand does, one stretch at a time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Move {
    /// Play at `speed` (1.0 is the record's own speed) for `seconds`.
    Play { seconds: f64, speed: f64 },
    /// Change speed smoothly from `from` to `to` over `seconds` — the platter
    /// starting, braking, or a back-spin's slowing.
    Ramp { seconds: f64, from: f64, to: f64 },
    /// Back and forth, the speed swinging `depth` either side of nought,
    /// `rate` times a second: a baby scratch.
    Scratch { seconds: f64, depth: f64, rate: f64 },
    /// The needle up for `seconds`: no record, only the cable's hum and hiss.
    Lift { seconds: f64 },
    /// The needle put down `at` seconds into the record.
    Drop { at: f64 },
}

impl Move {
    /// How long it lasts; a drop is instant.
    #[must_use]
    pub const fn seconds(&self) -> f64 {
        match *self {
            Self::Play { seconds, .. }
            | Self::Ramp { seconds, .. }
            | Self::Scratch { seconds, .. }
            | Self::Lift { seconds } => seconds,
            Self::Drop { .. } => 0.0,
        }
    }

    /// The speed the hand asks for, `t` seconds into this move.
    #[must_use]
    pub fn speed_at(&self, t: f64) -> f64 {
        match *self {
            Self::Play { speed, .. } => speed,
            Self::Ramp { seconds, from, to } => from + (to - from) * (t / seconds),
            Self::Scratch { depth, rate, .. } => depth * (std::f64::consts::TAU * rate * t).sin(),
            Self::Lift { .. } | Self::Drop { .. } => 0.0,
        }
    }
}

/// The deck's own imperfections, as fractions and amplitudes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Deck {
    /// The record's peak level at the input, 1.0 being full scale.
    pub level: f32,
    /// The right channel's level against the left's.
    pub balance: f32,
    /// Slow speed wobble from the platter, as a fraction (0.001 is 0.1 %).
    pub wow: f64,
    /// Fast speed wobble from the drive, as a fraction.
    pub flutter: f64,
    /// Low rumble from the bearing, as an amplitude.
    pub rumble: f32,
    /// Broadband hiss, as an amplitude.
    pub hiss: f32,
    /// Mains hum at 50 Hz, as an amplitude.
    pub hum: f32,
    pub seed: u32,
}

impl Deck {
    /// A clean signal at full scale: what [`crate::Synth`] alone gives.
    pub const CLEAN: Self = Self {
        level: 1.0,
        balance: 1.0,
        wow: 0.0,
        flutter: 0.0,
        rumble: 0.0,
        hiss: 0.0,
        hum: 0.0,
        seed: 1,
    };

    /// A working deck through a line-level preamp, by textbook figures:
    /// peaks at −6 dBFS with the channels 1 dB apart, wow 0.1 %, flutter
    /// 0.05 %, rumble at −40 dB, hiss at −60 dB and hum at −56 dB.
    pub const WORKING: Self = Self {
        level: 0.5,
        balance: 0.89,
        wow: 0.001,
        flutter: 0.0005,
        rumble: 0.01,
        hiss: 0.001,
        hum: 0.0016,
        seed: 7,
    };
}

/// What was true at one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Truth {
    /// The record's speed, 1.0 being normal, negative backwards.
    pub speed: f64,
    /// Where the needle is, in seconds at normal speed; `None` while lifted.
    pub position: Option<f64>,
}

/// A turntable, playing a control record under a scripted hand.
#[derive(Debug, Clone)]
pub struct Turntable {
    synth: Synth,
    deck: Deck,
    sample_rate: f64,
}

/// A small seeded generator, so a failure reproduces exactly.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        // xorshift32
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        #[allow(clippy::cast_precision_loss)]
        let unit = x as f32 / u32::MAX as f32;
        unit * 2.0 - 1.0
    }
}

impl Turntable {
    #[must_use]
    pub fn new(synth: Synth, deck: Deck, sample_rate: f64) -> Self {
        Self {
            synth,
            deck,
            sample_rate,
        }
    }

    /// Play `moves` from `start` seconds into the record: interleaved stereo,
    /// and the truth at every frame.
    #[must_use]
    pub fn perform(&self, start: f64, moves: &[Move]) -> (Vec<f32>, Vec<Truth>) {
        let carrier = self.synth.format().carrier_hz;
        let rate = self.sample_rate;
        let deck = self.deck;
        let mut noise = Noise(deck.seed.max(1));
        let mut rumble = 0.0_f32;
        let mut cycles = start * carrier;
        let mut audio = Vec::new();
        let mut truth = Vec::new();
        let mut frame = 0_u64;

        for step in moves {
            if let Move::Drop { at } = *step {
                cycles = at * carrier;
                continue;
            }
            let seconds = step.seconds();
            let on_record = !matches!(step, Move::Lift { .. });
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let frames = (seconds * rate).round() as u64;
            for n in 0..frames {
                #[allow(clippy::cast_precision_loss)]
                let t = n as f64 / rate;
                #[allow(clippy::cast_precision_loss)]
                let clock = frame as f64 / rate;
                let hand = step.speed_at(t);
                // The platter's wobble rides on whatever the hand asks for.
                let wobble = 1.0
                    + deck.wow * (std::f64::consts::TAU * 0.55 * clock).sin()
                    + deck.flutter * (std::f64::consts::TAU * 6.0 * clock).sin();
                let speed = if on_record { hand * wobble } else { 0.0 };
                let [left, right] = if on_record {
                    self.synth.frame_at(cycles)
                } else {
                    [0.0, 0.0]
                };
                // Rumble: noise through a one-pole low-pass near 20 Hz.
                rumble += (noise.next() - rumble) * 0.0026;
                #[allow(clippy::cast_possible_truncation)]
                let hum = deck.hum * (std::f64::consts::TAU * 50.0 * clock).sin() as f32;
                let bed = deck.rumble * rumble * 20.0 + hum;
                audio.push(left * deck.level + bed + deck.hiss * noise.next());
                audio.push(right * deck.level * deck.balance + bed + deck.hiss * noise.next());
                truth.push(Truth {
                    speed,
                    position: on_record.then_some(cycles / carrier),
                });
                if on_record {
                    cycles += carrier * speed / rate;
                }
                frame += 1;
            }
        }
        (audio, truth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Decoder, Reading, TimecodeFormat};

    const SR: f64 = 48_000.0;
    /// An audio callback's worth of frames.
    const BLOCK: usize = 256;

    fn format() -> TimecodeFormat {
        TimecodeFormat::bundled()[0].clone()
    }

    fn deck(deck: Deck) -> Turntable {
        Turntable::new(Synth::new(format(), SR).expect("a synth"), deck, SR)
    }

    /// Feed `audio` to a fresh decoder a block at a time, and pair each
    /// block's reading with the truth at the block's last frame.
    fn decode(audio: &[f32], truth: &[Truth]) -> Vec<(Reading, Truth)> {
        let mut decoder = Decoder::new(format(), SR).expect("a decoder");
        audio
            .chunks(BLOCK * 2)
            .enumerate()
            .map(|(i, block)| {
                let reading = decoder.feed(block);
                let last = ((i + 1) * BLOCK).min(truth.len()) - 1;
                (reading, truth[last])
            })
            .collect()
    }

    /// The blocks between `from` and `to` seconds.
    fn between(read: &[(Reading, Truth)], from: f64, to: f64) -> &[(Reading, Truth)] {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let at = |s: f64| ((s * SR) as usize / BLOCK).min(read.len());
        &read[at(from)..at(to)]
    }

    /// The worst gap between what the decoder said and what the hand did.
    fn worst(read: &[(Reading, Truth)]) -> (f64, f64, usize) {
        let mut speed = 0.0_f64;
        let mut place = 0.0_f64;
        let mut lost = 0;
        for (reading, truth) in read {
            speed = speed.max((reading.speed - truth.speed).abs());
            match (reading.position, truth.position) {
                (Some(said), Some(was)) => place = place.max((said - was).abs()),
                (None, Some(_)) => lost += 1,
                _ => {}
            }
        }
        (speed, place, lost)
    }

    /// A set's worth of hand on one record: playing, braked to a stop,
    /// scratched, lifted, and dropped two and a half minutes further on.
    const SET: [Move; 7] = [
        Move::Play {
            seconds: 2.0,
            speed: 1.0,
        },
        Move::Ramp {
            seconds: 0.5,
            from: 1.0,
            to: 0.0,
        },
        Move::Play {
            seconds: 0.5,
            speed: 0.0,
        },
        Move::Scratch {
            seconds: 1.0,
            depth: 1.5,
            rate: 2.0,
        },
        Move::Lift { seconds: 0.5 },
        Move::Drop { at: 200.0 },
        Move::Play {
            seconds: 1.0,
            speed: 1.0,
        },
    ];

    /// **The load-bearing one: a working deck is followed through a set.**
    ///
    /// Peaks at −6 dBFS, the channels a decibel apart, wow, flutter, rumble,
    /// hiss and hum, read a callback's worth at a time. While it plays the
    /// speed is right to three percent and the place to ten milliseconds,
    /// with no block lost once the needle has settled; braked to a halt it
    /// reads as halted; dropped somewhere else it is found there. **Before
    /// this stand-in existed the decoder found no position at all on this
    /// deck** — its threshold for a one was fixed at full scale — and read a
    /// stopped platter as still turning at seven percent.
    #[test]
    fn a_working_deck_is_followed_through_play_a_stop_and_a_needle_drop() {
        let (audio, truth) = deck(Deck::WORKING).perform(30.0, &SET);
        let read = decode(&audio, &truth);

        let (speed, place, lost) = worst(between(&read, 0.2, 2.0));
        assert!(speed < 0.03, "playing: speed off by {speed}");
        assert!(place < 0.01, "playing: place off by {place} s");
        assert_eq!(lost, 0, "playing: the place was lost");

        let (speed, _, _) = worst(between(&read, 2.7, 3.0));
        assert!(speed < 0.02, "stopped: still reading {speed}");

        let (speed, place, lost) = worst(between(&read, 4.7, 5.5));
        assert!(speed < 0.03, "after the drop: speed off by {speed}");
        assert!(place < 0.01, "after the drop: place off by {place} s");
        assert_eq!(lost, 0, "after the drop: the place was not found");
    }

    /// **A record below full scale still finds its place**, from full scale
    /// down to −12 dBFS — where a preamp is commonly set.
    #[test]
    fn a_record_below_full_scale_finds_its_place() {
        for level in [1.0, 0.5, 0.25] {
            let quiet = Deck {
                level,
                ..Deck::CLEAN
            };
            let (audio, truth) = deck(quiet).perform(
                90.0,
                &[Move::Play {
                    seconds: 1.0,
                    speed: 1.0,
                }],
            );
            let read = decode(&audio, &truth);
            let (_, place, lost) = worst(between(&read, 0.1, 1.0));
            assert_eq!(lost, 0, "at {level} the place was never found");
            assert!(place < 0.01, "at {level} the place is off by {place} s");
        }
    }

    /// **Never a wrong place.** Across the whole set on the working deck,
    /// every place reported is within fifty milliseconds of the needle, and
    /// none is reported while it is lifted: a decoder that sometimes jumped
    /// the playhead to another part of the record would be worse than one
    /// that sometimes said nothing.
    #[test]
    fn no_place_is_ever_reported_that_the_needle_is_not_at() {
        let (audio, truth) = deck(Deck::WORKING).perform(30.0, &SET);
        for (reading, truth) in decode(&audio, &truth) {
            match (reading.position, truth.position) {
                (Some(said), Some(was)) => {
                    assert!((said - was).abs() < 0.05, "said {said}, needle at {was}");
                }
                (Some(said), None) => panic!("said {said} with the needle up"),
                _ => {}
            }
        }
    }

    /// **Dropped where the record begins with zeros, the place is still
    /// right.** The first bits after a drop are judged against an envelope
    /// that has only just started; a run of zeros there, read before a one
    /// has set the envelope, would be read as ones — a wrong place, reported
    /// with confidence. Dropped at four such places on the record.
    #[test]
    fn a_drop_onto_a_run_of_zeros_is_not_misread() {
        let synth = Synth::new(format(), SR).expect("a synth");
        let runs: Vec<u32> = (10_000..2_000_000u32)
            .filter(|i| (0..6).all(|k| synth.bit_at(i + k) == 0) && synth.bit_at(i - 1) == 1)
            .step_by(997)
            .take(4)
            .collect();
        assert_eq!(runs.len(), 4, "the record has runs of zeros to drop onto");
        for at in runs {
            let start = f64::from(at) / format().carrier_hz;
            let (audio, truth) = deck(Deck::WORKING).perform(
                start,
                &[
                    Move::Lift { seconds: 0.2 },
                    Move::Drop { at: start },
                    Move::Play {
                        seconds: 0.4,
                        speed: 1.0,
                    },
                ],
            );
            for (reading, truth) in decode(&audio, &truth) {
                if let (Some(said), Some(was)) = (reading.position, truth.position) {
                    assert!(
                        (said - was).abs() < 0.05,
                        "dropped at bit {at}: said {said}, needle at {was}"
                    );
                }
            }
        }
    }

    /// **A hand stopping the platter reads as stopped within a twentieth of
    /// a second**, not after the smoothing has drifted down: the playhead must
    /// stop when the record does.
    #[test]
    fn a_platter_stopped_by_hand_reads_as_stopped_at_once() {
        let (audio, truth) = deck(Deck::CLEAN).perform(
            30.0,
            &[
                Move::Play {
                    seconds: 1.0,
                    speed: 1.0,
                },
                Move::Play {
                    seconds: 0.5,
                    speed: 0.0,
                },
            ],
        );
        let read = decode(&audio, &truth);
        let (speed, _, _) = worst(between(&read, 1.05, 1.5));
        assert!(speed < 0.03, "50 ms after the hand stopped it: {speed}");
    }

    /// **Too quiet to read is said as no record, not misread.** Below about
    /// −20 dBFS the zeros fall under the decoder's floor for silence, and a
    /// record there needs more gain; what matters is that it reports nothing
    /// rather than a wrong place.
    #[test]
    fn a_record_too_quiet_to_read_reports_no_place() {
        let faint = Deck {
            level: 0.08,
            ..Deck::CLEAN
        };
        let (audio, truth) = deck(faint).perform(
            90.0,
            &[Move::Play {
                seconds: 1.0,
                speed: 1.0,
            }],
        );
        assert!(
            decode(&audio, &truth)
                .iter()
                .all(|(reading, _)| reading.position.is_none())
        );
    }

    /// **A scratch is followed both ways.** Wherever the hand is moving the
    /// record faster than a third of normal, the decoder says which way.
    #[test]
    fn a_scratch_is_followed_both_ways() {
        let (audio, truth) = deck(Deck::WORKING).perform(
            60.0,
            &[Move::Scratch {
                seconds: 2.0,
                depth: 1.5,
                rate: 2.0,
            }],
        );
        let read = decode(&audio, &truth);
        let moving: Vec<_> = read
            .iter()
            .filter(|(_, truth)| truth.speed.abs() > 0.33)
            .collect();
        let agree = moving
            .iter()
            .filter(|(reading, truth)| reading.speed.signum() == truth.speed.signum())
            .count();
        #[allow(clippy::cast_precision_loss)]
        let share = agree as f64 / moving.len() as f64;
        assert!(share > 0.95, "the direction was right {share} of the time");
        let (speed, _, _) = worst(&read[read.len() / 8..]);
        assert!(speed < 0.3, "the scratch's speed was off by {speed}");
    }
}
