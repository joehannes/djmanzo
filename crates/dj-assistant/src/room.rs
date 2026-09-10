//! What the room is doing, as far as anything can honestly tell.
//!
//! # Why every reading is relative to tonight
//!
//! A camera's idea of "bright" is its own. Two phones pointed at the same wall
//! report different numbers, the same phone reports different numbers when
//! somebody moves it, and a microphone's level depends on where it was put
//! down. So an absolute threshold — "movement above 0.4 means the floor is
//! busy" — is a number that means something in one venue on one device and
//! nothing anywhere else.
//!
//! What *is* portable is a comparison with the same room earlier the same
//! night, through the same lens, from the same place. "Stiller than it has
//! been all night" is a true sentence about a number nobody calibrated. That
//! is the only kind of sentence this produces.
//!
//! # Why it never names a mood
//!
//! A camera can measure how much of the frame changed. It cannot tell whether
//! people are dancing or leaving, and a module that says "the crowd is loving
//! it" from a difference of pixels is lying with statistics. So the vocabulary
//! here is movement, light and loudness — the things actually measured — and
//! the one interpretation offered is a *disagreement*: the room is doing
//! something other than what the DJ set the night up to be. That is a fact
//! about two numbers, and it is the DJ who decides what it means.
//!
//! # What is not here
//!
//! **Weather.** It is not a sensor reading; it is a location plus somebody
//! else's API, and pretending a camera can see rain would be inventing data.
//! **Time of day** is here, because a clock is a real instrument.

use crate::Occasion;
use dj_core::{SessionPhase, Spread};
use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

/// How long a reading stays in the near window.
///
/// Three minutes. Long enough that somebody walking past the lens, or one
/// shout near the microphone, does not move it; short enough that a floor
/// emptying shows up while there is still a record left to change it with.
pub const NEAR: Duration = Duration::from_secs(3 * 60);

/// How many readings the near window needs before anything is said.
///
/// At the cadence the interface sends them — one every two seconds — this is
/// half a minute of looking. Below it the answer is "not yet", which is a
/// better answer than a confident one drawn from four frames.
pub const ENOUGH: usize = 15;

/// How long a reading stays in the recent window.
///
/// Twenty minutes: §35's *recent room activity*, as distinct from both the
/// three minutes that are *current* and the whole night that is *earlier*.
/// Chosen as roughly five records — long enough that it is a stretch of the
/// night rather than a moment, short enough that a DJ can remember what they
/// played across it and therefore act on the comparison.
pub const RECENT: Duration = Duration::from_secs(20 * 60);

/// How many readings a phase needs before the room can be compared against it.
///
/// **The guard against a self-referential answer.** A reading taken now is
/// filed under the phase the night is in now, so the near window is *inside*
/// the phase's own distribution — and a phase entered two minutes ago would be
/// comparing the room mostly with itself and answering "about usual" with
/// confidence. Four times the near window's minimum means at least three
/// quarters of what is being compared against is some other stretch at this
/// phase.
pub const ENOUGH_AT_PHASE: usize = ENOUGH * 4;

/// What is being measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sense {
    /// How bright the frame is. Nothing to do with the lighting rig's
    /// intentions -- a smoke machine reads as bright, and that is honest.
    Light,
    /// How much of the frame changed since the last one.
    Movement,
    /// How loud it is where the microphone is.
    Loudness,
}

impl Sense {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Movement => "movement",
            Self::Loudness => "loudness",
        }
    }

    /// The word for more of this, as a DJ would say it.
    ///
    /// Per sense rather than from a template, for the reason
    /// [`Against::phrase`] gives: "movement is higher" is not a sentence
    /// anybody says. Only the *comparative* is composed here; what it is
    /// being compared against comes from [`Horizon`], which is a different
    /// half of the sentence and cannot be got wrong by the same mistake.
    #[must_use]
    pub const fn more(self) -> &'static str {
        match self {
            Self::Light => "brighter",
            Self::Movement => "busier",
            Self::Loudness => "louder",
        }
    }

    /// The word for less of it.
    #[must_use]
    pub const fn less(self) -> &'static str {
        match self {
            Self::Light => "darker",
            Self::Movement => "stiller",
            Self::Loudness => "quieter",
        }
    }
}

/// Where a reading sits against the same room earlier tonight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Against {
    Lowest,
    Lower,
    Usual,
    Higher,
    Highest,
}

impl Against {
    /// Whether this is worth a sentence. The usual is not news.
    #[must_use]
    pub fn is_notable(self) -> bool {
        self != Self::Usual
    }

