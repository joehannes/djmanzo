//! What the UI is told, and how often.
//!
//! The engine writes into the [`ParameterRegistry`] at audio rate. The UI needs
//! it at frame rate. This module is the bridge: a thread that samples the
//! registry 60 times a second and emits a typed snapshot.
//!
//! Sampling rather than streaming events is deliberate. The playhead changes
//! every callback -- roughly 190 times a second at 256 frames -- and forwarding
//! each change would flood the IPC channel with data the display cannot use.

use dj_control::ParameterRegistry;
use dj_core::param::{DeckParam, FxParams, GlobalParam};
use dj_core::{CrossfaderAssign, DeckId, EffectKind, ParamId, SessionContext};
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// UI refresh rate. Matches a 60 Hz display; the engine runs far faster.
pub const SNAPSHOT_HZ: u64 = 60;

/// How long the pump may stay silent before emitting anyway.
///
/// Purely so a late subscriber is not left staring at an empty interface until
/// something happens to change.
///
/// Measured against the clock rather than counted in ticks: `thread::sleep`
/// guarantees only a *minimum*, and it overshoots by several milliseconds per
/// call on macOS. Counting 60 ticks of a nominal 16.7 ms sleep gives an interval
/// anywhere from 1.0 s to well past 1.3 s depending on the platform's timer
/// granularity and load.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeckSnapshot {
    /// 1-based, as shown in the interface.
    pub number: u8,
    /// What is loaded, by name. Comes from the snapshot rather than from the
    /// deck component, so a track loaded from the browser, the assistant or a
    /// controller shows its name just like one loaded from the deck itself.
    pub title: Option<String>,
    pub artist: Option<String>,
    pub playing: bool,
    pub loaded: bool,
    pub position_frames: f32,
    pub length_frames: f32,
    pub position_seconds: f32,
    pub length_seconds: f32,
    pub rate: f32,
    pub pitch: f32,
    pub volume: f32,
    pub gain_db: f32,
    pub peak: f32,
    pub eq_low: f32,
    pub eq_mid: f32,
    pub eq_high: f32,
    pub filter: f32,
    pub cue_enabled: bool,
    pub pre_fader_level: f32,
    /// Holding the musical key while the pitch fader changes tempo.
    pub keylock: bool,
    /// What keylock costs, in milliseconds, before the deck compensates for it.
    /// Surfaced so the figure is stated rather than guessed at.
    pub keylock_latency_ms: f32,
    /// Deliberate transposition in semitones, for harmonic mixing.
    pub key_shift: i32,
    /// Which side of the crossfader cuts this deck, or neither.
    pub crossfader_assign: CrossfaderAssign,
    /// Mute states for the 4 stems (Vocal, Drums, Bass, Other).
    pub stem_mutes: [bool; 4],
    /// Volume states for the 4 stems (Vocal, Drums, Bass, Other).
    pub stem_volumes: [f32; 4],
    /// What the analyser made of this track. `None` while it is still running,
    /// which is the normal state for the first second after a load.
    pub analysis: Option<TrackAnalysisSnapshot>,
    /// True when this deck's tempo is locked to another's.
    pub synced: bool,
    /// Tempo actually being played, pitch fader included. `None` when the track
    /// has no grid — which is different from 0 BPM and shown differently.
    pub effective_bpm: Option<f32>,
    /// True when the grid is solid enough for sync to accept it. The interface
    /// uses this to disable the button rather than let it fail silently.
    pub can_sync: bool,
    /// How much the *current* grid is trusted, 0.0..=1.0.
    ///
    /// Live, from the engine, rather than the number in the cached analysis:
    /// a hand-edited grid is certain by construction, and a header still
    /// showing "38% confidence" next to an enabled Sync button would be the
    /// interface arguing with itself.
    pub grid_confidence: f32,
    /// Where in the current beat the playhead is, 0.0..=1.0.
    ///
    /// The living interface's clock: everything that pulses pulses on this, so
    /// the screen moves in time with the room rather than with wall clock. Two
    /// synced decks report the same value. See
    /// [ADR-0009](../../../docs/adr/0009-the-living-interface.md).
    pub beat_phase: f32,
    /// Slip mode is armed: a shadow playhead runs at the natural rate while
    /// something diverts this one, and the deck lands there when it stops.
    pub slip: bool,
    /// Playing backwards, from reverse or from a held censor.
    pub reversed: bool,
    /// The platter is coasting — braking, or thrown backwards.
    pub spinning: bool,
    /// A loop roll is being held.
    ///
    /// Distinct from `active_loop` because the two look alike and end
    /// differently: a loop stays, a roll ends when the pad is released. The
    /// interface has to be able to say which one is on screen.
    pub rolling: bool,
    /// The slicer: how many beats the eight pads divide, which one the
    /// playhead is in, and whether a pad is held.
    pub slice: SliceSnapshot,
    /// Where the track would be if nothing were diverting it, in frames.
    /// `None` when nothing is being slipped over.
    pub slip_position: Option<f32>,
    /// The region repeating right now, if any.
    pub active_loop: Option<LoopSnapshot>,
    /// This deck's three effect slots, in order.
    pub fx: Vec<FxSlotSnapshot>,
    /// Hot cue positions in frames, slot 1 first. `None` for an empty slot —
    /// which is not the same as a cue at frame zero.
    pub hot_cues: Vec<Option<f32>>,
}

/// Read one slot's six values, whichever rack they came from.
///
/// Generic over the parameter type so the deck racks and the master rack share
/// it: they hold the same six things, and writing the conversion twice is how
/// the two drift apart.
fn fx_slot<P: Copy>(slot: u8, get: impl Fn(fn(FxParams<P>) -> P) -> f32) -> FxSlotSnapshot {
    let kind = EffectKind::from_index(get(|p| p.kind).max(0.0) as usize);
    FxSlotSnapshot {
        slot,
        kind: kind.name().to_owned(),
        enabled: get(|p| p.enabled) >= 0.5,
        wet: get(|p| p.wet),
        beats: get(|p| p.beats),
        amount: get(|p| p.amount),
        amount_label: kind.amount_label().to_owned(),
        timed: kind.is_timed(),
        post_fader: get(|p| p.post) >= 0.5,
    }
}

