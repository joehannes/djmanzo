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
    /// Where in the current beat the playhead is, 0.0..=1.0.
    ///
    /// Published so the interface can be *in time with the room* rather than
    /// with wall clock -- see
    /// [ADR-0009](../../../docs/adr/0009-the-living-interface.md), where every
    /// pulse in the living interface runs off this. Two synced decks report the
    /// same value, which is what makes their crests align on screen for the
    /// same reason they align in the air.
    ///
    /// 0.0 when the deck has no grid. That is indistinguishable from being
    /// exactly on a beat, and deliberately so: with no grid nothing pulses, so
    /// the value is never read.
    BeatPhase,
    /// 1.0 when slip mode is armed.
    Slip,
    /// 1.0 while the deck is playing backwards, whether from reverse or a
    /// held censor -- the interface draws the same thing either way.
    Reversed,
    /// Where the track would be if nothing were diverting it, in frames.
    ///
    /// **-1.0 when nothing is being slipped over**, because frame zero is a
    /// real position and cannot mean "none" as it can in an `Option`.
    SlipPosition,
    /// 1.0 while the platter is coasting — braking or thrown backwards.
    Spinning,
    /// 1.0 while a loop roll is being held.
    ///
    /// Separate from [`DeckParam::LoopActive`] because a roll and a loop look
    /// the same and behave differently: a loop stays when you stop touching it
    /// and a roll does not. An interface that draws both as "looping" is
    /// telling a DJ the wrong thing about what happens next.
    Rolling,
    /// How many beats the slicer's eight pads divide up.
    SliceBeats,
    /// Which slice the playhead is in, 1-based; 0 when there is no grid.
    ///
    /// Published so the pad page can light the slice you are *in* rather than
    /// the one you pressed — which is what lets a DJ see the next one coming.
    SliceIndex,
    /// 1.0 while a slice pad is held.
    Slicing,
    /// The three effect slots.
    ///
    /// Six values each rather than one packed number: a packed one would be
    /// unreadable in a log and impossible to watch a single control of. They
    /// are laid out slot by slot so that a reader scanning the registry sees a
    /// slot's whole state together.
    /// Slot 1: Which effect is loaded, as an index into [`crate::fx::EffectKind::ALL`].
    Fx1Kind,
    /// Slot 1: 1.0 while the slot is switched on.
    Fx1Enabled,
    /// Slot 1: Dry-to-wet mix, 0..=1.
    Fx1Wet,
    /// Slot 1: Length in beats. Meaningless for effects with no time in them.
    Fx1Beats,
    /// Slot 1: The effect's own knob, 0..=1; see `EffectKind::amount_label`.
    Fx1Amount,
    /// Slot 1: 1.0 when the slot sits after the channel fader rather than before it.
    Fx1Post,
    /// Slot 2: Which effect is loaded, as an index into [`crate::fx::EffectKind::ALL`].
    Fx2Kind,
    /// Slot 2: 1.0 while the slot is switched on.
    Fx2Enabled,
    /// Slot 2: Dry-to-wet mix, 0..=1.
    Fx2Wet,
    /// Slot 2: Length in beats. Meaningless for effects with no time in them.
    Fx2Beats,
    /// Slot 2: The effect's own knob, 0..=1; see `EffectKind::amount_label`.
    Fx2Amount,
    /// Slot 2: 1.0 when the slot sits after the channel fader rather than before it.
    Fx2Post,
    /// Slot 3: Which effect is loaded, as an index into [`crate::fx::EffectKind::ALL`].
    Fx3Kind,
    /// Slot 3: 1.0 while the slot is switched on.
    Fx3Enabled,
    /// Slot 3: Dry-to-wet mix, 0..=1.
    Fx3Wet,
    /// Slot 3: Length in beats. Meaningless for effects with no time in them.
    Fx3Beats,
    /// Slot 3: The effect's own knob, 0..=1; see `EffectKind::amount_label`.
    Fx3Amount,
    /// Slot 3: 1.0 when the slot sits after the channel fader rather than before it.
    Fx3Post,
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
    /// Which side of the crossfader this deck is cut by.
    ///
    /// [`CrossfaderAssign::as_param`] defines the encoding: negative left,
    /// positive right, zero through.
    CrossfaderAssign,
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
    pub const COUNT: usize = 61;

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
            BeatPhase,
            Slip,
            Reversed,
            SlipPosition,
            Spinning,
            Rolling,
            SliceBeats,
            SliceIndex,
            Slicing,
            Fx1Kind,
            Fx1Enabled,
            Fx1Wet,
            Fx1Beats,
            Fx1Amount,
            Fx1Post,
            Fx2Kind,
            Fx2Enabled,
            Fx2Wet,
            Fx2Beats,
            Fx2Amount,
            Fx2Post,
            Fx3Kind,
            Fx3Enabled,
            Fx3Wet,
            Fx3Beats,
            Fx3Amount,
            Fx3Post,
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
            CrossfaderAssign,
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
    /// Which bank the sampler's pads are showing, 1-based.
    SamplerBank,
    /// The sampler's own level, 0..=1.
    SamplerVolume,
    /// Peak the sampler put into the master this block.
    SamplerPeak,
    /// 1.0 when the recorder has a buffer to write into.
    ///
    /// A state the interface has to be able to show, because it is not one the
    /// DJ caused: the buffer is out being turned into a sample, and pressing
    /// record in that moment does nothing. Better a greyed button than a button
    /// that silently declines.
    RecordReady,
    /// 1.0 while a capture is running.
    Recording,
    /// The slot being recorded into, 1-based; 0 when nothing is.
    RecordSlot,
    /// How long the running capture has been going, in seconds.
    RecordSeconds,
    /// Where the running capture is coming from: 0 for the master, otherwise
    /// the deck's 1-based number.
    RecordSourceDeck,
    /// The eight slots of the bank that is showing.
    ///
    /// Only the showing bank: the others keep playing, but the pads cannot
    /// reach them, and publishing thirty-two slots so the interface can ignore
    /// twenty-four of them is thirty-two atomics read per frame for nothing.
    /// Sample 1: 1.0 when something is in the slot.
    Sample1Loaded,
    /// Sample 1: 1.0 while it is sounding.
    Sample1Playing,
    /// Sample 1: Index into [`crate::TriggerMode::ALL`].
    Sample1Mode,
    /// Sample 1: The slot's own level, 0..=1.
    Sample1Volume,
    /// Sample 1: How far through, 0..=1.
    Sample1Progress,
    /// Sample 1: 1.0 when it goes to the headphones instead of the mix.
    Sample1Cue,
    /// Sample 1: 1.0 when it stretches to the room's tempo.
    Sample1Synced,
    /// Sample 1: The sample's own tempo, or 0.0 when it has none — which is why the sync switch is hidden rather than greyed.
    Sample1Bpm,
    /// Sample 2: 1.0 when something is in the slot.
    Sample2Loaded,
    /// Sample 2: 1.0 while it is sounding.
    Sample2Playing,
    /// Sample 2: Index into [`crate::TriggerMode::ALL`].
    Sample2Mode,
    /// Sample 2: The slot's own level, 0..=1.
    Sample2Volume,
    /// Sample 2: How far through, 0..=1.
    Sample2Progress,
    /// Sample 2: 1.0 when it goes to the headphones instead of the mix.
    Sample2Cue,
    /// Sample 2: 1.0 when it stretches to the room's tempo.
    Sample2Synced,
    /// Sample 2: The sample's own tempo, or 0.0 when it has none — which is why the sync switch is hidden rather than greyed.
    Sample2Bpm,
    /// Sample 3: 1.0 when something is in the slot.
    Sample3Loaded,
    /// Sample 3: 1.0 while it is sounding.
    Sample3Playing,
    /// Sample 3: Index into [`crate::TriggerMode::ALL`].
    Sample3Mode,
    /// Sample 3: The slot's own level, 0..=1.
    Sample3Volume,
    /// Sample 3: How far through, 0..=1.
    Sample3Progress,
    /// Sample 3: 1.0 when it goes to the headphones instead of the mix.
    Sample3Cue,
    /// Sample 3: 1.0 when it stretches to the room's tempo.
    Sample3Synced,
    /// Sample 3: The sample's own tempo, or 0.0 when it has none — which is why the sync switch is hidden rather than greyed.
    Sample3Bpm,
    /// Sample 4: 1.0 when something is in the slot.
    Sample4Loaded,
    /// Sample 4: 1.0 while it is sounding.
    Sample4Playing,
    /// Sample 4: Index into [`crate::TriggerMode::ALL`].
    Sample4Mode,
    /// Sample 4: The slot's own level, 0..=1.
    Sample4Volume,
    /// Sample 4: How far through, 0..=1.
    Sample4Progress,
    /// Sample 4: 1.0 when it goes to the headphones instead of the mix.
    Sample4Cue,
    /// Sample 4: 1.0 when it stretches to the room's tempo.
    Sample4Synced,
    /// Sample 4: The sample's own tempo, or 0.0 when it has none — which is why the sync switch is hidden rather than greyed.
    Sample4Bpm,
    /// Sample 5: 1.0 when something is in the slot.
    Sample5Loaded,
    /// Sample 5: 1.0 while it is sounding.
    Sample5Playing,
    /// Sample 5: Index into [`crate::TriggerMode::ALL`].
    Sample5Mode,
    /// Sample 5: The slot's own level, 0..=1.
    Sample5Volume,
    /// Sample 5: How far through, 0..=1.
    Sample5Progress,
    /// Sample 5: 1.0 when it goes to the headphones instead of the mix.
    Sample5Cue,
    /// Sample 5: 1.0 when it stretches to the room's tempo.
    Sample5Synced,
    /// Sample 5: The sample's own tempo, or 0.0 when it has none — which is why the sync switch is hidden rather than greyed.
    Sample5Bpm,
    /// Sample 6: 1.0 when something is in the slot.
    Sample6Loaded,
    /// Sample 6: 1.0 while it is sounding.
    Sample6Playing,
    /// Sample 6: Index into [`crate::TriggerMode::ALL`].
    Sample6Mode,
    /// Sample 6: The slot's own level, 0..=1.
    Sample6Volume,
    /// Sample 6: How far through, 0..=1.
    Sample6Progress,
    /// Sample 6: 1.0 when it goes to the headphones instead of the mix.
    Sample6Cue,
    /// Sample 6: 1.0 when it stretches to the room's tempo.
    Sample6Synced,
    /// Sample 6: The sample's own tempo, or 0.0 when it has none — which is why the sync switch is hidden rather than greyed.
    Sample6Bpm,
    /// Sample 7: 1.0 when something is in the slot.
    Sample7Loaded,
    /// Sample 7: 1.0 while it is sounding.
    Sample7Playing,
    /// Sample 7: Index into [`crate::TriggerMode::ALL`].
    Sample7Mode,
    /// Sample 7: The slot's own level, 0..=1.
    Sample7Volume,
    /// Sample 7: How far through, 0..=1.
    Sample7Progress,
    /// Sample 7: 1.0 when it goes to the headphones instead of the mix.
    Sample7Cue,
    /// Sample 7: 1.0 when it stretches to the room's tempo.
    Sample7Synced,
    /// Sample 7: The sample's own tempo, or 0.0 when it has none — which is why the sync switch is hidden rather than greyed.
    Sample7Bpm,
    /// Sample 8: 1.0 when something is in the slot.
    Sample8Loaded,
    /// Sample 8: 1.0 while it is sounding.
    Sample8Playing,
    /// Sample 8: Index into [`crate::TriggerMode::ALL`].
    Sample8Mode,
    /// Sample 8: The slot's own level, 0..=1.
    Sample8Volume,
    /// Sample 8: How far through, 0..=1.
    Sample8Progress,
    /// Sample 8: 1.0 when it goes to the headphones instead of the mix.
    Sample8Cue,
    /// Sample 8: 1.0 when it stretches to the room's tempo.
    Sample8Synced,
    /// Sample 8: The sample's own tempo, or 0.0 when it has none — which is why the sync switch is hidden rather than greyed.
    Sample8Bpm,
    /// The three master effect slots. Same six values per slot as a deck's,
    /// because it is the same rack in a different place.
    /// Master slot 1: Which effect is loaded, as an index into [`crate::fx::EffectKind::ALL`].
    MasterFx1Kind,
    /// Master slot 1: 1.0 while the slot is switched on.
    MasterFx1Enabled,
    /// Master slot 1: Dry-to-wet mix, 0..=1.
    MasterFx1Wet,
    /// Master slot 1: Length in beats.
    MasterFx1Beats,
    /// Master slot 1: The effect's own knob, 0..=1.
    MasterFx1Amount,
    /// Master slot 1: 1.0 when the slot runs later in the chain.
    MasterFx1Post,
    /// Master slot 2: Which effect is loaded, as an index into [`crate::fx::EffectKind::ALL`].
    MasterFx2Kind,
    /// Master slot 2: 1.0 while the slot is switched on.
    MasterFx2Enabled,
    /// Master slot 2: Dry-to-wet mix, 0..=1.
    MasterFx2Wet,
    /// Master slot 2: Length in beats.
    MasterFx2Beats,
    /// Master slot 2: The effect's own knob, 0..=1.
    MasterFx2Amount,
    /// Master slot 2: 1.0 when the slot runs later in the chain.
    MasterFx2Post,
    /// Master slot 3: Which effect is loaded, as an index into [`crate::fx::EffectKind::ALL`].
    MasterFx3Kind,
    /// Master slot 3: 1.0 while the slot is switched on.
    MasterFx3Enabled,
    /// Master slot 3: Dry-to-wet mix, 0..=1.
    MasterFx3Wet,
    /// Master slot 3: Length in beats.
    MasterFx3Beats,
    /// Master slot 3: The effect's own knob, 0..=1.
    MasterFx3Amount,
    /// Master slot 3: 1.0 when the slot runs later in the chain.
    MasterFx3Post,
    /// Spectral band 1 (Bass) energy, 0.0..=1.0.
    MasterBandBass,
    /// Spectral band 2 (Low Mid) energy, 0.0..=1.0.
    MasterBandLowMid,
    /// Spectral band 3 (High Mid) energy, 0.0..=1.0.
    MasterBandHighMid,
    /// Spectral band 4 (Treble) energy, 0.0..=1.0.
    MasterBandTreble,
}