    /// The stable slug, so an interface can style it without re-deriving it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Lowest => "lowest",
            Self::Lower => "lower",
            Self::Usual => "usual",
            Self::Higher => "higher",
            Self::Highest => "highest",
        }
    }

    /// Which way it is off, or `None` when it is where it usually is.
    ///
    /// The half of a comparison that is about the *number*. [`Horizon`]
    /// supplies the half that is about what it was compared with.
    #[must_use]
    pub const fn direction(self, sense: Sense) -> Option<&'static str> {
        match self {
            Self::Lowest | Self::Lower => Some(sense.less()),
            Self::Usual => None,
            Self::Higher | Self::Highest => Some(sense.more()),
        }
    }

    /// How it reads, in the words that sense deserves.
    ///
    /// Written out per sense rather than assembled from a template, because
    /// "darker than it has been" and "quieter than it has been" are not the
    /// same sentence with a word swapped, and a template would produce
    /// "movement is lower than it has been", which nobody says.
    #[must_use]
    pub fn phrase(self, sense: Sense) -> &'static str {
        match (sense, self) {
            (Sense::Light, Self::Lowest) => "darker than it has been all night",
            (Sense::Light, Self::Lower) => "darker than usual tonight",
            (Sense::Light, Self::Usual) => "about as lit as usual",
            (Sense::Light, Self::Higher) => "brighter than usual tonight",
            (Sense::Light, Self::Highest) => "brighter than it has been all night",
            (Sense::Movement, Self::Lowest) => "stiller than it has been all night",
            (Sense::Movement, Self::Lower) => "stiller than usual tonight",
            (Sense::Movement, Self::Usual) => "moving about as usual",
            (Sense::Movement, Self::Higher) => "busier than usual tonight",
            (Sense::Movement, Self::Highest) => "busier than it has been all night",
            (Sense::Loudness, Self::Lowest) => "quieter than it has been all night",
            (Sense::Loudness, Self::Lower) => "quieter than usual tonight",
            (Sense::Loudness, Self::Usual) => "about as loud as usual",
            (Sense::Loudness, Self::Higher) => "louder than usual tonight",
            (Sense::Loudness, Self::Highest) => "louder than it has been all night",
        }
    }
}

/// One moment's look at the room.
///
/// Every field optional because a source may offer some and not others: a
/// camera with no microphone permission has light and movement and no
/// loudness, and half a reading is worth keeping.
#[derive(Debug, Clone, Copy)]
pub struct Reading {
    pub at: SystemTime,
    /// 0..1. Average luminance of the frame.
    pub light: Option<f32>,
    /// 0..1. How much of the frame changed since the last one.
    pub movement: Option<f32>,
    /// 0..1. Loudness where the microphone is.
    pub loudness: Option<f32>,
}

impl Reading {
    #[must_use]
    pub fn at(at: SystemTime) -> Self {
        Self {
            at,
            light: None,
            movement: None,
            loudness: None,
        }
    }

    /// Clamped on the way in, so one bad frame cannot skew the night's range.
    #[must_use]
    pub fn with(mut self, sense: Sense, value: f32) -> Self {
        let value = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            return self;
        };
        match sense {
            Sense::Light => self.light = Some(value),
            Sense::Movement => self.movement = Some(value),
            Sense::Loudness => self.loudness = Some(value),
        }
        self
    }

    #[must_use]
    fn get(&self, sense: Sense) -> Option<f32> {
        match sense {
            Sense::Light => self.light,
            Sense::Movement => self.movement,
            Sense::Loudness => self.loudness,
        }
    }

    /// Whether anything at all was measured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.light.is_none() && self.movement.is_none() && self.loudness.is_none()
    }
}

/// Where the near window sits inside the night.
///
/// The bands are deliberately wide in the middle: a fifth of a night at each
/// end is "notable", and the three fifths between them are the room carrying
/// on, which is not something to interrupt a DJ about.
fn against(fraction: f32) -> Against {
    match fraction {
        f if f < 0.10 => Against::Lowest,
        f if f < 0.30 => Against::Lower,
        f if f < 0.70 => Against::Usual,
        f if f < 0.90 => Against::Higher,
        _ => Against::Highest,
    }
}

/// What one stretch of the night measured, per sense.
///
/// Three distributions kept together because they are always filled together
/// and always asked together. Separate `light`/`movement`/`loudness` fields at
/// every level was how the first version of this grew, and adding a fourth
/// horizon to it would have meant three more fields in three more places.
#[derive(Debug, Default)]
struct Senses {
    light: Spread,
    movement: Spread,
    loudness: Spread,
}

impl Senses {
    fn add(&mut self, reading: &Reading) {
        if let Some(light) = reading.light {
            self.light.add(light);
        }
        if let Some(movement) = reading.movement {
            self.movement.add(movement);
        }
        if let Some(loudness) = reading.loudness {
            self.loudness.add(loudness);
        }
    }

    const fn of(&self, sense: Sense) -> &Spread {
        match sense {
            Sense::Light => &self.light,
            Sense::Movement => &self.movement,
            Sense::Loudness => &self.loudness,
        }
    }
}

/// What the room is being compared against.
///
/// [§35 of the directive](../../../docs/DIRECTIVE.md) names four things:
/// *current* room activity, *recent* room activity, *earlier* room activity,
/// and *activity at similar session phases*. The first is the number being
/// placed; these are the three places to put it.
///
/// It is an enum rather than three methods because §35's point is that they
/// are the **same comparison at different reaches** — and because a sentence
/// that says how the room is doing has to say what it is doing it against, or
/// it is the global threshold §35 opens by refusing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Horizon {
    /// The last twenty minutes. §35's *recent room activity*.
    Recent,
    /// The whole night so far. §35's *earlier room activity*.
    Tonight,
    /// Every stretch of tonight that was at this phase.
    ///
    /// §35's *activity at similar session phases*, and the only one of the
    /// four that is not simply a longer window: a peak an hour ago is a better
    /// comparison for a peak now than the twenty minutes of cooldown between
    /// them, however recent those are.
    LikePhase(SessionPhase),
}