/// The slicer's state, for the pad page that draws it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SliceSnapshot {
    /// Beats the eight pads divide up.
    pub beats: f32,
    /// Which slice the playhead is in, 1-based. `None` without a grid.
    pub at: Option<u8>,
    /// A slice pad is held.
    pub holding: bool,
}

/// The sampler, as the interface draws it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SamplerSnapshot {
    /// 1-based.
    pub bank: u8,
    pub volume: f32,
    /// Peak the sampler put into the master, for its own meter.
    pub peak: f32,
    /// The showing bank's eight slots. The other banks keep playing; the pads
    /// simply cannot reach them.
    pub slots: Vec<SampleSlotSnapshot>,
    /// Capturing into a slot. See [`RecordSnapshot`].
    pub record: RecordSnapshot,
}

/// What the record control needs to draw itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordSnapshot {
    /// Whether there is a buffer to record into.
    ///
    /// Not something the DJ caused when it is false — the buffer is out being
    /// turned into a sample — so the interface says so rather than offering a
    /// button that silently declines.
    pub ready: bool,
    pub recording: bool,
    /// The slot being recorded into, 1-based; `None` when nothing is.
    pub slot: Option<u8>,
    /// How long the running capture has been going.
    pub seconds: f32,
    /// The longest one can run, so the interface can draw a bar rather than a
    /// number climbing towards a limit nobody told it about.
    pub max_seconds: f32,
    /// `master`, or `deck 2`.
    pub source: Option<String>,
}

/// One sampler pad.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SampleSlotSnapshot {
    /// 1-based.
    pub slot: u8,
    /// What is in it, by name.
    ///
    /// From the application rather than the engine — the engine holds audio and
    /// nothing else, the same as it does for a deck. In the snapshot rather
    /// than kept by the panel, because a panel that only knows the loads it
    /// made itself shows nothing for a sample a script or a preset put there.
    pub name: Option<String>,
    pub loaded: bool,
    pub playing: bool,
    /// `one_shot`, `hold`, `loop`, `stutter`.
    pub mode: String,
    pub volume: f32,
    /// How far through, 0..=1.
    pub progress: f32,
    /// True when it goes to the headphones rather than the mix.
    pub cue: bool,
    pub synced: bool,
    /// The sample's own tempo. `None` when it has none — which is why the sync
    /// switch is hidden rather than greyed out.
    pub bpm: Option<f32>,
}

/// One effect slot, as the interface draws it.
///
/// The kind comes back as a name rather than an index: the registry has to
/// carry it as a number because it holds `f32`, but a number in the interface
/// is a thing to look up, and every lookup is somewhere the two sides can
/// disagree about what effect 3 is.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FxSlotSnapshot {
    /// 1-based, as the interface and controllers number them.
    pub slot: u8,
    /// `none`, `echo`, `gate`, `crush`, `flanger`.
    pub kind: String,
    pub enabled: bool,
    pub wet: f32,
    pub beats: f32,
    pub amount: f32,
    /// What the amount knob does, in the DJ's words. Empty for an empty slot.
    pub amount_label: String,
    /// Whether the beat control means anything for this effect.
    pub timed: bool,
    /// True when the slot sits after the channel fader.
    pub post_fader: bool,
}

/// A loop, as the interface draws it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LoopSnapshot {
    pub start_frames: f32,
    pub end_frames: f32,
    /// Length in beats, for a label. `None` without a grid to measure against —
    /// the loop is still real, it just cannot be named in beats.
    pub beats: Option<f32>,
}

/// Tempo, key and loudness, as the interface needs them.
///
/// Every field is optional for the same reason the analyser's own fields are:
/// a field recording has no tempo, a drum loop has no key, and showing a
/// plausible zero instead of "could not tell" is how a DJ ends up syncing to a
/// grid that was never there.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackAnalysisSnapshot {
    pub bpm: Option<f32>,
    /// 0..=1. Below `sync_worthy` the grid is a guess and the interface should
    /// say so rather than offering sync.
    pub bpm_confidence: Option<f32>,
    /// The rejected octave — usually half or double. Offered so a wrong guess
    /// is one click to fix instead of a retapped grid.
    pub bpm_alternative: Option<f32>,
    /// Whether the grid is solid enough to sync to.
    pub sync_worthy: bool,
    /// Camelot notation, e.g. `8A`.
    pub key_camelot: Option<String>,
    /// Standard notation, e.g. `Am`.
    pub key_standard: Option<String>,
    pub key_confidence: Option<f32>,
    /// The runner-up, usually the relative major or minor.
    pub key_alternative: Option<String>,
    /// Integrated loudness, LUFS. `None` for a silent or unmeasurable track.
    pub lufs: Option<f32>,
    /// Trim that would bring this track to the reference loudness.
    pub auto_gain_db: f32,
}

impl TrackAnalysisSnapshot {
    fn from_analysis(analysis: &dj_analysis::Analysis) -> Self {
        let tempo = analysis.tempo.as_ref();
        let key = analysis.key.as_ref();
        Self {
            bpm: tempo.map(|t| t.grid.bpm.get() as f32),
            bpm_confidence: tempo.map(|t| t.grid.confidence.get() as f32),
            bpm_alternative: tempo.and_then(|t| t.alternative).map(|b| b.get() as f32),
            sync_worthy: analysis.is_sync_worthy(),
            key_camelot: key.map(|k| k.key.camelot()),
            key_standard: key.map(|k| k.key.standard().to_owned()),
            key_confidence: key.map(|k| k.correlation as f32),
            key_alternative: key.and_then(|k| k.alternative).map(|a| a.camelot()),
            lufs: analysis
                .loudness
                .get()
                .is_finite()
                .then(|| analysis.loudness.get() as f32),
            auto_gain_db: analysis.auto_gain_db() as f32,
        }
    }
}