impl DeckParam {
    /// The six parameters of one effect slot, 1-based.
    ///
    /// A lookup rather than a match at every call site: the publisher and the
    /// reader both want to walk the slots, and writing out eighteen arms twice
    /// is where a copied line ends up pointing at the wrong slot.
    #[must_use]
    pub const fn fx(slot: u8) -> Option<FxParams> {
        match slot {
            1 => Some(FxParams {
                kind: DeckParam::Fx1Kind,
                enabled: DeckParam::Fx1Enabled,
                wet: DeckParam::Fx1Wet,
                beats: DeckParam::Fx1Beats,
                amount: DeckParam::Fx1Amount,
                post: DeckParam::Fx1Post,
            }),
            2 => Some(FxParams {
                kind: DeckParam::Fx2Kind,
                enabled: DeckParam::Fx2Enabled,
                wet: DeckParam::Fx2Wet,
                beats: DeckParam::Fx2Beats,
                amount: DeckParam::Fx2Amount,
                post: DeckParam::Fx2Post,
            }),
            3 => Some(FxParams {
                kind: DeckParam::Fx3Kind,
                enabled: DeckParam::Fx3Enabled,
                wet: DeckParam::Fx3Wet,
                beats: DeckParam::Fx3Beats,
                amount: DeckParam::Fx3Amount,
                post: DeckParam::Fx3Post,
            }),
            _ => None,
        }
    }
}

