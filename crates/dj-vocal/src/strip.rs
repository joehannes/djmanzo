//! One microphone's strip, and the chain behind it.
//!
//! ```text
//!   input ─→ high-pass ─→ gate ─→ EQ ─→ compressor ─→ de-esser ─→ echo ─→ reverb ─→ sends
//! ```
//!
//! In that order (docs/KARAOKE.md §6), every stage bypassable, and each
//! strip's its own: two singers sharing a stage do not want the same reverb,
//! and a guest who shouts wants a harder compressor than one who whispers.
//!
//! # What an idle strip costs
//!
//! Nothing. A strip that is closed, or whose voice has been gone for longer
//! than its echo and reverb take to die away, is not processed: the tail is
//! let finish first, because cutting a reverb short is audible, and then the
//! chain stops. An open strip that has gone quiet still reads its input — one
//! comparison a sample — so the first word wakes it.

use crate::dynamics::{Compressor, DeEsser, Gate, db_to_linear};
use crate::space::{Echo, Reverb};
use dj_dsp::{Biquad, Ducker, PeakMeter, SmoothedValue};
use serde::{Deserialize, Serialize};

/// Everything a DJ sets on a strip. Plain numbers, so a singer's settings can
/// be kept and put back — and `Copy`, so they cross the engine's command
/// queue by value and leave nothing behind for the audio thread to free.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StripSettings {
    /// What the strip is for — the preset last chosen on it.
    pub preset: Preset,
    /// Whether the microphone is live.
    pub open: bool,
    /// The fader, in dB.
    pub gain_db: f32,
    /// Where the voice sits, −1 left to 1 right.
    pub pan: f32,
    /// Whether this voice pulls the music down when it speaks. The MC's
    /// microphone, yes; a singer's, no — talkover under a singer would pull
    /// the backing track down every time they sang.
    pub talkover: bool,
    /// How much of the voice goes to the room, the DJ's headphones and the
    /// singers' monitor, each 0 to 1.
    pub to_main: f32,
    pub to_cue: f32,
    pub to_monitor: f32,
    /// Below this the voice is cut, in Hz; 0 turns the high-pass off.
    pub high_pass_hz: f32,
    pub gate: Option<GateSettings>,
    pub eq: Option<EqSettings>,
    pub compressor: Option<CompressorSettings>,
    pub de_esser: Option<DeEsserSettings>,
    pub echo: Option<EchoSettings>,
    pub reverb: Option<ReverbSettings>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GateSettings {
    pub threshold_db: f32,
    pub range_db: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EqSettings {
    pub low_db: f32,
    pub mid_db: f32,
    pub mid_hz: f32,
    pub high_db: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CompressorSettings {
    pub threshold_db: f32,
    pub ratio: f32,
    pub makeup_db: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DeEsserSettings {
    pub frequency_hz: f32,
    pub threshold_db: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EchoSettings {
    pub delay_ms: f32,
    pub feedback: f32,
    /// How much of the echo is heard, 0 to 1.
    pub level: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReverbSettings {
    pub seconds: f32,
    /// How much of the room is heard, 0 to 1.
    pub level: f32,
}

impl Default for StripSettings {
    /// A singer's microphone, closed: a high-pass for handling noise, a gate,
    /// a compressor, a de-esser and a little room; no echo; no talkover; to
    /// the room and the singers' monitor, not the DJ's headphones.
    fn default() -> Self {
        Self {
            preset: Preset::Singer,
            open: false,
            gain_db: 0.0,
            pan: 0.0,
            talkover: false,
            to_main: 1.0,
            to_cue: 0.0,
            to_monitor: 1.0,
            high_pass_hz: 100.0,
            gate: Some(GateSettings {
                threshold_db: Gate::DEFAULT_THRESHOLD_DB,
                range_db: Gate::DEFAULT_RANGE_DB,
            }),
            eq: None,
            compressor: Some(CompressorSettings {
                threshold_db: Compressor::DEFAULT_THRESHOLD_DB,
                ratio: Compressor::DEFAULT_RATIO,
                makeup_db: 3.0,
            }),
            de_esser: Some(DeEsserSettings {
                frequency_hz: DeEsser::DEFAULT_FREQUENCY_HZ,
                threshold_db: DeEsser::DEFAULT_THRESHOLD_DB,
            }),
            echo: None,
            reverb: Some(ReverbSettings {
                seconds: Reverb::DEFAULT_SECONDS,
                level: 0.18,
            }),
        }
    }
}

/// What a strip is for (docs/KARAOKE.md §6): named for the job, not for
/// what is switched on, and one tap on the strip's row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Preset {
    /// A singer: a high-pass for handling noise, a gate, a compressor, a
    /// de-esser and a little room. What every strip starts as.
    Singer,
    /// A singer who holds back: the gate opens lower and the compressor
    /// brings more of them up.
    SoftSinger,
    /// A singer who shouts: a harder compressor, the gate opens higher.
    LoudSinger,
    /// The host's microphone: talkover on, so the music drops under
    /// announcements, and no room on the voice.
    Mc,
    /// A guitar or a keyboard: no gate and no de-esser to chop it up.
    Instrument,
}

impl Preset {
    pub const ALL: [Preset; 5] = [
        Preset::Singer,
        Preset::SoftSinger,
        Preset::LoudSinger,
        Preset::Mc,
        Preset::Instrument,
    ];

    /// The name a host reads on the row.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Preset::Singer => "Singer",
            Preset::SoftSinger => "Soft singer",
            Preset::LoudSinger => "Loud singer",
            Preset::Mc => "MC",
            Preset::Instrument => "Instrument",
        }
    }

    /// `settings` made what this preset is for. What the host set by hand on
    /// the row — open or closed, the fader, where the voice sits and where
    /// it is sent — is kept; the chain is the preset's.
    #[must_use]
    pub fn applied_to(self, settings: &StripSettings) -> StripSettings {
        let singer = StripSettings::default();
        let chain = match self {
            Preset::Singer => singer,
            Preset::SoftSinger => StripSettings {
                gate: Some(GateSettings {
                    threshold_db: -52.0,
                    range_db: Gate::DEFAULT_RANGE_DB,
                }),
                compressor: Some(CompressorSettings {
                    threshold_db: -28.0,
                    ratio: 3.0,
                    makeup_db: 8.0,
                }),
                ..singer
            },
            Preset::LoudSinger => StripSettings {
                gate: Some(GateSettings {
                    threshold_db: -38.0,
                    range_db: Gate::DEFAULT_RANGE_DB,
                }),
                compressor: Some(CompressorSettings {
                    threshold_db: -16.0,
                    ratio: 6.0,
                    makeup_db: 0.0,
                }),
                ..singer
            },
            Preset::Mc => StripSettings {
                talkover: true,
                reverb: None,
                echo: None,
                ..singer
            },
            Preset::Instrument => StripSettings {
                high_pass_hz: 40.0,
                gate: None,
                de_esser: None,
                compressor: Some(CompressorSettings {
                    threshold_db: -18.0,
                    ratio: 2.0,
                    makeup_db: 0.0,
                }),
                reverb: None,
                ..singer
            },
        };
        StripSettings {
            preset: self,
            open: settings.open,
            gain_db: settings.gain_db,
            pan: settings.pan,
            to_main: settings.to_main,
            to_cue: settings.to_cue,
            to_monitor: settings.to_monitor,
            ..chain
        }
    }
}

/// What one strip gives each bus for one frame, left and right.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StripFrame {
    pub main: [f32; 2],
    pub cue: [f32; 2],
    pub monitor: [f32; 2],
    /// What the music should be multiplied by for this strip's sake: 1 unless
    /// its talkover is on and it is speaking.
    pub music_gain: f32,
}

impl StripFrame {
    const SILENT: Self = Self {
        main: [0.0; 2],
        cue: [0.0; 2],
        monitor: [0.0; 2],
        music_gain: 1.0,
    };
}

/// A microphone's strip.
#[derive(Debug)]
pub struct Strip {
    settings: StripSettings,
    sample_rate: f32,
    fader: SmoothedValue,
    high_pass: Biquad,
    gate: Gate,
    low: Biquad,
    mid: Biquad,
    high: Biquad,
    compressor: Compressor,
    de_esser: DeEsser,
    echo: Echo,
    reverb: Reverb,
    ducker: Ducker,
    meter: PeakMeter,
    /// Frames since a voice last came through. Counts the tail down.
    since_voice: usize,
    /// How long the tail is, in frames: the echo's and the reverb's, the
    /// longer, and a little over.
    tail: usize,
    /// Frames the chain has actually run, for the tests and the cost.
    processed: u64,
}

/// Quieter than this, in the chain's input, is no voice.
const VOICE: f32 = 1e-4;

impl Strip {
    /// Everything sized here, once: the echo's and the reverb's buffers for
    /// their longest settings, so no setting ever allocates.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let mut strip = Self {
            settings: StripSettings::default(),
            sample_rate,
            fader: SmoothedValue::new(0.0, sample_rate),
            high_pass: Biquad::default(),
            gate: Gate::new(sample_rate),
            low: Biquad::default(),
            mid: Biquad::default(),
            high: Biquad::default(),
            compressor: Compressor::new(sample_rate),
            de_esser: DeEsser::new(sample_rate),
            echo: Echo::new(sample_rate),
            reverb: Reverb::new(sample_rate),
            ducker: Ducker::new(sample_rate),
            meter: PeakMeter::new(sample_rate),
            since_voice: usize::MAX,
            tail: 0,
            processed: 0,
        };
        strip.apply(&StripSettings::default());
        strip
    }

    /// Take a whole set of settings. Arithmetic only, no allocation: fit to
    /// call on the audio thread when a DJ moves a control.
    pub fn apply(&mut self, settings: &StripSettings) {
        let rate = self.sample_rate;
        self.fader.set_target(if settings.open {
            db_to_linear(settings.gain_db.clamp(-60.0, 12.0))
        } else {
            0.0
        });
        if settings.high_pass_hz > 0.0 {
            self.high_pass.set_coefficients_from(&Biquad::high_pass(
                rate,
                settings.high_pass_hz.clamp(20.0, 400.0),
                std::f32::consts::FRAC_1_SQRT_2,
            ));
        }
        if let Some(gate) = settings.gate {
            self.gate.set(gate.threshold_db, gate.range_db);
        }
        if let Some(eq) = settings.eq {
            self.low
                .set_coefficients_from(&Biquad::low_shelf(rate, 200.0, eq.low_db));
            self.mid.set_coefficients_from(&Biquad::peaking(
                rate,
                eq.mid_hz.clamp(200.0, 8_000.0),
                1.0,
                eq.mid_db,
            ));
            self.high
                .set_coefficients_from(&Biquad::high_shelf(rate, 6_000.0, eq.high_db));
        }
        if let Some(compressor) = settings.compressor {
            self.compressor.set(
                compressor.threshold_db,
                compressor.ratio,
                compressor.makeup_db,
            );
        }
        if let Some(de_esser) = settings.de_esser {
            self.de_esser
                .set(de_esser.frequency_hz, de_esser.threshold_db);
        }
        if let Some(echo) = settings.echo {
            self.echo.set(echo.delay_ms, echo.feedback);
        }
        if let Some(reverb) = settings.reverb {
            self.reverb.set(reverb.seconds);
        }
        self.ducker.set_enabled(settings.talkover);
        let echo = settings.echo.map_or(0, |_| self.echo.tail_frames());
        let reverb = settings.reverb.map_or(0, |_| self.reverb.tail_frames());
        // A tenth of a second over, for the filters and the gate to settle.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let settle = (0.1 * rate) as usize;
        self.tail = echo.max(reverb) + settle;
        self.settings = *settings;
    }

    #[must_use]
    pub fn settings(&self) -> &StripSettings {
        &self.settings
    }

    /// One sample of the microphone in; what it gives each bus out.
    pub fn process(&mut self, input: f32) -> StripFrame {
        let fader = self.fader.next_value();
        let live = input * fader;
        // A voice is what would open the gate — the room's hum under it is
        // not, or an open microphone in a loud room would never rest.
        let heard = if self.settings.gate.is_some() {
            self.gate.opens_at(live)
        } else {
            live.abs() > VOICE
        };
        if heard {
            self.since_voice = 0;
        } else {
            self.since_voice = self.since_voice.saturating_add(1);
        }
        if self.is_idle() {
            // Still told there is nothing, so a duck lets the music back up.
            let music_gain = self.ducker.process_frame(0.0);
            return StripFrame {
                music_gain,
                ..StripFrame::SILENT
            };
        }
        self.processed += 1;
        let settings = &self.settings;
        let mut voice = live;
        if settings.high_pass_hz > 0.0 {
            voice = self.high_pass.process(voice);
        }
        if settings.gate.is_some() {
            voice = self.gate.process(voice);
        }
        if settings.eq.is_some() {
            voice = self.high.process(self.mid.process(self.low.process(voice)));
        }
        if settings.compressor.is_some() {
            voice = self.compressor.process(voice);
        }
        if settings.de_esser.is_some() {
            voice = self.de_esser.process(voice);
        }
        // The talkover listens to the voice before its space: a reverb tail
        // is not somebody speaking.
        let music_gain = self.ducker.process_frame(voice);
        self.meter.process(&[voice]);

        let (left_gain, right_gain) = pan(settings.pan);
        let mut left = voice * left_gain;
        let mut right = voice * right_gain;
        if let Some(echo) = settings.echo {
            let repeat = self.echo.process(voice) * echo.level;
            left += repeat * left_gain;
            right += repeat * right_gain;
        }
        if let Some(reverb) = settings.reverb {
            let (l, r) = self.reverb.process(voice);
            left += l * reverb.level;
            right += r * reverb.level;
        }
        StripFrame {
            main: [left * settings.to_main, right * settings.to_main],
            cue: [left * settings.to_cue, right * settings.to_cue],
            monitor: [left * settings.to_monitor, right * settings.to_monitor],
            music_gain,
        }
    }

    /// Whether the chain has stopped: closed or silent, with its tail done.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.since_voice > self.tail && (self.fader.is_settled() || !self.settings.open)
    }

    /// How loud the voice is, after the chain, 0 to 1.
    #[must_use]
    pub fn level(&self) -> f32 {
        self.meter.peak()
    }

    /// How far the compressor is turning the voice down, in dB.
    #[must_use]
    pub fn compressing_db(&self) -> f32 {
        if self.settings.compressor.is_some() {
            self.compressor.reduction_db()
        } else {
            0.0
        }
    }

    /// Whether the gate is letting a voice through.
    #[must_use]
    pub fn gate_open(&self) -> bool {
        self.settings.gate.is_none() || self.gate.is_open()
    }

    /// Frames the chain has run since the strip was made.
    #[must_use]
    pub fn processed_frames(&self) -> u64 {
        self.processed
    }
}