/// Recording the set to disk.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SetRecordingSnapshot {
    pub active: bool,
    pub seconds: f64,
    /// Samples that never reached the disk. Non-zero means a gap in the file,
    /// and the interface says so rather than letting it be discovered later.
    pub dropped: u64,
    /// The writer thread gave up — a full disk, usually.
    pub failed: bool,
}

/// The master's plugin insert.
///
/// Only what changes at 60 Hz: whether there is one and whether it is in the
/// path. The plugin's *name* and its parameter list do not change on their own
/// and are fetched by `plugin_state`, which is a command rather than a stream —
/// pushing a two-hundred-parameter list sixty times a second for numbers that
/// move when somebody drags a slider would be most of the traffic to the
/// interface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ClapSnapshot {
    pub loaded: bool,
    /// Loaded but out of the signal path.
    pub bypassed: bool,
}

/// The automix, when the DJ has handed the mix over.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct AutomixSnapshot {
    pub enabled: bool,
    /// True only while a transition is actually running.
    pub mixing: bool,
    /// How long a transition lasts, in beats.
    pub beats: f32,
    /// One of `cut`, `fade`, `blend`, `echo`.
    pub style: &'static str,
}

/// The microphone / line input strip.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MicSnapshot {
    /// An input device is attached.
    ///
    /// Distinct from `open`: a DJ can arm the channel with nothing plugged in,
    /// and an interface that showed those the same way would leave someone
    /// talking into a microphone that was never connected.
    pub present: bool,
    /// The channel is open.
    pub open: bool,
    pub gain_db: f32,
    /// Peak level after the gain, 0..=1.
    pub level: f32,
    /// The microphone is going to the headphones as well.
    pub cue: bool,
    /// Talkover is switched on. Off is the aux case — a phone or a second
    /// laptop should not duck the mix every time it makes a sound.
    pub talkover: bool,
    /// How far the music is being ducked right now, in positive decibels.
    /// Zero most of the time, which is what makes it worth showing.
    pub ducking_db: f32,
    /// How far the music drops when talkover engages, in positive decibels.
    pub duck_db: f32,
    pub threshold_db: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    /// Frames the input ring could not supply. Non-zero means the input is not
    /// keeping up — a real fault with a real fix, and invisible without this.
    pub starved_frames: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MasterSnapshot {
    /// The sampler: which bank is showing, its level, and that bank's slots.
    pub sampler: SamplerSnapshot,
    /// Recording the whole mix to disk.
    pub recording: SetRecordingSnapshot,
    /// The master rack's three slots, in order.
    pub fx: Vec<FxSlotSnapshot>,
    pub crossfader: f32,
    pub gain_db: f32,
    pub peak_left: f32,
    pub peak_right: f32,
    pub sample_rate: f32,
    pub xruns: f32,
    pub cpu_load: f32,
    pub cue_mix: f32,
    pub cue_split: bool,
    pub booth_gain_db: f32,
    /// False on a two-channel device, where there is nowhere to send a cue.
    pub cue_available: bool,
    /// False when the master limiter has been bypassed.
    pub limiter_enabled: bool,
    /// Gain reduction the limiter is applying, in positive decibels.
    ///
    /// The master meter reads post-limiter and so can never show over 0 dB.
    /// This is the number that says how hard the mix is being driven, which is
    /// the thing the meter alone cannot tell you.
    pub limiter_reduction_db: f32,
    /// Delay the output chain adds after the decks, in milliseconds.
    ///
    /// Stated rather than left to be discovered. Constant whether the limiter
    /// is engaged or bypassed.
    pub output_latency_ms: f32,
    /// Present only when the headphone cue is on a second sound card.
    pub split_output: Option<SplitOutputSnapshot>,
    /// True when beat jumps snap to the grid.
    pub quantize: bool,
    /// The microphone / line input strip.
    pub mic: MicSnapshot,
    /// The automix.
    pub automix: AutomixSnapshot,
    /// The plugin insert.
    pub clap: ClapSnapshot,
}

/// How the two-card bridge is doing.
///
/// Worth showing rather than hiding. The drift figure is the measured
/// disagreement between two crystals: one that settles near zero says the pair
/// is well matched, and one that keeps climbing says a device is misreporting
/// its rate — a real fault that is otherwise completely invisible until the
/// headphones start clicking mid-set.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SplitOutputSnapshot {
    /// Measured clock disagreement, in parts per million.
    pub drift_ppm: f32,
    /// Extra latency the headphone path carries, in milliseconds.
    pub queue_ms: f32,
    pub target_ms: f32,
    /// Non-zero means the headphones went silent for a moment.
    pub starved_frames: f64,
    /// Non-zero means the headphone device stopped consuming.
    pub dropped_samples: f64,
    pub healthy: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    /// The global session context driving the UI's contextual expression.
    pub context: SessionContext,
    pub decks: Vec<DeckSnapshot>,
    pub master: MasterSnapshot,
}

impl Snapshot {
    /// Read the current state of `deck_count` decks.
    #[must_use]
    pub fn capture(registry: &ParameterRegistry, deck_count: usize) -> Self {
        Self::capture_with(registry, deck_count, None)
    }

    /// As [`Self::capture`], plus the two-card bridge when one is running.
    #[must_use]
    pub fn capture_with(
        registry: &ParameterRegistry,
        deck_count: usize,
        bridge: Option<&dj_audio::BridgeStats>,
    ) -> Self {
        Self::capture_full(registry, deck_count, bridge, None)
    }

    /// As [`Self::capture_with`], plus what the analyser has worked out.
    #[must_use]
    pub fn capture_full(
        registry: &ParameterRegistry,
        deck_count: usize,
        bridge: Option<&dj_audio::BridgeStats>,
        analysis: Option<&crate::analysis::AnalysisStore>,
    ) -> Self {
        Self::capture_all(
            registry,
            deck_count,
            bridge,
            analysis,
            Names::default(),
            None,
        )
    }

