//! §34 / §36: what the floor is doing beyond light, movement and loudness.
//!
//! [`crate::room`] places three senses against the same room earlier tonight.
//! §34 lists more aggregate measurements than three, and §36 asks for each to
//! be **evidence, not a pretend-objective measurement**. This is the rest that
//! a camera and a microphone can honestly give:
//!
//! - **the room's sound in three bands** — low, mid and high, from the
//!   microphone's own spectrum;
//! - **applause-like clicks** — sharp broadband transients that do not fall
//!   on a grid, which is what clapping is and a hi-hat is not;
//! - **the room over the music** — how much louder the microphone is than the
//!   music djmanzo is sending explains, which is the one way to tell a crowd's
//!   noise from the record without knowing what a crowd sounds like;
//! - **uneven movement** — how much the picture's motion differs from one
//!   part of the frame to another (the variance of a coarse optical flow);
//! - **moving together** — how much of that motion goes the same way, §36's
//!   *cohesion*;
//! - **scene change** — the picture changing as a whole between one look and
//!   the next, §34's *scene change* and §36's *visual change*.
//!
//! # Each its own line, never a score
//!
//! Every signal is placed against **its own** night and said on **its own**
//! line with **its own** confidence ([`Line`]). Nothing here adds two signals
//! together, weights one against another, or names what they mean between
//! them: §36 forbids a crowd score in so many words, and a number made of
//! eight measurements nobody calibrated is a mood with decimals. A test holds
//! that each line speaks only of its own signal.
//!
//! # What none of it can tell
//!
//! Whether the room is enjoying itself. Every line is a comparison with the
//! same microphone and the same lens earlier the same night, and the
//! [`Signal::cannot`] sentence for each says what it is easily confused with.
//! None of it has been near a real room: the measurements were built against
//! drawn footage and generated sound (`ui/e2e/floor.spec.ts`), which proves
//! the path runs and which way the numbers move, and nothing else.

use crate::room::{Against, ENOUGH, NEAR, against};
use dj_core::Spread;
use std::collections::VecDeque;
use std::time::SystemTime;

/// How many looks over the night before a placement is more than a first
/// impression.
///
/// Eight times [`ENOUGH`]: four minutes at the interface's two-second cadence.
/// Before that a "higher than usual" is a comparison with a night that has
/// barely started, and the line says so.
pub const SETTLED: usize = ENOUGH * 8;

/// How many paired looks the room-over-music fit needs.
///
/// Two minutes of looking. A line through fewer points than that is drawn
/// through whichever record happened to be playing.
pub const FIT_ENOUGH: usize = 60;

/// How far the music's level must have moved before the fit is believed.
///
/// A variance on the 0..1 loudness scale; 0.0009 is a standard deviation of
/// about two decibels. A night held flat by the limiter gives the fit nothing
/// to learn from — every look is the same music level, and the room's share of
/// the sound cannot be told from the music's — so the line says *not yet*
/// rather than calling the microphone's loudness the crowd's.
pub const FIT_SPREAD: f64 = 0.0009;

/// One of the measurements, by what it measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// The microphone's level below 250 Hz.
    Low,
    /// Between 250 Hz and 2 kHz, where voices are.
    Mid,
    /// Above 2 kHz.
    High,
    /// Sharp broadband clicks off any grid.
    Applause,
    /// The microphone's level beyond what the music's level explains.
    OverMusic,
    /// How much the picture's motion differs across the frame.
    Uneven,
    /// How much of the picture's motion goes the same way.
    Together,
    /// How much the picture changed as a whole since the last look.
    Scene,
}

impl Signal {
    /// Every signal, in the order they are said.
    pub const ALL: [Self; 8] = [
        Self::Low,
        Self::Mid,
        Self::High,
        Self::Applause,
        Self::OverMusic,
        Self::Uneven,
        Self::Together,
        Self::Scene,
    ];

