//! Musical properties: tempo, beat grids, keys.

use crate::time::{FramePos, SampleRate};
use serde::{Deserialize, Serialize};

/// Beats per minute. Constrained to a range that covers everything a DJ plays,
/// which also stops a bad analysis result from producing a grid with billions of
/// beats in it.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Bpm(f64);

impl Bpm {
    pub const MIN: f64 = 20.0;
    pub const MAX: f64 = 400.0;

    #[must_use]
    pub fn new(bpm: f64) -> Option<Self> {
        if bpm.is_finite() && (Self::MIN..=Self::MAX).contains(&bpm) {
            Some(Self(bpm))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }

    /// Length of one beat in frames.
    #[must_use]
    pub fn beat_frames(self, rate: SampleRate) -> f64 {
        rate.as_f64() * 60.0 / self.0
    }

    /// The rate a deck must run at to play this tempo as `target`.
    #[must_use]
    pub fn rate_to_match(self, target: Bpm) -> f64 {
        target.0 / self.0
    }
}

/// How much to trust a beat grid.
///
/// This exists so the engine can refuse to auto-sync against a grid the analyser
/// was guessing at. Silently syncing to a wrong grid is worse than not syncing:
/// it derails a mix at the moment the DJ has stopped watching.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(f64);

impl Confidence {
    pub const NONE: Confidence = Confidence(0.0);
    pub const CERTAIN: Confidence = Confidence(1.0);

    /// Below this, sync and quantize are disabled and the UI flags the grid.
    pub const SYNC_THRESHOLD: f64 = 0.5;

    #[must_use]
    pub fn new(value: f64) -> Self {
        if value.is_finite() {
            Self(value.clamp(0.0, 1.0))
        } else {
            Self::NONE
        }
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }

    #[must_use]
    pub fn is_sync_worthy(self) -> bool {
        self.0 >= Self::SYNC_THRESHOLD
    }
}

/// A constant-tempo beat grid: one anchor beat plus a tempo.
///
/// Variable-tempo tracks (live recordings, anything not made to a click) need a
/// list of tempo changes instead. That arrives with the analyser in M2; this
/// type stays as the common case and the fallback.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Beatgrid {
    /// Position of some beat -- not necessarily the first, and not necessarily a
    /// downbeat. Beats extend infinitely in both directions from here.
    pub anchor: FramePos,
    pub bpm: Bpm,
    /// Beats per bar. 4 unless something says otherwise.
    pub beats_per_bar: u8,
    pub confidence: Confidence,
}

impl Beatgrid {
    #[must_use]
    pub fn new(anchor: FramePos, bpm: Bpm, confidence: Confidence) -> Self {
        Self {
            anchor,
            bpm,
            beats_per_bar: 4,
            confidence,
        }
    }

    /// Index of the beat at or before `pos`, counted from the anchor. Negative
    /// before the anchor.
    #[must_use]
    pub fn beat_index_at(&self, pos: FramePos, rate: SampleRate) -> i64 {
        let beat = self.bpm.beat_frames(rate);
        ((pos.get() - self.anchor.get()) / beat).floor() as i64
    }

    /// Position of beat number `index`, counted from the anchor.
    #[must_use]
    pub fn beat_position(&self, index: i64, rate: SampleRate) -> FramePos {
        let beat = self.bpm.beat_frames(rate);
        FramePos::new(self.anchor.get() + (index as f64) * beat)
    }

    /// Nearest beat to `pos` in either direction -- what quantize snaps to.
    #[must_use]
    pub fn nearest_beat(&self, pos: FramePos, rate: SampleRate) -> FramePos {
        let beat = self.bpm.beat_frames(rate);
        let index = ((pos.get() - self.anchor.get()) / beat).round() as i64;
        self.beat_position(index, rate)
    }
}

/// Musical key in the 24 major/minor tonalities, stored as a Camelot wheel
/// position because that is what DJs actually mix by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MusicalKey {
    /// 1..=12, the hour on the Camelot wheel.
    hour: u8,
    mode: Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    /// Camelot "A" ring.
    Minor,
    /// Camelot "B" ring.
    Major,
}

impl MusicalKey {
    #[must_use]
    pub fn new(hour: u8, mode: Mode) -> Option<Self> {
        if (1..=12).contains(&hour) {
            Some(Self { hour, mode })
        } else {
            None
        }
    }

    #[must_use]
    pub const fn hour(self) -> u8 {
        self.hour
    }

    #[must_use]
    pub const fn mode(self) -> Mode {
        self.mode
    }

    /// Camelot notation, e.g. `8A`, `12B`.
    #[must_use]
    pub fn camelot(self) -> String {
        let ring = match self.mode {
            Mode::Minor => 'A',
            Mode::Major => 'B',
        };
        format!("{}{}", self.hour, ring)
    }