    /// The full picture, including what each deck and each sample is called.
    #[must_use]
    pub fn capture_all(
        registry: &ParameterRegistry,
        deck_count: usize,
        bridge: Option<&dj_audio::BridgeStats>,
        analysis: Option<&crate::analysis::AnalysisStore>,
        titles: Names<'_>,
        recording: Option<&crate::setrec::RecordingState>,
    ) -> Self {
        let names = titles.decks.and_then(|t| t.lock().ok());
        let sample_names = titles.samples.and_then(|t| t.lock().ok());
        // Read once, before the slots: the names are keyed by bank, and reading
        // it per slot would let a bank switch land in the middle of the eight.
        let bank = registry
            .get(ParamId::Global(GlobalParam::SamplerBank))
            .max(1.0) as u8;
        let sample_rate = registry.get(ParamId::Global(GlobalParam::SampleRate));
        // Before a device is open the rate is zero; dividing by it would put
        // infinities on screen.
        let to_seconds = |frames: f32| {
            if sample_rate > 0.0 {
                frames / sample_rate
            } else {
                0.0
            }
        };

        let decks = (0..deck_count)
            .filter_map(|index| DeckId::new(index as u8))
            .map(|id| {
                let get = |param| registry.get(ParamId::Deck(id, param));
                let position = get(DeckParam::Position);
                let length = get(DeckParam::LengthFrames);
                DeckSnapshot {
                    number: id.human_number(),
                    title: names
                        .as_ref()
                        .and_then(|m| m.get(&id.human_number()))
                        .map(|t| t.title.clone()),
                    artist: names
                        .as_ref()
                        .and_then(|m| m.get(&id.human_number()))
                        .and_then(|t| t.artist.clone()),
                    playing: get(DeckParam::Playing) >= 0.5,
                    loaded: get(DeckParam::Loaded) >= 0.5,
                    position_frames: position,
                    length_frames: length,
                    position_seconds: to_seconds(position),
                    length_seconds: to_seconds(length),
                    rate: get(DeckParam::Rate),
                    pitch: get(DeckParam::Pitch),
                    volume: get(DeckParam::Volume),
                    gain_db: get(DeckParam::GainDb),
                    peak: get(DeckParam::PeakLevel),
                    eq_low: get(DeckParam::EqLow),
                    eq_mid: get(DeckParam::EqMid),
                    eq_high: get(DeckParam::EqHigh),
                    filter: get(DeckParam::Filter),
                    cue_enabled: get(DeckParam::CueEnabled) >= 0.5,
                    pre_fader_level: get(DeckParam::PreFaderLevel),
                    keylock: get(DeckParam::Keylock) >= 0.5,
                    keylock_latency_ms: to_seconds(get(DeckParam::KeylockLatencyFrames)) * 1000.0,
                    key_shift: get(DeckParam::KeyShift).round() as i32,
                    crossfader_assign: CrossfaderAssign::from_param(get(
                        DeckParam::CrossfaderAssign,
                    )),
                    stem_mutes: [
                        get(DeckParam::StemVocalMute) >= 0.5,
                        get(DeckParam::StemDrumsMute) >= 0.5,
                        get(DeckParam::StemBassMute) >= 0.5,
                        get(DeckParam::StemOtherMute) >= 0.5,
                    ],
                    stem_volumes: [
                        get(DeckParam::StemVocalVolume),
                        get(DeckParam::StemDrumsVolume),
                        get(DeckParam::StemBassVolume),
                        get(DeckParam::StemOtherVolume),
                    ],
                    synced: get(DeckParam::Synced) >= 0.5,
                    effective_bpm: {
                        let bpm = get(DeckParam::EffectiveBpm);
                        (bpm > 0.0).then_some(bpm)
                    },
                    can_sync: get(DeckParam::GridConfidence)
                        >= dj_core::Confidence::SYNC_THRESHOLD as f32,
                    grid_confidence: get(DeckParam::GridConfidence),
                    beat_phase: get(DeckParam::BeatPhase),
                    slip: get(DeckParam::Slip) > 0.5,
                    reversed: get(DeckParam::Reversed) > 0.5,
                    rolling: get(DeckParam::Rolling) > 0.5,
                    slice: SliceSnapshot {
                        beats: get(DeckParam::SliceBeats),
                        // Zero means no grid, which is not slice zero — the pads
                        // are numbered from one precisely so the two differ.
                        at: match get(DeckParam::SliceIndex) as u8 {
                            0 => None,
                            n => Some(n),
                        },
                        holding: get(DeckParam::Slicing) > 0.5,
                    },
                    spinning: get(DeckParam::Spinning) > 0.5,
                    slip_position: {
                        // Negative means "nothing to slip over", because frame
                        // zero is a real position. See `state::NOT_SLIPPING`.
                        let at = get(DeckParam::SlipPosition);
                        (at >= 0.0).then_some(at)
                    },
                    active_loop: (get(DeckParam::LoopActive) >= 0.5).then(|| {
                        let beats = get(DeckParam::LoopBeats);
                        LoopSnapshot {
                            start_frames: get(DeckParam::LoopStart),
                            end_frames: get(DeckParam::LoopEnd),
                            beats: (beats > 0.0).then_some(beats),
                        }
                    }),
                    fx: (1..=dj_core::FX_SLOTS as u8)
                        .filter_map(|slot| {
                            let param = DeckParam::fx(slot)?;
                            Some(fx_slot(slot, |p| get(p(param))))
                        })
                        .collect(),
                    hot_cues: (1..=dj_core::HOT_CUE_SLOTS as u8)
                        .map(|slot| {
                            let value = DeckParam::hot_cue(slot).map(&get)?;
                            // Negative means empty. Frame zero is a real cue.
                            (value >= 0.0).then_some(value)
                        })
                        .collect(),
                    analysis: analysis
                        .and_then(|store| store.for_deck(id.human_number()))
                        .map(|found| TrackAnalysisSnapshot::from_analysis(&found)),
                }
            })
            .collect();
        // Measured, and only measured. `session` stays `None` until M9 has
        // something that actually reads the room -- see `dj_core::context`.
        let bands = GlobalParam::BANDS.map(|param| registry.get(ParamId::Global(param)));
        let context = SessionContext {
            audio: dj_core::AudioMetrics::from_bands(bands),
            session: None,
        };

        Self {
            context,
            decks,
            master: MasterSnapshot {
                recording: SetRecordingSnapshot {
                    active: recording
                        .is_some_and(|r| r.active.load(std::sync::atomic::Ordering::Relaxed)),
                    seconds: recording.map(|r| r.seconds()).unwrap_or(0.0),
                    // From the engine rather than from the writer: the engine is
                    // the only side that knows what it could not hand over.
                    dropped: registry
                        .get(ParamId::Global(GlobalParam::SetRecordDropped))
                        .max(0.0) as u64,
                    failed: recording
                        .is_some_and(|r| r.failed.load(std::sync::atomic::Ordering::Relaxed)),
                },
                sampler: SamplerSnapshot {
                    bank,
                    volume: registry.get(ParamId::Global(GlobalParam::SamplerVolume)),
                    peak: registry.get(ParamId::Global(GlobalParam::SamplerPeak)),
                    record: {
                        let get = |p| registry.get(ParamId::Global(p));
                        let recording = get(GlobalParam::Recording) >= 0.5;
                        let deck = get(GlobalParam::RecordSourceDeck) as u8;
                        RecordSnapshot {
                            ready: get(GlobalParam::RecordReady) >= 0.5,
                            recording,
                            slot: recording.then(|| get(GlobalParam::RecordSlot) as u8),
                            seconds: get(GlobalParam::RecordSeconds),
                            max_seconds: dj_core::MAX_RECORD_SECONDS as f32,
                            source: recording.then(|| match deck {
                                0 => "master".to_owned(),
                                n => format!("deck {n}"),
                            }),
                        }
                    },
                    slots: (1..=dj_core::SAMPLE_SLOTS as u8)
                        .filter_map(|slot| {
                            let param = GlobalParam::sample(slot)?;
                            let get = |p| registry.get(ParamId::Global(p));
                            let bpm = get(param.bpm);
                            Some(SampleSlotSnapshot {
                                slot,
                                name: sample_names
                                    .as_ref()
                                    .and_then(|names| names.get(&(bank, slot)).cloned()),
                                loaded: get(param.loaded) >= 0.5,
                                playing: get(param.playing) >= 0.5,
                                mode: dj_core::TriggerMode::from_index(
                                    get(param.mode).max(0.0) as usize
                                )
                                .name()
                                .to_owned(),
                                volume: get(param.volume),
                                progress: get(param.progress),
                                cue: get(param.cue) >= 0.5,
                                synced: get(param.synced) >= 0.5,
                                // Zero means "no tempo of its own", because a
                                // sample at 0 BPM is not a thing.
                                bpm: (bpm > 0.0).then_some(bpm),
                            })
                        })
                        .collect(),
                },
                fx: (1..=dj_core::FX_SLOTS as u8)
                    .filter_map(|slot| {
                        let param = GlobalParam::fx(slot)?;
                        Some(fx_slot(slot, |p| registry.get(ParamId::Global(p(param)))))
                    })
                    .collect(),
                crossfader: registry.get(ParamId::Global(GlobalParam::Crossfader)),
                gain_db: registry.get(ParamId::Global(GlobalParam::MasterGainDb)),
                peak_left: registry.get(ParamId::Global(GlobalParam::MasterPeakLeft)),
                peak_right: registry.get(ParamId::Global(GlobalParam::MasterPeakRight)),
                sample_rate,
                xruns: registry.get(ParamId::Global(GlobalParam::Xruns)),
                cpu_load: registry.get(ParamId::Global(GlobalParam::CpuLoad)),
                cue_mix: registry.get(ParamId::Global(GlobalParam::CueMix)),
                cue_split: registry.get(ParamId::Global(GlobalParam::CueSplit)) >= 0.5,
                booth_gain_db: registry.get(ParamId::Global(GlobalParam::BoothGainDb)),
                cue_available: registry.get(ParamId::Global(GlobalParam::CueAvailable)) >= 0.5,
                limiter_enabled: registry.get(ParamId::Global(GlobalParam::LimiterEnabled)) >= 0.5,
                limiter_reduction_db: registry
                    .get(ParamId::Global(GlobalParam::LimiterReductionDb)),
                output_latency_ms: to_seconds(
                    registry.get(ParamId::Global(GlobalParam::OutputLatencyFrames)),
                ) * 1000.0,
                quantize: registry.get(ParamId::Global(GlobalParam::Quantize)) >= 0.5,
                clap: {
                    let get = |p| registry.get(ParamId::Global(p));
                    ClapSnapshot {
                        loaded: get(GlobalParam::ClapLoaded) >= 0.5,
                        bypassed: get(GlobalParam::ClapBypass) >= 0.5,
                    }
                },
                automix: {
                    let get = |p| registry.get(ParamId::Global(p));
                    AutomixSnapshot {
                        enabled: get(GlobalParam::AutomixEnabled) >= 0.5,
                        mixing: get(GlobalParam::AutomixMixing) >= 0.5,
                        beats: get(GlobalParam::AutomixBeats),
                        style: dj_core::action::TransitionStyle::from_index(get(
                            GlobalParam::AutomixStyle,
                        )
                            as usize)
                        .as_str(),
                    }
                },
                mic: {
                    let get = |p| registry.get(ParamId::Global(p));
                    let flag = |p| get(p) >= 0.5;
                    MicSnapshot {
                        present: flag(GlobalParam::MicPresent),
                        open: flag(GlobalParam::MicOpen),
                        gain_db: get(GlobalParam::MicGainDb),
                        level: get(GlobalParam::MicLevel),
                        cue: flag(GlobalParam::MicCue),
                        talkover: flag(GlobalParam::MicTalkover),
                        ducking_db: get(GlobalParam::MicDuckingDb),
                        duck_db: get(GlobalParam::MicDuckDb),
                        threshold_db: get(GlobalParam::MicThresholdDb),
                        attack_ms: get(GlobalParam::MicAttackMs),
                        release_ms: get(GlobalParam::MicReleaseMs),
                        starved_frames: f64::from(get(GlobalParam::MicStarvedFrames)),
                    }
                },
                split_output: bridge.map(|stats| SplitOutputSnapshot {
                    drift_ppm: stats.drift_ppm() as f32,
                    queue_ms: to_seconds(stats.queued_frames() as f32) * 1000.0,
                    target_ms: to_seconds(stats.target_frames() as f32) * 1000.0,
                    // Through f64 rather than an integer: these cross into
                    // JavaScript, where every number is a double anyway, and a
                    // u64 would serialise as something the interface cannot add up.
                    starved_frames: stats.starved_frames() as f64,
                    dropped_samples: stats.dropped_samples() as f64,
                    healthy: stats.is_healthy(),
                }),
            },
        }
    }
}