impl Horizon {
    /// The tail of the sentence: what the room is being compared with.
    #[must_use]
    pub fn than(self) -> String {
        match self {
            Self::Recent => "than it has been for the last twenty minutes".to_owned(),
            Self::Tonight => "than it has been tonight".to_owned(),
            Self::LikePhase(phase) => format!(
                "than it usually is when the night is at {}",
                phase.name().replace('_', " ")
            ),
        }
    }
}

/// Where the room is now, against every reach §35 asks for.
///
/// **A baseline, not a threshold.** Every field is a position inside a
/// distribution this room produced tonight, so nothing here means anything
/// absolute and nothing here needs a calibrated camera — which is §35's
/// opening sentence expressed as a type.
#[derive(Debug, Clone, PartialEq)]
pub struct Baseline {
    pub sense: Sense,
    /// The middle of the last three minutes. §35's *current room activity*,
    /// and the number the three comparisons are of.
    pub now: f32,
    /// Where it sits against each reach, in §35's order. A reach with too
    /// little in it is absent rather than guessed — a night that has not
    /// reached this phase twice has nothing to say about it.
    pub against: Vec<(Horizon, Against)>,
}

impl Baseline {
    /// What it says, one sentence per reach that is not the usual.
    ///
    /// The usual is not news at any reach, which is why a room carrying on
    /// produces nothing at all rather than three sentences saying so.
    #[must_use]
    pub fn notes(&self) -> Vec<String> {
        self.against
            .iter()
            .filter_map(|(horizon, against)| {
                let direction = against.direction(self.sense)?;
                Some(format!("The room is {direction} {}.", horizon.than()))
            })
            .collect()
    }

    /// Whether any reach has something to say.
    #[must_use]
    pub fn is_notable(&self) -> bool {
        self.against.iter().any(|(_, a)| a.is_notable())
    }
}

/// Where a value sits inside a distribution, when it can be placed at all.
///
/// `least` is how many readings the distribution needs first. Note what is
/// *not* guarded: a spread whose readings all land in one bucket still
/// answers, and answers `Usual` — see [`Spread::below`]. A room that has not
/// changed all night should read as no news, not as no answer.
fn place(spread: &Spread, value: f32, least: usize) -> Option<Against> {
    if (spread.total() as usize) < least {
        return None;
    }
    spread.below(value).map(against)
}

/// What the room has been doing.
#[derive(Debug, Default)]
pub struct Room {
    /// Readings back to [`RECENT`], newest last. The near window is the tail
    /// of it rather than a second buffer: two deques of the same readings is
    /// two chances to forget from one and not the other.
    seen: VecDeque<Reading>,
    /// The whole night, which nothing ages out of.
    tonight: Senses,
    /// The night again, split by the phase it was in at the time.
    ///
    /// An array with a slot per phase rather than a map: every phase has a
    /// place whether or not the night has reached it, so "we have never been
    /// here" is an empty distribution — which [`place`] already refuses —
    /// rather than a missing key that every caller has to remember to handle.
    phases: [Senses; SessionPhase::ALL.len()],
}

/// Which slot a phase's readings go in.
///
/// Derived from `SessionPhase::ALL` rather than written out, so a sixth phase
/// is one line in `dj_core` and not a silent mis-filing here. `ALL` is the
/// complete list, so the fallback is unreachable; it is a zero rather than a
/// panic because a reading is not worth crashing a set for.
fn slot(phase: SessionPhase) -> usize {
    SessionPhase::ALL
        .iter()
        .position(|other| *other == phase)
        .unwrap_or(0)
}

impl Room {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take a reading, and forget the ones that have aged out of the window.
    ///
    /// An empty reading is dropped: a source that measured nothing has said
    /// nothing, and counting it would dilute the window with silence.
    ///
    /// `phase` is what the night was judged to be **at the moment of the
    /// reading**, and it is taken here rather than looked up later for the
    /// reason §35 exists: "what was the room like the last time we were at
    /// peak" is a question about the past, and asking it of the current phase
    /// would file every reading under whatever the night happens to be by the
    /// time somebody looks. `None` when the night has not read yet, which is a
    /// real state and not a default — see `dj_app::night`.
    pub fn saw(&mut self, reading: Reading, phase: Option<SessionPhase>) {
        if reading.is_empty() {
            return;
        }
        self.tonight.add(&reading);
        if let Some(phase) = phase {
            self.phases[slot(phase)].add(&reading);
        }
        self.seen.push_back(reading);
        self.forget_before(reading.at);
    }

    fn forget_before(&mut self, now: SystemTime) {
        while let Some(oldest) = self.seen.front() {
            let old = now
                .duration_since(oldest.at)
                .is_ok_and(|since| since > RECENT);
            if old {
                self.seen.pop_front();
            } else {
                break;
            }
        }
    }

