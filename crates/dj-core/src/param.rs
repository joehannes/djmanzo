//! The parameter address space.
//!
//! Every observable value in the application has an address here. The addresses
//! map onto a flat, fixed-size array (see `dj-control`), so a lookup from the
//! audio thread is an array index -- no hashing, no allocation, no locking.
//!
//! This is deliberately *not* a dynamic registry keyed by string. See
//! `docs/adr/0003-action-bus-and-parameter-registry.md` for why.

use crate::deck::{DeckId, MAX_DECKS};
use serde::{Deserialize, Serialize};

/// Per-deck parameters. The discriminants are the offsets within a deck's block
/// of the parameter table, so this enum's order defines the memory layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum DeckParam {
    /// 1.0 when the transport is running.
    Playing = 0,
    /// Playhead, in frames.
    Position,
    /// Current playback rate.
    Rate,
    /// Pitch fader as a fraction; 0.0 is centre.
    Pitch,
    /// Channel fader, 0.0..=1.0.
    Volume,
    /// Trim, in decibels.
    GainDb,
    /// 1.0 when a track is loaded.
    Loaded,
    /// Length of the loaded track in frames; 0.0 when empty.
    LengthFrames,
    /// Post-fader peak level, 0.0..=1.0, for the VU meter.
    PeakLevel,
    /// Isolator EQ low band, linear gain.
    EqLow,
    /// Isolator EQ mid band, linear gain.
    EqMid,
    /// Isolator EQ high band, linear gain.
    EqHigh,
    /// Filter sweep position, -1.0..=1.0.
    Filter,
    /// 1.0 when this deck is sent to the headphones.
    CueEnabled,
    /// Pre-fader peak, for the cue meter and setting trim.
    PreFaderLevel,
    /// 1.0 when keylock is holding the musical key.
    Keylock,
    /// Frames of latency keylock is adding, before compensation. 0.0 when off.
    KeylockLatencyFrames,
    /// Deliberate transposition in semitones. 0.0 when not shifted.
    KeyShift,
    /// 1.0 when this deck's tempo is locked to another's.
    Synced,
    /// Tempo the deck is actually playing at, pitch fader included. 0.0 when
    /// the track has no grid, which is different from 0 BPM and shown as such.
    EffectiveBpm,
    /// Confidence in this deck's grid, 0.0..=1.0. 0.0 when there is no grid.
    GridConfidence,
    /// 1.0 when a loop is repeating.
    LoopActive,
    /// Loop start and end, in frames. Meaningful only while `LoopActive`.
    LoopStart,
    LoopEnd,
    /// Loop length in beats, for the interface to show "4" rather than "96000
    /// frames". 0.0 when the deck has no grid to measure it against.
    LoopBeats,
    /// Hot cue positions in frames, 1-based in the interface.
    ///
    /// [`UNSET_HOT_CUE`] means empty. Frame zero is a legitimate cue position —
    /// the very start of a track is a perfectly ordinary place to put one — so
    /// zero cannot double as "not set".
    HotCue1,
    HotCue2,
    HotCue3,
    HotCue4,
    HotCue5,
    HotCue6,
    HotCue7,
    HotCue8,
}

/// What a hot cue parameter reads when the slot is empty.
///
/// Negative because no real position is, and because a parameter table of
/// `f32` has no room for an `Option`.
pub const UNSET_HOT_CUE: f32 = -1.0;

impl DeckParam {
    /// The hot cue parameter for a 1-based slot, or `None` if there is no such
    /// slot.
    ///
    /// A lookup rather than arithmetic on the discriminant: the enum's order is
    /// the memory layout, and computing an address from it would break silently
    /// the first time somebody reorders a variant.
    #[must_use]
    pub const fn hot_cue(slot: u8) -> Option<DeckParam> {
        match slot {
            1 => Some(DeckParam::HotCue1),
            2 => Some(DeckParam::HotCue2),
            3 => Some(DeckParam::HotCue3),
            4 => Some(DeckParam::HotCue4),
            5 => Some(DeckParam::HotCue5),
            6 => Some(DeckParam::HotCue6),
            7 => Some(DeckParam::HotCue7),
            8 => Some(DeckParam::HotCue8),
            _ => None,
        }
    }

    /// Number of parameters each deck occupies.
    pub const COUNT: usize = 33;