    /// The stable slug.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Mid => "mid",
            Self::High => "high",
            Self::Applause => "applause",
            Self::OverMusic => "over_music",
            Self::Uneven => "uneven",
            Self::Together => "together",
            Self::Scene => "scene",
        }
    }

    /// What it is called on its line.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Low => "Low band",
            Self::Mid => "Mid band",
            Self::High => "High band",
            Self::Applause => "Applause-like clicks",
            Self::OverMusic => "Room over the music",
            Self::Uneven => "Uneven movement",
            Self::Together => "Moving together",
            Self::Scene => "Scene change",
        }
    }

    /// How it is measured, in one sentence.
    #[must_use]
    pub const fn measures(self) -> &'static str {
        match self {
            Self::Low => "the microphone's level below 250 Hz",
            Self::Mid => "the microphone's level from 250 Hz to 2 kHz",
            Self::High => "the microphone's level above 2 kHz",
            Self::Applause => "sharp clicks in the sound that fall on no regular beat",
            Self::OverMusic => {
                "how much louder the microphone is than the music's own level explains"
            }
            Self::Uneven => "how differently the parts of the picture move",
            Self::Together => "how much of the picture's movement goes the same way",
            Self::Scene => {
                "how much the picture's brightness changed as a whole since the last look"
            }
        }
    }

    /// What it is easily confused with. Said beside the reading, because a
    /// measurement offered without its blind spot reads as a fact.
    #[must_use]
    pub const fn cannot(self) -> &'static str {
        match self {
            Self::Low => "mostly the bass from the speakers, whatever the room is doing",
            Self::Mid => "voices and the music's own vocal alike",
            Self::High => "cymbals in the record as much as anything in the room",
            Self::Applause => "a fast, loose percussion part can look the same",
            Self::OverMusic => "anything loud near the microphone counts, a shout in the booth too",
            Self::Uneven => "one person close to the lens moves more than a crowd far away",
            Self::Together => "a sweeping light makes the whole picture move one way too",
            Self::Scene => "the lighting rig changing looks like the room changing",
        }
    }

    /// Why there is nothing to place when the room has been looked at
    /// enough and this signal still has too few readings: the looks came, and
    /// this one had nothing to measure in them. Said rather than "keep
    /// looking", which would be wrong however long the DJ waited.
    #[must_use]
    pub const fn absent(self) -> &'static str {
        match self {
            Self::Low | Self::Mid | Self::High | Self::Applause | Self::OverMusic => {
                "nothing has been heard through a microphone"
            }
            Self::Uneven => "the picture has too little detail to follow",
            Self::Together => "too little of the picture has moved to say which way",
            Self::Scene => "there has been no picture to compare from one look to the next",
        }
    }

    const fn slot(self) -> usize {
        self as usize
    }
}

/// One look's measurements. Any may be missing: a microphone alone measures
/// no picture, and half a look is worth keeping.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    pub at: SystemTime,
    values: [Option<f32>; Signal::ALL.len()],
}

impl Look {
    #[must_use]
    pub const fn at(at: SystemTime) -> Self {
        Self {
            at,
            values: [None; Signal::ALL.len()],
        }
    }

    /// Clamped to 0..1 on the way in; a non-finite value is dropped.
    #[must_use]
    pub fn with(mut self, signal: Signal, value: f32) -> Self {
        if value.is_finite() {
            self.values[signal.slot()] = Some(value.clamp(0.0, 1.0));
        }
        self
    }

    #[must_use]
    pub const fn get(&self, signal: Signal) -> Option<f32> {
        self.values[signal.slot()]
    }

    fn is_empty(&self) -> bool {
        self.values.iter().all(Option::is_none)
    }
}

/// How far a line can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    /// Not enough to say anything.
    NotYet,
    /// Placed against a night that has only just started.
    Early,
    /// Placed against enough of the night to mean something.
    Settled,
}

impl Confidence {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NotYet => "not_yet",
            Self::Early => "early",
            Self::Settled => "settled",
        }
    }
}

/// One signal's line.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub signal: Signal,
    /// The middle of the last three minutes, on this signal's own 0..1 scale.
    pub now: Option<f32>,
    /// Where that sits in tonight, once it can be placed.
    pub against: Option<Against>,
    pub confidence: Confidence,
    /// How many looks in the last three minutes measured it.
    pub looks: usize,
    /// The line as it is said.
    pub says: String,
}