/// Where the pump looks for the current two-card bridge, if there is one.
pub type BridgeHandle = Arc<std::sync::Mutex<Option<Arc<dj_audio::BridgeStats>>>>;

/// Where the pump looks for what is loaded on each deck.
pub type DeckTracks =
    std::sync::Mutex<std::collections::HashMap<u8, crate::state::LoadedTrackInfo>>;

/// And for what is in each sampler slot, by `(bank, slot)`.
pub type SampleNames = std::sync::Mutex<std::collections::HashMap<(u8, u8), String>>;

/// What the application remembers that the engine does not.
///
/// The engine holds audio and numbers; names live in the application, for decks
/// and for samples alike. Grouped into one parameter rather than added to
/// [`Snapshot::capture_all`] one at a time — this was the third such thing, and
/// a fourth would have made that function take six arguments.
#[derive(Debug, Default, Clone, Copy)]
pub struct Names<'a> {
    pub decks: Option<&'a DeckTracks>,
    pub samples: Option<&'a SampleNames>,
}

/// Everything the pump reads besides the registry.
///
/// One struct rather than four parameters: `run` had grown to eight arguments,
/// which is where a caller starts passing them in the wrong order and the
/// compiler cannot tell, because three of them are `Option<Arc<_>>`.
#[derive(Debug, Default)]
pub struct Sources {
    pub bridge: Option<BridgeHandle>,
    pub analysis: Option<Arc<crate::analysis::AnalysisStore>>,
    pub tracks: Option<Arc<DeckTracks>>,
    pub samples: Option<Arc<SampleNames>>,
    /// The set recording's counters. See [`crate::setrec::RecordingState`].
    pub recording: Option<Arc<crate::setrec::RecordingState>>,
}

