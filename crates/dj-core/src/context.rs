//! What the interface is allowed to know about the room.
//!
//! The living interface ([`docs/VISUAL-LANGUAGE.md`](../../docs/VISUAL-LANGUAGE.md))
//! moves to the music and to the shape of the night. This is the channel that
//! carries both — and the reason it is split the way it is.
//!
//! # Measured and unmeasured are different kinds of fact
//!
//! [`AudioMetrics`] is measured, every snapshot, off the master bus. It is
//! always true.
//!
//! [`SessionRead`] is a *judgement*, and [`ContextEngine`] is what makes it.
//! Whether a set is warming up or peaking is not a number any instrument
//! reports, so the engine says nothing at all until it has two things to work
//! from: what the DJ declared the night to be, and enough of tonight to compare
//! the last few minutes against. Before either exists the honest value is
//! `None`, and a theme keyed to it shows its neutral treatment. The first
//! version of this module defaulted it to *Peak* at *0.95 energy* on every
//! snapshot, which meant the interface confidently announced peak time thirty
//! seconds into a warm-up — a claim nothing had made and nothing could check.
//!
//! # Every reading is relative to tonight
//!
//! The same argument `dj_assistant::room` makes about a camera holds for a
//! master bus: "loud" is a number about a gain structure, a venue and a
//! mastering engineer, and an absolute threshold for *peak* would be right in
//! one room and wrong everywhere else. What is portable is a comparison with
//! the same night earlier on, through the same output, and that is all
//! [`Spread`] does — which is why it lives here and is shared rather than
//! written twice.
//!
//! # The DJ's word and the room's evidence are different kinds of claim
//!
//! An occasion is what the DJ *set the night up to be*; the spread is what has
//! actually been played. [`Basis`] names which of the two produced the phase,
//! [`Certainty`] says how much to believe it, and where they disagree the
//! declaration wins and [`Drift`] names which way the evidence points. The
//! engine reports a disagreement; it never overrules a DJ from a histogram.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Duration;

/// Where a set is in its arc.
///
/// Named for what a DJ would say about the room rather than for a number, which
/// is what makes it worth having: "peak" is a decision about people, and no
/// amount of loudness measures it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionPhase {
    #[default]
    WarmUp,
    Heat,
    Peak,
    Cooldown,
    ChillOut,
}

impl SessionPhase {
    pub const ALL: [SessionPhase; 5] = [
        SessionPhase::WarmUp,
        SessionPhase::Heat,
        SessionPhase::Peak,
        SessionPhase::Cooldown,
        SessionPhase::ChillOut,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            SessionPhase::WarmUp => "warm_up",
            SessionPhase::Heat => "heat",
            SessionPhase::Peak => "peak",
            SessionPhase::Cooldown => "cooldown",
            SessionPhase::ChillOut => "chill_out",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|phase| phase.name() == name)
    }

    /// Where in tonight's own range this phase sits, as a half-open band.
    ///
    /// **Not an absolute loudness.** The number compared against it is a
    /// position in the night's own spread — see [`Spread`] — so the same band
    /// means the same thing in a basement and in a stadium.
    ///
    /// Two pairs of phases share a band, because the level alone does not
    /// separate them: a room at half of tonight's range is heating up or
    /// cooling down depending on which way it is going, and the same is true
    /// at the bottom. [`Self::reading`] is the one place that resolves it.
    #[must_use]
    pub const fn band(self) -> (f32, f32) {
        match self {
            SessionPhase::ChillOut | SessionPhase::WarmUp => (0.0, MIDDLE),
            SessionPhase::Cooldown | SessionPhase::Heat => (MIDDLE, HOT),
            SessionPhase::Peak => (HOT, 1.0),
        }
    }

    /// The phase a level and a direction read as.
    ///
    /// The inverse of [`Self::band`] and deliberately its neighbour: a table
    /// that answers in one direction only is a table that eventually disagrees
    /// with itself, and a test here asserts the two agree for every phase.
    #[must_use]
    pub const fn reading(level: f32, rising: bool) -> Self {
        if level >= HOT {
            // Nothing is above peak, so which way it is going does not change
            // the name. It changes what happens next, which is not this
            // function's question.
            SessionPhase::Peak
        } else if level >= MIDDLE {
            if rising {
                SessionPhase::Heat
            } else {
                SessionPhase::Cooldown
            }
        } else if rising {
            SessionPhase::WarmUp
        } else {
            SessionPhase::ChillOut
        }
    }

    /// Whether this phase is one the night is climbing through.
    #[must_use]
    pub const fn is_rising(self) -> bool {
        matches!(self, SessionPhase::WarmUp | SessionPhase::Heat)
    }
}

/// Where the middle of a night begins, as a fraction of its own range.
const MIDDLE: f32 = 0.35;

/// And where the top of one does.
const HOT: f32 = 0.70;