/// The room-over-music fit: the microphone's level against the music's, as a
/// straight line through the night, both on the 0..1 loudness scale (−60 dB
/// to 0 dB).
///
/// The microphone hears the speakers and the room. Where the music alone
/// explains it, the microphone follows the master level one for one; what is
/// left over — above the line — is the room. A running fit rather than a
/// stored series: five sums, so a whole night costs forty bytes.
#[derive(Debug, Default, Clone, Copy)]
struct Fit {
    n: f64,
    x: f64,
    y: f64,
    xx: f64,
    xy: f64,
}

/// Why the fit cannot yet say anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unfit {
    TooFew,
    MusicFlat,
    Unfollowed,
}

impl Unfit {
    const fn why(self) -> &'static str {
        match self {
            Self::TooFew => "it takes two minutes of looking alongside the music",
            Self::MusicFlat => {
                "the music has stayed at one level, so the room's share of the sound \
                 cannot be told from the record's"
            }
            Self::Unfollowed => {
                "the microphone does not follow the music's level, so it may not hear the speakers"
            }
        }
    }
}

impl Fit {
    fn add(&mut self, music: f64, heard: f64) {
        self.n += 1.0;
        self.x += music;
        self.y += heard;
        self.xx += music * music;
        self.xy += music * heard;
    }

    /// The line, `(intercept, slope)`, or why there is none.
    fn line(&self) -> Result<(f64, f64), Unfit> {
        #[allow(clippy::cast_precision_loss)]
        if self.n < FIT_ENOUGH as f64 {
            return Err(Unfit::TooFew);
        }
        let mean_x = self.x / self.n;
        let spread = self.xx / self.n - mean_x * mean_x;
        if spread < FIT_SPREAD {
            return Err(Unfit::MusicFlat);
        }
        let slope = (self.xy / self.n - mean_x * (self.y / self.n)) / spread;
        if slope <= 0.0 {
            return Err(Unfit::Unfollowed);
        }
        Ok((self.y / self.n - slope * mean_x, slope))
    }
}

/// The loudness scale both halves of the fit are on: −60 dBFS to 0 across
/// 0..1, the scale the room surface already reads its microphone on.
#[must_use]
pub fn loudness_of_peak(peak: f32) -> f32 {
    let db = 20.0 * peak.max(1e-6).log10();
    ((db + 60.0) / 60.0).clamp(0.0, 1.0)
}

/// What the floor has been doing tonight.
#[derive(Debug, Default)]
pub struct Floor {
    /// Looks back to [`NEAR`], newest last.
    seen: VecDeque<Look>,
    /// The whole night, per signal.
    tonight: [Spread; Signal::ALL.len()],
    fit: Fit,
}

