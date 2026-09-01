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

/// How many buckets the night's own distribution is kept in.
///
/// Twenty over the 0..1 range, so a reading is placed to within five percent
/// of the range without keeping every reading of a six-hour night.
const BUCKETS: usize = 20;

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

/// The night's own distribution of one sense, coarsely.
#[derive(Debug, Default, Clone)]
struct Spread {
    counts: [u32; BUCKETS],
    total: u32,
}

impl Spread {
    fn add(&mut self, value: f32) {
        let bucket = bucket_of(value);
        self.counts[bucket] = self.counts[bucket].saturating_add(1);
        self.total = self.total.saturating_add(1);
    }

    /// What fraction of tonight's readings were below `value`.
    ///
    /// Half of its own bucket counts as below, so a night where every reading
    /// lands in one bucket answers "about half" rather than "none" — which is
    /// what "no news" should look like when nothing has changed.
    fn below(&self, value: f32) -> Option<f32> {
        if self.total == 0 {
            return None;
        }
        let bucket = bucket_of(value);
        let under: u32 = self.counts[..bucket].iter().sum();
        let within = f64::from(self.counts[bucket]) / 2.0;
        #[allow(clippy::cast_possible_truncation)]
        Some(((f64::from(under) + within) / f64::from(self.total)) as f32)
    }
}

fn bucket_of(value: f32) -> usize {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let bucket = (value.clamp(0.0, 1.0) * BUCKETS as f32) as usize;
    bucket.min(BUCKETS - 1)
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

/// What the room has been doing.
#[derive(Debug, Default)]
pub struct Room {
    near: VecDeque<Reading>,
    light: Spread,
    movement: Spread,
    loudness: Spread,
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
    pub fn saw(&mut self, reading: Reading) {
        if reading.is_empty() {
            return;
        }
        if let Some(light) = reading.light {
            self.light.add(light);
        }
        if let Some(movement) = reading.movement {
            self.movement.add(movement);
        }
        if let Some(loudness) = reading.loudness {
            self.loudness.add(loudness);
        }
        self.near.push_back(reading);
        self.forget_before(reading.at);
    }

    fn forget_before(&mut self, now: SystemTime) {
        while let Some(oldest) = self.near.front() {
            let old = now
                .duration_since(oldest.at)
                .is_ok_and(|since| since > NEAR);
            if old {
                self.near.pop_front();
            } else {
                break;
            }
        }
    }

    /// The middle of the near window for one sense.
    ///
    /// A median rather than a mean: one frame where somebody walked across the
    /// lens is an outlier, and a mean carries it for three minutes.
    #[must_use]
    pub fn lately(&self, sense: Sense) -> Option<f32> {
        let mut values: Vec<f32> = self.near.iter().filter_map(|r| r.get(sense)).collect();
        if values.len() < ENOUGH {
            return None;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        Some(values[values.len() / 2])
    }

    /// Where one sense sits against the rest of tonight.
    #[must_use]
    pub fn against(&self, sense: Sense) -> Option<Against> {
        let lately = self.lately(sense)?;
        let spread = match sense {
            Sense::Light => &self.light,
            Sense::Movement => &self.movement,
            Sense::Loudness => &self.loudness,
        };
        spread.below(lately).map(against)
    }

    /// How many readings are in the near window.
    #[must_use]
    pub fn recent(&self) -> usize {
        self.near.len()
    }

    /// When the last reading arrived, if any has.
    ///
    /// So that "something is watching" is answered by the readings themselves
    /// rather than by a flag somebody has to remember to clear: a window that
    /// closed without saying so cannot leave the panel claiming to watch a
    /// room nothing is looking at.
    #[must_use]
    pub fn last_seen(&self) -> Option<SystemTime> {
        self.near.back().map(|reading| reading.at)
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
    #[must_use]
    pub fn notes(&self) -> Vec<String> {
        [Sense::Movement, Sense::Loudness, Sense::Light]
            .into_iter()
            .filter_map(|sense| {
                let against = self.against(sense)?;
                against
                    .is_notable()
                    .then(|| format!("The room is {}.", against.phrase(sense)))
            })
            .collect()
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
            room.saw(Reading::at(at(n * 30)).with(sense, early));
        }
        // Then half a minute of `late`, which is the near window.
        let start = 200 * 30;
        for n in 0..ENOUGH as u64 {
            room.saw(Reading::at(at(start + n * 2)).with(sense, late));
        }
        room
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
            room.saw(Reading::at(at(n * 2)).with(Sense::Movement, 0.5));
        }
        assert!(!room.has_looked_enough());
        assert_eq!(room.against(Sense::Movement), None);
        assert!(room.notes().is_empty());

        room.saw(Reading::at(at(100)).with(Sense::Movement, 0.5));
        assert!(room.has_looked_enough());
    }

    /// **A floor going still says so, and a floor filling up says so.**
    #[test]
    fn the_room_is_read_against_its_own_night() {
        let quietened = night(0.6, 0.05, Sense::Movement);
        assert_eq!(quietened.against(Sense::Movement), Some(Against::Lowest));
        assert!(
            quietened.notes()[0].contains("stiller"),
            "{:?}",
            quietened.notes()
        );

        let filled = night(0.1, 0.9, Sense::Movement);
        assert_eq!(filled.against(Sense::Movement), Some(Against::Highest));
        assert!(filled.notes()[0].contains("busier"), "{:?}", filled.notes());
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
        assert!(steady.notes().is_empty(), "{:?}", steady.notes());
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
            room.saw(Reading::at(at(start + n * 2)).with(Sense::Movement, 1.0));
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
            room.saw(Reading::at(at(n * 2)).with(Sense::Movement, 0.5));
        }
        assert_eq!(room.recent(), 40);
        // One reading, well past the window: everything older goes.
        room.saw(Reading::at(at(10_000)).with(Sense::Movement, 0.5));
        assert_eq!(room.recent(), 1);
        assert!(!room.has_looked_enough(), "a stale window still answered");
    }

    /// **Whether anything is watching comes from the readings.**
    #[test]
    fn the_last_reading_says_when_it_was() {
        let mut room = Room::new();
        assert_eq!(room.last_seen(), None);
        room.saw(Reading::at(at(5)).with(Sense::Light, 0.5));
        assert_eq!(room.last_seen(), Some(at(5)));
        room.saw(Reading::at(at(9)).with(Sense::Light, 0.5));
        assert_eq!(room.last_seen(), Some(at(9)));
        // An empty reading is not a sighting.
        room.saw(Reading::at(at(20)));
        assert_eq!(room.last_seen(), Some(at(9)));
    }

    /// **A source with no microphone still contributes.**
    #[test]
    fn half_a_reading_is_kept() {
        let mut room = Room::new();
        for n in 0..40u64 {
            room.saw(Reading::at(at(n * 2)).with(Sense::Light, 0.5));
        }
        assert!(room.lately(Sense::Light).is_some());
        assert_eq!(room.lately(Sense::Loudness), None);
    }

    /// **A reading of nothing is not a reading.**
    #[test]
    fn an_empty_reading_is_dropped() {
        let mut room = Room::new();
        for n in 0..40u64 {
            room.saw(Reading::at(at(n * 2)));
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
        assert!(room.notes().is_empty());
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