/// Roughly when it is.
///
/// An enum rather than a string because there are five of these and there will
/// always be five; a `String` here means every consumer writes its own spelling
/// check and one of them gets it wrong. Derived from the system clock, so
/// unlike the rest of [`EnvironmentContext`] it is a fact rather than a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TimeOfDay {
    /// 05:00–08:00.
    Dawn,
    /// 08:00–17:00.
    Day,
    /// 17:00–20:00.
    Dusk,
    #[default]
    /// 20:00–24:00.
    Night,
    /// 00:00–05:00 — a different thing from the evening, and every DJ knows it.
    SmallHours,
}

impl TimeOfDay {
    /// From an hour of the day, 0–23. Out-of-range hours fall back to the
    /// default rather than panicking: a clock that says 25 is a broken clock,
    /// not a reason to take the interface down.
    #[must_use]
    pub const fn from_hour(hour: u32) -> Self {
        match hour {
            5..=7 => TimeOfDay::Dawn,
            8..=16 => TimeOfDay::Day,
            17..=19 => TimeOfDay::Dusk,
            20..=23 => TimeOfDay::Night,
            0..=4 => TimeOfDay::SmallHours,
            _ => TimeOfDay::Night,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            TimeOfDay::Dawn => "dawn",
            TimeOfDay::Day => "day",
            TimeOfDay::Dusk => "dusk",
            TimeOfDay::Night => "night",
            TimeOfDay::SmallHours => "small_hours",
        }
    }
}

/// Where the set is happening.
///
/// Only what can actually be known. An earlier version carried weather and a
/// temperature, both hardcoded to "Clear, 20°C" — a reading no instrument had
/// taken. They belong here when there is a source for them, and not before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EnvironmentContext {
    pub time_of_day: TimeOfDay,
}

/// What the master bus sounds like, right now.
///
/// Measured in the engine every block and published through the parameter
/// registry; see `dj_dsp::Spectrum`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct AudioMetrics {
    /// Overall level, 0..=1, as an RMS rather than a peak.
    ///
    /// A peak would be useless here: the master limiter holds peaks near 1.0
    /// whenever anything is playing, so an interface driven by one would be
    /// pinned open all night.
    pub loudness: f32,
    /// Bass, low mid, high mid and treble, 0..=1 and comparable with each other.
    pub bands: [f32; 4],
}

impl AudioMetrics {
    /// Total level from the bands.
    ///
    /// The bands are RMS amplitudes of disjoint parts of the spectrum, so by
    /// Parseval the whole is the root of the sum of their squares — which is
    /// why loudness is derived here rather than metered separately.
    #[must_use]
    pub fn from_bands(bands: [f32; 4]) -> Self {
        let loudness = bands
            .iter()
            .map(|band| band * band)
            .sum::<f32>()
            .sqrt()
            .clamp(0.0, 1.0);
        Self { loudness, bands }
    }
}

/// Somebody's reading of the room.
///
/// `None` at the top level until M9 puts something behind it. Grouped into one
/// struct rather than three separate `Option`s because they arrive together:
/// whatever works out the phase works out the energy at the same time, from the
/// same evidence.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionRead {
    pub phase: SessionPhase,
    /// 0..=1. How hard the room is going.
    ///
    /// A judgement where there is one: the position of the last few minutes in
    /// the whole night's own range, which is what makes "hard" mean something
    /// without calibrating anything. Where there is not one — a phase the DJ
    /// declared before the night has a range to compare against — it falls back
    /// to the measured loudness, which is a level rather than a judgement.
    /// [`Self::certainty`] and [`Self::basis`] say which of the two is on
    /// screen, so nothing has to guess.
    pub energy: f32,
    pub environment: EnvironmentContext,
    /// How much to believe [`Self::phase`].
    pub certainty: Certainty,
    /// What produced [`Self::phase`].
    pub basis: Basis,
    /// Which way the evidence pulls against what the DJ declared.
    ///
    /// `Some` only when [`Self::basis`] is [`Basis::Disputed`]: the rest of the
    /// time there is nothing to disagree with.
    pub drift: Option<Drift>,
}

/// How much to believe a reading of the night.
///
/// Ordered, and the ordering is the point: [§9 of the
/// directive](../../../docs/DIRECTIVE.md) keeps autonomy and certainty
/// orthogonal, and *high autonomy on a low-certainty read* is the one
/// combination that is never allowed. `dj_assistant::Warrant` is where that
/// becomes a type rather than a rule somebody remembers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Certainty {
    /// One source, and it is contradicted. Suggest at most.
    #[default]
    Unsure,
    /// One source, uncontradicted.
    Fair,
    /// The DJ's word and the night's evidence say the same thing.
    Sure,
}

