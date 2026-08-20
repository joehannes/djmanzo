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
//! [`SessionRead`] is not measured by anything yet. Whether a set is warming up
//! or peaking, and how hard, is what M9's context loop is for. Until that
//! exists the honest value is `None`, and a theme keyed to it shows its neutral
//! treatment. The first version of this module defaulted it to *Peak* at
//! *0.95 energy* on every snapshot, which meant the interface confidently
//! announced peak time thirty seconds into a warm-up — a claim nothing had
//! made and nothing could check.

use serde::{Deserialize, Serialize};

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
}

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
    /// 0..=1. How hard the room is going — a judgement, not a level.
    pub energy: f32,
    pub environment: EnvironmentContext,
}

/// Everything the interface may morph to.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionContext {
    /// Always present, always measured.
    pub audio: AudioMetrics,
    /// Present once something has read the room. See the module note.
    pub session: Option<SessionRead>,
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

    /// The default has to be the honest one: nothing has read the room.
    #[test]
    fn a_fresh_context_claims_nothing_about_the_room() {
        assert!(SessionContext::default().session.is_none());
    }
}