    #[must_use]
    pub const fn offset(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn all() -> [DeckParam; Self::COUNT] {
        use DeckParam::*;
        [
            Playing,
            Position,
            Rate,
            Pitch,
            Volume,
            GainDb,
            Loaded,
            LengthFrames,
            PeakLevel,
            EqLow,
            EqMid,
            EqHigh,
            Filter,
            CueEnabled,
            PreFaderLevel,
            Keylock,
            KeylockLatencyFrames,
            KeyShift,
            Synced,
            EffectiveBpm,
            GridConfidence,
            LoopActive,
            LoopStart,
            LoopEnd,
            LoopBeats,
            HotCue1,
            HotCue2,
            HotCue3,
            HotCue4,
            HotCue5,
            HotCue6,
            HotCue7,
            HotCue8,
        ]
    }
}

/// Parameters that belong to the application rather than to a deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum GlobalParam {
    /// -1.0 hard left .. +1.0 hard right.
    Crossfader = 0,
    MasterGainDb,
    MasterPeakLeft,
    MasterPeakRight,
    /// Device sample rate, so the UI can convert frames to time.
    SampleRate,
    /// Count of buffer under/overruns since start. Non-zero means the user is
    /// hearing dropouts, so it is surfaced rather than logged and forgotten.
    Xruns,
    /// Fraction of the callback budget the engine used on its last pass.
    CpuLoad,
    /// Headphone blend: 0.0 all cue, 1.0 all master.
    CueMix,
    /// 1.0 when split cue is on.
    CueSplit,
    BoothGainDb,
    /// 1.0 when the open device has channels for a headphone cue.
    CueAvailable,
    /// 1.0 when the master limiter is engaged.
    LimiterEnabled,
    /// Gain reduction the limiter is applying, in positive decibels. Zero means
    /// it is doing nothing, which is where it should sit most of the night.
    LimiterReductionDb,
    /// 1.0 when beat jumps snap to the grid.
    Quantize,
    /// Frames of latency the output chain adds after the decks. The interface
    /// needs it to explain the delay rather than let someone discover it.
    OutputLatencyFrames,
}

impl GlobalParam {
    pub const COUNT: usize = 15;

    #[must_use]
    pub const fn offset(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn all() -> [GlobalParam; Self::COUNT] {
        use GlobalParam::*;
        [
            Crossfader,
            MasterGainDb,
            MasterPeakLeft,
            MasterPeakRight,
            SampleRate,
            Xruns,
            CpuLoad,
            CueMix,
            CueSplit,
            BoothGainDb,
            CueAvailable,
            LimiterEnabled,
            LimiterReductionDb,
            OutputLatencyFrames,
            Quantize,
        ]
    }
}

/// A parameter address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParamId {
    Deck(DeckId, DeckParam),
    Global(GlobalParam),
}

impl ParamId {
    /// Total size of the parameter table.
    pub const COUNT: usize = MAX_DECKS * DeckParam::COUNT + GlobalParam::COUNT;

    /// Index into the flat table. Deck blocks come first so that a deck's
    /// parameters are contiguous and share cache lines.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            ParamId::Deck(deck, param) => deck.index() * DeckParam::COUNT + param.offset(),
            ParamId::Global(param) => MAX_DECKS * DeckParam::COUNT + param.offset(),
        }
    }

    /// Every address, for initialising the table and for diffing snapshots.
    pub fn all() -> impl Iterator<Item = ParamId> {
        let decks = DeckId::all().flat_map(|d| {
            DeckParam::all()
                .into_iter()
                .map(move |p| ParamId::Deck(d, p))
        });
        let globals = GlobalParam::all().into_iter().map(ParamId::Global);
        decks.chain(globals)
    }

    /// Stable name for the UI, scripting and the network API, e.g.
    /// `deck.1.position`, `master.crossfader`.
    #[must_use]
    pub fn name(self) -> String {
        match self {
            ParamId::Deck(deck, param) => {
                format!("deck.{}.{}", deck.human_number(), deck_param_name(param))
            }
            ParamId::Global(param) => format!("master.{}", global_param_name(param)),
        }
    }
}

