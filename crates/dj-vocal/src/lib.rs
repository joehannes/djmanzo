//! K3: the singers' microphones.
//!
//! One strip per input the interface has, each with a full vocal chain of its
//! own (`strip`), summed into three buses — the room, the DJ's headphones and
//! the singers' monitor — with the gain the music should take for any strip
//! whose talkover is on. The design, and the owner's decisions behind it, are
//! in `docs/KARAOKE.md` §6 and `docs/ROADMAP.md` (K3).
//!
//! # Where it sits
//!
//! A crate of its own, depending on `dj-dsp` and never on `dj-engine`. The
//! engine holds a [`Vocals`] the way it holds the microphone strip today and
//! asks it for one frame at a time; everything here is sized when the
//! interface opens — the strips to its input count — and nothing allocates
//! after that (`tests/realtime.rs` counts).
//!
//! # One ring, every input
//!
//! The host opens every input the interface has as one stream, and hands the
//! engine the reading end of one ring of interleaved frames. [`Vocals`] splits
//! them: channel *n* is strip *n*. One stream and one ring because the
//! microphones and the music are on one interface with one clock — two
//! devices are two clocks, and two clocks drift.
//!
//! # What it costs
//!
//! N chains cost N times one, kept in check by idle strips costing nothing
//! (see `strip`) and by saying what a chain costs on this machine:
//! [`measure_chain`] runs one with every stage on, off the audio thread, and
//! reports the share of a processor core it took, so the interface can say
//! *this many singers is too much for this machine* before the room hears it.

mod dynamics;
mod space;
mod strip;

pub use dynamics::{Compressor, DeEsser, Gate};
pub use space::{Echo, LONGEST_ECHO_SECONDS, LONGEST_REVERB_SECONDS, Reverb};
pub use strip::{
    CompressorSettings, DeEsserSettings, EchoSettings, EqSettings, GateSettings, ReverbSettings,
    Strip, StripFrame, StripSettings,
};

/// The most microphones djmanzo will run: the input count of a large
/// interface. A strip is a few hundred kilobytes, most of it the reverb's and
/// the echo's buffers.
pub const MOST_STRIPS: usize = 16;

/// Every bus's sum for one frame, left and right, and the music's gain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VocalFrame {
    pub main: [f32; 2],
    pub cue: [f32; 2],
    pub monitor: [f32; 2],
    /// The gain for everything that is not a voice: the lowest any
    /// talking-over strip asks for, 1 when none is.
    pub music_gain: f32,
}

impl VocalFrame {
    /// No voices, and the music left alone: what a rig with no microphones
    /// gives.
    pub const SILENT: Self = Self {
        main: [0.0; 2],
        cue: [0.0; 2],
        monitor: [0.0; 2],
        music_gain: 1.0,
    };
}

/// The singers' microphones.
#[derive(Debug)]
pub struct Vocals {
    strips: Vec<Strip>,
    /// How many channels each frame of the ring carries: the interface's
    /// input count, which may be more than there are strips.
    width: usize,
    input: Option<rtrb::Consumer<f32>>,
    /// Frames the ring could not supply.
    starved: u64,
}

impl Vocals {
    /// One strip per input channel, up to [`MOST_STRIPS`]; an interface with
    /// more inputs than that has its first ones used and the rest read and
    /// let go, so every frame is still taken whole. Called when the interface
    /// opens, never on the audio thread.
    #[must_use]
    pub fn new(sample_rate: f32, inputs: usize) -> Self {
        Self {
            strips: (0..inputs.min(MOST_STRIPS))
                .map(|_| Strip::new(sample_rate))
                .collect(),
            width: inputs,
            input: None,
            starved: 0,
        }
    }

    /// How many strips there are.
    #[must_use]
    pub fn inputs(&self) -> usize {
        self.strips.len()
    }

    /// Install the input ring, handing back whatever was there — handed back,
    /// not dropped, because dropping a ring's end can free it, and freeing is
    /// not for the audio thread.
    pub fn set_input(&mut self, input: Option<rtrb::Consumer<f32>>) -> Option<rtrb::Consumer<f32>> {
        std::mem::replace(&mut self.input, input)
    }

    #[must_use]
    pub fn strip(&self, index: usize) -> Option<&Strip> {
        self.strips.get(index)
    }

    #[must_use]
    pub fn strip_mut(&mut self, index: usize) -> Option<&mut Strip> {
        self.strips.get_mut(index)
    }