/// Where one sampler slot's eight values live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleParams {
    pub loaded: GlobalParam,
    pub playing: GlobalParam,
    pub mode: GlobalParam,
    pub volume: GlobalParam,
    pub progress: GlobalParam,
    pub cue: GlobalParam,
    pub synced: GlobalParam,
    pub bpm: GlobalParam,
}

/// Where one effect slot's six values live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxParams<P = DeckParam> {
    pub kind: P,
    pub enabled: P,
    pub wet: P,
    pub beats: P,
    pub amount: P,
    pub post: P,
}

impl GlobalParam {
    /// The eight parameters of one sampler slot, 1-based.
    #[must_use]
    pub const fn sample(slot: u8) -> Option<SampleParams> {
        match slot {
            1 => Some(SampleParams {
                loaded: GlobalParam::Sample1Loaded,
                playing: GlobalParam::Sample1Playing,
                mode: GlobalParam::Sample1Mode,
                volume: GlobalParam::Sample1Volume,
                progress: GlobalParam::Sample1Progress,
                cue: GlobalParam::Sample1Cue,
                synced: GlobalParam::Sample1Synced,
                bpm: GlobalParam::Sample1Bpm,
            }),
            2 => Some(SampleParams {
                loaded: GlobalParam::Sample2Loaded,
                playing: GlobalParam::Sample2Playing,
                mode: GlobalParam::Sample2Mode,
                volume: GlobalParam::Sample2Volume,
                progress: GlobalParam::Sample2Progress,
                cue: GlobalParam::Sample2Cue,
                synced: GlobalParam::Sample2Synced,
                bpm: GlobalParam::Sample2Bpm,
            }),
            3 => Some(SampleParams {
                loaded: GlobalParam::Sample3Loaded,
                playing: GlobalParam::Sample3Playing,
                mode: GlobalParam::Sample3Mode,
                volume: GlobalParam::Sample3Volume,
                progress: GlobalParam::Sample3Progress,
                cue: GlobalParam::Sample3Cue,
                synced: GlobalParam::Sample3Synced,
                bpm: GlobalParam::Sample3Bpm,
            }),
            4 => Some(SampleParams {
                loaded: GlobalParam::Sample4Loaded,
                playing: GlobalParam::Sample4Playing,
                mode: GlobalParam::Sample4Mode,
                volume: GlobalParam::Sample4Volume,
                progress: GlobalParam::Sample4Progress,
                cue: GlobalParam::Sample4Cue,
                synced: GlobalParam::Sample4Synced,
                bpm: GlobalParam::Sample4Bpm,
            }),
            5 => Some(SampleParams {
                loaded: GlobalParam::Sample5Loaded,
                playing: GlobalParam::Sample5Playing,
                mode: GlobalParam::Sample5Mode,
                volume: GlobalParam::Sample5Volume,
                progress: GlobalParam::Sample5Progress,
                cue: GlobalParam::Sample5Cue,
                synced: GlobalParam::Sample5Synced,
                bpm: GlobalParam::Sample5Bpm,
            }),
            6 => Some(SampleParams {
                loaded: GlobalParam::Sample6Loaded,
                playing: GlobalParam::Sample6Playing,
                mode: GlobalParam::Sample6Mode,
                volume: GlobalParam::Sample6Volume,
                progress: GlobalParam::Sample6Progress,
                cue: GlobalParam::Sample6Cue,
                synced: GlobalParam::Sample6Synced,
                bpm: GlobalParam::Sample6Bpm,
            }),
            7 => Some(SampleParams {
                loaded: GlobalParam::Sample7Loaded,
                playing: GlobalParam::Sample7Playing,
                mode: GlobalParam::Sample7Mode,
                volume: GlobalParam::Sample7Volume,
                progress: GlobalParam::Sample7Progress,
                cue: GlobalParam::Sample7Cue,
                synced: GlobalParam::Sample7Synced,
                bpm: GlobalParam::Sample7Bpm,
            }),
            8 => Some(SampleParams {
                loaded: GlobalParam::Sample8Loaded,
                playing: GlobalParam::Sample8Playing,
                mode: GlobalParam::Sample8Mode,
                volume: GlobalParam::Sample8Volume,
                progress: GlobalParam::Sample8Progress,
                cue: GlobalParam::Sample8Cue,
                synced: GlobalParam::Sample8Synced,
                bpm: GlobalParam::Sample8Bpm,
            }),
            _ => None,
        }
    }

