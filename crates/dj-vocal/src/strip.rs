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

// What each stage starts at when the host switches it on: the stage's own
// defaults, which are also a singer's ([`StripSettings::default`]).

impl Default for GateSettings {
    fn default() -> Self {
        Self {
            threshold_db: Gate::DEFAULT_THRESHOLD_DB,
            range_db: Gate::DEFAULT_RANGE_DB,
        }
    }
}

impl Default for EqSettings {
    /// Flat, with the middle band where a voice's body is.
    fn default() -> Self {
        Self {
            low_db: 0.0,
            mid_db: 0.0,
            mid_hz: 1_000.0,
            high_db: 0.0,
        }
    }
}

impl Default for CompressorSettings {
    fn default() -> Self {
        Self {
            threshold_db: Compressor::DEFAULT_THRESHOLD_DB,
            ratio: Compressor::DEFAULT_RATIO,
            makeup_db: 3.0,
        }
    }
}

impl Default for DeEsserSettings {
    fn default() -> Self {
        Self {
            frequency_hz: DeEsser::DEFAULT_FREQUENCY_HZ,
            threshold_db: DeEsser::DEFAULT_THRESHOLD_DB,
        }
    }
}

impl Default for EchoSettings {
    fn default() -> Self {
        Self {
            delay_ms: Echo::DEFAULT_DELAY_MS,
            feedback: Echo::DEFAULT_FEEDBACK,
            level: 0.25,
        }
    }
}

impl Default for ReverbSettings {
    fn default() -> Self {
        Self {
            seconds: Reverb::DEFAULT_SECONDS,
            level: 0.18,
        }
    }
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
            gate: Some(GateSettings::default()),
            eq: None,
            compressor: Some(CompressorSettings::default()),
            de_esser: Some(DeEsserSettings::default()),
            echo: None,
            reverb: Some(ReverbSettings::default()),
        }
    }
}

/// The least and the most of every number a strip takes.
///
/// One table for both sides of the screen: the host holds settings to it
/// before keeping or sending them ([`StripSettings::held`]), and the
/// interface draws its controls over exactly these ranges ([`LIMITS`]), so
/// neither decides alone what a voice may be put through.
pub mod range {
    /// The fader, dB.
    pub const GAIN_DB: (f32, f32) = (-60.0, 12.0);
    pub const PAN: (f32, f32) = (-1.0, 1.0);
    /// Any send, 0 to 1.
    pub const SEND: (f32, f32) = (0.0, 1.0);
    /// 0 is off; anything else is at least 20 Hz.
    pub const HIGH_PASS_HZ: (f32, f32) = (0.0, 400.0);
    pub const GATE_THRESHOLD_DB: (f32, f32) = (-80.0, -10.0);
    pub const GATE_RANGE_DB: (f32, f32) = (0.0, 80.0);
    /// Any EQ band's cut or boost, dB.
    pub const EQ_DB: (f32, f32) = (-12.0, 12.0);
    pub const EQ_MID_HZ: (f32, f32) = (200.0, 8_000.0);
    pub const COMPRESSOR_THRESHOLD_DB: (f32, f32) = (-50.0, 0.0);
    /// 1 is no compression at all.
    pub const COMPRESSOR_RATIO: (f32, f32) = (1.0, 20.0);
    pub const COMPRESSOR_MAKEUP_DB: (f32, f32) = (0.0, 24.0);
    pub const DE_ESSER_HZ: (f32, f32) = (2_000.0, 12_000.0);
    pub const DE_ESSER_THRESHOLD_DB: (f32, f32) = (-60.0, 0.0);
    /// Up to the longest echo a strip is built for.
    pub const ECHO_DELAY_MS: (f32, f32) = (20.0, crate::LONGEST_ECHO_SECONDS * 1_000.0);
    /// Short of 1, where the repeats would never die away.
    pub const ECHO_FEEDBACK: (f32, f32) = (0.0, 0.9);
    /// How much of an echo or a room is heard, 0 to 1.
    pub const LEVEL: (f32, f32) = (0.0, 1.0);
    /// Up to the longest room a strip is built for.
    pub const REVERB_SECONDS: (f32, f32) = (0.2, crate::LONGEST_REVERB_SECONDS);
    /// The lowest a high-pass that is on goes.
    pub(crate) const HIGH_PASS_LOWEST_HZ: f32 = 20.0;
}