    /// How many strips are working now: open, or still sounding a tail.
    #[must_use]
    pub fn working(&self) -> usize {
        self.strips.iter().filter(|strip| !strip.is_idle()).count()
    }

    /// Frames the ring could not supply: a real fault — the input is not
    /// keeping up — with a real fix, a larger buffer.
    #[must_use]
    pub fn starved_frames(&self) -> u64 {
        self.starved
    }

    /// One frame from every microphone, through its chain, onto the buses.
    ///
    /// Always called, whether any strip is open or not, because the ring has
    /// to be drained either way: a ring left to fill while every microphone
    /// was closed would deliver seconds of stale room the moment one opened.
    pub fn next_frame(&mut self) -> VocalFrame {
        let mut frame = VocalFrame::SILENT;
        let width = self.width;
        let whole = self
            .input
            .as_ref()
            .is_some_and(|input| input.slots() >= width);
        if self.input.is_some() && !whole && width > 0 {
            self.starved += 1;
        }
        for strip in &mut self.strips {
            // All of a frame or none of it: taking part of one would put every
            // later frame's channels on the wrong strips.
            let sample = if whole {
                self.input
                    .as_mut()
                    .and_then(|input| input.pop().ok())
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            let out = strip.process(sample);
            for side in 0..2 {
                frame.main[side] += out.main[side];
                frame.cue[side] += out.cue[side];
                frame.monitor[side] += out.monitor[side];
            }
            frame.music_gain = frame.music_gain.min(out.music_gain);
        }
        // Inputs beyond the last strip: read, so the next frame starts where
        // it should, and let go.
        if whole && let Some(input) = self.input.as_mut() {
            for _ in self.strips.len()..width {
                let _ = input.pop();
            }
        }
        frame
    }
}

/// The share of one processor core a strip with every stage on takes, on this
/// machine: one second of a voice-like signal through it, timed. Never on the
/// audio thread — it takes as long as it measures.
#[must_use]
pub fn measure_chain(sample_rate: f32) -> f64 {
    let mut strip = Strip::new(sample_rate);
    strip.apply(&StripSettings {
        open: true,
        eq: Some(EqSettings {
            low_db: -2.0,
            mid_db: 2.0,
            mid_hz: 2_500.0,
            high_db: 1.0,
        }),
        echo: Some(EchoSettings {
            delay_ms: Echo::DEFAULT_DELAY_MS,
            feedback: Echo::DEFAULT_FEEDBACK,
            level: 0.3,
        }),
        ..StripSettings::default()
    });
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let frames = sample_rate as usize;
    let mut seed = 0x2545_f491_u32;
    let started = std::time::Instant::now();
    let mut keep = 0.0f32;
    for n in 0..frames {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        #[allow(clippy::cast_precision_loss)]
        let noise = (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5;
        #[allow(clippy::cast_precision_loss)]
        let t = n as f32 / sample_rate;
        let sample = (std::f32::consts::TAU * 180.0 * t).sin() * 0.3 + noise * 0.05;
        keep += strip.process(sample).main[0];
    }
    let elapsed = started.elapsed().as_secs_f64();
    // Kept, so the work cannot be optimised away.
    std::hint::black_box(keep);
    elapsed
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn singer() -> StripSettings {
        StripSettings {
            open: true,
            reverb: None,
            compressor: None,
            de_esser: None,
            ..StripSettings::default()
        }
    }

    /// **Channel n is strip n.** Two microphones on one ring: a voice on the
    /// first and nothing on the second comes out of the first strip's pan
    /// position only — here hard left — so the channels were not swapped
    /// or smeared.
    #[test]
    fn each_channel_of_the_ring_is_its_own_strip() {
        let mut vocals = Vocals::new(RATE, 2);
        vocals.strip_mut(0).unwrap().apply(&StripSettings {
            pan: -1.0,
            ..singer()
        });
        vocals.strip_mut(1).unwrap().apply(&StripSettings {
            pan: 1.0,
            ..singer()
        });
        let (mut producer, consumer) = rtrb::RingBuffer::new(48_000 * 2);
        for n in 0..24_000 {
            #[allow(clippy::cast_precision_loss)]
            let first = (std::f32::consts::TAU * 220.0 * n as f32 / RATE).sin() * 0.3;
            producer.push(first).unwrap();
            producer.push(0.0).unwrap();
        }
        assert!(vocals.set_input(Some(consumer)).is_none());
        let (mut left, mut right) = (0.0f32, 0.0f32);
        for _ in 0..24_000 {
            let frame = vocals.next_frame();
            left = left.max(frame.main[0].abs());
            right = right.max(frame.main[1].abs());
        }
        assert!(left > 0.1, "the first microphone was lost ({left})");
        assert!(
            right < 1e-6,
            "the first microphone reached the second's side ({right})"
        );
        assert_eq!(vocals.starved_frames(), 0);
    }

    /// **A ring that runs dry is silence and a count**, never half a frame:
    /// with one sample left of a two-channel frame, nothing is taken.
    #[test]
    fn a_ring_that_runs_dry_is_counted_and_never_half_read() {
        let mut vocals = Vocals::new(RATE, 2);
        let (mut producer, consumer) = rtrb::RingBuffer::new(8);
        producer.push(0.5).unwrap();
        vocals.set_input(Some(consumer));
        vocals.next_frame();
        vocals.next_frame();
        assert_eq!(vocals.starved_frames(), 2);
        // The lone sample is still there, for the frame it belongs to.
        producer.push(0.25).unwrap();
        let taken = vocals.input.as_ref().unwrap().slots();
        assert_eq!(taken, 2);
        vocals.next_frame();
        assert_eq!(vocals.input.as_ref().unwrap().slots(), 0);
        assert_eq!(vocals.starved_frames(), 2);
    }

    /// **An interface with more inputs than strips keeps its frames whole.**
    /// Eighteen inputs, sixteen strips: the last two channels of each frame
    /// are read and let go, so the next frame's first channel still lands on
    /// the first strip — a voice there is heard, and nothing leaks across.
    #[test]
    fn inputs_beyond_the_last_strip_are_read_and_let_go() {
        const WIDTH: usize = MOST_STRIPS + 2;
        let mut vocals = Vocals::new(RATE, WIDTH);
        assert_eq!(vocals.inputs(), MOST_STRIPS);
        vocals.strip_mut(0).unwrap().apply(&StripSettings {
            pan: -1.0,
            ..singer()
        });
        let (mut producer, consumer) = rtrb::RingBuffer::new(WIDTH * 24_000);
        for n in 0..12_000 {
            #[allow(clippy::cast_precision_loss)]
            let voice = (std::f32::consts::TAU * 220.0 * n as f32 / RATE).sin() * 0.3;
            producer.push(voice).unwrap();
            for _ in 1..MOST_STRIPS {
                producer.push(0.0).unwrap();
            }
            // The two inputs no strip is for: loud, and never to be heard.
            producer.push(0.9).unwrap();
            producer.push(0.9).unwrap();
        }
        vocals.set_input(Some(consumer));
        let (mut left, mut right) = (0.0f32, 0.0f32);
        for _ in 0..12_000 {
            let frame = vocals.next_frame();
            left = left.max(frame.main[0].abs());
            right = right.max(frame.main[1].abs());
        }
        assert!(left > 0.1 && left < 0.35, "the first strip heard {left}");
        assert!(right < 1e-6, "an input beyond the strips leaked ({right})");
        assert_eq!(vocals.starved_frames(), 0);
        assert_eq!(vocals.input.as_ref().unwrap().slots(), 0);
    }

    /// **The music follows the lowest talkover**: an MC speaking pulls it
    /// down even while a singer's strip, with talkover off, sings over it.
    #[test]
    fn the_music_takes_the_lowest_gain_any_talkover_asks_for() {
        let mut vocals = Vocals::new(RATE, 2);
        vocals.strip_mut(0).unwrap().apply(&singer());
        vocals.strip_mut(1).unwrap().apply(&StripSettings {
            talkover: true,
            ..singer()
        });
        let (mut producer, consumer) = rtrb::RingBuffer::new(48_000 * 2);
        for n in 0..24_000 {
            #[allow(clippy::cast_precision_loss)]
            let v = (std::f32::consts::TAU * 220.0 * n as f32 / RATE).sin() * 0.3;
            producer.push(v).unwrap();
            producer.push(v).unwrap();
        }
        vocals.set_input(Some(consumer));
        let mut lowest = 1.0f32;
        for _ in 0..24_000 {
            lowest = lowest.min(vocals.next_frame().music_gain);
        }
        assert!(lowest < 0.5, "the MC's talkover was lost ({lowest})");
    }

    /// **The cost is measured, and is a share of a core**: more than nothing,
    /// and on any machine that can run djmanzo, well under all of one.
    #[test]
    fn a_chains_cost_is_measured() {
        let cost = measure_chain(RATE);
        assert!(cost > 0.0);
        assert!(cost < 0.5, "one chain took {cost} of a core");
    }
}