impl Certainty {
    pub const ALL: [Certainty; 3] = [Certainty::Unsure, Certainty::Fair, Certainty::Sure];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Certainty::Unsure => "unsure",
            Certainty::Fair => "fair",
            Certainty::Sure => "sure",
        }
    }

    /// One line, for an interface that shows its working.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Certainty::Unsure => "One source, and something disagrees with it.",
            Certainty::Fair => "One source, and nothing disagrees with it.",
            Certainty::Sure => "Your word and the night's own range agree.",
        }
    }
}

/// What produced a phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// Nothing yet. The engine is still watching.
    #[default]
    Nothing,
    /// The occasion the DJ set. Their word, and no evidence either way.
    Declared,
    /// Tonight's own range. Nobody said what the night is.
    Measured,
    /// Both, saying the same thing.
    Agreed,
    /// Both, saying different things. The declaration is the phase; see
    /// [`SessionRead::drift`] for which way the evidence points.
    Disputed,
}

impl Basis {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Basis::Nothing => "nothing",
            Basis::Declared => "declared",
            Basis::Measured => "measured",
            Basis::Agreed => "agreed",
            Basis::Disputed => "disputed",
        }
    }
}

/// Which way the evidence pulls away from what the DJ declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Drift {
    /// The music is above the band the declared phase occupies.
    Hotter,
    /// And below it.
    Cooler,
}

impl Drift {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Drift::Hotter => "hotter",
            Drift::Cooler => "cooler",
        }
    }

    /// How it reads, in the words a DJ would use.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Drift::Hotter => "the music has been harder than that",
            Drift::Cooler => "the music has been softer than that",
        }
    }
}

/// Everything the interface may morph to.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionContext {
    /// Always present, always measured.
    pub audio: AudioMetrics,
    /// Present once something has read the room. See the module note.
    pub session: Option<SessionRead>,
}

// -- the night's own distribution -------------------------------------------

/// How many buckets a night's readings are kept in.
///
/// Twenty over the 0..=1 range, so a reading is placed to within five percent
/// without keeping every reading of a six-hour night.
pub const BUCKETS: usize = 20;

/// One night's own distribution of one measurement.
///
/// The whole of the "relative to tonight" argument, in one small type. A
/// reading is placed against the readings that came before it through the same
/// output in the same room, so nothing has to be calibrated and no threshold
/// has to be right in two venues at once.
///
/// Shared rather than written twice: `dj_assistant::room` reads a camera and a
/// microphone this way and this reads a master bus, and two histograms with the
/// same job eventually disagree about which bucket a value is in.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Spread {
    counts: [u32; BUCKETS],
    total: u32,
}

impl Spread {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one reading.
    ///
    /// Clamped on the way in and a non-finite value dropped, so one broken
    /// frame cannot move the night's range.
    pub fn add(&mut self, value: f32) {
        if !value.is_finite() {
            return;
        }
        let bucket = bucket_of(value);
        self.counts[bucket] = self.counts[bucket].saturating_add(1);
        self.total = self.total.saturating_add(1);
    }

    /// What fraction of the night's readings were below `value`.
    ///
    /// Half of its own bucket counts as below, so a night where every reading
    /// lands in one bucket answers "about half" rather than "none" — which is
    /// what "no news" should look like when nothing has changed.
    #[must_use]
    pub fn below(&self, value: f32) -> Option<f32> {
        if self.total == 0 || !value.is_finite() {
            return None;
        }
        let bucket = bucket_of(value);
        let under: u32 = self.counts[..bucket].iter().sum();
        let within = f64::from(self.counts[bucket]) / 2.0;
        #[allow(clippy::cast_possible_truncation)]
        Some(((f64::from(under) + within) / f64::from(self.total)) as f32)
    }

    /// How many readings there have been.
    #[must_use]
    pub const fn total(&self) -> u32 {
        self.total
    }

    /// How many buckets have anything in them.
    ///
    /// The guard against a self-referential answer: a position inside a
    /// distribution that has not spread out yet is arithmetic rather than
    /// information, and every night starts that way.
    #[must_use]
    pub fn width(&self) -> usize {
        self.counts.iter().filter(|count| **count > 0).count()
    }
}

fn bucket_of(value: f32) -> usize {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let bucket = (value.clamp(0.0, 1.0) * BUCKETS as f32) as usize;
    bucket.min(BUCKETS - 1)
}

// -- the context engine -----------------------------------------------------

/// How long between the readings the engine keeps.
///
/// The caller ticks at whatever rate it likes — the snapshot pump runs at 60 Hz
/// — and the engine keeps one reading every four seconds regardless, so its
/// memory and its idea of "lately" do not depend on how often it is asked.
pub const SAMPLE: Duration = Duration::from_secs(4);

/// How many readings before a phase may be read from evidence.
///
/// Six minutes of music. Shorter than this and the night has no range to place
/// anything in; the engine says nothing instead, which is the answer the module
/// docs exist to defend.
pub const ENOUGH: usize = 90;

/// How many of the twenty buckets tonight must span before a position in it
/// means anything.
pub const VARIED: usize = 3;