/// A running snapshot pump. Stops when dropped.
#[derive(Debug)]
pub struct SnapshotPump {
    alive: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl SnapshotPump {
    /// Start sampling, handing each snapshot to `emit`.
    pub fn start(
        registry: Arc<ParameterRegistry>,
        deck_count: usize,
        emit: impl FnMut(Snapshot) + Send + 'static,
    ) -> Self {
        Self::with_heartbeat(registry, deck_count, HEARTBEAT_INTERVAL, emit)
    }

    /// Start sampling, including the two-card bridge when one is open.
    ///
    /// The handle is shared rather than a copy of the current bridge, because
    /// the bridge is replaced on every device change; a pump holding one
    /// directly would keep reporting drift from a closed stream.
    pub fn start_with_bridge(
        registry: Arc<ParameterRegistry>,
        deck_count: usize,
        sources: Sources,
        emit: impl FnMut(Snapshot) + Send + 'static,
    ) -> Self {
        Self::run(registry, deck_count, sources, HEARTBEAT_INTERVAL, emit)
    }

    /// Start sampling with an explicit heartbeat interval.
    ///
    /// Exists so tests can exercise the heartbeat in milliseconds rather than
    /// sleeping for the production interval.
    pub fn with_heartbeat(
        registry: Arc<ParameterRegistry>,
        deck_count: usize,
        heartbeat: Duration,
        emit: impl FnMut(Snapshot) + Send + 'static,
    ) -> Self {
        Self::run(registry, deck_count, Sources::default(), heartbeat, emit)
    }

    fn run(
        registry: Arc<ParameterRegistry>,
        deck_count: usize,
        sources: Sources,
        heartbeat: Duration,
        mut emit: impl FnMut(Snapshot) + Send + 'static,
    ) -> Self {
        let Sources {
            bridge,
            analysis,
            tracks,
            samples,
            recording,
        } = sources;
        let alive = Arc::new(AtomicBool::new(true));
        let thread = {
            let alive = Arc::clone(&alive);
            std::thread::Builder::new()
                .name("dj-snapshot".to_owned())
                .spawn(move || {
                    let period = Duration::from_micros(1_000_000 / SNAPSHOT_HZ);
                    let mut previous: Option<Snapshot> = None;
                    let mut last_emit = std::time::Instant::now();
                    while alive.load(Ordering::Relaxed) {
                        // Re-read through the handle every tick: the bridge is
                        // replaced whenever a device is opened.
                        let current = bridge.as_ref().and_then(|slot| slot.lock().ok()?.clone());
                        let snapshot = Snapshot::capture_all(
                            &registry,
                            deck_count,
                            current.as_deref(),
                            analysis.as_deref(),
                            Names {
                                decks: tracks.as_deref(),
                                samples: samples.as_deref(),
                            },
                            recording.as_deref(),
                        );
                        let changed = previous.as_ref() != Some(&snapshot);

                        // Skip identical frames -- an idle application should not
                        // wake the webview 60 times a second for no reason. But
                        // emit anyway on the heartbeat, because a listener that
                        // subscribes during a quiet period would otherwise never
                        // receive anything and sit on a blank interface forever.
                        if changed || last_emit.elapsed() >= heartbeat {
                            emit(snapshot.clone());
                            previous = Some(snapshot);
                            last_emit = std::time::Instant::now();
                        }
                        std::thread::sleep(period);
                    }
                })
                .expect("failed to spawn snapshot thread")
        };

        Self {
            alive,
            thread: Some(thread),
        }
    }
}

impl Drop for SnapshotPump {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn capture_reads_the_registry() {
        let registry = ParameterRegistry::new();
        let deck = DeckId::from_human(1).unwrap();
        registry.set(ParamId::Global(GlobalParam::SampleRate), 48_000.0);
        registry.set(ParamId::Deck(deck, DeckParam::Playing), 1.0);
        registry.set(ParamId::Deck(deck, DeckParam::Position), 96_000.0);
        registry.set(ParamId::Deck(deck, DeckParam::LengthFrames), 480_000.0);

