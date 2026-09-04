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

    /// Length of one beat in seconds.
    ///
    /// Sample-rate-free, for the things that live in wall time rather than in
    /// frames: MIDI clock, network phase, a tap tempo.
    #[must_use]
    pub fn beat_seconds(self) -> f64 {
        60.0 / self.0
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

/// Where the DJ wants the next track to take the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Trajectory {
    /// Harder than what is playing. The peak-hour default.
    Lift,
    /// About the same. Holding a plateau, which is most of a set.
    #[default]
    Hold,
    /// Softer. A come-down, or making room before a bigger record.
    Ease,
}

/// A track's phrase structure: how long a phrase is, and which beat starts one.
///
/// Dance music is built in phrases -- usually 16 or 32 beats -- and a DJ mixes
/// on their boundaries. A beat grid alone cannot say where those are: dropping
/// a track on beat 37 of a 32-beat phrase lands it three beats into the next
/// one, and the result is two records whose drums agree and whose music does
/// not.
///
/// Beats are counted from the grid's anchor, so this is meaningless without the
/// grid it was measured against -- which is why a deck clears it when the grid
/// goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phrase {
    /// Phrase length in beats. 8, 16 or 32 in practice.
    pub beats: u32,
    /// The beat, counted from the grid anchor, on which a phrase starts.
    /// Always less than `beats`.
    ///
    /// Not always zero: plenty of records open with a four- or eight-beat
    /// pickup before the first full phrase.
    pub anchor: u32,
}

impl Phrase {
    /// A phrase structure, or `None` if the numbers do not describe one.
    ///
    /// Rejects a zero length -- which would divide by zero on every use -- and
    /// normalises the anchor into range rather than refusing it, since "the
    /// phrase starts on beat 20 of a 16-beat phrase" is unambiguous.
    #[must_use]
    pub fn new(beats: u32, anchor: u32) -> Option<Self> {
        (beats > 0).then(|| Self {
            beats,
            anchor: anchor % beats,
        })
    }

    /// How far into a phrase the given beat index is, in beats.
    ///
    /// Zero on a phrase boundary. Handles negative beat indices, which are
    /// ordinary: the grid extends backwards from its anchor, and a track whose
    /// first downbeat is not its first beat has audio at negative indices.
    #[must_use]
    pub fn beat_within(self, beat: i64) -> u32 {
        let length = i64::from(self.beats);
        // `rem_euclid` rather than `%`: the sign of `%` follows the dividend in
        // Rust, so a beat three before the anchor would answer -3 and every
        // caller would have to remember to correct it.
        let offset = (beat - i64::from(self.anchor)).rem_euclid(length);
        u32::try_from(offset).unwrap_or(0)
    }

    /// Whether the given beat index starts a phrase.
    #[must_use]
    pub fn starts_at(self, beat: i64) -> bool {
        self.beat_within(beat) == 0
    }
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

    /// *How* two keys are related, rather than only whether they mix.
    ///
    /// [`Self::is_compatible_with`] answers yes or no, which is what a filter
    /// needs. A DJ looking at two records wants the reason: a neighbour on the
    /// wheel and a relative major are both "compatible" and they do not sound
    /// the same, and 8A against 3A is not merely "no" -- it is the tritone,
    /// the one distance worth naming out loud.
    #[must_use]
    pub fn relation_to(self, other: MusicalKey) -> KeyRelation {
        if self == other {
            return KeyRelation::Same;
        }
        if self.hour == other.hour {
            return KeyRelation::RelativeMode;
        }
        // Distance around a twelve-hour wheel, taking the short way round.
        let apart = i16::from(self.hour).abs_diff(i16::from(other.hour));
        let apart = apart.min(12 - apart);
        match (apart, self.mode == other.mode) {
            (1, true) => KeyRelation::Neighbour,
            (_, _) if apart == 6 => KeyRelation::Tritone,
            _ => KeyRelation::Distant,
        }
    }
}

/// How one key stands to another, in the terms a DJ mixes by.
///
/// The Camelot wheel's own vocabulary rather than music theory's: a DJ reading
/// this mid-set is deciding whether to hold a blend open for eight bars, and
/// "neighbour" answers that where "subdominant" does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyRelation {
    /// The same key. Nothing to manage.
    Same,
    /// One hour around the wheel, same ring. The ordinary harmonic step.
    Neighbour,
    /// Same hour, the other ring -- relative major or minor. A change of
    /// colour rather than of key, and the reason a set can lift without
    /// moving.
    RelativeMode,
    /// Six hours apart: the tritone. Named because it is the distance that
    /// sounds like a mistake rather than like a modulation.
    Tritone,
    /// Anywhere else on the wheel. Mixable in a cut, tiring in a long blend.
    Distant,
}

impl KeyRelation {
    /// True when the two sit together well enough to hold a blend open.
    ///
    /// Agrees with [`MusicalKey::is_compatible_with`] by construction: the
    /// three relations that answer true here are exactly the three that
    /// [`MusicalKey::compatible`] lists besides the key itself. Asserted by
    /// test, because two rules that must agree and are written down twice
    /// eventually do not.
    #[must_use]
    pub const fn mixes(self) -> bool {
        matches!(
            self,
            KeyRelation::Same | KeyRelation::Neighbour | KeyRelation::RelativeMode
        )
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            KeyRelation::Same => "same key",
            KeyRelation::Neighbour => "neighbour",
            KeyRelation::RelativeMode => "relative major/minor",
            KeyRelation::Tritone => "tritone",
            KeyRelation::Distant => "distant",
        }
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