/// How far back the trend looks.
pub const TREND: Duration = Duration::from_secs(3 * 60);

/// How much of that window must be filled before a trend is read at all.
const TREND_ENOUGH: Duration = Duration::from_secs(60);

/// How much movement counts as movement.
///
/// Below this the night is level, and a level night reads as rising: a set that
/// has not turned yet is still on its way up, and calling a steady room a
/// cool-down would be the interface inventing a decline.
const STIRRING: f32 = 0.03;

/// How many consecutive readings a new phase needs before it is adopted.
///
/// Three readings is twelve seconds of agreement. Hysteresis rather than a
/// smoother because the thing being stabilised is a *name*: an interface that
/// flickers between "heat" and "cool-down" every few seconds is worse than one
/// that is twelve seconds late.
pub const STEADY: u32 = 3;

/// How fast the published energy follows the measured loudness, in seconds.
const SETTLE_TAU: f32 = 8.0;

/// One look at the set, as the engine needs it.
///
/// Everything here is either measured or declared. There is deliberately no
/// field for anything inferred: inference is what the engine is for, and a
/// caller that did it first would be the second place phase logic lived.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Observation {
    /// How long the set has been running. The engine's only clock, so a test
    /// can run a night in a millisecond and a replay can run one exactly.
    pub elapsed: Duration,
    /// Hour of the day, 0..=23, when it is known.
    pub hour: Option<u32>,
    /// How many decks are audibly playing.
    pub playing: u8,
    /// Tempo of the deck that is leading, in BPM, when one is known.
    pub tempo: Option<f32>,
    /// Measured off the master bus. Always true.
    pub audio: AudioMetrics,
    /// The phase the DJ's chosen occasion declares, when it declares one.
    pub declared: Option<SessionPhase>,
}

/// What the night has been, and therefore what it is.
///
/// Holds tonight's own range and nothing else that persists — no library, no
/// history, no model. It is fed by whoever is already looking at the set and
/// answers the one question every consumer in [§11 of the
/// directive](../../../docs/DIRECTIVE.md) asks, so that none of them works it
/// out separately.
#[derive(Debug, Default)]
pub struct ContextEngine {
    loudness: Spread,
    tempo: Spread,
    /// `(elapsed, level)` for the readings inside the trend window.
    trail: VecDeque<(Duration, f32)>,
    kept: usize,
    last_sample: Option<Duration>,
    last_seen: Option<Duration>,
    smoothed: f32,
    /// The phase the evidence has settled on, after hysteresis.
    settled: Option<SessionPhase>,
    candidate: Option<(SessionPhase, u32)>,
    read: Option<SessionRead>,
}

impl ContextEngine {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take one look at the set, and answer with what the night is.
    ///
    /// Cheap enough to call on every snapshot: it keeps a reading only when one
    /// is due, and the rest of the work is a handful of comparisons.
    pub fn observe(&mut self, observation: &Observation) -> Option<SessionRead> {
        self.follow(observation);
        if observation.playing > 0 && self.due(observation.elapsed) {
            self.sample(observation);
        }
        self.read = self.judge(observation);
        self.read
    }

    /// The last answer, without taking another look.
    ///
    /// So that a caller outside the loop — a panel, a command — sees exactly
    /// what the interface is showing rather than a second opinion computed from
    /// a slightly different moment.
    #[must_use]
    pub fn read(&self) -> Option<SessionRead> {
        self.read
    }

    /// How many readings tonight's range is built from.
    #[must_use]
    pub const fn readings(&self) -> usize {
        self.kept
    }

    /// The phase the evidence alone has settled on, ignoring the declaration.
    ///
    /// Published so an interface can draw the disagreement rather than only
    /// describe it: [`SessionRead::phase`] is the DJ's word whenever they have
    /// given one, and a panel that showed only that could never mark where the
    /// music actually reads.
    #[must_use]
    pub const fn measured(&self) -> Option<SessionPhase> {
        self.settled
    }

    /// How many readings are still needed before evidence can name a phase.
    #[must_use]
    pub const fn still_needed(&self) -> usize {
        ENOUGH.saturating_sub(self.kept)
    }

    /// Where the last reading sits in the whole night's range, 0..=1.
    ///
    /// `None` until there is a night to sit in.
    #[must_use]
    pub fn level(&self) -> Option<f32> {
        if !self.ready() {
            return None;
        }
        self.trail.back().map(|(_, level)| *level)
    }

    /// Whether tonight has enough of a range to read anything from.
    fn ready(&self) -> bool {
        self.kept >= ENOUGH && self.loudness.width() >= VARIED
    }

