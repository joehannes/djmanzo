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
}

impl DeckParam {
    /// Number of parameters each deck occupies.
    pub const COUNT: usize = 9;

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
}

impl GlobalParam {
    pub const COUNT: usize = 7;

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