    /// Standard notation, e.g. `Am`, `C`.
    #[must_use]
    pub fn standard(self) -> &'static str {
        const MINOR: [&str; 12] = [
            "Abm", "Ebm", "Bbm", "Fm", "Cm", "Gm", "Dm", "Am", "Em", "Bm", "F#m", "Dbm",
        ];
        const MAJOR: [&str; 12] = [
            "B", "F#", "Db", "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E",
        ];
        let i = (self.hour - 1) as usize;
        match self.mode {
            Mode::Minor => MINOR[i],
            Mode::Major => MAJOR[i],
        }
    }

    /// Keys that mix cleanly with this one: the same key, its neighbours on the
    /// wheel, and its relative major/minor.
    #[must_use]
    pub fn compatible(self) -> [MusicalKey; 4] {
        let up = if self.hour == 12 { 1 } else { self.hour + 1 };
        let down = if self.hour == 1 { 12 } else { self.hour - 1 };
        let other_mode = match self.mode {
            Mode::Minor => Mode::Major,
            Mode::Major => Mode::Minor,
        };
        [
            self,
            Self {
                hour: up,
                mode: self.mode,
            },
            Self {
                hour: down,
                mode: self.mode,
            },
            Self {
                hour: self.hour,
                mode: other_mode,
            },
        ]
    }

    #[must_use]
    pub fn is_compatible_with(self, other: MusicalKey) -> bool {
        self.compatible().contains(&other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rate() -> SampleRate {
        SampleRate::new(48_000).unwrap()
    }

    #[test]
    fn bpm_range_is_enforced() {
        assert!(Bpm::new(128.0).is_some());
        assert!(Bpm::new(0.0).is_none());
        assert!(Bpm::new(1000.0).is_none());
        assert!(Bpm::new(f64::NAN).is_none());
    }

    #[test]
    fn beat_length_matches_tempo() {
        // 120 BPM is two beats a second, so one beat is half a second.
        let beat = Bpm::new(120.0).unwrap().beat_frames(rate());
        assert!((beat - 24_000.0).abs() < 1e-9);
    }

    #[test]
    fn rate_to_match_scales_correctly() {
        let from = Bpm::new(128.0).unwrap();
        let to = Bpm::new(140.0).unwrap();
        assert!((from.rate_to_match(to) - 140.0 / 128.0).abs() < 1e-12);
    }

    #[test]
    fn grid_indexes_beats_from_the_anchor() {
        let grid = Beatgrid::new(
            FramePos::new(1000.0),
            Bpm::new(120.0).unwrap(),
            Confidence::CERTAIN,
        );
        assert_eq!(grid.beat_index_at(FramePos::new(1000.0), rate()), 0);
        assert_eq!(grid.beat_index_at(FramePos::new(25_000.0), rate()), 1);
        // Before the anchor the count goes negative rather than clamping.
        assert_eq!(grid.beat_index_at(FramePos::new(0.0), rate()), -1);
    }

    #[test]
    fn quantize_snaps_to_the_closer_beat() {
        let grid = Beatgrid::new(
            FramePos::ZERO,
            Bpm::new(120.0).unwrap(),
            Confidence::CERTAIN,
        );
        // Beat length is 24000; 13000 is past the midpoint so it snaps forward.
        assert_eq!(
            grid.nearest_beat(FramePos::new(13_000.0), rate()).get(),
            24_000.0
        );
        assert_eq!(
            grid.nearest_beat(FramePos::new(11_000.0), rate()).get(),
            0.0
        );
    }

    #[test]
    fn low_confidence_blocks_sync() {
        assert!(!Confidence::new(0.2).is_sync_worthy());
        assert!(Confidence::new(0.9).is_sync_worthy());
        assert!(!Confidence::new(f64::NAN).is_sync_worthy());
    }

    #[test]
    fn camelot_and_standard_notation_agree() {
        let a_minor = MusicalKey::new(8, Mode::Minor).unwrap();
        assert_eq!(a_minor.camelot(), "8A");
        assert_eq!(a_minor.standard(), "Am");

        let c_major = MusicalKey::new(8, Mode::Major).unwrap();
        assert_eq!(c_major.camelot(), "8B");
        assert_eq!(c_major.standard(), "C");
    }

    #[test]
    fn compatible_keys_wrap_around_the_wheel() {
        let twelve_a = MusicalKey::new(12, Mode::Minor).unwrap();
        let one_a = MusicalKey::new(1, Mode::Minor).unwrap();
        assert!(twelve_a.is_compatible_with(one_a));

        let one = MusicalKey::new(1, Mode::Major).unwrap();
        let twelve = MusicalKey::new(12, Mode::Major).unwrap();
        assert!(one.is_compatible_with(twelve));
    }

    #[test]
    fn relative_major_minor_is_compatible() {
        let a_minor = MusicalKey::new(8, Mode::Minor).unwrap();
        let c_major = MusicalKey::new(8, Mode::Major).unwrap();
        assert!(a_minor.is_compatible_with(c_major));
    }

    #[test]
    fn distant_keys_are_not_compatible() {
        let a = MusicalKey::new(1, Mode::Minor).unwrap();
        let b = MusicalKey::new(6, Mode::Minor).unwrap();
        assert!(!a.is_compatible_with(b));
    }

    #[test]
    fn key_hour_is_validated() {
        assert!(MusicalKey::new(0, Mode::Major).is_none());
        assert!(MusicalKey::new(13, Mode::Major).is_none());
    }
}