    /// The readings inside `window` of the newest one.
    ///
    /// Measured back from the last reading rather than from the clock: a
    /// window that emptied because nothing has looked for five minutes should
    /// say "not enough" through [`ENOUGH`], not silently become a window of
    /// one. It also makes every test here a matter of the timestamps it wrote
    /// rather than of when it ran.
    fn within(&self, window: Duration, sense: Sense) -> Vec<f32> {
        let Some(newest) = self.seen.back().map(|reading| reading.at) else {
            return Vec::new();
        };
        self.seen
            .iter()
            .filter(|reading| {
                newest
                    .duration_since(reading.at)
                    .is_ok_and(|since| since <= window)
            })
            .filter_map(|reading| reading.get(sense))
            .collect()
    }

    /// The middle of the near window for one sense.
    ///
    /// A median rather than a mean: one frame where somebody walked across the
    /// lens is an outlier, and a mean carries it for three minutes.
    #[must_use]
    pub fn lately(&self, sense: Sense) -> Option<f32> {
        let mut values = self.within(NEAR, sense);
        if values.len() < ENOUGH {
            return None;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        Some(values[values.len() / 2])
    }

    /// Where one sense sits against the rest of tonight.
    #[must_use]
    pub fn against(&self, sense: Sense) -> Option<Against> {
        self.at(Horizon::Tonight, sense)
    }

    /// Where one sense sits against a given reach. §35's comparison, once.
    ///
    /// `None` when there is not enough to place it: too few readings in the
    /// near window, or a reach that has not spread out enough to place
    /// anything inside. The second is the guard `Spread::width` exists for —
    /// a position inside a distribution of one value is arithmetic rather
    /// than information, and it would read as a confident "about usual".
    #[must_use]
    pub fn at(&self, horizon: Horizon, sense: Sense) -> Option<Against> {
        let lately = self.lately(sense)?;
        match horizon {
            Horizon::Recent => {
                let mut spread = Spread::new();
                for value in self.within(RECENT, sense) {
                    spread.add(value);
                }
                place(&spread, lately, ENOUGH)
            }
            Horizon::Tonight => place(self.tonight.of(sense), lately, ENOUGH),
            Horizon::LikePhase(phase) => {
                place(self.phases[slot(phase)].of(sense), lately, ENOUGH_AT_PHASE)
            }
        }
    }

    /// Where the room is now against every reach §35 names.
    ///
    /// `phase` is what the night is *now* — the one place a current phase
    /// belongs, because the question being asked is "how does this compare
    /// with the other times we were here". `None` leaves the phase reach out
    /// rather than guessing one, and the other two still answer.
    #[must_use]
    pub fn baseline(&self, sense: Sense, phase: Option<SessionPhase>) -> Option<Baseline> {
        let now = self.lately(sense)?;
        let reaches = [
            Some(Horizon::Recent),
            Some(Horizon::Tonight),
            phase.map(Horizon::LikePhase),
        ];
        Some(Baseline {
            sense,
            now,
            against: reaches
                .into_iter()
                .flatten()
                .filter_map(|horizon| Some((horizon, self.at(horizon, sense)?)))
                .collect(),
        })
    }

    /// How many readings are in the near window.
    #[must_use]
    pub fn recent(&self) -> usize {
        let Some(newest) = self.seen.back().map(|reading| reading.at) else {
            return 0;
        };
        self.seen
            .iter()
            .filter(|reading| {
                newest
                    .duration_since(reading.at)
                    .is_ok_and(|since| since <= NEAR)
            })
            .count()
    }

    /// When the last reading arrived, if any has.
    ///
    /// So that "something is watching" is answered by the readings themselves
    /// rather than by a flag somebody has to remember to clear: a window that
    /// closed without saying so cannot leave the panel claiming to watch a
    /// room nothing is looking at.
    #[must_use]
    pub fn last_seen(&self) -> Option<SystemTime> {
        self.seen.back().map(|reading| reading.at)
    }

    /// Whether there is enough to say anything at all.
    #[must_use]
    pub fn has_looked_enough(&self) -> bool {
        [Sense::Light, Sense::Movement, Sense::Loudness]
            .into_iter()
            .any(|sense| self.lately(sense).is_some())
    }

    /// Everything worth saying, in the order it matters.
    ///
    /// Movement first: it is the one about the floor. Nothing is said about a
    /// sense sitting where it usually sits.
    ///
    /// # Why the shorter reaches have to earn their sentence
    ///
    /// §35 asks for four comparisons and does not ask for four sentences. A
    /// floor that has emptied is below its night, below its last twenty
    /// minutes and below every other peak, and saying all three is a panel a
    /// DJ stops reading. So the night's sentence is the one always said, and
    /// the other reaches speak only when they **disagree** with it — which is
    /// the case they were worth adding for: *quieter than it has been tonight,
    /// busier than the last twenty minutes* is a room coming back, and neither
    /// half says that alone.
    #[must_use]
    pub fn notes(&self, phase: Option<SessionPhase>) -> Vec<String> {
        let mut said = Vec::new();
        for sense in [Sense::Movement, Sense::Loudness, Sense::Light] {
            let tonight = self.at(Horizon::Tonight, sense);
            if let Some(against) = tonight.filter(|a| a.is_notable()) {
                said.push(format!("The room is {}.", against.phrase(sense)));
            }
            let night_way = tonight.and_then(|a| a.direction(sense));
            for horizon in [Horizon::Recent]
                .into_iter()
                .chain(phase.map(Horizon::LikePhase))
            {
                let Some(against) = self.at(horizon, sense) else {
                    continue;
                };
                let Some(way) = against.direction(sense) else {
                    continue;
                };
                if Some(way) != night_way {
                    said.push(format!("The room is {way} {}.", horizon.than()));
                }
            }
        }
        said
    }

    /// Where the room disagrees with the night the DJ set up.
    ///
    /// The one interpretation offered, and it is a comparison of two things
    /// djmanzo actually knows: what the DJ said the night is, and what the
    /// sensors have measured. It never says what to play — that is the
    /// planner's job and [ADR-0005](../../../docs/adr/0005-assistant-speaks-actions.md)'s
    /// rule — only that the two do not match.
    #[must_use]
    pub fn disagrees_with(&self, occasion: Occasion) -> Option<String> {
        let movement = self.against(Sense::Movement)?;
        let quiet = matches!(movement, Against::Lowest | Against::Lower);
        let busy = matches!(movement, Against::Higher | Against::Highest);

        match occasion {
            Occasion::Peak if quiet => Some(format!(
                "You have tonight set to peak, and the floor is {}.",
                movement.phrase(Sense::Movement)
            )),
            Occasion::WarmUp | Occasion::Background if busy => Some(format!(
                "You have tonight set to {}, and the floor is {}.",
                occasion.name(),
                movement.phrase(Sense::Movement)
            )),
            Occasion::Close if busy => Some(format!(
                "You have tonight set to close, and the floor is {}.",
                movement.phrase(Sense::Movement)
            )),
            _ => None,
        }
    }
}

/// What hour it is, for a night that has a shape.
///
/// Separate from the sensors because a clock is not a camera: it is exact,
/// always available, and needs no permission. It is here so that the one place
/// asking "what is the room like" gets the answer that matters most and is
/// cheapest, rather than only the expensive uncertain ones.
#[must_use]
pub fn hour_of(at: SystemTime) -> Option<u8> {
    let since = at.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    let seconds_today = since.as_secs() % (24 * 60 * 60);
    #[allow(clippy::cast_possible_truncation)]
    Some((seconds_today / 3600) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + seconds)
    }