/// Every number a strip takes, by the name the interface knows it by — a
/// stage's numbers as `stage.number` — with its range from [`range`].
pub const LIMITS: [(&str, (f32, f32)); 22] = [
    ("gain_db", range::GAIN_DB),
    ("pan", range::PAN),
    ("to_main", range::SEND),
    ("to_cue", range::SEND),
    ("to_monitor", range::SEND),
    ("high_pass_hz", range::HIGH_PASS_HZ),
    ("gate.threshold_db", range::GATE_THRESHOLD_DB),
    ("gate.range_db", range::GATE_RANGE_DB),
    ("eq.low_db", range::EQ_DB),
    ("eq.mid_db", range::EQ_DB),
    ("eq.mid_hz", range::EQ_MID_HZ),
    ("eq.high_db", range::EQ_DB),
    ("compressor.threshold_db", range::COMPRESSOR_THRESHOLD_DB),
    ("compressor.ratio", range::COMPRESSOR_RATIO),
    ("compressor.makeup_db", range::COMPRESSOR_MAKEUP_DB),
    ("de_esser.frequency_hz", range::DE_ESSER_HZ),
    ("de_esser.threshold_db", range::DE_ESSER_THRESHOLD_DB),
    ("echo.delay_ms", range::ECHO_DELAY_MS),
    ("echo.feedback", range::ECHO_FEEDBACK),
    ("echo.level", range::LEVEL),
    ("reverb.seconds", range::REVERB_SECONDS),
    ("reverb.level", range::LEVEL),
];