    /// The six parameters of one *master* slot, 1-based.
    #[must_use]
    pub const fn fx(slot: u8) -> Option<FxParams<GlobalParam>> {
        match slot {
            1 => Some(FxParams {
                kind: GlobalParam::MasterFx1Kind,
                enabled: GlobalParam::MasterFx1Enabled,
                wet: GlobalParam::MasterFx1Wet,
                beats: GlobalParam::MasterFx1Beats,
                amount: GlobalParam::MasterFx1Amount,
                post: GlobalParam::MasterFx1Post,
            }),
            2 => Some(FxParams {
                kind: GlobalParam::MasterFx2Kind,
                enabled: GlobalParam::MasterFx2Enabled,
                wet: GlobalParam::MasterFx2Wet,
                beats: GlobalParam::MasterFx2Beats,
                amount: GlobalParam::MasterFx2Amount,
                post: GlobalParam::MasterFx2Post,
            }),
            3 => Some(FxParams {
                kind: GlobalParam::MasterFx3Kind,
                enabled: GlobalParam::MasterFx3Enabled,
                wet: GlobalParam::MasterFx3Wet,
                beats: GlobalParam::MasterFx3Beats,
                amount: GlobalParam::MasterFx3Amount,
                post: GlobalParam::MasterFx3Post,
            }),
            _ => None,
        }
    }