    /// Follow the measured loudness, smoothly.
    ///
    /// Silence between records is not a quiet room, so nothing playing holds
    /// the value rather than pulling it to zero — otherwise every gap between
    /// two records would read as the night collapsing.
    fn follow(&mut self, observation: &Observation) {
        let previous = self.last_seen.replace(observation.elapsed);
        if observation.playing == 0 {
            return;
        }
        let Some(before) = previous else {
            self.smoothed = observation.audio.loudness;
            return;
        };
        let delta = observation.elapsed.saturating_sub(before).as_secs_f32();
        let alpha = 1.0 - (-delta / SETTLE_TAU).exp();
        self.smoothed += alpha * (observation.audio.loudness - self.smoothed);
    }

    fn due(&self, elapsed: Duration) -> bool {
        self.last_sample
            .is_none_or(|last| elapsed.saturating_sub(last) >= SAMPLE)
    }

    fn sample(&mut self, observation: &Observation) {
        self.last_sample = Some(observation.elapsed);
        self.loudness.add(observation.audio.loudness);
        if let Some(bpm) = observation.tempo {
            self.tempo.add(tempo_position(bpm));
        }
        self.kept = self.kept.saturating_add(1);

        // Placed against the range *including* this reading, which is what
        // makes the first readings of a night sit near the middle. They are
        // never used: `ready` holds the answer back until there are ninety of
        // them, by which time the oldest reading still in the trend window was
        // placed against a range of at least forty-five.
        let Some(level) = self.place(observation) else {
            return;
        };
        self.trail.push_back((observation.elapsed, level));
        while let Some((at, _)) = self.trail.front() {
            if observation.elapsed.saturating_sub(*at) > TREND {
                self.trail.pop_front();
            } else {
                break;
            }
        }
        let fresh = self.evidence_reads();
        self.settle(fresh);
    }

    /// Where one observation sits in the night so far.
    ///
    /// Two signals, averaged, because they are confounded differently. Loudness
    /// off the master bus moves with the master fader as well as with the
    /// music, and tempo does not move with it at all; tempo says little on its
    /// own — a slow record can be the hardest thing played all night — and
    /// loudness says a great deal. Neither alone is worth trusting; together
    /// they are worth reporting with a certainty attached.
    fn place(&self, observation: &Observation) -> Option<f32> {
        let loud = self.loudness.below(observation.audio.loudness)?;
        let fast = observation
            .tempo
            .and_then(|bpm| self.tempo.below(tempo_position(bpm)));
        Some(fast.map_or(loud, |fast| (loud + fast) / 2.0))
    }

    /// The phase the evidence alone reads as, before hysteresis.
    fn evidence_reads(&self) -> Option<SessionPhase> {
        if !self.ready() {
            return None;
        }
        let (_, level) = *self.trail.back()?;
        let rising = self.trend().unwrap_or(0.0) > -STIRRING;
        Some(SessionPhase::reading(level, rising))
    }

    /// How much the level has moved across the trend window.
    ///
    /// `None` until the window holds a real span of time: two readings a few
    /// seconds apart say nothing about where a night is going.
    fn trend(&self) -> Option<f32> {
        let (first_at, first) = *self.trail.front()?;
        let (last_at, last) = *self.trail.back()?;
        (last_at.saturating_sub(first_at) >= TREND_ENOUGH).then_some(last - first)
    }

    /// Adopt a new phase only once it has held.
    fn settle(&mut self, fresh: Option<SessionPhase>) {
        match fresh {
            None => self.candidate = None,
            Some(phase) if Some(phase) == self.settled => self.candidate = None,
            Some(phase) => {
                let held = match self.candidate {
                    Some((standing, count)) if standing == phase => count + 1,
                    _ => 1,
                };
                if held >= STEADY {
                    self.settled = Some(phase);
                    self.candidate = None;
                } else {
                    self.candidate = Some((phase, held));
                }
            }
        }
    }

    /// Put the DJ's word and the night's evidence together.
    ///
    /// Where they disagree the declaration is the phase. The engine has a
    /// histogram; the DJ has been in the room all night, and software that
    /// overrules that from a spread of loudness readings would be wrong in
    /// exactly the situations a DJ most needs it to be quiet.
    fn judge(&self, observation: &Observation) -> Option<SessionRead> {
        let environment = EnvironmentContext {
            time_of_day: observation
                .hour
                .map_or_else(TimeOfDay::default, TimeOfDay::from_hour),
        };
        let measured = self.settled;
        let level = self.level();
        let (phase, basis, certainty, drift, energy) = match (observation.declared, measured) {
            (None, None) => return None,
            (Some(said), None) => (said, Basis::Declared, Certainty::Fair, None, self.smoothed),
            (None, Some(seen)) => (
                seen,
                Basis::Measured,
                Certainty::Fair,
                None,
                level.unwrap_or(self.smoothed),
            ),
            (Some(said), Some(seen)) if said == seen => (
                said,
                Basis::Agreed,
                Certainty::Sure,
                None,
                level.unwrap_or(self.smoothed),
            ),
            (Some(said), Some(seen)) => {
                let energy = level.unwrap_or(self.smoothed);
                (
                    said,
                    Basis::Disputed,
                    Certainty::Unsure,
                    drift_between(said, seen, energy),
                    energy,
                )
            }
        };
        Some(SessionRead {
            phase,
            energy,
            environment,
            certainty,
            basis,
            drift,
        })
    }
}