        let snapshot = Snapshot::capture(&registry, 2);
        assert_eq!(snapshot.decks.len(), 2);
        assert_eq!(snapshot.decks[0].number, 1);
        assert!(snapshot.decks[0].playing);
        assert!((snapshot.decks[0].position_seconds - 2.0).abs() < 1e-6);
        assert!((snapshot.decks[0].length_seconds - 10.0).abs() < 1e-6);
        assert!(!snapshot.decks[1].playing);
    }

    /// Before a device is open the sample rate is zero. Naive division would
    /// put `Infinity` or `NaN` on screen.
    #[test]
    fn capture_survives_a_zero_sample_rate() {
        let registry = ParameterRegistry::new();
        let snapshot = Snapshot::capture(&registry, 2);
        assert_eq!(snapshot.decks[0].position_seconds, 0.0);
        assert!(snapshot.decks[0].length_seconds.is_finite());
    }

    #[test]
    fn pump_emits_when_state_changes() {
        let registry = Arc::new(ParameterRegistry::new());
        let seen = Arc::new(Mutex::new(Vec::new()));

        let pump = {
            let seen = Arc::clone(&seen);
            SnapshotPump::start(Arc::clone(&registry), 2, move |snapshot| {
                seen.lock().unwrap().push(snapshot);
            })
        };

        std::thread::sleep(Duration::from_millis(50));
        let baseline = seen.lock().unwrap().len();
        assert!(baseline >= 1, "should emit an initial snapshot");

        registry.set(
            ParamId::Deck(DeckId::from_human(1).unwrap(), DeckParam::Playing),
            1.0,
        );
        std::thread::sleep(Duration::from_millis(80));
        assert!(
            seen.lock().unwrap().len() > baseline,
            "a state change should produce a new snapshot"
        );
        drop(pump);
    }

    /// An idle application must not wake the webview 60 times a second.
    #[test]
    fn pump_stays_quiet_when_nothing_changes() {
        let registry = Arc::new(ParameterRegistry::new());
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Heartbeat pushed far out so this measures deduplication alone, with
        // no dependence on timing.
        let pump = {
            let count = Arc::clone(&count);
            SnapshotPump::with_heartbeat(
                Arc::clone(&registry),
                2,
                Duration::from_secs(60),
                move |_| {
                    count.fetch_add(1, Ordering::Relaxed);
                },
            )
        };

        std::thread::sleep(Duration::from_millis(200));
        drop(pump);

        let emitted = count.load(Ordering::Relaxed);
        assert_eq!(
            emitted, 1,
            "idle pump emitted {emitted} snapshots; should be exactly the initial one"
        );
    }

    /// A UI that subscribes during a quiet period must still receive state.
    /// Without the heartbeat it waits forever on a blank interface -- which is
    /// exactly what happened the first time the application was run.
    #[test]
    fn pump_heartbeats_so_late_subscribers_get_state() {
        let registry = Arc::new(ParameterRegistry::new());
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // A short heartbeat keeps the test fast; the mechanism is identical.
        let pump = {
            let count = Arc::clone(&count);
            SnapshotPump::with_heartbeat(
                Arc::clone(&registry),
                2,
                Duration::from_millis(100),
                move |_| {
                    count.fetch_add(1, Ordering::Relaxed);
                },
            )
        };

        // Several heartbeat intervals, with nothing changing at all. Generous
        // margin because sleep only guarantees a minimum.
        std::thread::sleep(Duration::from_millis(600));
        drop(pump);

        assert!(
            count.load(Ordering::Relaxed) >= 2,
            "expected at least one heartbeat beyond the initial snapshot"
        );
    }