    /// The spectral bands, in order, low to high.
    ///
    /// A table rather than four names spelt out wherever they are read: they
    /// are only ever used together, and `dj_dsp::Spectrum` reports them as an
    /// array of the same length.
    pub const BANDS: [GlobalParam; 4] = [
        GlobalParam::MasterBandBass,
        GlobalParam::MasterBandLowMid,
        GlobalParam::MasterBandHighMid,
        GlobalParam::MasterBandTreble,
    ];

    /// 100 before the spectrum and the recorder; four bands and five recorder
    /// readings since.
    pub const COUNT: usize = 109;

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
            SamplerBank,
            SamplerVolume,
            SamplerPeak,
            RecordReady,
            Recording,
            RecordSlot,
            RecordSeconds,
            RecordSourceDeck,
            Sample1Loaded,
            Sample1Playing,
            Sample1Mode,
            Sample1Volume,
            Sample1Progress,
            Sample1Cue,
            Sample1Synced,
            Sample1Bpm,
            Sample2Loaded,
            Sample2Playing,
            Sample2Mode,
            Sample2Volume,
            Sample2Progress,
            Sample2Cue,
            Sample2Synced,
            Sample2Bpm,
            Sample3Loaded,
            Sample3Playing,
            Sample3Mode,
            Sample3Volume,
            Sample3Progress,
            Sample3Cue,
            Sample3Synced,
            Sample3Bpm,
            Sample4Loaded,
            Sample4Playing,
            Sample4Mode,
            Sample4Volume,
            Sample4Progress,
            Sample4Cue,
            Sample4Synced,
            Sample4Bpm,
            Sample5Loaded,
            Sample5Playing,
            Sample5Mode,
            Sample5Volume,
            Sample5Progress,
            Sample5Cue,
            Sample5Synced,
            Sample5Bpm,
            Sample6Loaded,
            Sample6Playing,
            Sample6Mode,
            Sample6Volume,
            Sample6Progress,
            Sample6Cue,
            Sample6Synced,
            Sample6Bpm,
            Sample7Loaded,
            Sample7Playing,
            Sample7Mode,
            Sample7Volume,
            Sample7Progress,
            Sample7Cue,
            Sample7Synced,
            Sample7Bpm,
            Sample8Loaded,
            Sample8Playing,
            Sample8Mode,
            Sample8Volume,
            Sample8Progress,
            Sample8Cue,
            Sample8Synced,
            Sample8Bpm,
            MasterFx1Kind,
            MasterFx1Enabled,
            MasterFx1Wet,
            MasterFx1Beats,
            MasterFx1Amount,
            MasterFx1Post,
            MasterFx2Kind,
            MasterFx2Enabled,
            MasterFx2Wet,
            MasterFx2Beats,
            MasterFx2Amount,
            MasterFx2Post,
            MasterFx3Kind,
            MasterFx3Enabled,
            MasterFx3Wet,
            MasterFx3Beats,
            MasterFx3Amount,
            MasterFx3Post,
            MasterBandBass,
            MasterBandLowMid,
            MasterBandHighMid,
            MasterBandTreble,
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
        DeckParam::BeatPhase => "beat_phase",
        DeckParam::Slip => "slip",
        DeckParam::Reversed => "reversed",
        DeckParam::SlipPosition => "slip_position",
        DeckParam::Spinning => "spinning",
        DeckParam::Rolling => "rolling",
        DeckParam::SliceBeats => "slice_beats",
        DeckParam::SliceIndex => "slice_index",
        DeckParam::Slicing => "slicing",
        DeckParam::Fx1Kind => "fx1_kind",
        DeckParam::Fx1Enabled => "fx1_enabled",
        DeckParam::Fx1Wet => "fx1_wet",
        DeckParam::Fx1Beats => "fx1_beats",
        DeckParam::Fx1Amount => "fx1_amount",
        DeckParam::Fx1Post => "fx1_post",
        DeckParam::Fx2Kind => "fx2_kind",
        DeckParam::Fx2Enabled => "fx2_enabled",
        DeckParam::Fx2Wet => "fx2_wet",
        DeckParam::Fx2Beats => "fx2_beats",
        DeckParam::Fx2Amount => "fx2_amount",
        DeckParam::Fx2Post => "fx2_post",
        DeckParam::Fx3Kind => "fx3_kind",
        DeckParam::Fx3Enabled => "fx3_enabled",
        DeckParam::Fx3Wet => "fx3_wet",
        DeckParam::Fx3Beats => "fx3_beats",
        DeckParam::Fx3Amount => "fx3_amount",
        DeckParam::Fx3Post => "fx3_post",
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
        DeckParam::CrossfaderAssign => "crossfader_assign",
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
        GlobalParam::SamplerBank => "sampler_bank",
        GlobalParam::SamplerVolume => "sampler_volume",
        GlobalParam::SamplerPeak => "sampler_peak",
        GlobalParam::RecordReady => "record_ready",
        GlobalParam::Recording => "recording",
        GlobalParam::RecordSlot => "record_slot",
        GlobalParam::RecordSeconds => "record_seconds",
        GlobalParam::RecordSourceDeck => "record_source_deck",
        GlobalParam::Sample1Loaded => "sample1_loaded",
        GlobalParam::Sample1Playing => "sample1_playing",
        GlobalParam::Sample1Mode => "sample1_mode",
        GlobalParam::Sample1Volume => "sample1_volume",
        GlobalParam::Sample1Progress => "sample1_progress",
        GlobalParam::Sample1Cue => "sample1_cue",
        GlobalParam::Sample1Synced => "sample1_synced",
        GlobalParam::Sample1Bpm => "sample1_bpm",
        GlobalParam::Sample2Loaded => "sample2_loaded",
        GlobalParam::Sample2Playing => "sample2_playing",
        GlobalParam::Sample2Mode => "sample2_mode",
        GlobalParam::Sample2Volume => "sample2_volume",
        GlobalParam::Sample2Progress => "sample2_progress",
        GlobalParam::Sample2Cue => "sample2_cue",
        GlobalParam::Sample2Synced => "sample2_synced",
        GlobalParam::Sample2Bpm => "sample2_bpm",
        GlobalParam::Sample3Loaded => "sample3_loaded",
        GlobalParam::Sample3Playing => "sample3_playing",
        GlobalParam::Sample3Mode => "sample3_mode",
        GlobalParam::Sample3Volume => "sample3_volume",
        GlobalParam::Sample3Progress => "sample3_progress",
        GlobalParam::Sample3Cue => "sample3_cue",
        GlobalParam::Sample3Synced => "sample3_synced",
        GlobalParam::Sample3Bpm => "sample3_bpm",
        GlobalParam::Sample4Loaded => "sample4_loaded",
        GlobalParam::Sample4Playing => "sample4_playing",
        GlobalParam::Sample4Mode => "sample4_mode",
        GlobalParam::Sample4Volume => "sample4_volume",
        GlobalParam::Sample4Progress => "sample4_progress",
        GlobalParam::Sample4Cue => "sample4_cue",
        GlobalParam::Sample4Synced => "sample4_synced",
        GlobalParam::Sample4Bpm => "sample4_bpm",
        GlobalParam::Sample5Loaded => "sample5_loaded",
        GlobalParam::Sample5Playing => "sample5_playing",
        GlobalParam::Sample5Mode => "sample5_mode",
        GlobalParam::Sample5Volume => "sample5_volume",
        GlobalParam::Sample5Progress => "sample5_progress",
        GlobalParam::Sample5Cue => "sample5_cue",
        GlobalParam::Sample5Synced => "sample5_synced",
        GlobalParam::Sample5Bpm => "sample5_bpm",
        GlobalParam::Sample6Loaded => "sample6_loaded",
        GlobalParam::Sample6Playing => "sample6_playing",
        GlobalParam::Sample6Mode => "sample6_mode",
        GlobalParam::Sample6Volume => "sample6_volume",
        GlobalParam::Sample6Progress => "sample6_progress",
        GlobalParam::Sample6Cue => "sample6_cue",
        GlobalParam::Sample6Synced => "sample6_synced",
        GlobalParam::Sample6Bpm => "sample6_bpm",
        GlobalParam::Sample7Loaded => "sample7_loaded",
        GlobalParam::Sample7Playing => "sample7_playing",
        GlobalParam::Sample7Mode => "sample7_mode",
        GlobalParam::Sample7Volume => "sample7_volume",
        GlobalParam::Sample7Progress => "sample7_progress",
        GlobalParam::Sample7Cue => "sample7_cue",
        GlobalParam::Sample7Synced => "sample7_synced",
        GlobalParam::Sample7Bpm => "sample7_bpm",
        GlobalParam::Sample8Loaded => "sample8_loaded",
        GlobalParam::Sample8Playing => "sample8_playing",
        GlobalParam::Sample8Mode => "sample8_mode",
        GlobalParam::Sample8Volume => "sample8_volume",
        GlobalParam::Sample8Progress => "sample8_progress",
        GlobalParam::Sample8Cue => "sample8_cue",
        GlobalParam::Sample8Synced => "sample8_synced",
        GlobalParam::Sample8Bpm => "sample8_bpm",
        GlobalParam::MasterFx1Kind => "master_fx1_kind",
        GlobalParam::MasterFx1Enabled => "master_fx1_enabled",
        GlobalParam::MasterFx1Wet => "master_fx1_wet",
        GlobalParam::MasterFx1Beats => "master_fx1_beats",
        GlobalParam::MasterFx1Amount => "master_fx1_amount",
        GlobalParam::MasterFx1Post => "master_fx1_post",
        GlobalParam::MasterFx2Kind => "master_fx2_kind",
        GlobalParam::MasterFx2Enabled => "master_fx2_enabled",
        GlobalParam::MasterFx2Wet => "master_fx2_wet",
        GlobalParam::MasterFx2Beats => "master_fx2_beats",
        GlobalParam::MasterFx2Amount => "master_fx2_amount",
        GlobalParam::MasterFx2Post => "master_fx2_post",
        GlobalParam::MasterFx3Kind => "master_fx3_kind",
        GlobalParam::MasterFx3Enabled => "master_fx3_enabled",
        GlobalParam::MasterFx3Wet => "master_fx3_wet",
        GlobalParam::MasterFx3Beats => "master_fx3_beats",
        GlobalParam::MasterFx3Amount => "master_fx3_amount",
        GlobalParam::MasterFx3Post => "master_fx3_post",
        GlobalParam::MasterBandBass => "master_band_bass",
        GlobalParam::MasterBandLowMid => "master_band_low_mid",
        GlobalParam::MasterBandHighMid => "master_band_high_mid",
        GlobalParam::MasterBandTreble => "master_band_treble",
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