    /// Fill a night at one level, then look at it at another.
    fn night(early: f32, late: f32, sense: Sense) -> Room {
        let mut room = Room::new();
        // Two hours of the night at `early`, spaced so they age out of the
        // near window and survive only in the night's distribution.
        for n in 0..200u64 {
            room.saw(Reading::at(at(n * 30)).with(sense, early), None);
        }
        // Then half a minute of `late`, which is the near window.
        let start = 200 * 30;
        for n in 0..ENOUGH as u64 {
            room.saw(Reading::at(at(start + n * 2)).with(sense, late), None);
        }
        room
    }

    /// A night that ran at `early`, then `middle` for the last twenty minutes,
    /// then `late` for the last half minute.
    ///
    /// Three levels because §35's whole point is that two reaches can
    /// disagree: the near window against the night is one answer and against
    /// the last twenty minutes is another, and a fixture with only two levels
    /// cannot tell them apart.
    fn three_ways(early: f32, middle: f32, late: f32, sense: Sense) -> Room {
        let mut room = Room::new();
        // The timings are the point of the fixture, so they are written out.
        // The last reading lands at 7128; the near window is the 180 seconds
        // before it and the recent window the 1200 before it. Each stretch is
        // placed to fall inside exactly the windows it is meant to:
        //
        //   early   0 .. 5910   before the recent window opens at 5928
        //   middle  6000 .. 6870   inside recent, before the near window at 6948
        //   late    7100 .. 7128   the near window
        //
        // The first draft spaced `middle` two seconds apart and ran it up to
        // the near window, so seventy-six of its readings were *inside* that
        // window and the median it measured was the middle level rather than
        // the late one. The fixture was wrong, and it looked like the median.
        for n in 0..198u64 {
            room.saw(Reading::at(at(n * 30)).with(sense, early), None);
        }
        for n in 0..30u64 {
            room.saw(Reading::at(at(6000 + n * 30)).with(sense, middle), None);
        }
        for n in 0..ENOUGH as u64 {
            room.saw(Reading::at(at(7100 + n * 2)).with(sense, late), None);
        }
        room
    }

    /// **The four reaches §35 asks for, and no absolute number anywhere.**
    ///
    /// The section opens by refusing "80 dB = high energy". What replaces it
    /// is a position inside a distribution this room produced, at three
    /// reaches, and the test of that is that the *same* absolute value reads
    /// differently depending on which stretch it is put against.
    #[test]
    fn the_same_number_reads_differently_at_different_reaches() {
        // A quiet night, a loud last twenty minutes, and a middling now: above
        // the night, below the recent stretch. One number, two answers.
        let room = three_ways(0.1, 0.9, 0.5, Sense::Movement);
        let read = room
            .baseline(Sense::Movement, None)
            .expect("enough to say something");

        assert!((read.now - 0.5).abs() < 0.06, "now is {}", read.now);
        let at_recent = room.at(Horizon::Recent, Sense::Movement).expect("recent");
        let at_night = room.at(Horizon::Tonight, Sense::Movement).expect("tonight");
        assert!(
            at_recent < Against::Usual,
            "against the loud last twenty minutes it should read low, got {at_recent:?}"
        );
        assert!(
            at_night > Against::Usual,
            "against the quiet night it should read high, got {at_night:?}"
        );
    }