    #[test]
    fn a_relation_is_named_for_what_it_is() {
        let a_minor = MusicalKey::new(8, Mode::Minor).unwrap();
        assert_eq!(a_minor.relation_to(a_minor), KeyRelation::Same);
        assert_eq!(
            a_minor.relation_to(MusicalKey::new(9, Mode::Minor).unwrap()),
            KeyRelation::Neighbour
        );
        assert_eq!(
            a_minor.relation_to(MusicalKey::new(8, Mode::Major).unwrap()),
            KeyRelation::RelativeMode
        );
        assert_eq!(
            a_minor.relation_to(MusicalKey::new(2, Mode::Minor).unwrap()),
            KeyRelation::Tritone
        );
        assert_eq!(
            a_minor.relation_to(MusicalKey::new(4, Mode::Minor).unwrap()),
            KeyRelation::Distant
        );
    }

    /// **The wheel wraps for relations too.**
    ///
    /// 12A into 1A is one hour apart, not eleven. The arithmetic that gets
    /// this wrong looks right at every hour except the two either side of
    /// midnight -- which is where a set that has been climbing all night ends
    /// up.
    #[test]
    fn a_neighbour_across_midnight_is_still_a_neighbour() {
        let twelve_a = MusicalKey::new(12, Mode::Minor).unwrap();
        let one_a = MusicalKey::new(1, Mode::Minor).unwrap();
        assert_eq!(twelve_a.relation_to(one_a), KeyRelation::Neighbour);
        assert_eq!(one_a.relation_to(twelve_a), KeyRelation::Neighbour);
    }

    /// **The two rules about mixing agree, over all 576 pairs.**
    ///
    /// `is_compatible_with` is what filters and the suggester ask; `mixes` is
    /// what the pair view draws. They are written down separately, so without
    /// this they would eventually disagree about one hour of the wheel and the
    /// interface would contradict the ranking that put the record there.
    #[test]
    fn naming_the_relation_never_contradicts_compatibility() {
        let all = (1..=12u8).flat_map(|hour| {
            [Mode::Minor, Mode::Major]
                .into_iter()
                .map(move |mode| MusicalKey::new(hour, mode).unwrap())
        });
        for a in all.clone() {
            for b in all.clone() {
                assert_eq!(
                    a.relation_to(b).mixes(),
                    a.is_compatible_with(b),
                    "{} against {} disagreed",
                    a.camelot(),
                    b.camelot()
                );
            }
        }
    }
}

#[cfg(test)]
mod beat_seconds_tests {
    use super::Bpm;

    #[test]
    fn a_beat_at_120_is_half_a_second() {
        let bpm = Bpm::new(120.0).expect("a real tempo");
        assert!((bpm.beat_seconds() - 0.5).abs() < 1e-12);
    }

    /// The bound is what keeps a beat period from collapsing towards zero.
    /// Anything that schedules by dividing a span into beat periods -- MIDI
    /// clock output, network phase -- would otherwise be asked for an
    /// unbounded number of them.
    #[test]
    fn a_beat_period_is_never_absurdly_short() {
        let fastest = Bpm::new(Bpm::MAX).expect("the top of the range");
        assert!(
            fastest.beat_seconds() > 0.1,
            "even the fastest tempo has a beat of {} s",
            fastest.beat_seconds()
        );
        assert_eq!(Bpm::new(f64::MAX), None, "an absurd tempo is not a tempo");
        assert_eq!(Bpm::new(0.0), None);
    }
}

#[cfg(test)]
mod phrase_tests {
    use super::*;

    /// A phrase length of zero would divide by zero on every use.
    #[test]
    fn a_phrase_needs_a_length() {
        assert!(Phrase::new(0, 0).is_none());
        assert!(Phrase::new(16, 0).is_some());
    }

    /// An anchor past the phrase length is normalised rather than refused:
    /// "starts on beat 20 of a 16-beat phrase" is unambiguous.
    #[test]
    fn an_anchor_beyond_the_phrase_wraps_into_it() {
        assert_eq!(Phrase::new(16, 20).unwrap().anchor, 4);
        assert_eq!(Phrase::new(16, 16).unwrap().anchor, 0);
    }

    /// **Beats before the anchor count correctly.**
    ///
    /// The grid extends backwards from its anchor, so negative beat indices are
    /// ordinary -- a track whose first downbeat is not its first beat has audio
    /// at them. Rust's `%` takes the sign of the dividend, so a plain remainder
    /// would answer -3 here and every caller would have to remember to correct
    /// it. One that forgot would draw a phrase marker in the wrong place at the
    /// start of every such track.
    #[test]
    fn a_beat_before_the_anchor_counts_forwards_not_backwards() {
        let phrase = Phrase::new(16, 0).unwrap();
        assert_eq!(phrase.beat_within(-1), 15);
        assert_eq!(phrase.beat_within(-16), 0);
        assert_eq!(phrase.beat_within(-17), 15);
    }

    #[test]
    fn a_phrase_starts_where_the_anchor_says() {
        let phrase = Phrase::new(16, 5).unwrap();
        assert!(phrase.starts_at(5));
        assert!(phrase.starts_at(21));
        assert!(!phrase.starts_at(0));
        assert_eq!(phrase.beat_within(6), 1);
        assert_eq!(phrase.beat_within(4), 15);
    }
}