impl Floor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take a look.
    ///
    /// `heard` is the microphone's loudness and `music` the master's, both on
    /// the 0..1 loudness scale ([`loudness_of_peak`] for a peak meter), taken
    /// at the same moment: the pair feeds the room-over-music fit, and once the
    /// fit holds, the look gains its [`Signal::OverMusic`] — how far above the
    /// line it sits, half way up the scale meaning *on* it.
    pub fn saw(&mut self, mut look: Look, heard: Option<f32>, music: Option<f32>) {
        if let (Some(heard), Some(music)) = (heard, music)
            && heard.is_finite()
            && music.is_finite()
        {
            let (heard, music) = (f64::from(heard), f64::from(music));
            self.fit.add(music, heard);
            if let Ok((intercept, slope)) = self.fit.line() {
                let over = heard - (intercept + slope * music);
                #[allow(clippy::cast_possible_truncation)]
                let value = (0.5 + over) as f32;
                look = look.with(Signal::OverMusic, value);
            }
        }
        if look.is_empty() {
            return;
        }
        for signal in Signal::ALL {
            if let Some(value) = look.get(signal) {
                self.tonight[signal.slot()].add(value);
            }
        }
        self.seen.push_back(look);
        while let Some(oldest) = self.seen.front() {
            if look
                .at
                .duration_since(oldest.at)
                .is_ok_and(|since| since > NEAR)
            {
                self.seen.pop_front();
            } else {
                break;
            }
        }
    }

    /// When the last look was.
    #[must_use]
    pub fn last_seen(&self) -> Option<SystemTime> {
        self.seen.back().map(|look| look.at)
    }

    fn near(&self, signal: Signal) -> Vec<f32> {
        self.seen
            .iter()
            .filter_map(|look| look.get(signal))
            .collect()
    }

    /// One signal's line.
    #[must_use]
    pub fn line(&self, signal: Signal) -> Line {
        let mut values = self.near(signal);
        let looks = values.len();
        let title = signal.title();
        let not_yet = |why: &str| Line {
            signal,
            now: None,
            against: None,
            confidence: Confidence::NotYet,
            looks,
            says: format!("{title}: not yet — {why}"),
        };
        if signal == Signal::OverMusic
            && let Err(unfit) = self.fit.line()
        {
            return not_yet(unfit.why());
        }
        if looks < ENOUGH {
            return not_yet(if self.seen.len() < ENOUGH {
                "it takes about half a minute of looking"
            } else {
                signal.absent()
            });
        }
        values.sort_by(f32::total_cmp);
        let now = values[looks / 2];
        let night = &self.tonight[signal.slot()];
        if (night.total() as usize) < ENOUGH || night.width() < 2 {
            return Line {
                now: Some(now),
                ..not_yet("tonight has not varied enough to compare with")
            };
        }
        let Some(placed) = night.below(now).map(against) else {
            return not_yet("tonight has nothing to compare with");
        };
        let confidence = if (night.total() as usize) < SETTLED {
            Confidence::Early
        } else {
            Confidence::Settled
        };
        let where_ = match placed {
            Against::Lowest => "lower than it has been all night",
            Against::Lower => "lower than usual tonight",
            Against::Usual => "about as usual tonight",
            Against::Higher => "higher than usual tonight",
            Against::Highest => "higher than it has been all night",
        };
        let early = if confidence == Confidence::Early {
            ", early in the night"
        } else {
            ""
        };
        Line {
            signal,
            now: Some(now),
            against: Some(placed),
            confidence,
            looks,
            says: format!(
                "{title}: {where_} ({looks} looks{early}; {})",
                signal.cannot()
            ),
        }
    }

    /// Every signal's line, in [`Signal::ALL`]'s order: one each, always, so a
    /// signal with nothing to say says *not yet* rather than vanishing.
    #[must_use]
    pub fn lines(&self) -> Vec<Line> {
        Signal::ALL
            .iter()
            .map(|signal| self.line(*signal))
            .collect()
    }

    /// The lines worth telling the assistant: placed, and not usual.
    #[must_use]
    pub fn notes(&self) -> Vec<String> {
        self.lines()
            .into_iter()
            .filter(|line| line.against.is_some_and(Against::is_notable))
            .map(|line| line.says)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 + seconds)
    }

    /// Looks before the last three minutes, and in them (three minutes at
    /// one look every two seconds).
    const BEFORE: u64 = 1000;
    const LAST: u64 = 90;

    /// A small count as a float.
    #[allow(clippy::cast_precision_loss)]
    fn f(i: u64) -> f32 {
        i as f32
    }

    /// A night of looks at one signal: `quiet` for most of it, then `now` for
    /// the last three minutes.
    fn night_of(signal: Signal, quiet: impl Fn(u64) -> f32, now: f32) -> Floor {
        let mut floor = Floor::new();
        for i in 0..BEFORE {
            floor.saw(Look::at(at(i * 2)).with(signal, quiet(i)), None, None);
        }
        for i in BEFORE..BEFORE + LAST {
            floor.saw(Look::at(at(i * 2)).with(signal, now), None, None);
        }
        floor
    }

    /// A value that wanders over a range, so the night has a spread.
    fn wander(i: u64) -> f32 {
        0.2 + 0.02 * f(i % 20)
    }

    /// **Each signal is placed against its own night, on its own line.**
    ///
    /// The load-bearing rule of §36: a signal that went up is said to have gone
    /// up, and the others — which did not move — say *not yet* or *usual* on
    /// their own lines rather than being pulled along. A model that averaged
    /// them would move every line, or one combined line, and both fail here.
    #[test]
    fn each_signal_is_placed_against_its_own_night_and_said_alone() {
        for signal in Signal::ALL.into_iter().filter(|s| *s != Signal::OverMusic) {
            let floor = night_of(signal, wander, 0.95);
            let lines = floor.lines();
            assert_eq!(lines.len(), Signal::ALL.len(), "one line per signal");
            for line in &lines {
                if line.signal == signal {
                    assert_eq!(line.against, Some(Against::Highest), "{}", line.says);
                    assert_eq!(line.confidence, Confidence::Settled, "{}", line.says);
                    assert!(line.says.starts_with(signal.title()), "{}", line.says);
                    assert!(line.says.contains(signal.cannot()), "{}", line.says);
                } else {
                    assert_eq!(line.confidence, Confidence::NotYet, "{}", line.says);
                    assert!(line.now.is_none());
                }
                for other in Signal::ALL {
                    if other != line.signal {
                        assert!(
                            !line.says.contains(other.title()),
                            "the {} line speaks of {}: {}",
                            line.signal.name(),
                            other.name(),
                            line.says
                        );
                    }
                }
            }
            assert_eq!(floor.notes().len(), 1);

            let quiet = night_of(signal, wander, 0.05);
            assert_eq!(
                quiet.line(signal).against,
                Some(Against::Lowest),
                "{}",
                signal.name()
            );
        }
    }

    /// **A line says how far it can be trusted, and says nothing before it
    /// can.**
    #[test]
    fn a_line_waits_for_enough_and_says_when_it_is_early() {
        let mut floor = Floor::new();
        for i in 0..(ENOUGH as u64 - 1) {
            floor.saw(
                Look::at(at(i * 2)).with(Signal::High, wander(i)),
                None,
                None,
            );
        }
        let line = floor.line(Signal::High);
        assert_eq!(line.confidence, Confidence::NotYet);
        assert!(line.says.contains("not yet"), "{}", line.says);

        for i in (ENOUGH as u64 - 1)..(ENOUGH as u64 * 2) {
            floor.saw(
                Look::at(at(i * 2)).with(Signal::High, wander(i)),
                None,
                None,
            );
        }
        let line = floor.line(Signal::High);
        assert_eq!(line.confidence, Confidence::Early, "{}", line.says);
        assert!(line.says.contains("early in the night"), "{}", line.says);

        // A long night does not stand in for the last three minutes: after a
        // gap, a handful of looks is not enough however much came before.
        let mut lapsed = night_of(Signal::High, wander, 0.5);
        for i in 0..5 {
            lapsed.saw(
                Look::at(at((BEFORE + LAST) * 2 + 600 + i * 2)).with(Signal::High, 0.9),
                None,
                None,
            );
        }
        let line = lapsed.line(Signal::High);
        assert_eq!(line.confidence, Confidence::NotYet, "{}", line.says);
        assert_eq!(line.looks, 5);

        // Looked at enough, and this signal had nothing in the looks: it says
        // what was missing rather than asking for more time.
        let mut still = Floor::new();
        for i in 0..(ENOUGH as u64 * 2) {
            still.saw(Look::at(at(i * 2)).with(Signal::Uneven, 0.0), None, None);
        }
        let line = still.line(Signal::Together);
        assert!(
            line.says.contains(Signal::Together.absent()),
            "{}",
            line.says
        );
        assert!(!line.says.contains("half a minute"), "{}", line.says);

        // A night that has never varied cannot place anything inside itself.
        let mut flat = Floor::new();
        for i in 0..200 {
            flat.saw(Look::at(at(i * 2)).with(Signal::Low, 0.4), None, None);
        }
        assert_eq!(flat.line(Signal::Low).confidence, Confidence::NotYet);
        assert_eq!(flat.line(Signal::Low).now, Some(0.4));
    }

    /// The microphone hearing the music at `music` with the room adding
    /// `room` on top, on the loudness scale.
    fn heard(music: f32, room: f32) -> f32 {
        music * 0.9 + 0.05 + room
    }

    /// **The room is told apart from the music by what the music explains.**
    ///
    /// The microphone follows the master one for one where it hears only the
    /// speakers. A crowd that gets loud while the music does not lifts the
    /// microphone above that line, and the line says so; the same rise in
    /// loudness that came from the DJ pushing the master does **not**, because
    /// the music explains it. A model that read the microphone's loudness alone
    /// — the room surface's *loudness* — would call both a louder room.
    #[test]
    fn the_room_over_the_music_is_what_the_music_does_not_explain() {
        let music_at = |i: u64| 0.5 + 0.03 * f(i % 10);
        let night = |room_at_end: f32, music_at_end: f32| {
            let mut floor = Floor::new();
            for i in 0..BEFORE {
                let music = music_at(i);
                let room = if i % 7 == 0 { 0.03 } else { 0.0 };
                floor.saw(Look::at(at(i * 2)), Some(heard(music, room)), Some(music));
            }
            for i in BEFORE..BEFORE + LAST {
                let music = music_at_end + 0.01 * f(i % 3);
                floor.saw(
                    Look::at(at(i * 2)),
                    Some(heard(music, room_at_end)),
                    Some(music),
                );
            }
            floor
        };

        let crowd = night(0.2, 0.6).line(Signal::OverMusic);
        assert_eq!(crowd.against, Some(Against::Highest), "{}", crowd.says);

        let pushed = night(0.0, 0.8).line(Signal::OverMusic);
        assert!(
            pushed.against.is_some_and(|a| a <= Against::Usual),
            "the DJ pushing the master read as the room: {}",
            pushed.says
        );
    }

    /// **And it refuses when the music gives it nothing to learn from.**
    ///
    /// A limiter holding the master flat all night, or a microphone that does
    /// not hear the speakers, leaves the room's share of the sound unknowable;
    /// the line says which, rather than quietly becoming the microphone's
    /// loudness.
    #[test]
    fn the_room_over_the_music_says_why_it_cannot_tell() {
        let mut flat = Floor::new();
        for i in 0..200 {
            flat.saw(Look::at(at(i * 2)), Some(0.6 + 0.001 * f(i % 5)), Some(0.7));
        }
        let line = flat.line(Signal::OverMusic);
        assert_eq!(line.confidence, Confidence::NotYet);
        assert!(line.says.contains("stayed at one level"), "{}", line.says);

        let mut deaf = Floor::new();
        for i in 0..200 {
            let music = 0.4 + 0.05 * f(i % 6);
            deaf.saw(Look::at(at(i * 2)), Some(0.9 - music), Some(music));
        }
        let line = deaf.line(Signal::OverMusic);
        assert!(line.says.contains("does not follow"), "{}", line.says);

        let mut early = Floor::new();
        for i in 0..(FIT_ENOUGH as u64 - 1) {
            let music = 0.4 + 0.05 * f(i % 6);
            early.saw(Look::at(at(i * 2)), Some(heard(music, 0.0)), Some(music));
        }
        assert!(early.line(Signal::OverMusic).says.contains("two minutes"),);
    }

    /// **No line names a mood.** §36's evidence, not a reading of minds.
    #[test]
    fn no_line_names_a_mood() {
        const MOODS: [&str; 9] = [
            "enjoy", "loving", "bored", "dead", "hyped", "happy", "energy", "vibe", "mood",
        ];
        for signal in Signal::ALL {
            let floor = night_of(signal, wander, 0.95);
            let words = [
                floor.line(signal).says,
                signal.measures().to_owned(),
                signal.cannot().to_owned(),
            ]
            .join(" ")
            .to_lowercase();
            for mood in MOODS {
                assert!(!words.contains(mood), "{} says `{mood}`", signal.name());
            }
        }
    }

    #[test]
    fn a_peak_meter_reads_on_the_room_surfaces_scale() {
        assert!((loudness_of_peak(1.0) - 1.0).abs() < 1e-6);
        assert!((loudness_of_peak(0.001) - 0.0).abs() < 1e-6);
        assert!((loudness_of_peak(0.1) - (40.0 / 60.0)).abs() < 1e-4);
        assert!(loudness_of_peak(0.0).abs() < 1e-6);
    }
}