/// A tempo as a position between the slowest and the fastest a DJ plays.
///
/// The two ends are arbitrary and it does not matter: the number is only ever
/// placed against tonight's own tempos, so any monotone mapping cancels. What
/// it must not do is saturate inside the range that actually gets used, which
/// is why it is 60..200 rather than something tighter around one genre.
fn tempo_position(bpm: f32) -> f32 {
    ((bpm - 60.0) / 140.0).clamp(0.0, 1.0)
}

/// Which way the evidence pulls away from the declaration.
///
/// Level first, direction second — because a phase says both, and two phases
/// can share a band and differ only in which way the night is going.
fn drift_between(declared: SessionPhase, measured: SessionPhase, level: f32) -> Option<Drift> {
    let (low, high) = declared.band();
    if level >= high {
        return Some(Drift::Hotter);
    }
    if level < low {
        return Some(Drift::Cooler);
    }
    if measured.is_rising() == declared.is_rising() {
        None
    } else if measured.is_rising() {
        Some(Drift::Hotter)
    } else {
        Some(Drift::Cooler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_phase_round_trips_through_its_name() {
        for phase in SessionPhase::ALL {
            assert_eq!(SessionPhase::parse(phase.name()), Some(phase));
        }
        assert_eq!(SessionPhase::parse("fiesta"), None);
    }

    /// Every hour of the day has to land somewhere, or a set that runs past a
    /// boundary hits an hour the interface has no answer for.
    #[test]
    fn every_hour_of_the_day_belongs_to_a_part_of_it() {
        let mut seen = [0usize; 5];
        for hour in 0..24 {
            let part = TimeOfDay::from_hour(hour);
            seen[part as usize] += 1;
        }
        assert!(
            seen.iter().all(|count| *count > 0),
            "some part of the day is unreachable: {seen:?}"
        );
        assert_eq!(seen.iter().sum::<usize>(), 24);
        // A clock reading nonsense must not take the interface down.
        assert_eq!(TimeOfDay::from_hour(99), TimeOfDay::default());
    }

    /// Loudness is derived from the bands rather than metered twice, so it has
    /// to actually follow them.
    #[test]
    fn loudness_is_the_whole_of_the_bands() {
        assert_eq!(AudioMetrics::from_bands([0.0; 4]).loudness, 0.0);

        let quiet = AudioMetrics::from_bands([0.1, 0.1, 0.1, 0.1]);
        let loud = AudioMetrics::from_bands([0.5, 0.5, 0.5, 0.5]);
        assert!(loud.loudness > quiet.loudness);

        // Four equal bands of 0.5 is sqrt(4 * 0.25) = 1.0.
        assert!((loud.loudness - 1.0).abs() < 1e-6);
        // And it never exceeds what the interface expects.
        assert_eq!(AudioMetrics::from_bands([1.0; 4]).loudness, 1.0);
    }

    /// Drive a stretch of a night through the engine, one reading every
    /// [`SAMPLE`], and answer with the last read.
    fn watch(
        engine: &mut ContextEngine,
        from: usize,
        to: usize,
        declared: Option<SessionPhase>,
        level: &dyn Fn(usize) -> f32,
    ) -> Option<SessionRead> {
        let mut last = None;
        for index in from..to {
            let loudness = level(index).clamp(0.0, 1.0);
            last = engine.observe(&Observation {
                elapsed: SAMPLE * u32::try_from(index).expect("a night of readings fits in u32"),
                hour: Some(23),
                playing: 1,
                // Tempo tracks the level, which is what a night that builds
                // actually does. It is a second signal, not a second opinion.
                tempo: Some(110.0 + loudness * 30.0),
                audio: AudioMetrics {
                    loudness,
                    bands: [loudness / 2.0; 4],
                },
                declared,
            });
        }
        last
    }

    /// A night that climbs steadily from quiet to loud.
    fn climbing(index: usize) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let step = index as f32;
        (step / 250.0).min(1.0)
    }

    /// The promise the module docs make, as a test.
    ///
    /// Six minutes of music before the evidence is allowed to name anything.
    /// The failure this defends against shipped once: a phase asserted from the
    /// first snapshot, which announced peak time thirty seconds into a warm-up.
    #[test]
    fn the_evidence_names_nothing_until_it_has_a_night_to_compare_against() {
        let mut engine = ContextEngine::new();
        let short = watch(&mut engine, 0, ENOUGH - 1, None, &climbing);
        assert!(
            short.is_none(),
            "named a phase from {} readings, before there was a night to place them in",
            engine.readings()
        );
        assert!(engine.level().is_none());

        let full = watch(
            &mut engine,
            ENOUGH - 1,
            ENOUGH + STEADY as usize + 2,
            None,
            &climbing,
        );
        let read = full.expect("a night with a range in it reads as something");
        assert_eq!(read.basis, Basis::Measured);
        assert_eq!(read.certainty, Certainty::Fair);
        assert!(engine.level().is_some());
    }

    /// A night where nothing changes has no range, and a position inside a
    /// distribution that has not spread out is arithmetic rather than news.
    #[test]
    fn a_night_that_never_varies_reads_as_nothing() {
        let mut engine = ContextEngine::new();
        let flat = watch(&mut engine, 0, ENOUGH * 2, None, &|_| 0.5);
        assert!(
            flat.is_none(),
            "read a phase off {} identical readings",
            engine.readings()
        );
    }

    /// The DJ's word is believed on its own, and says so.
    #[test]
    fn a_declaration_is_believed_and_labelled_as_a_declaration() {
        let mut engine = ContextEngine::new();
        let read = engine
            .observe(&Observation {
                elapsed: Duration::from_secs(30),
                hour: Some(21),
                playing: 1,
                tempo: Some(128.0),
                audio: AudioMetrics::from_bands([0.3; 4]),
                declared: Some(SessionPhase::Peak),
            })
            .expect("a declared night is a night");
        assert_eq!(read.phase, SessionPhase::Peak);
        assert_eq!(read.basis, Basis::Declared);
        assert_eq!(read.certainty, Certainty::Fair);
        assert_eq!(read.drift, None);
        assert_eq!(read.environment.time_of_day, TimeOfDay::Night);
    }

    /// Nothing declared and nothing measured is not a warm-up. It is nothing.
    #[test]
    fn silence_with_nothing_declared_reads_as_nothing() {
        let mut engine = ContextEngine::new();
        for index in 0..600u32 {
            assert_eq!(
                engine.observe(&Observation {
                    elapsed: SAMPLE * index,
                    hour: Some(2),
                    playing: 0,
                    tempo: None,
                    audio: AudioMetrics::default(),
                    declared: None,
                }),
                None
            );
        }
        assert_eq!(engine.readings(), 0, "counted readings of silence");
    }

    /// Where the two disagree the DJ wins, and the disagreement is reported
    /// rather than swallowed.
    #[test]
    fn a_disagreement_keeps_the_declaration_and_names_the_drift() {
        let mut engine = ContextEngine::new();
        // A night that climbs to the top of its own range, declared as a
        // warm-up throughout.
        let read = watch(
            &mut engine,
            0,
            ENOUGH + 40,
            Some(SessionPhase::WarmUp),
            &climbing,
        )
        .expect("a night with both sources reads as something");
        assert_eq!(read.phase, SessionPhase::WarmUp, "overruled the DJ");
        assert_eq!(read.basis, Basis::Disputed);
        assert_eq!(read.certainty, Certainty::Unsure);
        assert_eq!(read.drift, Some(Drift::Hotter));
    }

    /// And where they agree, that is the one thing worth being sure about.
    #[test]
    fn agreement_is_the_only_way_to_be_sure() {
        let mut engine = ContextEngine::new();
        let measured = watch(&mut engine, 0, ENOUGH + 40, None, &climbing)
            .expect("a night with a range reads as something");
        let mut engine = ContextEngine::new();
        let agreed = watch(&mut engine, 0, ENOUGH + 40, Some(measured.phase), &climbing)
            .expect("the same night, declared");
        assert_eq!(agreed.phase, measured.phase);
        assert_eq!(agreed.basis, Basis::Agreed);
        assert_eq!(agreed.certainty, Certainty::Sure);
        assert!(
            Certainty::Sure > Certainty::Fair && Certainty::Fair > Certainty::Unsure,
            "certainty has to be ordered for autonomy to be gated on it"
        );
    }

    /// A name that changes every few seconds is worse than a name that is
    /// twelve seconds late.
    #[test]
    fn one_odd_reading_does_not_rename_the_night() {
        let mut engine = ContextEngine::new();
        let settled = watch(&mut engine, 0, ENOUGH + 40, None, &climbing)
            .expect("a night with a range reads as something");

        // One reading at the very top, which on its own reads as peak.
        let after_one =
            watch(&mut engine, ENOUGH + 40, ENOUGH + 41, None, &|_| 1.0).expect("still a night");
        assert_eq!(
            after_one.phase, settled.phase,
            "renamed the night on a single reading"
        );

        // Held for three, and it is the night now.
        let after_three =
            watch(&mut engine, ENOUGH + 41, ENOUGH + 44, None, &|_| 1.0).expect("still a night");
        assert_eq!(after_three.phase, SessionPhase::Peak);
    }

    /// The two halves of the phase table have to agree with each other.
    #[test]
    fn a_phase_reads_back_from_the_band_it_occupies() {
        for phase in SessionPhase::ALL {
            let (low, high) = phase.band();
            assert!(low < high, "{} has an empty band", phase.name());
            let middle = (low + high) / 2.0;
            assert_eq!(
                SessionPhase::reading(middle, phase.is_rising()),
                phase,
                "{} does not read back from its own band",
                phase.name()
            );
        }
    }

    /// Every band of the range belongs to some phase, in both directions.
    #[test]
    fn every_level_belongs_to_a_phase() {
        for step in 0..=100 {
            #[allow(clippy::cast_precision_loss)]
            let level = step as f32 / 100.0;
            for rising in [true, false] {
                let phase = SessionPhase::reading(level, rising);
                let (low, high) = phase.band();
                assert!(
                    level >= low && level <= high,
                    "{level} read as {} but that phase is {low}..{high}",
                    phase.name()
                );
            }
        }
    }

    /// A disagreement that cannot say which way it disagrees is not worth
    /// reporting, and the interface has nothing to draw.
    #[test]
    fn a_disagreement_always_names_a_direction() {
        let middle_of = |phase: SessionPhase| {
            let (low, high) = phase.band();
            (low + high) / 2.0
        };
        for declared in SessionPhase::ALL {
            for measured in SessionPhase::ALL {
                if declared == measured {
                    continue;
                }
                assert!(
                    drift_between(declared, measured, middle_of(measured)).is_some(),
                    "{} against {} names no direction",
                    declared.name(),
                    measured.name()
                );
            }
        }
    }

    /// The night's own range, and what a position in it means.
    #[test]
    fn a_spread_places_a_reading_against_its_own_night() {
        let mut spread = Spread::new();
        assert_eq!(spread.below(0.5), None, "an empty night placed something");
        for step in 0..100 {
            #[allow(clippy::cast_precision_loss)]
            spread.add(step as f32 / 100.0);
        }
        assert_eq!(spread.total(), 100);
        assert_eq!(spread.width(), BUCKETS);
        let low = spread
            .below(0.05)
            .expect("a night with readings places one");
        let high = spread.below(0.95).expect("and another");
        assert!(low < 0.2, "the bottom of the night placed at {low}");
        assert!(high > 0.8, "the top of the night placed at {high}");

        // A broken reading moves nothing.
        spread.add(f32::NAN);
        assert_eq!(spread.total(), 100);
        assert_eq!(spread.below(f32::NAN), None);
    }

    /// A night with no variety at all still answers "about half", which is what
    /// no news looks like.
    #[test]
    fn a_night_of_one_value_places_it_in_the_middle() {
        let mut spread = Spread::new();
        for _ in 0..50 {
            spread.add(0.42);
        }
        assert_eq!(spread.width(), 1);
        let placed = spread.below(0.42).expect("fifty readings place one");
        assert!((placed - 0.5).abs() < 1e-6, "placed at {placed}");
    }

    /// Energy is a fraction, whatever it was derived from.
    #[test]
    fn energy_stays_inside_its_range() {
        let mut engine = ContextEngine::new();
        for index in 0..(ENOUGH * 3) {
            #[allow(clippy::cast_precision_loss)]
            let wobble = ((index as f32) * 0.7).sin().mul_add(0.5, 0.5);
            if let Some(read) = watch(&mut engine, index, index + 1, None, &|_| wobble) {
                assert!(
                    (0.0..=1.0).contains(&read.energy),
                    "energy left its range at {}",
                    read.energy
                );
            }
        }
    }

    /// The gap between two records is not the night collapsing.
    #[test]
    fn silence_between_records_holds_the_energy_it_had() {
        let mut engine = ContextEngine::new();
        let playing = watch(&mut engine, 0, 60, Some(SessionPhase::Heat), &|_| 0.8)
            .expect("a declared night is a night");
        let quiet = engine
            .observe(&Observation {
                elapsed: SAMPLE * 61,
                hour: Some(23),
                playing: 0,
                tempo: None,
                audio: AudioMetrics::default(),
                declared: Some(SessionPhase::Heat),
            })
            .expect("still a declared night");
        assert!(
            (quiet.energy - playing.energy).abs() < 1e-6,
            "a gap between records dropped the energy from {} to {}",
            playing.energy,
            quiet.energy
        );
    }

    /// The last answer and a fresh one are the same answer.
    #[test]
    fn reading_back_gives_what_was_last_published() {
        let mut engine = ContextEngine::new();
        assert_eq!(engine.read(), None);
        let read = watch(&mut engine, 0, 10, Some(SessionPhase::ChillOut), &|_| 0.2);
        assert_eq!(engine.read(), read);
    }

    /// The default has to be the honest one: nothing has read the room.
    #[test]
    fn a_fresh_context_claims_nothing_about_the_room() {
        assert!(SessionContext::default().session.is_none());
    }
}