const fn deck_param_name(param: DeckParam) -> &'static str {
    match param {
        DeckParam::Playing => "playing",
        DeckParam::Position => "position",
        DeckParam::Rate => "rate",
        DeckParam::Pitch => "pitch",
        DeckParam::Volume => "volume",
        DeckParam::GainDb => "gain_db",
        DeckParam::Loaded => "loaded",
        DeckParam::LengthFrames => "length_frames",
        DeckParam::PeakLevel => "peak_level",
        DeckParam::EqLow => "eq_low",
        DeckParam::EqMid => "eq_mid",
        DeckParam::EqHigh => "eq_high",
        DeckParam::Filter => "filter",
        DeckParam::CueEnabled => "cue_enabled",
        DeckParam::PreFaderLevel => "pre_fader_level",
        DeckParam::Keylock => "keylock",
        DeckParam::KeylockLatencyFrames => "keylock_latency_frames",
        DeckParam::KeyShift => "key_shift",
        DeckParam::Synced => "synced",
        DeckParam::EffectiveBpm => "effective_bpm",
        DeckParam::GridConfidence => "grid_confidence",
        DeckParam::LoopActive => "loop_active",
        DeckParam::LoopStart => "loop_start",
        DeckParam::LoopEnd => "loop_end",
        DeckParam::LoopBeats => "loop_beats",
        DeckParam::HotCue1 => "hot_cue_1",
        DeckParam::HotCue2 => "hot_cue_2",
        DeckParam::HotCue3 => "hot_cue_3",
        DeckParam::HotCue4 => "hot_cue_4",
        DeckParam::HotCue5 => "hot_cue_5",
        DeckParam::HotCue6 => "hot_cue_6",
        DeckParam::HotCue7 => "hot_cue_7",
        DeckParam::HotCue8 => "hot_cue_8",
    }
}

const fn global_param_name(param: GlobalParam) -> &'static str {
    match param {
        GlobalParam::Crossfader => "crossfader",
        GlobalParam::MasterGainDb => "gain_db",
        GlobalParam::MasterPeakLeft => "peak_left",
        GlobalParam::MasterPeakRight => "peak_right",
        GlobalParam::SampleRate => "sample_rate",
        GlobalParam::Xruns => "xruns",
        GlobalParam::CpuLoad => "cpu_load",
        GlobalParam::CueMix => "cue_mix",
        GlobalParam::CueSplit => "cue_split",
        GlobalParam::BoothGainDb => "booth_gain_db",
        GlobalParam::CueAvailable => "cue_available",
        GlobalParam::LimiterEnabled => "limiter_enabled",
        GlobalParam::LimiterReductionDb => "limiter_reduction_db",
        GlobalParam::OutputLatencyFrames => "output_latency_frames",
        GlobalParam::Quantize => "quantize",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The whole design rests on indices being unique and dense. If this breaks,
    /// two parameters silently alias and the bug is nearly unfindable at runtime.
    #[test]
    fn indices_are_unique_and_cover_the_table() {
        let indices: HashSet<usize> = ParamId::all().map(ParamId::index).collect();
        assert_eq!(indices.len(), ParamId::COUNT, "duplicate parameter index");
        assert_eq!(indices.iter().copied().max().unwrap(), ParamId::COUNT - 1);
        assert_eq!(indices.iter().copied().min().unwrap(), 0);
    }

    #[test]
    fn all_yields_exactly_count_entries() {
        assert_eq!(ParamId::all().count(), ParamId::COUNT);
    }

    #[test]
    fn deck_parameters_are_contiguous() {
        let deck = DeckId::from_human(2).unwrap();
        let base = ParamId::Deck(deck, DeckParam::Playing).index();
        for (offset, param) in DeckParam::all().into_iter().enumerate() {
            assert_eq!(ParamId::Deck(deck, param).index(), base + offset);
        }
    }

    #[test]
    fn count_constants_match_the_enums() {
        assert_eq!(DeckParam::all().len(), DeckParam::COUNT);
        assert_eq!(GlobalParam::all().len(), GlobalParam::COUNT);
    }

    #[test]
    fn names_are_stable_and_unique() {
        let names: HashSet<String> = ParamId::all().map(ParamId::name).collect();
        assert_eq!(names.len(), ParamId::COUNT, "duplicate parameter name");
        assert_eq!(
            ParamId::Deck(DeckId::from_human(1).unwrap(), DeckParam::Position).name(),
            "deck.1.position"
        );
        assert_eq!(
            ParamId::Global(GlobalParam::Crossfader).name(),
            "master.crossfader"
        );
    }
}