    #[test]
    fn dropping_the_pump_stops_it() {
        let registry = Arc::new(ParameterRegistry::new());
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let pump = {
            let count = Arc::clone(&count);
            SnapshotPump::start(Arc::clone(&registry), 2, move |_| {
                count.fetch_add(1, Ordering::Relaxed);
            })
        };
        std::thread::sleep(Duration::from_millis(30));
        drop(pump);
        let after = count.load(Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(
            count.load(Ordering::Relaxed),
            after,
            "pump outlived its handle"
        );
    }

    /// **The path the deck header depends on.** A result sitting in the store
    /// has to appear in the snapshot, or the analyser would be running
    /// perfectly and the interface would show nothing.
    #[test]
    fn analysis_reaches_the_snapshot() {
        use crate::analysis::AnalysisStore;
        use dj_analysis::{Analysis, KeyAnalysis, Lufs, TempoAnalysis};
        use dj_core::{Beatgrid, Bpm, Confidence, FramePos, Mode, MusicalKey};

        let registry = ParameterRegistry::new();
        let store = AnalysisStore::new();
        let deck = DeckId::from_human(1).unwrap();

        store.record(
            deck,
            dj_core::TrackId::from_bytes([1; 32]),
            Analysis {
                tempo: Some(TempoAnalysis {
                    grid: Beatgrid::new(
                        FramePos::new(0.0),
                        Bpm::new(124.0).unwrap(),
                        Confidence::new(0.9),
                    ),
                    alternative: Bpm::new(62.0),
                }),
                key: Some(KeyAnalysis {
                    key: MusicalKey::new(8, Mode::Minor).unwrap(),
                    correlation: 0.71,
                    alternative: MusicalKey::new(8, Mode::Major),
                }),
                loudness: Lufs::new(-11.0),
            },
        );

        let snapshot = Snapshot::capture_full(&registry, 2, None, Some(&store));
        let found = snapshot.decks[0]
            .analysis
            .as_ref()
            .expect("deck 1 should carry its analysis");

        assert_eq!(found.bpm, Some(124.0));
        assert_eq!(
            found.bpm_alternative,
            Some(62.0),
            "the octave was not offered"
        );
        assert!(found.sync_worthy);
        assert_eq!(found.key_camelot.as_deref(), Some("8A"));
        assert_eq!(found.key_standard.as_deref(), Some("Am"));
        assert_eq!(found.key_alternative.as_deref(), Some("8B"));
        assert_eq!(found.lufs, Some(-11.0));
        assert!((found.auto_gain_db - -3.0).abs() < 0.01);

        // Deck 2 has nothing loaded, and must say so rather than borrowing
        // deck 1's numbers.
        assert!(snapshot.decks[1].analysis.is_none());
    }

    /// A grid the analyser is unsure of must be flagged, not offered. Syncing
    /// to a guess derails a mix at the moment the DJ has stopped watching.
    #[test]
    fn a_weak_grid_is_not_sync_worthy_in_the_snapshot() {
        use crate::analysis::AnalysisStore;
        use dj_analysis::{Analysis, Lufs, TempoAnalysis};
        use dj_core::{Beatgrid, Bpm, Confidence, FramePos};

        let registry = ParameterRegistry::new();
        let store = AnalysisStore::new();
        let deck = DeckId::from_human(1).unwrap();
        store.record(
            deck,
            dj_core::TrackId::from_bytes([2; 32]),
            Analysis {
                tempo: Some(TempoAnalysis {
                    grid: Beatgrid::new(
                        FramePos::new(0.0),
                        Bpm::new(97.0).unwrap(),
                        Confidence::new(0.2),
                    ),
                    alternative: None,
                }),
                key: None,
                loudness: Lufs::new(-14.0),
            },
        );

        let snapshot = Snapshot::capture_full(&registry, 1, None, Some(&store));
        let found = snapshot.decks[0].analysis.as_ref().unwrap();
        assert_eq!(found.bpm, Some(97.0), "a weak grid still has a number");
        assert!(!found.sync_worthy, "a weak grid was offered for sync");
        assert!(found.key_camelot.is_none());
    }

    /// The bug this covers is the one the device taught: a panel that knows
    /// only the loads it made itself shows nothing for a sample a script, a
    /// preset or the assistant put there. The name belongs to the application,
    /// the same as a deck's title, and it has to reach the interface the same
    /// way — through the snapshot.
    #[test]
    fn a_sample_name_reaches_the_interface_whoever_loaded_it() {
        let registry = ParameterRegistry::new();
        registry.set(ParamId::Global(GlobalParam::SampleRate), 48_000.0);
        registry.set(ParamId::Global(GlobalParam::SamplerBank), 2.0);
        let param = GlobalParam::sample(3).expect("slot 3 exists");
        registry.set(ParamId::Global(param.loaded), 1.0);

        let samples: SampleNames = std::sync::Mutex::new(std::collections::HashMap::from([
            ((2, 3), "airhorn".to_owned()),
            // A name in another bank must not leak into the one showing.
            ((1, 3), "wrong bank".to_owned()),
        ]));

        let snapshot = Snapshot::capture_all(
            &registry,
            2,
            None,
            None,
            Names {
                samples: Some(&samples),
                ..Default::default()
            },
            None,
        );
        let slots = &snapshot.master.sampler.slots;
        assert_eq!(snapshot.master.sampler.bank, 2);
        assert_eq!(slots[2].name.as_deref(), Some("airhorn"));
        assert!(slots[2].loaded);
        // And an empty slot has no name to show.
        assert_eq!(slots[0].name, None);
    }

    /// **The bug this field exists to fix.** The deck name used to be component
    /// state, set only by that deck's own Load button — so a track arriving
    /// from the browser, the assistant, a preset or a controller played
    /// perfectly while the header still read "no track". Caught by loading a
    /// track through the benchmark harness, which does exactly that.
    #[test]
    fn the_deck_name_travels_in_the_snapshot() {
        use crate::state::LoadedTrackInfo;
        use std::collections::HashMap;

        let registry = ParameterRegistry::new();
        let tracks: DeckTracks = std::sync::Mutex::new(HashMap::from([(
            1u8,
            LoadedTrackInfo {
                title: "Suavemente".to_owned(),
                artist: Some("Elvis Crespo".to_owned()),
                id: dj_core::TrackId::from_bytes([1; 32]),
            },
        )]));

        let snapshot = Snapshot::capture_all(
            &registry,
            2,
            None,
            None,
            Names {
                decks: Some(&tracks),
                ..Default::default()
            },
            None,
        );
        assert_eq!(snapshot.decks[0].title.as_deref(), Some("Suavemente"));
        assert_eq!(snapshot.decks[0].artist.as_deref(), Some("Elvis Crespo"));

        // An empty deck says nothing rather than borrowing its neighbour's name.
        assert!(snapshot.decks[1].title.is_none());
        assert!(snapshot.decks[1].artist.is_none());
    }

    /// A track with no artist tag is normal and must not become the string
    /// "None" or an empty artist line pretending to be one.
    #[test]
    fn a_track_with_no_artist_reports_none() {
        use crate::state::LoadedTrackInfo;
        use std::collections::HashMap;

        let registry = ParameterRegistry::new();
        let tracks: DeckTracks = std::sync::Mutex::new(HashMap::from([(
            1u8,
            LoadedTrackInfo {
                title: "untitled.wav".to_owned(),
                artist: None,
                id: dj_core::TrackId::from_bytes([2; 32]),
            },
        )]));

        let snapshot = Snapshot::capture_all(
            &registry,
            1,
            None,
            None,
            Names {
                decks: Some(&tracks),
                ..Default::default()
            },
            None,
        );
        assert_eq!(snapshot.decks[0].title.as_deref(), Some("untitled.wav"));
        assert!(snapshot.decks[0].artist.is_none());
    }
}