    /// **Two reaches that disagree both get a sentence; two that agree do not.**
    ///
    /// The reason the shorter reaches are in §35 at all. "Quieter than it has
    /// been tonight, busier than the last twenty minutes" is a room coming
    /// back, and neither half says that alone. Three sentences all saying the
    /// floor has emptied is a panel nobody finishes.
    #[test]
    fn a_reach_earns_a_sentence_by_disagreeing() {
        // Busy night, dead last twenty minutes, coming back now.
        let coming_back = three_ways(0.9, 0.05, 0.5, Sense::Movement);
        let said = coming_back.notes(None);
        assert_eq!(
            said.len(),
            2,
            "a room coming back has two things to say: {said:?}"
        );
        assert!(said[0].contains("stiller"), "{said:?}");
        assert!(
            said[1].contains("busier") && said[1].contains("twenty minutes"),
            "{said:?}"
        );

        // And a floor that has simply emptied says it once, not twice.
        let emptied = three_ways(0.9, 0.9, 0.05, Sense::Movement);
        let said = emptied.notes(None);
        assert_eq!(said.len(), 1, "one fact, one sentence: {said:?}");
        assert!(said[0].contains("stiller"), "{said:?}");
    }

    /// **A phase cannot be compared with itself.**
    ///
    /// The guard that matters most here: a reading is filed under the phase
    /// the night is in *now*, so the near window is inside the phase's own
    /// distribution. Without [`ENOUGH_AT_PHASE`] a night that reached peak a
    /// minute ago would confidently report the room as usual for a peak, on
    /// the evidence of that minute.
    #[test]
    fn a_phase_just_entered_says_nothing_about_itself() {
        let mut room = Room::new();
        // A long warm-up, then a peak that is only the near window long.
        for n in 0..200u64 {
            room.saw(
                Reading::at(at(n * 30)).with(Sense::Movement, 0.2),
                Some(SessionPhase::WarmUp),
            );
        }
        let start = 200 * 30;
        for n in 0..ENOUGH as u64 {
            room.saw(
                Reading::at(at(start + n * 2)).with(Sense::Movement, 0.9),
                Some(SessionPhase::Peak),
            );
        }

        assert_eq!(
            room.at(Horizon::LikePhase(SessionPhase::Peak), Sense::Movement),
            None,
            "a peak two minutes old has nothing to say about peaks"
        );
        let read = room
            .baseline(Sense::Movement, Some(SessionPhase::Peak))
            .expect("the other reaches still answer");
        assert!(
            !read
                .against
                .iter()
                .any(|(h, _)| matches!(h, Horizon::LikePhase(_))),
            "the phase reach appeared without the evidence for it: {read:?}"
        );
        assert_eq!(read.against.len(), 2, "{read:?}");
    }

    /// **A phase says the thing no other reach can.**
    ///
    /// The case §35 adds the fourth comparison for. A long quiet warm-up drags
    /// the night's own middle down, so a floor at 0.6 reads as *unremarkable
    /// for tonight* — and it is well below where this room has been every
    /// other time the night was at peak. Neither the night nor the last twenty
    /// minutes can say that; only the phase can, and the twenty minutes in
    /// between were a cooldown that has nothing to do with it.
    #[test]
    fn a_phase_with_a_history_says_what_no_other_reach_can() {
        let mut room = Room::new();
        let fill = |room: &mut Room, from: u64, count: u64, level: f32, phase: SessionPhase| {
            for n in 0..count {
                room.saw(
                    Reading::at(at(from + n * 30)).with(Sense::Movement, level),
                    Some(phase),
                );
            }
        };
        // A long warm-up, an hour of peak with the floor full, a cooldown, and
        // then peak again with the floor half of what it was.
        fill(&mut room, 0, 200, 0.2, SessionPhase::WarmUp);
        fill(&mut room, 6000, 120, 0.9, SessionPhase::Peak);
        fill(&mut room, 9600, 40, 0.3, SessionPhase::Cooldown);
        for n in 0..ENOUGH as u64 {
            room.saw(
                Reading::at(at(11_000 + n * 2)).with(Sense::Movement, 0.6),
                Some(SessionPhase::Peak),
            );
        }

        let phase = SessionPhase::Peak;
        let at_phase = room
            .at(Horizon::LikePhase(phase), Sense::Movement)
            .expect("an hour of peak is enough to compare against");
        assert!(
            at_phase < Against::Usual,
            "0.6 against an hour of 0.9 at peak should read low, got {at_phase:?}"
        );
        assert_eq!(
            room.at(Horizon::Tonight, Sense::Movement),
            Some(Against::Usual),
            "against the whole night, including the warm-up, 0.6 is unremarkable \
             — which is exactly why the phase reach is worth having"
        );

        let said = room.notes(Some(phase));
        assert!(
            said.iter()
                .any(|note| note.contains("stiller") && note.contains("when the night is at peak")),
            "the one sentence only the phase could say was not said: {said:?}"
        );
    }