/// Constant-power pan: the voice is as loud in the middle as at either side.
fn pan(position: f32) -> (f32, f32) {
    let angle = (position.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (angle.cos(), angle.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn voice(n: usize) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let t = n as f32 / RATE;
        (std::f32::consts::TAU * 220.0 * t).sin() * 0.3
    }

    fn open() -> StripSettings {
        StripSettings {
            open: true,
            ..StripSettings::default()
        }
    }

    /// **A closed microphone is silent, and costs nothing once it is.**
    #[test]
    fn a_closed_strip_is_silent_and_does_no_work() {
        let mut strip = Strip::new(RATE);
        for n in 0..48_000 {
            let frame = strip.process(voice(n));
            assert_eq!(frame.main, [0.0, 0.0]);
            assert!((frame.music_gain - 1.0).abs() < 1e-6);
        }
        assert_eq!(strip.processed_frames(), 0);
    }

    /// The hum of a room, sixty decibels down: under any gate's threshold.
    fn room(n: usize) -> f32 {
        voice(n) * 0.003
    }

    /// **The tail is let finish; then the chain stops; the first word wakes
    /// it.** A voice, then only the room: the reverb still sounds after the
    /// voice stops, the chain stops once the tail is done — the room's hum
    /// does not keep an open microphone working — and a new phrase is heard
    /// from its first sample.
    #[test]
    fn a_silent_strip_lets_its_tail_finish_then_stops_until_the_next_word() {
        let mut strip = Strip::new(RATE);
        strip.apply(&open());
        for n in 0..24_000 {
            strip.process(voice(n));
        }
        // Just after the voice: the room is still answering.
        let mut tail = 0.0f32;
        for n in 0..4_800 {
            tail = tail.max(strip.process(room(n)).main[0].abs());
        }
        assert!(tail > 1e-3, "the reverb was cut off ({tail})");
        assert!(!strip.is_idle());
        // The rest of the reverb's time, and a fifth of a second over.
        let reverb = ((Reverb::DEFAULT_SECONDS + 0.2) * RATE) as usize;
        for n in 0..reverb {
            strip.process(room(n));
        }
        assert!(strip.is_idle());
        let stopped_at = strip.processed_frames();
        for n in 0..48_000 {
            assert_eq!(strip.process(room(n)).main, [0.0, 0.0]);
        }
        assert_eq!(strip.processed_frames(), stopped_at, "an idle strip ran");
        let woken = strip.process(voice(12));
        assert!(!strip.is_idle());
        assert_eq!(strip.processed_frames(), stopped_at + 1);
        assert!(woken.main[0] != 0.0 || woken.main[1] != 0.0 || strip.gate_open());
    }

    /// **A preset changes the chain and keeps the host's hands.** Choosing
    /// MC on an open strip at −6 dB panned left keeps it open, at −6 dB,
    /// left — and turns talkover on and the room off; choosing Singer again
    /// gives the room back and talkover up, and every preset has a name.
    #[test]
    fn a_preset_is_the_chain_and_leaves_the_hosts_settings_alone() {
        let set = StripSettings {
            open: true,
            gain_db: -6.0,
            pan: -0.5,
            to_cue: 0.3,
            ..StripSettings::default()
        };
        let mc = Preset::Mc.applied_to(&set);
        assert_eq!(mc.preset, Preset::Mc);
        assert!(mc.open && mc.talkover && mc.reverb.is_none());
        assert!((mc.gain_db + 6.0).abs() < f32::EPSILON && (mc.pan + 0.5).abs() < f32::EPSILON);
        assert!((mc.to_cue - 0.3).abs() < f32::EPSILON);
        let back = Preset::Singer.applied_to(&mc);
        assert!(!back.talkover && back.reverb.is_some() && back.open);
        let instrument = Preset::Instrument.applied_to(&set);
        assert!(instrument.gate.is_none() && instrument.de_esser.is_none());
        let loud = Preset::LoudSinger.applied_to(&set).compressor.unwrap();
        let soft = Preset::SoftSinger.applied_to(&set).compressor.unwrap();
        assert!(loud.ratio > soft.ratio && soft.makeup_db > loud.makeup_db);
        assert!(Preset::ALL.iter().all(|preset| !preset.name().is_empty()));
    }

    /// **A singer's microphone leaves the music alone; the MC's pulls it
    /// down.** Talkover is per strip.
    #[test]
    fn talkover_is_the_strips_own() {
        let mut singer = Strip::new(RATE);
        singer.apply(&open());
        let mut mc = Strip::new(RATE);
        mc.apply(&StripSettings {
            talkover: true,
            ..open()
        });
        let (mut sung, mut spoken) = (1.0f32, 1.0f32);
        for n in 0..24_000 {
            sung = sung.min(singer.process(voice(n)).music_gain);
            spoken = spoken.min(mc.process(voice(n)).music_gain);
        }
        assert!(
            (sung - 1.0).abs() < 1e-6,
            "a singer ducked the backing track"
        );
        assert!(spoken < 0.5, "the MC did not duck the music ({spoken})");
    }

    /// **Each bus gets its own send.** A strip sent to the room and the
    /// singers' monitor but not the DJ's headphones gives the headphones
    /// nothing; and the pan keeps the voice as loud in the middle as at a side.
    #[test]
    fn each_bus_takes_its_own_send_and_the_pan_keeps_the_power() {
        let mut strip = Strip::new(RATE);
        strip.apply(&StripSettings {
            to_main: 1.0,
            to_cue: 0.0,
            to_monitor: 0.5,
            reverb: None,
            ..open()
        });
        let mut last = StripFrame::default();
        for n in 0..24_000 {
            last = strip.process(voice(n));
        }
        assert_eq!(last.cue, [0.0, 0.0]);
        assert!((last.monitor[0] - last.main[0] * 0.5).abs() < 1e-6);
        for position in [-1.0f32, -0.3, 0.0, 0.6, 1.0] {
            let (l, r) = pan(position);
            assert!((l * l + r * r - 1.0).abs() < 1e-5);
        }
        assert!(pan(-1.0).1.abs() < 1e-6 && pan(1.0).0.abs() < 1e-6);
    }
}