/// `value` inside `(min, max)`; `fallback` when it is not a number at all.
fn hold(value: f32, (min, max): (f32, f32), fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

impl StripSettings {
    /// A singer's strip with every stage on, each at what it starts at when
    /// the host switches it on — what the interface turns a stage on to, so
    /// that a stage's starting point is Rust's, not the screen's.
    #[must_use]
    pub fn every_stage() -> Self {
        Self {
            eq: Some(EqSettings::default()),
            echo: Some(EchoSettings::default()),
            ..Self::default()
        }
    }

    /// These settings held to [`LIMITS`]: every number inside its range, and
    /// one that is not a number at all put back to its default. What the host
    /// keeps and what the engine is sent, whatever the screen — or a hand-
    /// edited `vocal.json` — asked for: a makeup gain of +200 dB is not a
    /// setting, it is a speaker.
    #[must_use]
    pub fn held(&self) -> Self {
        let singer = Self::default();
        let high_pass_hz = match hold(self.high_pass_hz, range::HIGH_PASS_HZ, singer.high_pass_hz) {
            hz if hz <= 0.0 => 0.0,
            hz => hz.max(range::HIGH_PASS_LOWEST_HZ),
        };
        Self {
            preset: self.preset,
            open: self.open,
            gain_db: hold(self.gain_db, range::GAIN_DB, singer.gain_db),
            pan: hold(self.pan, range::PAN, singer.pan),
            talkover: self.talkover,
            to_main: hold(self.to_main, range::SEND, singer.to_main),
            to_cue: hold(self.to_cue, range::SEND, singer.to_cue),
            to_monitor: hold(self.to_monitor, range::SEND, singer.to_monitor),
            high_pass_hz,
            gate: self.gate.map(|gate| {
                let start = GateSettings::default();
                GateSettings {
                    threshold_db: hold(
                        gate.threshold_db,
                        range::GATE_THRESHOLD_DB,
                        start.threshold_db,
                    ),
                    range_db: hold(gate.range_db, range::GATE_RANGE_DB, start.range_db),
                }
            }),
            eq: self.eq.map(|eq| {
                let start = EqSettings::default();
                EqSettings {
                    low_db: hold(eq.low_db, range::EQ_DB, start.low_db),
                    mid_db: hold(eq.mid_db, range::EQ_DB, start.mid_db),
                    mid_hz: hold(eq.mid_hz, range::EQ_MID_HZ, start.mid_hz),
                    high_db: hold(eq.high_db, range::EQ_DB, start.high_db),
                }
            }),
            compressor: self.compressor.map(|compressor| {
                let start = CompressorSettings::default();
                CompressorSettings {
                    threshold_db: hold(
                        compressor.threshold_db,
                        range::COMPRESSOR_THRESHOLD_DB,
                        start.threshold_db,
                    ),
                    ratio: hold(compressor.ratio, range::COMPRESSOR_RATIO, start.ratio),
                    makeup_db: hold(
                        compressor.makeup_db,
                        range::COMPRESSOR_MAKEUP_DB,
                        start.makeup_db,
                    ),
                }
            }),
            de_esser: self.de_esser.map(|de_esser| {
                let start = DeEsserSettings::default();
                DeEsserSettings {
                    frequency_hz: hold(
                        de_esser.frequency_hz,
                        range::DE_ESSER_HZ,
                        start.frequency_hz,
                    ),
                    threshold_db: hold(
                        de_esser.threshold_db,
                        range::DE_ESSER_THRESHOLD_DB,
                        start.threshold_db,
                    ),
                }
            }),
            echo: self.echo.map(|echo| {
                let start = EchoSettings::default();
                EchoSettings {
                    delay_ms: hold(echo.delay_ms, range::ECHO_DELAY_MS, start.delay_ms),
                    feedback: hold(echo.feedback, range::ECHO_FEEDBACK, start.feedback),
                    level: hold(echo.level, range::LEVEL, start.level),
                }
            }),
            reverb: self.reverb.map(|reverb| {
                let start = ReverbSettings::default();
                ReverbSettings {
                    seconds: hold(reverb.seconds, range::REVERB_SECONDS, start.seconds),
                    level: hold(reverb.level, range::LEVEL, start.level),
                }
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
            db_to_linear(settings.gain_db.clamp(range::GAIN_DB.0, range::GAIN_DB.1))
        } else {
            0.0
        });
        if settings.high_pass_hz > 0.0 {
            self.high_pass.set_coefficients_from(&Biquad::high_pass(
                rate,
                settings
                    .high_pass_hz
                    .clamp(range::HIGH_PASS_LOWEST_HZ, range::HIGH_PASS_HZ.1),
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
                eq.mid_hz.clamp(range::EQ_MID_HZ.0, range::EQ_MID_HZ.1),
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
    ///
    /// Nothing once the strip is idle: the chain has stopped, and the meter
    /// with it, so what it last read is no longer true.
    #[must_use]
    pub fn level(&self) -> f32 {
        if self.is_idle() {
            0.0
        } else {
            self.meter.peak()
        }
    }

    /// How far the compressor is turning the voice down, in dB. Nothing once
    /// the strip is idle, for the meter's reason.
    #[must_use]
    pub fn compressing_db(&self) -> f32 {
        if self.settings.compressor.is_some() && !self.is_idle() {
            self.compressor.reduction_db()
        } else {
            0.0
        }
    }

    /// Whether the gate is letting a voice through. Never on an idle strip:
    /// nothing is coming through anything.
    #[must_use]
    pub fn gate_open(&self) -> bool {
        !self.is_idle() && (self.settings.gate.is_none() || self.gate.is_open())
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

    /// Every number in `settings`, by its `LIMITS` name.
    fn numbers(settings: &StripSettings) -> Vec<(String, f64)> {
        fn walk(prefix: &str, value: &serde_json::Value, into: &mut Vec<(String, f64)>) {
            match value {
                serde_json::Value::Number(n) => {
                    into.push((prefix.to_owned(), n.as_f64().unwrap_or(f64::NAN)));
                }
                serde_json::Value::Object(map) => {
                    for (key, inner) in map {
                        let name = if prefix.is_empty() {
                            key.clone()
                        } else {
                            format!("{prefix}.{key}")
                        };
                        walk(&name, inner, into);
                    }
                }
                _ => {}
            }
        }
        let mut found = Vec::new();
        walk(
            "",
            &serde_json::to_value(settings).expect("settings serialize"),
            &mut found,
        );
        found.sort_by(|a, b| a.0.cmp(&b.0));
        found
    }

    /// **Every number a strip takes has a range, and the table names nothing
    /// else.** Read off the settings themselves — a strip with every stage
    /// on, as it is serialized for the screen — so a number added to a stage
    /// without a range fails here instead of reaching a slider unbounded.
    #[test]
    fn every_number_a_strip_takes_has_a_limit() {
        let found: Vec<String> = numbers(&StripSettings::every_stage())
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        let mut limited: Vec<String> = LIMITS.iter().map(|(name, _)| (*name).to_owned()).collect();
        limited.sort();
        assert_eq!(found, limited);
        for (name, (min, max)) in LIMITS {
            assert!(min < max, "{name}: {min} to {max}");
        }
    }

    /// **Held settings are inside their ranges, and what was inside is left
    /// alone.** Every preset, and every stage at what it starts at, comes
    /// back from `held` unchanged; a strip with every number far past its
    /// range comes back with every number at the edge of it; and a strip of
    /// numbers that are not numbers comes back at the defaults.
    #[test]
    fn held_settings_stay_inside_the_limits() {
        for preset in Preset::ALL {
            let settings = preset.applied_to(&StripSettings::every_stage());
            assert_eq!(settings.held(), settings, "{}", preset.name());
        }
        let start = StripSettings::every_stage();
        assert_eq!(start.held(), start);

        let everything = |value: f32| StripSettings {
            gain_db: value,
            pan: value,
            to_main: value,
            to_cue: value,
            to_monitor: value,
            high_pass_hz: value,
            gate: Some(GateSettings {
                threshold_db: value,
                range_db: value,
            }),
            eq: Some(EqSettings {
                low_db: value,
                mid_db: value,
                mid_hz: value,
                high_db: value,
            }),
            compressor: Some(CompressorSettings {
                threshold_db: value,
                ratio: value,
                makeup_db: value,
            }),
            de_esser: Some(DeEsserSettings {
                frequency_hz: value,
                threshold_db: value,
            }),
            echo: Some(EchoSettings {
                delay_ms: value,
                feedback: value,
                level: value,
            }),
            reverb: Some(ReverbSettings {
                seconds: value,
                level: value,
            }),
            ..StripSettings::default()
        };
        let limit = |name: &str| {
            LIMITS
                .iter()
                .find(|(limited, _)| *limited == name)
                .map(|(_, range)| *range)
                .expect("a limit")
        };
        for (name, held) in numbers(&everything(1.0e6).held()) {
            let (_, max) = limit(&name);
            assert!(
                (held - f64::from(max)).abs() < 1e-3,
                "{name}: {held}, not {max}"
            );
        }
        for (name, held) in numbers(&everything(-1.0e6).held()) {
            let (min, _) = limit(&name);
            assert!(
                (held - f64::from(min)).abs() < 1e-3,
                "{name}: {held}, not {min}"
            );
        }
        for wild in [f32::NAN, f32::INFINITY] {
            assert_eq!(numbers(&everything(wild).held()), numbers(&start));
        }
        // A high-pass that is on is never under 20 Hz.
        let low = StripSettings {
            high_pass_hz: 5.0,
            ..start
        };
        assert!((low.held().high_pass_hz - 20.0).abs() < f32::EPSILON);
    }

    /// **A strip that has stopped reads as stopped.** An MC's strip — no
    /// room, so almost no tail — speaks hard enough to be compressed, then
    /// falls silent: once it is idle its level is nothing, its compressor is
    /// doing nothing and no gate is letting anything through. The chain has
    /// stopped running, so its meters stopped with it; left alone, they would
    /// hold the last word's reading on the host's screen for as long as the
    /// MC stayed quiet.
    #[test]
    fn a_strip_that_has_stopped_reads_as_stopped() {
        let mut strip = Strip::new(RATE);
        strip.apply(&Preset::Mc.applied_to(&open()));
        for n in 0..24_000 {
            strip.process(voice(n) * 3.0);
        }
        assert!(
            strip.level() > 0.1,
            "the MC was not heard ({})",
            strip.level()
        );
        assert!(strip.compressing_db() > 0.5, "the compressor never worked");
        for _ in 0..RATE as usize {
            strip.process(0.0);
        }
        assert!(strip.is_idle(), "a silent MC's strip kept running");
        assert!(
            strip.level() < f32::EPSILON,
            "the level held ({})",
            strip.level()
        );
        assert!(strip.compressing_db() < f32::EPSILON);
        assert!(!strip.gate_open());
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