    /// The sentence names what it compared against, always.
    ///
    /// §35's opening line is a refusal of numbers that mean nothing without
    /// their context. A note that said "the room is busier" and stopped would
    /// be exactly that, one word shorter.
    #[test]
    fn every_reach_says_what_it_compared_with() {
        for horizon in [
            Horizon::Recent,
            Horizon::Tonight,
            Horizon::LikePhase(SessionPhase::ChillOut),
        ] {
            let than = horizon.than();
            assert!(than.starts_with("than "), "{than}");
            assert!(than.len() > 12, "{than} says nothing");
        }
        assert_eq!(
            Horizon::LikePhase(SessionPhase::ChillOut).than(),
            "than it usually is when the night is at chill out",
            "the phase's slug leaked into a sentence"
        );
    }

    /// Every comparative is written per sense, and the usual has none.
    #[test]
    fn each_sense_has_its_own_words_for_more_and_less() {
        let mut words = std::collections::BTreeSet::new();
        for sense in [Sense::Light, Sense::Movement, Sense::Loudness] {
            words.insert(sense.more());
            words.insert(sense.less());
            assert_eq!(Against::Usual.direction(sense), None);
            assert_eq!(Against::Highest.direction(sense), Some(sense.more()));
            assert_eq!(Against::Lowest.direction(sense), Some(sense.less()));
        }
        assert_eq!(words.len(), 6, "two senses share a comparative: {words:?}");
    }

    /// **Nothing is said until there is enough to say it from.**
    ///
    /// Four frames is not a room, and a confident sentence drawn from four
    /// frames is worse than silence: a DJ who acts on it once and finds it
    /// wrong stops reading the panel for the rest of the night.
    #[test]
    fn a_glance_is_not_a_reading() {
        let mut room = Room::new();
        for n in 0..(ENOUGH as u64 - 1) {
            room.saw(Reading::at(at(n * 2)).with(Sense::Movement, 0.5), None);
        }
        assert!(!room.has_looked_enough());
        assert_eq!(room.against(Sense::Movement), None);
        assert!(room.notes(None).is_empty());

        room.saw(Reading::at(at(100)).with(Sense::Movement, 0.5), None);
        assert!(room.has_looked_enough());
    }

    /// **A floor going still says so, and a floor filling up says so.**
    #[test]
    fn the_room_is_read_against_its_own_night() {
        let quietened = night(0.6, 0.05, Sense::Movement);
        assert_eq!(quietened.against(Sense::Movement), Some(Against::Lowest));
        assert!(
            quietened.notes(None)[0].contains("stiller"),
            "{:?}",
            quietened.notes(None)
        );

        let filled = night(0.1, 0.9, Sense::Movement);
        assert_eq!(filled.against(Sense::Movement), Some(Against::Highest));
        assert!(
            filled.notes(None)[0].contains("busier"),
            "{:?}",
            filled.notes(None)
        );
    }

    /// **A camera nobody calibrated still works.**
    ///
    /// The whole design: two rooms whose absolute numbers share no range at
    /// all read the same way, because each is read against itself. A threshold
    /// would call the first dark all night and the second bright all night,
    /// and neither would ever be news.
    #[test]
    fn two_uncalibrated_cameras_read_the_same() {
        // A dim lens: everything it ever sees is under a tenth.
        let dim = night(0.02, 0.09, Sense::Light);
        // A lens that blows out: everything it sees is over four fifths.
        let blown = night(0.81, 0.98, Sense::Light);
        assert_eq!(dim.against(Sense::Light), blown.against(Sense::Light));
        assert_eq!(dim.against(Sense::Light), Some(Against::Highest));
    }

    /// **A room carrying on is not news.**
    #[test]
    fn nothing_is_said_when_nothing_has_changed() {
        let steady = night(0.5, 0.5, Sense::Movement);
        assert_eq!(steady.against(Sense::Movement), Some(Against::Usual));
        assert!(steady.notes(None).is_empty(), "{:?}", steady.notes(None));
    }

    /// **A handful of odd frames do not move it.**
    ///
    /// Why the near window is a median and not a mean. Somebody stands in
    /// front of the lens for ten seconds: a quarter of the window goes to
    /// completely-changed, and a mean would call the room the busiest it has
    /// been all night, on the evidence of one person's back.
    #[test]
    fn a_few_odd_frames_are_outvoted() {
        let mut room = night(0.5, 0.5, Sense::Movement);
        assert_eq!(room.against(Sense::Movement), Some(Against::Usual));

        // Five frames of a body across the lens, against fifteen of the room.
        let start = 200 * 30 + 100;
        for n in 0..5u64 {
            room.saw(
                Reading::at(at(start + n * 2)).with(Sense::Movement, 1.0),
                None,
            );
        }
        assert_eq!(
            room.lately(Sense::Movement),
            Some(0.5),
            "the middle of the window moved"
        );
        assert_eq!(
            room.against(Sense::Movement),
            Some(Against::Usual),
            "five frames of somebody's back read as the busiest night"
        );
    }

    /// **Readings age out of the near window.**
    #[test]
    fn the_window_forgets() {
        let mut room = Room::new();
        for n in 0..40u64 {
            room.saw(Reading::at(at(n * 2)).with(Sense::Movement, 0.5), None);
        }
        assert_eq!(room.recent(), 40);
        // One reading, well past the window: everything older goes.
        room.saw(Reading::at(at(10_000)).with(Sense::Movement, 0.5), None);
        assert_eq!(room.recent(), 1);
        assert!(!room.has_looked_enough(), "a stale window still answered");
    }

    /// **Whether anything is watching comes from the readings.**
    #[test]
    fn the_last_reading_says_when_it_was() {
        let mut room = Room::new();
        assert_eq!(room.last_seen(), None);
        room.saw(Reading::at(at(5)).with(Sense::Light, 0.5), None);
        assert_eq!(room.last_seen(), Some(at(5)));
        room.saw(Reading::at(at(9)).with(Sense::Light, 0.5), None);
        assert_eq!(room.last_seen(), Some(at(9)));
        // An empty reading is not a sighting.
        room.saw(Reading::at(at(20)), None);
        assert_eq!(room.last_seen(), Some(at(9)));
    }

    /// **A source with no microphone still contributes.**
    #[test]
    fn half_a_reading_is_kept() {
        let mut room = Room::new();
        for n in 0..40u64 {
            room.saw(Reading::at(at(n * 2)).with(Sense::Light, 0.5), None);
        }
        assert!(room.lately(Sense::Light).is_some());
        assert_eq!(room.lately(Sense::Loudness), None);
    }

    /// **A reading of nothing is not a reading.**
    #[test]
    fn an_empty_reading_is_dropped() {
        let mut room = Room::new();
        for n in 0..40u64 {
            room.saw(Reading::at(at(n * 2)), None);
        }
        assert_eq!(room.recent(), 0);
    }

    /// **Impossible numbers cannot skew the night.**
    #[test]
    fn a_reading_outside_the_range_is_clamped_or_refused() {
        let clamped = Reading::at(at(0)).with(Sense::Movement, 40.0);
        assert_eq!(clamped.movement, Some(1.0));
        let negative = Reading::at(at(0)).with(Sense::Movement, -3.0);
        assert_eq!(negative.movement, Some(0.0));
        let nonsense = Reading::at(at(0)).with(Sense::Movement, f32::NAN);
        assert_eq!(nonsense.movement, None, "NaN became a reading");
    }

    /// **The disagreement is about the floor, and only when there is one.**
    #[test]
    fn a_still_floor_at_peak_is_worth_saying() {
        let still = night(0.6, 0.05, Sense::Movement);
        let said = still
            .disagrees_with(Occasion::Peak)
            .expect("a disagreement");
        assert!(said.contains("peak"), "{said}");
        assert!(said.contains("stiller"), "{said}");
        // A still floor during a warm up is a warm up working.
        assert_eq!(still.disagrees_with(Occasion::WarmUp), None);
        assert_eq!(still.disagrees_with(Occasion::Open), None);
    }

    /// **A floor that will not go home is worth saying too.**
    #[test]
    fn a_busy_floor_at_the_close_is_worth_saying() {
        let busy = night(0.2, 0.95, Sense::Movement);
        assert!(busy.disagrees_with(Occasion::Close).is_some());
        assert!(busy.disagrees_with(Occasion::WarmUp).is_some());
        assert!(busy.disagrees_with(Occasion::Background).is_some());
        // At peak, a busy floor is the plan working.
        assert_eq!(busy.disagrees_with(Occasion::Peak), None);
    }

    /// **Nothing is claimed before there is anything to claim it from.**
    #[test]
    fn an_unwatched_room_disagrees_with_nothing() {
        let room = Room::new();
        for occasion in Occasion::ALL {
            assert_eq!(room.disagrees_with(occasion), None, "{occasion:?}");
        }
        assert!(room.notes(None).is_empty());
    }

    /// **Every band and sense has words of its own.**
    #[test]
    fn every_phrase_is_written_out() {
        let bands = [
            Against::Lowest,
            Against::Lower,
            Against::Usual,
            Against::Higher,
            Against::Highest,
        ];
        let mut seen = std::collections::BTreeSet::new();
        for sense in [Sense::Light, Sense::Movement, Sense::Loudness] {
            for band in bands {
                let phrase = band.phrase(sense);
                assert!(!phrase.trim().is_empty());
                assert!(
                    seen.insert(phrase),
                    "{phrase:?} is used for more than one thing"
                );
            }
        }
        assert_eq!(seen.len(), 15);
    }

    #[test]
    fn the_hour_comes_off_the_clock() {
        // 1_700_000_000 is a Tuesday at 22:13:20 UTC.
        assert_eq!(hour_of(at(0)), Some(22));
        assert_eq!(hour_of(at(2 * 3600)), Some(0));
    }
}
