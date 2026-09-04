/**
 * The entire surface between the interface and the engine.
 *
 * Two directions and nothing else: `dispatch` sends an action string down,
 * `onSnapshot` receives state 60 times a second coming up. There is no third
 * path — no direct mutation, no shared object. See
 * `docs/adr/0003-action-bus-and-parameter-registry.md`.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ResolvedTheme } from "./theme.svelte";

export interface Device {
  id: string;
  name: string;
  channels: number;
  sample_rate: number;
  is_default: boolean;
  supports_split_output: boolean;
}

export interface ActiveDevice {
  name: string;
  sample_rate: number;
  buffer_frames: number;
  channels: number;
  latency_ms: number;
  /** The second sound card carrying the headphone cue, when there is one. */
  cue: CueDevice | null;
  /**
   * Why a requested headphone device was not used. Not fatal — the master
   * still runs, and cueing falls back to the main device if it has the
   * channels for it.
   */
  cue_error: string | null;
}

export interface CueDevice {
  name: string;
  sample_rate: number;
  buffer_frames: number;
  latency_ms: number;
}

/**
 * How the two-card bridge is doing.
 *
 * Two sound cards run on independent crystals, so one genuinely produces more
 * audio per second than the other consumes. `drift_ppm` is the measured
 * disagreement: settling near zero means the pair is well matched, and a figure
 * that keeps climbing means a device is misreporting its rate.
 */
export interface SplitOutput {
  drift_ppm: number;
  queue_ms: number;
  target_ms: number;
  starved_frames: number;
  dropped_samples: number;
  healthy: boolean;
}

export interface LoadedTrack {
  deck: number;
  title: string;
  artist: string | null;
  album: string | null;
  duration_seconds: number;
  sample_rate: number;
  id: string;
}

/**
 * Which side of the crossfader a deck is on.
 *
 * `thru` means the crossfader is not in this deck's path at all — it plays at
 * whatever its channel fader says, wherever the crossfader is parked.
 */
export type CrossfaderAssign = "left" | "right" | "thru";

/** The slicer, for the pad page that draws it. */
export interface SliceState {
  /** Beats the eight pads divide up. */
  beats: number;
  /** Which slice the playhead is in, 1-based. Null without a grid. */
  at: number | null;
  holding: boolean;
}

export interface DeckState {
  number: number;
  /**
   * What is loaded, by name. From the snapshot rather than the deck component,
   * so a track loaded from the browser, the assistant or a controller shows its
   * name just like one loaded from the deck itself.
   */
  title: string | null;
  artist: string | null;
  playing: boolean;
  loaded: boolean;
  position_frames: number;
  length_frames: number;
  position_seconds: number;
  length_seconds: number;
  rate: number;
  pitch: number;
  volume: number;
  gain_db: number;
  peak: number;
  eq_low: number;
  eq_mid: number;
  eq_high: number;
  filter: number;
  cue_enabled: boolean;
  pre_fader_level: number;
  keylock: boolean;
  keylock_latency_ms: number;
  /** Whether a hand is on this deck's platter. */
  jog_touched: boolean;
  /** `"vinyl"` or `"cdj"` — how the platter behaves under that hand. */
  jog_mode: string;
  /** How far the wheel is bending the tempo, as a fraction of normal speed. */
  jog_bend: number;
  /** Deliberate transposition in semitones, for harmonic mixing. */
  key_shift: number;
  /** Which side of the crossfader cuts this deck, or neither. */
  /** Slip mode is armed: a shadow playhead runs while something diverts this one. */
  slip: boolean;
  /** Playing backwards, from reverse or from a held censor. */
  reversed: boolean;
  /**
   * A loop roll is being held.
   *
   * Distinct from `active_loop`: the two look alike and end differently, and a
   * DJ needs to know which of the two is on screen — a loop stays when you let
   * go of the pad, a roll does not.
   */
  rolling: boolean;
  slice: SliceState;
  /** The platter is coasting — braking, or thrown backwards. */
  spinning: boolean;
  /** Where the track would be if nothing were diverting it. Null when nothing is. */
  slip_position: number | null;
  /** Which side of the crossfader cuts this deck, or neither. */
  crossfader_assign: CrossfaderAssign;
  /** Mute states for the 4 stems (Vocal, Drums, Bass, Other). */
  stem_mutes: [boolean, boolean, boolean, boolean];
  /** Volume states for the 4 stems (Vocal, Drums, Bass, Other). */
  stem_volumes: [number, number, number, number];
  /**
   * Per-stem EQ trim for the 4 stems, each low/mid/high.
   *
   * The DJ's own setting on top of the deck's EQ rather than instead of it —
   * 1.0 is flat, which is what an untouched stem reads. Showing the product of
   * the two would make the knob jump whenever the channel strip moved.
   */
  stem_eq: [[number, number, number], [number, number, number], [number, number, number], [number, number, number]];
  /** Per-stem filter sweep, -1 (low-pass) through 0 (open) to 1 (high-pass). */
  stem_filters: [number, number, number, number];
  /**
   * True while a stem solo is held. Every stem mute is refused while it is,
   * so the panel needs this both to show the state and to release it.
   */
  stem_soloing: boolean;
  /**
   * What the analyser made of this track. Null while it is still running,
   * which is the normal state for the first second after a load.
   */
  analysis: TrackAnalysis | null;
  /** True when this deck's tempo is locked to another's. */
  synced: boolean;
  /**
   * Tempo actually being played, pitch fader included. Null when the track has
   * no grid — which is not the same as 0 BPM, and is shown differently.
   */
  effective_bpm: number | null;
  /**
   * True when the grid is solid enough for sync to accept it. Used to disable
   * the button rather than let it fail silently.
   */
  can_sync: boolean;
  /**
   * How much the *current* grid is trusted, 0..1. Live from the engine, so a
   * hand-edited grid reads as certain the moment it is edited — unlike the
   * number in `analysis`, which is what the analyser originally found.
   */
  grid_confidence: number;
  /** The region repeating right now, if any. */
  active_loop: LoopRegion | null;
  /** This deck's three effect slots, in order. */
  fx: FxSlot[];
  /**
   * Hot cue positions in frames, slot 1 first. `null` for an empty slot — which
   * is not the same as a cue at frame zero, and the start of a track is a
   * perfectly ordinary place to put one.
   */
  hot_cues: (number | null)[];
  /**
   * The loops saved against this record, by slot.
   *
   * Sparse rather than eight nullable entries like `hot_cues`: a loop is a
   * pair of frames, so an empty slot would be an object full of nothing.
   * Empty for a record nobody has saved a loop against, which is most of them.
   */
  saved_loops: SavedLoop[];
}

/** A loop kept with the record, drawn on the lane and recalled by its slot. */
export interface SavedLoop {
  slot: number;
  start_frames: number;
  end_frames: number;
}

/**
 * One effect slot.
 *
 * The kind is a name, not an index. The registry has to carry it as a number
 * because it holds `f32`, but a number here would be a thing to look up, and
 * every lookup is somewhere the two sides can disagree about what effect 3 is.
 */
export interface FxSlot {
  /** 1-based, as the interface and controllers number them. */
  slot: number;
  /** `none`, `echo`, `gate`, `crush`, `flanger`. */
  kind: string;
  enabled: boolean;
  wet: number;
  beats: number;
  amount: number;
  /** What the amount knob does, in the DJ's words. Empty for an empty slot. */
  amount_label: string;
  /** Whether the beat control means anything for this effect. */
  timed: boolean;
  /** True when the slot sits after the channel fader. */
  post_fader: boolean;
}

/**
 * What makes a pad light.
 *
 * Mirrors `dj_core::pads::Lit`. One condition the interface evaluates against
 * the snapshot, rather than a branch per page — so a new page is rows in a Rust
 * table, not cases in a Svelte component.
 */
export type Lit =
  | "Never"
  | { HotCueSet: number }
  | { SavedLoopSet: number }
  | { LoopBeats: number }
  | { RollBeats: number }
  | { FxSlotOn: number }
  | { FxSlotPost: number }
  | { SamplePlaying: number }
  | { SliceAt: number }
  | { StemMuted: "Vocal" | "Drums" | "Bass" | "Other" }
  | { StemSolo: "Vocal" | "Drums" | "Bass" | "Other" };

/** One pad, with its actions already written out by the backend. */
export interface PadDto {
  label: string;
  /** Null for a pad this page leaves blank. */
  press: string | null;
  /** Present only on a momentary pad. */
  release: string | null;
  /** The secondary gesture — right-click on screen, shift on hardware. */
  clear: string | null;
  lit: Lit;
}

/** One page of eight pads. */
export interface PadPageDto {
  name: string;
  /** True when every pad on it is measured in beats. */
  needs_grid: boolean;
  pads: PadDto[];
}

/**
 * Every pad page for a deck, with the action strings pre-rendered.
 *
 * From Rust rather than restated here: the same table is what a controller's
 * pads map onto, and two copies of a mapping is a pad that does one thing under
 * the finger and another on the screen.
 */
export const padPages = (deck: number) => invoke<PadPageDto[]>("pad_pages", { deck });

/** Every effect, in the order they are offered. */
export const EFFECTS = [
  "none",
  "echo",
  "delay",
  "reverb",
  "gate",
  "crush",
  "flanger",
  "phaser",
  "filter",
] as const;

export interface LoopRegion {
  start_frames: number;
  end_frames: number;
  /** Length in beats, for a label. Null without a grid to measure against. */
  beats: number | null;
}

/**
 * Tempo, key and loudness.
 *
 * Every field is optional on purpose: a field recording has no tempo, a drum
 * loop has no key, and showing a plausible zero instead of "could not tell" is
 * how a DJ ends up syncing to a grid that was never there.
 */
export interface TrackAnalysis {
  bpm: number | null;
  /** 0..=1. */
  bpm_confidence: number | null;
  /** The rejected octave — usually half or double. */
  bpm_alternative: number | null;
  /** Whether the grid is solid enough to sync to. */
  sync_worthy: boolean;
  /** Camelot notation, e.g. `8A`. */
  key_camelot: string | null;
  /** Standard notation, e.g. `Am`. */
  key_standard: string | null;
  key_confidence: number | null;
  /** The runner-up, usually the relative major or minor. */
  key_alternative: string | null;
  /** Integrated loudness, LUFS. */
  lufs: number | null;
  /** Trim that would bring this track to the reference loudness. */
  auto_gain_db: number;
  /**
   * Phrase length in beats — 8, 16 or 32 — when the track has a phrase
   * structure. `null` is a real answer: live and ambient records may have
   * none, and marking one anyway gives a DJ something to mix on that means
   * nothing.
   */
  phrase_beats: number | null;
  /**
   * Which beat, counted from the grid anchor, starts a phrase. Not always
   * zero: plenty of records open with a four- or eight-beat pickup.
   */
  phrase_anchor: number | null;
  phrase_confidence: number | null;
}

/** One sampler pad. */
export interface SampleSlot {
  /** 1-based. */
  slot: number;
  /**
   * What is in it, by name.
   *
   * From the snapshot rather than remembered by the panel: a panel that only
   * knows the loads it made itself shows nothing for a sample a script, a
   * preset or the assistant put there.
   */
  name: string | null;
  loaded: boolean;
  playing: boolean;
  /** `one_shot`, `hold`, `loop`, `stutter`. */
  mode: string;
  volume: number;
  /** How far through, 0..=1. */
  progress: number;
  /** True when it goes to the headphones rather than the mix. */
  cue: boolean;
  synced: boolean;
  /**
   * The sample's own tempo. Null when it has none — which is why the sync
   * switch is hidden rather than greyed out.
   */
  bpm: number | null;
}

/** Capturing into a sampler slot. Mirrors `RecordSnapshot` in Rust. */
export interface RecordState {
  /**
   * Whether there is a buffer to record into.
   *
   * False for the moment a finished capture is being turned into a sample.
   * Shown rather than hidden: it is not a state the DJ caused, and a button
   * that silently declines is worse than one that says why.
   */
  ready: boolean;
  recording: boolean;
  slot: number | null;
  seconds: number;
  max_seconds: number;
  /** "master", or "deck 2". */
  source: string | null;
}

export interface SamplerState {
  /** 1-based. */
  bank: number;
  volume: number;
  peak: number;
  /**
   * The showing bank's eight slots. The other banks keep playing; the pads
   * simply cannot reach them.
   */
  slots: SampleSlot[];
  record: RecordState;
}

/** Every trigger mode, in the order they are offered. */
export const TRIGGER_MODES = ["one_shot", "hold", "loop", "stutter"] as const;

/**
 * Put a file in a sampler slot.
 *
 * The bank is named rather than assumed, so a load cannot land in the wrong
 * place because the DJ switched banks while the file was being read.
 */
export const loadSample = (bank: number, slot: number, path: string) =>
  invoke<{ bank: number; slot: number; name: string; duration_seconds: number }>(
    "load_sample",
    { bank, slot, path },
  );

/** Recording the whole mix to disk. */
export interface SetRecordingState {
  active: boolean;
  seconds: number;
  /**
   * Samples that never reached the disk.
   *
   * Non-zero means a gap in the file. Shown rather than swallowed: the audio
   * thread will never wait for a slow disk, so a recording can have a hole in
   * it, and the DJ should find that out now rather than on playback.
   */
  dropped: number;
  /** The writer gave up — a full disk, usually. */
  failed: boolean;
}

/** One of a loaded plugin's own controls. */
export interface PluginParam {
  id: number;
  name: string;
  /** The plugin's own grouping, e.g. `Filter/Cutoff`. Empty when it does not group. */
  module: string;
  min: number;
  max: number;
  default: number;
  value: number;
  /** A mode switch rather than a knob: only whole numbers. */
  stepped: boolean;
  /** The plugin will not let a host change it. */
  readOnly: boolean;
}

/** A CLAP plugin found on disk, before anything is loaded. */
export interface PluginFile {
  path: string;
  name: string;
}

/** What is on the master's plugin insert. */
export interface PluginState {
  loaded: boolean;
  name: string;
  vendor: string;
  path: string;
  params: PluginParam[];
}

/**
 * The plugin insert, on the 60 Hz snapshot.
 *
 * Only what changes that fast. The name and the parameter list are fetched by
 * `pluginState()` — pushing a two-hundred-parameter list sixty times a second
 * would be most of the traffic to this window.
 */
export interface ClapState {
  loaded: boolean;
  /** Loaded but out of the signal path. */
  bypassed: boolean;
}

export type TransitionStyle = "cut" | "fade" | "blend" | "echo";

/** Every style, in the order the interface offers them. */
export const TRANSITION_STYLES: readonly TransitionStyle[] = [
  "blend",
  "fade",
  "cut",
  "echo",
];

/** What each style does, in the words a DJ would use. */
export const TRANSITION_HELP: Record<TransitionStyle, string> = {
  blend: "Crossfade with the outgoing bass pulled out. What a DJ does by hand.",
  fade: "A straight crossfade.",
  cut: "One stops, the next starts. Right for unrelated songs.",
  echo: "An echo over the outgoing track so it dissolves rather than ends.",
};

/** The automix, when the DJ has handed the mix over. */
export interface AutomixState {
  enabled: boolean;
  /** True only while a transition is actually running. */
  mixing: boolean;
  /** How long a transition lasts, in beats. */
  beats: number;
  style: TransitionStyle;
}

/** The microphone / line input strip. */
export interface MicState {
  /**
   * An input device is attached.
   *
   * Distinct from `open`: a DJ can arm the channel with nothing plugged in,
   * and showing those the same way would leave someone talking into a
   * microphone that was never connected.
   */
  present: boolean;
  /** The channel is open. */
  open: boolean;
  gain_db: number;
  /** Peak level after the gain, 0..=1. */
  level: number;
  /** The microphone is going to the headphones as well. */
  cue: boolean;
  /**
   * Talkover is switched on. Off is the aux case — a phone or a second laptop
   * should not duck the mix every time it makes a sound.
   */
  talkover: boolean;
  /** How far the music is being ducked right now, in positive decibels. */
  ducking_db: number;
  /** How far the music drops when talkover engages, in positive decibels. */
  duck_db: number;
  threshold_db: number;
  attack_ms: number;
  release_ms: number;
  /**
   * Frames the input ring could not supply. Non-zero means the input is not
   * keeping up — a real fault with a real fix, and invisible without this.
   */
  starved_frames: number;
}

/** An input device feeding the microphone strip. */
export interface MicDevice {
  name: string;
  sampleRate: number;
  bufferFrames: number;
  channels: number;
  /**
   * One-way latency of the input alone. The DJ hears themselves this much late
   * *plus* the output's own latency.
   */
  latencyMs: number;
}

export interface StemSwap {
  /** The stem's place in the project's one stem order. */
  stem: number;
  /** 1-based, as the decks are labelled. */
  from: number;
  to: number;
}

export interface MasterState {
  /** The sampler: which bank is showing, its level, and that bank's slots. */
  sampler: SamplerState;
  /** Recording the whole mix to disk. */
  recording: SetRecordingState;
  /** The master rack's three slots, in order. */
  fx: FxSlot[];
  crossfader: number;
  gain_db: number;
  peak_left: number;
  peak_right: number;
  sample_rate: number;
  xruns: number;
  cpu_load: number;
  cue_mix: number;
  cue_split: boolean;
  booth_gain_db: number;
  cue_available: boolean;
  /** One deck's stem playing over another's mix, if a swap is in force. */
  stem_swap: StemSwap | null;
  /** False when the master limiter has been bypassed. */
  limiter_enabled: boolean;
  /**
   * Gain reduction the limiter is applying, in positive decibels.
   *
   * The master meter reads post-limiter and so can never show over 0 dB. This
   * is the number that says how hard the mix is being driven.
   */
  limiter_reduction_db: number;
  /** Delay the output chain adds after the decks, in milliseconds. */
  output_latency_ms: number;
  /** Present only when the headphone cue is on a second sound card. */
  split_output: SplitOutput | null;
  /** True when beat jumps snap to the grid. */
  quantize: boolean;
  /** The microphone / line input strip. */
  mic: MicState;
  /** The automix. */
  automix: AutomixState;
  /** The plugin insert. */
  clap: ClapState;
}

export type SessionPhase = "warm_up" | "heat" | "peak" | "cooldown" | "chill_out";

export type TimeOfDay = "dawn" | "day" | "dusk" | "night" | "small_hours";

export interface EnvironmentContext {
  time_of_day: TimeOfDay;
}

/** Measured off the master bus every snapshot. See `dj_dsp::Spectrum`. */
export interface AudioMetrics {
  /** 0..=1, an RMS rather than a peak — the limiter pins peaks at 1. */
  loudness: number;
  /** Bass, low mid, high mid, treble. 0..=1 and comparable with each other. */
  bands: [number, number, number, number];
}

/**
 * Somebody's reading of the room.
 *
 * Only ever present once something has actually read it, which is M9. Until
 * then `SessionContext.session` is null and a theme shows its neutral
 * treatment rather than guessing.
 */
export interface SessionRead {
  phase: SessionPhase;
  energy: number;
  environment: EnvironmentContext;
}

export interface SessionContext {
  audio: AudioMetrics;
  session: SessionRead | null;
}

export interface Snapshot {
  context: SessionContext;
  decks: DeckState[];
  master: MasterState;
}

export const listDevices = () => invoke<Device[]>("list_devices");

/** Devices that can capture. Empty is a normal answer. */
export const listInputs = () => invoke<Device[]>("list_inputs");

/**
 * Attach an input device to the microphone strip.
 *
 * The cable, not the switch — `mic on` opens the channel. Separate because
 * opening a sound card takes long enough to miss a cue, so a DJ plugs in once
 * and toggles the channel all evening.
 */
export const openMic = (deviceId: string | null) =>
  invoke<MicDevice>("open_mic", { deviceId });

export const closeMic = () => invoke<void>("close_mic");

/**
 * Every CLAP plugin in the standard search paths.
 *
 * Scanning reads directory names and nothing else — no plugin code runs until
 * one is actually loaded.
 */
export const listPlugins = () => invoke<PluginFile[]>("list_plugins");

export const pluginState = () => invoke<PluginState>("plugin_state");

/** Which separator is running, and what a better one would need. */
export type StemsStatus = {
  /** True when something is separating — the built-in separator counts. */
  available: boolean;
  /** What is doing it, to show beside the controls. */
  backend: string | null;
  /** Why a downloaded model is not being used. `null` when one is. */
  reason: string | null;
};

/**
 * Ask which separator is running.
 *
 * Two different questions, deliberately kept apart. `available` is whether the
 * stem controls do anything; `reason` is why the *better* separator is not the
 * one doing it. A fresh install has no model, falls back to the built-in
 * separator, and is therefore available *and* has a reason.
 */
export const stemsStatus = () => invoke<StemsStatus>("stems_status");

/** Tempo sync with other djmanzo instances on the network. */
export type PeerStatus = {
  running: boolean;
  /** Where this instance is listening, once it is. */
  address: string | null;
  /** Where announcements go — a broadcast address, or one peer. */
  sendTo: string | null;
  /** How many other instances are on the network. */
  peers: number;
  /** The tempo the peers have settled on, when one of them is playing. */
  peerBpm: number | null;
  error: string | null;
};

/** Who is on the network, and what tempo they have settled on. */
export const peerStatus = () => invoke<PeerStatus>("peer_status");

/**
 * Start syncing tempo with other djmanzo instances.
 *
 * Both addresses default to loopback, so trying it out on one machine works
 * before anything is plugged in. This is djmanzo-to-djmanzo — it is not
 * Ableton Link, which is GPL-or-proprietary and cannot be linked here.
 */
export const startPeerSync = (listen?: string, sendTo?: string) =>
  invoke<PeerStatus>("start_peer_sync", { listen, sendTo });

export const stopPeerSync = () => invoke<PeerStatus>("stop_peer_sync");

/** Whether a deck can be sent out in parts, and which one is. */
export type StemOut = {
  /** The deck going out in parts, as the DJ numbers it. */
  deck: number | null;
  /**
   * How many decks are going out on pairs of their own.
   *
   * In the same shape as `deck` because the two are exclusive — they want the
   * same sockets, and a panel that could show both on would be describing an
   * engine state that cannot exist.
   */
  decks: number | null;
  /** The most decks this device could carry a pair for. */
  deckCapacity: number;
  /** Outputs on the open device. `null` when no device is open. */
  channels: number | null;
  /** How many outputs this needs. */
  required: number;
  /** True when the open device is wide enough. */
  supported: boolean;
};

/** Which deck is being sent out in parts, and whether the device allows it. */
export const stemOut = () => invoke<StemOut>("stem_out");

/**
 * Send one deck out in parts, or stop.
 *
 * Accepted even on an interface too narrow for it. The choice is remembered
 * and takes effect if a wider one is plugged in later, which is the order a DJ
 * setting up actually does things in.
 */
export const setStemOut = (deck: number | null) =>
  invoke<StemOut>("set_stem_out", { deck });

/**
 * Send every deck out on a pair of its own, or stop.
 *
 * `null` or zero puts the mix back. Choosing this puts stem out away, because
 * the two arrangements want the same sockets.
 */
export const setDeckOut = (decks: number | null) =>
  invoke<StemOut>("set_deck_out", { decks });

// -- timecode vinyl ---------------------------------------------------------

/** A control record djmanzo knows how to read. */
export type TimecodeFormat = {
  name: string;
  carrierHz: number;
  bits: number;
  /**
   * How long the record runs before a position could be mistaken for another.
   * The one number that decides whether a format suits a set.
   */
  unambiguousSeconds: number;
  /** Whether the numbers describe a record that could work at all. */
  usable: boolean;
};

/** One deck's relationship with a turntable. */
export type TimecodeDeck = {
  /** 1-based, as it is printed on the hardware. */
  deck: number;
  running: boolean;
  format: string | null;
  device: string | null;
  absolute: boolean;
  /**
   * How much of what is arriving looks like timecode, 0..1 — and **negative
   * when the deck is not on vinyl at all**. Zero means connected and hearing
   * nothing, which is a different problem with a different fix, so the panel
   * must not collapse the two.
   */
  quality: number;
  /** What the record is asking for; 1 is normal play, negative is backwards. */
  speed: number;
};

export type TimecodeStatus = {
  decks: TimecodeDeck[];
  formats: TimecodeFormat[];
  /** Nothing can be attached before an output is open. */
  engineRunning: boolean;
  /** The compatibility caveat, in the words the panel should print. */
  caveat: string;
};

/** Which decks are on vinyl, and how well it is going. */
export const timecodeStatus = () => invoke<TimecodeStatus>("timecode_status");

/**
 * Put a deck on a control record.
 *
 * `absolute` decides what the record means: in absolute mode the needle's place
 * on the record is the playhead's place in the track, so dropping the needle
 * two minutes in starts the track two minutes in. In relative mode only the
 * movement is followed, which is what most DJs want most of the time.
 */
export const startTimecode = (
  deck: number,
  deviceId: string | null,
  format: string | null,
  absolute: boolean,
) =>
  invoke<TimecodeStatus>("start_timecode", {
    deck,
    deviceId,
    format,
    absolute,
  });

/** Take a deck off vinyl and give it its transport back. */
export const stopTimecode = (deck: number) =>
  invoke<TimecodeStatus>("stop_timecode", { deck });

export type WrittenSignal = {
  path: string;
  seconds: number;
  sampleRate: number;
  format: string;
};

/**
 * Write djmanzo's control signal to a WAV file.
 *
 * The answer to "djmanzo ships no Serato format": burn this to a CD or put it
 * on a phone and any turntable, CD deck or media player becomes a controller.
 */
export const writeTimecodeSignal = (
  path: string,
  format: string | null,
  seconds: number | null,
  sampleRate: number | null,
) =>
  invoke<WrittenSignal>("write_timecode_signal", {
    path,
    format,
    seconds,
    sampleRate,
  });

// -- the mapping editor -----------------------------------------------------

/** What a control should do. */
export type Role =
  | { kind: "latching"; press: string }
  | { kind: "momentary"; press: string; release: string }
  | { kind: "continuous"; action: string; min?: number; max?: number }
  | { kind: "encoder"; up: string; down: string; encoding: string };

export type DraftBinding = { on: string; does: string };

export type MappingDraft = {
  name: string;
  device: string;
  bindings: DraftBinding[];
  /** Whether the port is describing controls rather than acting on them. */
  learning: boolean;
  /** The last control touched while learning, as a mapping file writes it. */
  learned: string | null;
};

/**
 * Turn learning on or off.
 *
 * While it is on a control says what it is instead of doing what it does —
 * otherwise learning the play button would start the deck every time.
 */
export const mappingLearn = (on: boolean) =>
  invoke<MappingDraft>("mapping_learn", { on });

/** The draft as it stands, including whatever control was last touched. */
export const mappingDraft = () => invoke<MappingDraft>("mapping_draft");

export const mappingRename = (name: string, device: string) =>
  invoke<MappingDraft>("mapping_rename", { name, device });

/** Give a control a job. Rejects an action the engine does not have. */
export const mappingBind = (on: string, role: Role) =>
  invoke<MappingDraft>("mapping_bind", { on, role });

export const mappingUnbind = (on: string) =>
  invoke<MappingDraft>("mapping_unbind", { on });

/** Start again, optionally from a mapping that already nearly fits. */
export const mappingDraftFrom = (name: string | null) =>
  invoke<MappingDraft>("mapping_draft_from", { name });

/** Write the draft into the user's mappings directory. Returns the path. */
export const mappingSave = () => invoke<string>("mapping_save");

/**
 * Put a plugin on the master.
 *
 * Loading runs third-party code in this process. There is no way to host
 * plugins that is not that, and it is worth saying out loud.
 */
export const loadPlugin = (path: string, pluginId?: string) =>
  invoke<PluginState>("load_plugin", { path, pluginId: pluginId ?? null });

export const clearPlugin = () => invoke<void>("clear_plugin");

/** A panel that can be given a window of its own. */
export interface PanelInfo {
  id: string;
  title: string;
  detached: boolean;
}

export const listPanels = () => invoke<PanelInfo[]>("list_panels");

/**
 * Give a panel a window of its own.
 *
 * The window is opened and nothing else: where it goes is the desktop's
 * business. djmanzo never asks how many screens there are — every attempt to
 * be cleverer than that ends with an application that puts a panel on a
 * projector.
 */
export const detachPanel = (panel: string) =>
  invoke<void>("detach_panel", { panel });

export const attachPanel = (panel: string) =>
  invoke<void>("attach_panel", { panel });

/**
 * Open the output.
 *
 * `cueDeviceId` puts the headphone cue on a *second* sound card. Only worth it
 * when the main device has no spare channels: two cards means two clocks, and a
 * drift-correcting resampler between them.
 */
export const openDevice = (
  deviceId: string | null,
  cueDeviceId: string | null,
  bufferFrames: number,
) =>
  invoke<ActiveDevice>("open_device", {
    deviceId,
    cueDeviceId,
    bufferFrames,
  });

/**
 * What is open, if anything.
 *
 * Asked on startup and whenever the interface finds a running engine it did not
 * start itself. Opening a device is not something only the Connect button does —
 * a preset, a script, the assistant or a restored session can all do it — and an
 * interface that learns about the device only from its own call shows "no
 * device" while audio is playing.
 */
export const activeDevice = () => invoke<ActiveDevice | null>("active_device");

export const startAudio = () => invoke<void>("start_audio");
export const stopAudio = () => invoke<void>("stop_audio");

export const loadTrack = (deck: number, path: string) =>
  invoke<LoadedTrack>("load_track", { deck, path });

/** Send an action in its text form, e.g. `deck 1 play`. */
export const dispatch = (action: string) => invoke<void>("dispatch", { action });

export const sessionLog = () => invoke<string[]>("session_log");

/** What a saved set contains. */
export interface SessionSummary {
  path: string;
  events: number;
  seconds: number;
  /** Distinct tracks that went on a deck. */
  tracks: number;
}

/**
 * Write the set so far to a file.
 *
 * Text, one event per line, in the same words an action is written in
 * everywhere else — so it can be read, annotated, and diffed.
 */
export const sessionSave = (path: string) =>
  invoke<SessionSummary>("session_save", { path });

/**
 * Read a saved set and say what is in it.
 *
 * Deliberately does *not* replay it. Opening a file and having a set start
 * playing would be the worst possible behaviour in a booth.
 */
export const sessionOpen = (path: string) =>
  invoke<SessionSummary>("session_open", { path });

/** One difference between two takes of a set. */
export interface DivergenceLine {
  kind: "only_in_first" | "only_in_second" | "drift";
  event: string;
  /** Seconds. For a drift, how much later the second take was. */
  seconds: number;
}

/**
 * Compare two takes of the same set.
 *
 * Not a text diff: two takes are the same decisions at different times, and a
 * line comparison of a file whose first column is a timestamp calls every line
 * changed. This reports which moves differ and how far they drifted.
 */
export const sessionDiff = (first: string, second: string) =>
  invoke<DivergenceLine[]>("session_diff", { first, second });

export interface WaveformInfo {
  deck: number;
  ready: boolean;
  total_frames: number;
  /**
   * Generation of this deck's tiles.
   *
   * Goes into every tile URL, and it has to: tiles are served immutable for a
   * year, so without it the webview would redisplay the previous track's
   * waveform after a load, and a beat-grid edit would appear to do nothing.
   */
  epoch: number;
  /**
   * Stretches of the record with the drums out — §25's breakdown layer.
   *
   * In frames, worked out in Rust from the beat indices the analyser reports
   * and the grid it counted them against. The interface does not convert: it
   * would need the grid, the tempo and the record's own sample rate to form a
   * second opinion about where beat 96 is.
   */
  breakdowns: FrameSpan[];
  /**
   * Frames where the drums come back. One per breakdown that ends before the
   * record does — a record that fades out has a breakdown and no drop, which
   * is most of them.
   */
  drops: number[];
  /**
   * Where each phrase starts, in frames.
   *
   * The lines are drawn into the tiles, where they align with the audio
   * pixel-exactly. These are the same places again so the lane can put a
   * *grab target* on each one — nothing here is drawn, which is why two lists
   * of the same boundaries is not two answers about where beat 96 is.
   */
  phrases: number[];
}

/** A stretch of a record, in frames. */
export interface FrameSpan {
  start_frame: number;
  end_frame: number;
}

export const waveformInfo = (deck: number) =>
  invoke<WaveformInfo>("waveform_info", { deck });

/**
 * URL for one waveform tile.
 *
 * Served by the Rust renderer over a custom protocol rather than pushed through
 * IPC, so the browser decodes it off the main thread and every subsequent frame
 * is a compositor translation. Zoom is carried as milli-units so fractional
 * values still key the cache exactly.
 *
 * The theme is in the path rather than a header because tiles are cached by
 * URL, hard and for a year. Two themes sharing a URL would mean switching kept
 * serving whichever palette was rendered first.
 */
export function tileUrl(
  deck: number,
  width: number,
  height: number,
  startFrame: number,
  framesPerPixel: number,
  theme: ResolvedTheme,
  epoch: number,
): string {
  const zoomMilli = Math.round(framesPerPixel * 1000);
  const start = Math.round(startFrame);
  const path = `tile/${deck}/${width}/${height}/${start}/${zoomMilli}/${theme}/${epoch}`;
  // Tauri rewrites custom schemes differently per platform: Linux/WebKitGTK
  // keeps `scheme://`, while Windows needs the `http://scheme.localhost` form.
  // macOS accepts the former.
  return navigator.userAgent.includes("Windows")
    ? `http://wave.localhost/${path}`
    : `wave://localhost/${path}`;
}

/**
 * URL for a record's sleeve, read out of the file's own tags.
 *
 * Served over a custom protocol for the same reason waveform tiles are: as an
 * `<img>` the browser fetches only what is on screen, decodes it off the main
 * thread and keeps it, where base64 through IPC would put a megabyte of JPEG
 * in a JSON string per card on every render.
 *
 * **404 is the ordinary answer.** Most collections are part-tagged, and a card
 * with no sleeve falls back to its lettering — see `Library.svelte`. Nothing
 * here can tell in advance which records have one, so the request is the test.
 */
/**
 * Move a hot cue to somewhere in the record.
 *
 * §26's first example — the DJ grabs the marker rather than driving the record
 * to it and pressing the pad again. The frame is what the pointer knew;
 * whether it snaps to a beat is djmanzo's business and the DJ's quantise
 * setting, because working out which beat a pixel is would need the grid, the
 * tempo and the record's own sample rate.
 *
 * Moving a slot with nothing in it does nothing, deliberately: see
 * `DeckAction::HotCueMove`.
 */
/**
 * Move one edge of the active loop.
 *
 * §26's "Loop — resize". The frame is the pointer's; whether it snaps, whether
 * the loop would be too short, and whether there is a loop to resize at all
 * are djmanzo's — see `Deck::move_loop_edge`.
 */
/**
 * Say that a phrase starts at this frame.
 *
 * §26's "Phrase marker — drag to adjust". The analyser can be right about how
 * long a phrase is and wrong about which beat starts one; nothing could
 * correct that before, and every mix djmanzo plans is placed on it.
 *
 * Moves the anchor of the phrase the record has. A record with no phrase
 * structure has no boundary to move, and says so.
 */
export const movePhrase = (deck: number, frame: number) =>
  invoke<void>("move_phrase", { deck, frame });

export const moveLoopEdge = (deck: number, edge: "start" | "end", frame: number) =>
  invoke<void>("move_loop_edge", { deck, edge, frame });

export const moveHotCue = (deck: number, slot: number, frame: number) =>
  invoke<void>("move_hot_cue", { deck, slot, frame });

export function coverUrl(track: string): string {
  // Same per-platform rewriting as `tileUrl`; see the comment there.
  return navigator.userAgent.includes("Windows")
    ? `http://cover.localhost/${track}`
    : `cover://localhost/${track}`;
}

// ---------------------------------------------------------------------------
// Sources
// ---------------------------------------------------------------------------

export interface Credential {
  id: string;
  label: string;
  signup_url: string;
  free_tier: string;
  is_set: boolean;
  /** Last four characters of a stored value. Never the value itself. */
  hint: string;
}

export type AudioAccess = "direct" | "user_supplied" | "none";
export type SourceStatus = "ready" | "needs_credentials" | "partner_gated" | "disabled";

export interface Source {
  id: string;
  label: string;
  summary: string;
  detail: string;
  can_search: boolean;
  audio: AudioAccess;
  /** Why audio is unavailable, when it is. */
  audio_note: string;
  partner_gated: boolean;
  credentials: Credential[];
  status: SourceStatus;
  status_detail: string;
}

export interface SourceTrack {
  provider: string;
  id: string;
  title: string;
  artist: string;
  album: string | null;
  duration_seconds: number | null;
  bpm: number | null;
  key: string | null;
  genre: string | null;
  artwork_url: string | null;
  web_url: string | null;
  playable: boolean;
}

export interface SearchResults {
  provider: string;
  label: string;
  tracks: SourceTrack[];
  error: string | null;
  /** How many results were matched to a file the user already owns. */
  matched_locally: number;
}

export interface Library {
  folders: string[];
  tracks: number;
}

export const listSources = () => invoke<Source[]>("list_sources");
export const setSecret = (id: string, value: string) =>
  invoke<void>("set_secret", { id, value });
export const clearSecret = (id: string) => invoke<void>("clear_secret", { id });
export const secretsPersist = () => invoke<boolean>("secrets_persist");

/**
 * Where this platform keeps music, when the folder is actually there.
 *
 * `null` rather than a guess: offering to scan a directory that does not exist
 * would fail on the click, which is worse than not offering.
 */
export const defaultMusicFolder = () => invoke<string | null>("default_music_folder");
export const addMusicFolder = (path: string) =>
  invoke<number>("add_music_folder", { path });
export const removeMusicFolder = (path: string) =>
  invoke<void>("remove_music_folder", { path });
export const musicLibrary = () => invoke<Library>("music_library");

export const searchSources = (text: string, provider?: string, limit?: number) =>
  invoke<SearchResults[]>("search_sources", { text, provider, limit });

/** Turn a search result into a path a deck can load, fetching it if need be. */
export const resolveSourceTrack = (track: SourceTrack) =>
  invoke<string>("resolve_source_track", {
    track: {
      provider: track.provider,
      id: track.id,
      title: track.title,
      artist: track.artist,
    },
  });

// ---------------------------------------------------------------------------
// Branding
// ---------------------------------------------------------------------------

export const setBrandLogo = (path: string) => invoke<void>("set_brand_logo", { path });
export const clearBrandLogo = () => invoke<void>("clear_brand_logo");
export const hasBrandLogo = () => invoke<boolean>("has_brand_logo");

/**
 * URL for the user's logo.
 *
 * Takes a cache-buster because the logo is replaced in place at the same URL —
 * without one, a newly-chosen logo would not appear until a restart, which
 * reads as the change having failed.
 */
export function logoUrl(version: number): string {
  const path = `logo?v=${version}`;
  return navigator.userAgent.includes("Windows")
    ? `http://brand.localhost/${path}`
    : `brand://localhost/${path}`;
}

// -- the library -----------------------------------------------------------

/** One track as the browser shows it. Pre-formatted in Rust — see the DTO. */
export interface LibraryTrack {
  id: string;
  path: string;
  title: string;
  artist: string;
  album: string | null;
  genre: string | null;
  year: number | null;
  duration_seconds: number;
  bpm: number | null;
  /** Camelot notation, which is what a DJ mixes by. */
  key: string | null;
  /** The same key in standard notation — `Am`, `C`. */
  key_name: string | null;
  loudness_lufs: number | null;
  /** True once the track has everything sync and harmonic mixing need. */
  analysed: boolean;
  play_count: number;
  /** 0..=5, when the DJ has rated it. */
  rating: number | null;
  /** `#rrggbb`, when the DJ has coloured it. */
  colour: string | null;
  /** Unix seconds, or null for a record that has never been played. */
  last_played: number | null;
  /** Phrase length in beats, when the analyser found one. */
  phrase_beats: number | null;
}

export interface FailedFile {
  path: string;
  reason: string;
}

export interface LibraryStatus {
  tracks: number;
  /** Files scanned but not yet identified. */
  pending: number;
  failed: FailedFile[];
  folders: string[];
  /** Identified since the application started. */
  identified: number;
  /** True while a file is actually being decoded. */
  working: boolean;
  /**
   * Where the database lives, or null when it is in memory only — in which
   * case everything in it is lost on restart, and the panel says so.
   */
  path: string | null;
}

export interface ScanReport {
  found: number;
  added: number;
  unchanged: number;
  unreadable_dirs: number;
  untaggable: number;
}

export const libraryStatus = () => invoke<LibraryStatus>("library_status");

// -- what a record is for ---------------------------------------------------

/**
 * One function a record can be for, with the words to show beside it.
 *
 * The label and the sentence come from Rust: the assistant and the network API
 * need the same words, and a vocabulary explained in two places drifts.
 */
export interface TrackFunction {
  slug: string;
  label: string;
  about: string;
  /** How many tracks carry it. Zero is reported, not omitted. */
  count: number;
}

export const trackFunctions = () => invoke<TrackFunction[]>("track_functions");
export const functionsOf = (track: string) =>
  invoke<string[]>("functions_of", { track });
/**
 * Set what some tracks are for, replacing what was there.
 *
 * The whole answer rather than a change to it — the picker shows every
 * function with the ones in force lit, so what it hands back is the state.
 */
export const setTrackFunctions = (tracks: string[], functions: string[]) =>
  invoke<number>("set_track_functions", { tracks, functions });

/** One thing the command palette can do. */
export interface PaletteEntry {
  /** What the DJ reads: `Deck 1 · play`, `Show Prepare`. */
  label: string;
  /** One line, from the vocabulary's own help. */
  about: string;
  /** `"action"` to send it through the bus, `"surface"` to open a panel. */
  kind: "action" | "surface";
  /** The action text, or the surface name. */
  run: string;
}

/**
 * What the palette should offer for `query`, best first.
 *
 * Assembled and ranked in Rust from `dj_core::vocabulary` and the cockpit's own
 * surfaces, so it cannot offer a command djmanzo does not have and a new verb
 * appears without anyone remembering to add it here. The matching is there too,
 * for the same reason the suggester's is: a ranking that lives in two places is
 * two rankings.
 */
export const palette = (query: string, decks: number) =>
  invoke<PaletteEntry[]>("palette", { query, decks });

/** Where the DJ wants the next record to take the room. */
export type Trajectory = "lift" | "hold" | "ease";

/** One suggested next track, with the reasoning that produced it. */
export interface Suggestion {
  track: LibraryTrack;
  score: number;
  /**
   * Short phrases, strongest first — "same key (8A)", "128 BPM fits", "+3 dB".
   * Rendered as chips beside the row. They come from typed reasons in
   * `dj_library::suggest`; the ranking can be argued with there, not here.
   */
  reasons: string[];
  /**
   * The same reasons on one line, as deltas: `+3 BPM · 8A→9A · +1 dB`.
   *
   * What the rail shows. Composed in Rust beside the typed reasons rather than
   * assembled here, so the assistant and the network API say the same thing.
   */
  summary: string;
  /** 0 to 1 — how much of the achievable score this candidate got. */
  confidence: number;
}

/**
 * What to play after whatever is on `deck`.
 *
 * Deterministic and local: no model, no network. A DJ deciding what to drop at
 * 01:40 needs an answer in the time it takes to look down.
 */
export const suggestNext = (deck: number, trajectory: Trajectory, limit = 12) =>
  invoke<Suggestion[]>("suggest_next", { deck, trajectory, limit });

/**
 * What the context engine makes of the night.
 *
 * `null` while the night has no shape yet — fewer than three analysed records
 * — which is a real answer rather than an error. djmanzo will not guess at a
 * phase: it announced "peak" thirty seconds into a warm-up once, which is a
 * claim nothing had made and nothing could check.
 */
export interface Night {
  /** `warm_up`, `heat`, `peak`, `cooldown` or `chill_out`. */
  phase: string;
  /** 0..=1, the energy of the recent stretch. */
  energy: number;
  /**
   * 0..=1. How much evidence is behind the reading, not how strongly it is
   * held — a DJ reading "peak" off three records should be told it is three.
   */
  confidence: number;
  /** How many measured records it is drawn from. */
  records: number;
  /** Short phrases, as the planner's are. */
  because: string[];
}

/** What the night is doing, and why. */
export const sessionRead = () => invoke<Night | null>("session_read");

/** One record of a pair, as the pair view draws it. */
export interface PairSide {
  /** 1-based deck number. */
  deck: number;
  track: LibraryTrack;
  /**
   * Phrase length in beats, when the analyser found a structure. `null` is a
   * real answer — plenty of records have none — and it is why a mix onto a bar
   * line is a weaker proposition than one onto a phrase.
   */
  phrase_beats: number | null;
  /** Standard notation for the key, beside the Camelot the row carries. */
  key_standard: string | null;
  /** What the record is for, as function slugs. */
  functions: string[];
}

/**
 * A transition, as an object rather than as an answer.
 *
 * §68's transition object: where the mix starts and ends, how long it runs,
 * which way, what the tempo and key do across it, how well the two records go
 * together, and why. Every field is Rust's — nothing here re-derives a mix
 * point, so a panel cannot disagree with djmanzo about where the mix is.
 */
export interface Transition {
  outgoing: PairSide;
  incoming: PairSide;
  /** Beat index in the outgoing track where the mix should begin. */
  start_beat: number;
  /** The same point in seconds, for a display that speaks in time. */
  start_seconds: number;
  /** Where it finishes, in the same terms. */
  end_seconds: number;
  /**
   * The same two points in frames, which is what the waveform is drawn in.
   *
   * From Rust rather than converted here: seconds times a sample rate this
   * side would have to infer from a deck's length is two roundings and a
   * division by zero waiting for an empty deck.
   */
  start_frame: number;
  end_frame: number;
  /**
   * Where the incoming record starts playing, in **its own** frames.
   *
   * The other half of the geometry: `start_frame` says where on the outgoing
   * record the mix begins, this says what arrives there. `null` when the
   * incoming record has no phrase structure to enter on, which is a real
   * answer about plenty of records rather than a failure.
   */
  incoming_frame: number | null;
  /**
   * How many frames of the incoming record fill one frame of the outgoing one,
   * beat for beat — so a preview can draw it beatmatched rather than drifting.
   * 1 when either tempo is not a tempo.
   */
  incoming_frame_scale: number;
  length_beats: number;
  style: string;
  /** Incoming tempo minus outgoing, signed. */
  bpm_delta: number;
  /**
   * `same key`, `neighbour`, `relative major/minor`, `tritone` or `distant`.
   * `null` when either record is unanalysed, which is not a clash.
   */
  key_relation: string | null;
  /** 0 to 1 — the same number the Next rail draws, from the same scorer. */
  confidence: number;
  /** True once a human has moved, shortened or restyled it. */
  edited: boolean;
  /** True when djmanzo is holding this one, rather than merely proposing it. */
  armed: boolean;
  /** Short phrases, as the suggester's are. */
  reasons: string[];
}

/**
 * Plan the mix out of one deck and into another.
 *
 * `null` when there is nothing sensible to propose — either deck empty, no
 * grid, or the outgoing track already past its last usable phrase. A planner
 * that always answers is one that answers wrongly at the end of a record.
 */
/** How the assistant is conducting itself right now. */
export interface Conduct {
  posture: string;
  occasion: string;
  /** Deck numbers with at least one control you have taken. */
  decks_held: number[];
  /** Whether anything at all is held. */
  anything_held: boolean;
  /** What it would do next — shown at every posture, including the ones that
   *  will not act. Seeing what it *would* do is how you decide whether to let
   *  it. */
  next_step: string;
  because: string;
  /**
   * Whether a mistake right now is expensive.
   *
   * What the interface reads to decide how hard the destructive controls
   * should be to hit. Sent by the backend rather than derived here, so the
   * occasion table has one home and cannot disagree with itself.
   */
  mistakes_are_costly: boolean;
  /** How much explanation to offer, 0..=2. */
  verbosity: number;
}

/** A pack: both dials under one name. */
export interface AssistantPack {
  name: string;
  posture: string;
  occasion: string;
  summary: string;
}

export const assistantPacks = () => invoke<AssistantPack[]>("assistant_packs");
export const assistantConduct = () => invoke<Conduct>("assistant_conduct");
export const assistantSetPosture = (posture: string) =>
  invoke<void>("assistant_set_posture", { posture });
export const assistantSetOccasion = (occasion: string) =>
  invoke<void>("assistant_set_occasion", { occasion });
export const assistantApplyPack = (name: string) =>
  invoke<void>("assistant_apply_pack", { name });
/** Take everything out of the assistant's hands, now. */
export const assistantTakeOver = () => invoke<void>("assistant_take_over");
/** Hand everything back, whatever was taken. */
export const assistantHandBack = () => invoke<void>("assistant_hand_back");

/** How much the assistant does, quietest first. */
export const POSTURES = [
  "off",
  "watch",
  "suggest",
  "prepare",
  "assist",
  "autopilot",
] as const;

/** One line each, for the tooltip. What changes, not what it is called. */
export const POSTURE_HELP: Record<string, string> = {
  off: "Nothing at all.",
  watch: "Records the set, says nothing. For practice you will review later.",
  suggest: "Offers, with reasons. Never acts.",
  prepare:
    "Loads and cues the next record, gain-matched, and stops there. You still do the mixing.",
  assist: "Does the small things, asks about the big ones.",
  autopilot: "Mixes on its own. Touch anything to take over.",
};

export const OCCASIONS = [
  "learning",
  "practice",
  "experimenting",
  "warm_up",
  "peak",
  "close",
  "background",
  "requests",
  "open",
] as const;

export const planTransition = (fromDeck: number, toDeck: number) =>
  invoke<Transition | null>("plan_transition", { fromDeck, toDeck });

/**
 * Plan the mix between two decks and hold it.
 *
 * The difference from {@link planTransition} is the whole of §68: an answer
 * djmanzo holds can be adjusted, read by something other than the panel that
 * asked for it, and still be there when that panel is closed and reopened.
 */
export const transitionArm = (fromDeck: number, toDeck: number) =>
  invoke<Transition | null>("transition_arm", { fromDeck, toDeck });

/**
 * What djmanzo is holding, if it still describes what is on the decks.
 *
 * A held transition whose records have been replaced comes back as `null`
 * rather than as a stale answer: a confident mix point for a record that left
 * the deck four minutes ago looks exactly like a current one.
 */
export const transitionCurrent = () =>
  invoke<Transition | null>("transition_current");

/**
 * Move the held transition, shorten it, or change how it is done.
 *
 * The three compose, so one press that both shortens and restyles is one call.
 * The reasons come back re-derived over the new geometry — a mix moved off its
 * phrase boundary says so rather than keeping the sentence the planner wrote.
 */
export const transitionAdjust = (change: {
  moveBeats?: number;
  lengthBeats?: number;
  style?: string;
}) => invoke<Transition | null>("transition_adjust", change);

/**
 * Move the held transition to a place in the record.
 *
 * Frames, because this is what a hand on the waveform produces — §26's
 * "grab the thing you are thinking about". Which beat that is stays djmanzo's
 * arithmetic: it has the grid, the tempo and the record's own sample rate,
 * and this side has a pointer.
 */
export const transitionDrag = (which: "start" | "end", frame: number) =>
  invoke<Transition | null>("transition_drag", { which, frame });

/** Throw the adjustments away and ask the planner again. */
export const transitionReplan = () =>
  invoke<Transition | null>("transition_replan");

/** Stop holding it. */
export const transitionClear = () => invoke<void>("transition_clear");

export const libraryAddFolder = (path: string) =>
  invoke<ScanReport>("library_add_folder", { path });

export const libraryRemoveFolder = (path: string) =>
  invoke<void>("library_remove_folder", { path });

export const libraryRescan = () => invoke<ScanReport>("library_rescan");

/** Search the collection, or list it when the query is empty. */
export const librarySearch = (query: string) =>
  invoke<LibraryTrack[]>("library_search", { query });

// -- playlists and history -------------------------------------------------

export interface Playlist {
  id: number;
  name: string;
  parent_id: number | null;
  /** "list", "folder" or "smart". */
  kind: string;
  track_count: number;
  /** The filter, for a smart folder. Null for the other kinds. */
  query: string | null;
}

/**
 * A track in a playlist. Carries the position because the same track can be in
 * a playlist twice, and removing one has to say which.
 */
export type PlaylistEntry = LibraryTrack & { position: number };

export interface PlayRecord {
  track_id: string;
  title: string;
  artist: string;
  /** Unix seconds. */
  played_at: number;
  session_id: string | null;
}

export const listPlaylists = () => invoke<Playlist[]>("list_playlists");

export const createPlaylist = (
  name: string,
  parent: number | null,
  kind: "list" | "folder" | "smart",
  query?: string,
) => invoke<number>("create_playlist", { name, parent, kind, query: query ?? null });

export const setPlaylistQuery = (id: number, query: string) =>
  invoke<void>("set_playlist_query", { id, query });

/**
 * Parse a filter without storing it, so the editor can say what is wrong while
 * it is being typed rather than when the folder is next opened.
 */
export const checkFilter = (query: string) => invoke<void>("check_filter", { query });

export const smartPlaylistTracks = (id: number) =>
  invoke<LibraryTrack[]>("smart_playlist_tracks", { id });

export const renamePlaylist = (id: number, name: string) =>
  invoke<void>("rename_playlist", { id, name });

export const deletePlaylist = (id: number) => invoke<void>("delete_playlist", { id });

export const movePlaylist = (id: number, parent: number | null) =>
  invoke<void>("move_playlist", { id, parent });

export const playlistTracks = (id: number) =>
  invoke<PlaylistEntry[]>("playlist_tracks", { id });

export const addToPlaylist = (playlist: number, track: string) =>
  invoke<void>("add_to_playlist", { playlist, track });

export const removeFromPlaylist = (playlist: number, position: number) =>
  invoke<void>("remove_from_playlist", { playlist, position });

export const reorderPlaylist = (playlist: number, order: number[]) =>
  invoke<void>("reorder_playlist", { playlist, order });

export const playHistory = () => invoke<PlayRecord[]>("play_history");

// -- editing, duplicates and sessions ---------------------------------------

/** Every field optional; absent means leave it alone, not clear it. */
export interface TrackEdit {
  genre?: string;
  label?: string;
  artist?: string;
  album?: string;
  comment?: string;
  year?: number;
  /** 0..=5. */
  rating?: number;
  /** `#rrggbb`. */
  colour?: string;
}

export const editTracks = (tracks: string[], edit: TrackEdit) =>
  invoke<number>("edit_tracks", { tracks, edit });

/** Empty a field across a selection — the verb that says what it does. */
export const clearTrackField = (tracks: string[], field: string) =>
  invoke<number>("clear_track_field", { tracks, field });

export type Duplicate = LibraryTrack & { paths: string[] };

export const findDuplicates = () => invoke<Duplicate[]>("find_duplicates");

/** Forget the library's memory of one copy. Never deletes the file. */
export const forgetTrackPath = (track: string, path: string) =>
  invoke<void>("forget_track_path", { track, path });

export interface Session {
  id: string;
  tracks: number;
  /** Unix seconds of the last play. */
  ended_at: number;
}

export const listSessions = () => invoke<Session[]>("list_sessions");

// -- SideView ---------------------------------------------------------------

/**
 * The Sidelist: tracks pulled aside for later.
 *
 * A real playlist behind the scenes, so it keeps its order and survives a
 * restart — see the migration that added system playlists.
 */
export const sidelist = () => invoke<PlaylistEntry[]>("sidelist");

export const sidelistAdd = (track: string) => invoke<void>("sidelist_add", { track });

export const sidelistRemove = (position: number) =>
  invoke<void>("sidelist_remove", { position });

export const sidelistClear = () => invoke<void>("sidelist_clear");

// -- layouts ----------------------------------------------------------------

/**
 * What a layout shows and how densely.
 *
 * Data, never behaviour: a layout can hide the FX rack, it cannot change what
 * a control does. Every behaviour is behind the action vocabulary — see
 * ADR-0003 — which is what makes it safe to load one somebody sent you.
 */
export interface Layout {
  name: string;
  description: string;
  /** 2 or 4. The engine always runs four. */
  decks: number;
  waveform_height: number;
  overview: boolean;
  pads: boolean;
  loops: boolean;
  /** Whether the three effect slots are on screen. */
  fx: boolean;
  beat_jump: boolean;
  eq: boolean;
  filter: boolean;
  keylock: boolean;
  browser: boolean;
  /** 0.8..=1.4, multiplying the root font size. */
  density: number;
}

export const listLayouts = () => invoke<Layout[]>("list_layouts");

/** Where a DJ puts their own layout files, so the panel can say. */
export const layoutFolder = () => invoke<string | null>("layout_folder");

/** The layout chosen last time, or null when it no longer names one. */
export const chosenLayout = () => invoke<Layout | null>("chosen_layout");

/** Whether the watershed was showing last time. */
export const watershedShowing = () => invoke<boolean>("watershed");

/** Remember whether the watershed is showing. */
export const setWatershed = (showing: boolean) =>
  invoke<void>("set_watershed", { showing });

/** Remember the chosen layout across restarts. */
export const chooseLayout = (name: string) => invoke<void>("choose_layout", { name });

/**
 * The widget vocabulary (ADR-0008).
 *
 * The interface is not the owner of this list -- Rust is -- so these types
 * describe what arrives rather than what the interface believes. A name the UI
 * has no renderer for is a widget the UI skips, the same way the resolver skips
 * a name it does not know.
 */
export type PropKind =
  | { kind: "flag"; default: boolean }
  | { kind: "count"; default: number; least: number; most: number }
  | { kind: "amount"; default: number; least: number; most: number }
  | { kind: "choice"; default: string; options: string[] };

export interface WidgetProp {
  name: string;
  about: string;
  kind: PropKind;
}

export interface Widget {
  name: string;
  about: string;
  /** Slots this may be placed in. */
  slots: string[];
  /** Slots this offers its own children. */
  offers: string[];
  props: WidgetProp[];
  /** What it reads from the snapshot. */
  needs: string[];
}

/** A widget instance that has been checked against the registry. */
export interface Placed {
  widget: string;
  props: Record<string, unknown>;
  children: Record<string, Placed[]>;
}

/** A layout the interface can render without checking anything itself. */
export interface ResolvedLayout {
  name: string;
  about: string;
  tokens: Record<string, string>;
  slots: Record<string, Placed[]>;
  /** What was dropped and why. Shown, not swallowed. */
  notes: string[];
}

export type TokenShape = "Colour" | "Length" | "Scale";

export interface LayoutVocabulary {
  slots: string[];
  tokens: [string, TokenShape][];
}

export const widgetCatalog = () => invoke<Widget[]>("widget_catalog");
export const layoutVocabulary = () => invoke<LayoutVocabulary>("layout_vocabulary");
/** The chosen layout, upconverted and checked. */
export const layoutTree = () => invoke<ResolvedLayout>("layout_tree");

// -- the cockpit ------------------------------------------------------------

/** Where a surface can be put. */
export type Dock = "left" | "right" | "bottom" | "overlay" | "detached";

export type SurfaceCategory =
  | "performance"
  | "library"
  | "planning"
  | "assistant"
  | "utility";

/**
 * A panel the cockpit can show, as Rust describes it.
 *
 * The same shape as a `Widget`, deliberately -- a name, what it is, where it
 * may go and what it costs -- because the two are checked by the same rules.
 */
export interface Surface {
  name: string;
  title: string;
  about: string;
  category: SurfaceCategory;
  /** Smallest size at which this is still usable, in CSS pixels. */
  least: [number, number];
  prefer: [number, number];
  priority: number;
  performance_critical: boolean;
  detachable: boolean;
  stackable: boolean;
  collapsible: boolean;
  contextual: boolean;
  docks: Dock[];
}

/** A surface, placed. */
export interface SurfacePlacement {
  surface: string;
  dock: Dock;
  order: number;
  size?: number | null;
  collapsed: boolean;
  pinned: boolean;
}

export type Density = "relaxed" | "standard" | "compact" | "pro-dense" | "ultra-dense";
export type Focus = "performing" | "preparing" | "planning" | "learning" | "supervising";

/** A saved arrangement of the cockpit. */
export interface Workspace {
  name: string;
  about: string;
  surfaces: SurfacePlacement[];
  density: Density;
  focus: Focus;
  theme: string;
  decks: number;
  frozen: boolean;
}

export interface ResolvedWorkspace {
  workspace: Workspace;
  /** What was corrected or skipped, and why. Shown, not swallowed. */
  notes: string[];
}

/**
 * The window heights each density band starts at, tallest first.
 *
 * `[least, name, scale]`. Fetched once and applied in the browser rather than
 * asked on every resize: Rust owns the rule, the interface owns the pixels.
 */
export type DensityBand = [number, string, number];
export const densityBands = () => invoke<DensityBand[]>("density_bands");

export const cockpitSurfaces = () => invoke<Surface[]>("cockpit_surfaces");
export const cockpitWorkspaces = () => invoke<Workspace[]>("cockpit_workspaces");
export const cockpitWorkspace = () => invoke<ResolvedWorkspace>("cockpit_workspace");
/**
 * Store an arrangement and take back what was kept.
 *
 * The round trip is the point: a placement Rust corrected comes back
 * corrected, so what is stored and what is drawn cannot drift apart.
 */
export const setCockpitWorkspace = (workspace: Workspace) =>
  invoke<ResolvedWorkspace>("set_cockpit_workspace", { workspace });

export const exportSession = (session: string, path: string) =>
  invoke<number>("export_session", { session, path });

// -- planning a set ---------------------------------------------------------

/** One record in a plan, and why it is there. */
/** How one record in a plan joins the one before it. */
export interface Link {
  /** The deltas across the seam, on one line: `+3 BPM · 8A→9A`. */
  summary: string;
  /** 0 to 1 — the same confidence scale the Next rail draws. */
  confidence: number;
  /**
   * True when the seam needs a decision rather than a blend.
   *
   * A key clash, a tempo outside the deck's range, or a change of rhythmic
   * grammar. Marked rather than avoided: a set with no difficult seams never
   * went anywhere, and what the DJ must not do is meet one for the first time
   * at 01:40.
   */
  risky: boolean;
}

export interface SetlistSlot {
  track: LibraryTrack;
  /** Where in the set it falls, 0 at the start and 1 at the end. */
  through: number;
  /** What the arc wanted here: "lift", "hold" or "ease". */
  trajectory: string;
  reasons: string[];
  /**
   * The seam from the record before this one, absent for the first.
   *
   * A plan is a list of tracks and a set is a list of *transitions*, and the
   * two are not the same list. What a DJ reading a plan wants to know is where
   * it is going to be difficult, which is a property of the join rather than
   * of either record.
   */
  link: Link | null;
}

/** The shape of a night. */
export const ARCS = ["rising", "journey", "flat", "descent"] as const;
export type Arc = (typeof ARCS)[number];

/** What each arc is for, in the words a DJ would use to pick one. */
export const ARC_HELP: Record<Arc, string> = {
  rising: "Up, and stay up. A support slot before the headliner.",
  journey: "Up, peak, and down. A whole night in one set.",
  flat: "Level throughout. A bar, a room where the music is not the point.",
  descent: "Down. The last hour.",
};

/**
 * Build a whole set before playing any of it.
 *
 * The suggester answers "what next"; this answers "what is the whole night".
 * `avoids` is honoured strictly — an avoided genre is not a preference to be
 * balanced against other factors.
 */
export const setlistBuild = (
  arc: Arc,
  minutes: number,
  favours: string[],
  avoids: string[],
) => invoke<SetlistSlot[]>("setlist_build", { arc, minutes, favours, avoids });

/** How the plan changed, and whether anything needed to. */
export interface Steered {
  plan: SetlistSlot[];
  summary: string;
  /** Zero is a real answer: "nothing needed to change" is not "done". */
  changed: number;
}

/**
 * Adjust a plan without throwing it away.
 *
 * Everything already played stays, and so does the next record — it may be
 * cued, staged or have a hand on its fader. `argument` is a genre name for
 * favour and avoid, and a track id for next, later and drop.
 */
export const setlistSteer = (
  plan: SetlistSlot[],
  played: number,
  instruction: string,
  argument?: string,
) =>
  invoke<Steered>("setlist_steer", {
    plan: plan.map((s) => ({
      track: s.track.id,
      through: s.through,
      trajectory: s.trajectory,
    })),
    played,
    instruction,
    argument: argument ?? null,
  });

/** Write a plan out as a playlist that outlives the panel. */
export const setlistSave = (name: string, tracks: string[]) =>
  invoke<number>("setlist_save", { name, tracks });

/** Hand a plan to the assistant, so autopilot plays it. */
export const assistantSetSetlist = (tracks: string[]) =>
  invoke<number>("assistant_set_setlist", { tracks });

// -- more like this ---------------------------------------------------------

/**
 * Records like a given one, tilted by what this DJ actually plays.
 *
 * Differs from {@link suggestNext} in seed and tilt: that answers "what next"
 * from a deck, this answers "more like this" from any track in the browser.
 * Taste is added to the score and bounded well below the gap between a key
 * clash and a match, so it reorders records that all work and never promotes
 * one that does not.
 */
export const similarTo = (track: string, limit = 20) =>
  invoke<Suggestion[]>("similar_to", { track, limit });

/** What the history says this DJ reaches for. */
export interface LearnedTaste {
  /** Families played more often than owning them would predict. */
  favourites: string[];
  plays: number;
  /** Whether there is enough history for the rest to mean anything. */
  confident: boolean;
}

/**
 * What djmanzo has worked out about this DJ's taste.
 *
 * Shown rather than hidden: it steers suggestions, so a DJ should be able to
 * see — and disagree with — what it thinks of them.
 */
export const learnedTaste = () => invoke<LearnedTaste>("learned_taste");

// -- the journal ------------------------------------------------------------

/** One note taken during a set. It belongs to a moment, not to a track. */
export interface JournalNote {
  id: number;
  session_id: string;
  /** Unix seconds, the same clock as a play. */
  at: number;
  body: string;
  /** What was playing when the moment was marked, as it read at the time. */
  playing: string;
  /** Marked, not yet written up. */
  bare: boolean;
}

/**
 * Mark this moment.
 *
 * The body is usually empty: the moment is what cannot be recovered later, and
 * the words are what can. Returns the new note's id.
 */
export const noteAdd = (body = "") => invoke<number>("note_add", { body });

/** Write up a note marked earlier. Only the body — the moment is fixed. */
export const noteWrite = (id: number, body: string) =>
  invoke<void>("note_write", { id, body });

export const noteDelete = (id: number) => invoke<void>("note_delete", { id });

/** One night's notes, oldest first. Omit the session for tonight's. */
export const listNotes = (session?: string) =>
  invoke<JournalNote[]>("notes", { session: session ?? null });

/** Which nights have notes, and how many. */
export const noteCounts = () => invoke<[string, number][]>("note_counts");

/** The session tonight's notes belong to. */
export const currentSession = () => invoke<string>("current_session");

// -- the coach --------------------------------------------------------------

/** A technique the coach recognised in the log. */
export interface Observed {
  technique: string;
  /** What it does, in one line. */
  what: string;
  /** The bridge from the world, for teaching. */
  metaphor: string;
  /** Seconds into the session. */
  at: number;
}

/** One thing worth saying, in three parts that do different jobs. */
export interface CoachNote {
  /** What happened. */
  what: string;
  /** Why it sounded the way it did. */
  why: string;
  /** What to do differently — the line a DJ mid-mix reads. */
  fix: string;
}

export interface CoachReport {
  /** Oldest first; reading it back is watching the mix again. */
  observed: Observed[];
  /** At most one. A learner handed three corrections applies none. */
  note: CoachNote | null;
  next: string | null;
  next_metaphor: string | null;
}

/**
 * What the coach makes of the last couple of minutes.
 *
 * Reads the action log rather than the audio, so what the DJ did is known
 * rather than inferred. It says nothing rather than something vague.
 */
export const coachReport = () => invoke<CoachReport>("coach_report");

// -- sharing a set ----------------------------------------------------------

/** A set written out ready to send, and what did not fit. */
export interface Share {
  /** The message itself, exactly as it will arrive. */
  message: string;
  /** How many records had to be left out of the link. Zero if it all fits. */
  dropped: number;
  /** How many there were altogether. */
  total: number;
}

/**
 * What the message will say, without opening anything.
 *
 * Always call this before {@link shareToWhatsApp}: it is where the DJ finds
 * out that a four-hour set does not fit in a link, while they can still
 * choose the file instead.
 */
export const sharePreview = (session: string, heading: string) =>
  invoke<Share>("share_preview", { session, heading });

/**
 * Open WhatsApp with the set already typed into the message box.
 *
 * Sends nothing and names no recipient. djmanzo prepares the share; the
 * person presses send.
 */
export const shareToWhatsApp = (session: string, heading: string) =>
  invoke<Share>("share_to_whatsapp", { session, heading });

/**
 * Open one of djmanzo's own links in the real browser.
 *
 * `target="_blank"` inside a Tauri window opens nothing at all on Linux,
 * which is why every "Get one" link needs this. The backend checks the
 * address against its own catalogs and refuses anything it did not publish.
 */
export const openSignupLink = (url: string) =>
  invoke<void>("open_signup_link", { url });

export interface ImportResult {
  /** "rekordbox XML", "Traktor NML" or "iTunes XML". */
  format: string;
  tracks: number;
  already_known: number;
  queued: number;
  playlists: number;
  folders: number;
  skipped: string[];
}

/**
 * Import a rekordbox, Traktor or iTunes library export.
 *
 * The format is chosen by what the file contains, not by its extension —
 * rekordbox and iTunes both write `.xml`.
 */
export const importLibrary = (path: string) =>
  invoke<ImportResult>("import_library", { path });

/** Read state once, so a freshly-mounted UI can paint without waiting. */
export const getSnapshot = () => invoke<Snapshot>("get_snapshot");

export const onSnapshot = (handler: (snapshot: Snapshot) => void): Promise<UnlistenFn> =>
  listen<Snapshot>("snapshot", (event) => handler(event.payload));

/**
 * How fast a deck's playhead advances, in frames of wall-clock second.
 *
 * Used to interpolate between snapshots, which arrive at 60 Hz while the
 * display refreshes faster than that.
 *
 * Derived from the track's own numbers rather than an assumed device rate:
 * `position_frames` counts frames of the *file*, so a 44.1 kHz track advances
 * 44100 frames per second of playback however the output device is clocked.
 */
export function playbackFramesPerSecond(deck: DeckState): number {
  if (!deck.playing || deck.length_seconds <= 0) return 0;
  return (deck.length_frames / deck.length_seconds) * deck.rate;
}

/** Format seconds as `m:ss`, the only time format a DJ reads mid-set. */
export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const total = Math.floor(seconds);
  const minutes = Math.floor(total / 60);
  return `${minutes}:${String(total % 60).padStart(2, "0")}`;
}

// ---------------------------------------------------------------------------
// The assistant
// ---------------------------------------------------------------------------

export interface LlmProvider {
  id: string;
  label: string;
  summary: string;
  detail: string;
  recommended: boolean;
  status: "ready" | "needs_key" | "not_running";
  status_detail: string;
  credential: string | null;
  credential_label: string | null;
  signup_url: string | null;
  free_tier: string | null;
  is_set: boolean;
  hint: string;
}

export interface LlmModel {
  id: string;
  name: string;
  free: boolean;
  context: number | null;
  input_price: number | null;
  output_price: number | null;
}

export interface AssistantState {
  provider: string;
  model: string;
  spent_usd: number;
  cap_usd: number;
  /** Calls the provider never priced — unknown spend, not zero spend. */
  unpriced_calls: number;
}

export interface Answer {
  reply: string;
  actions: string[];
  /** Model output that was not valid action text. */
  rejected: string[];
  source: "local" | "model";
  cost_usd: number | null;
  undelivered: string[];
}

export const listLlmProviders = () => invoke<LlmProvider[]>("list_llm_providers");
export const listLlmModels = (provider: string) =>
  invoke<LlmModel[]>("list_llm_models", { provider });
export const assistantState = () => invoke<AssistantState>("assistant_state");
export const setAssistantModel = (provider: string, model: string) =>
  invoke<AssistantState>("set_assistant_model", { provider, model });
export const setSpendCap = (usd: number) => invoke<AssistantState>("set_spend_cap", { usd });
export const resetSpend = () => invoke<AssistantState>("reset_spend");

/** Ask the assistant to do something. It interprets, then dispatches. */
export const ask = (text: string) => invoke<Answer>("ask", { text });

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

export interface PresetItem {
  id: string;
  name: string;
  description: string;
  category: "phase" | "prep" | "move" | "eq" | "mixer";
  per_deck: boolean;
  /** Exactly what it will run. Shown so nothing is hidden. */
  actions: string[];
}

export interface PresetPack {
  id: string;
  name: string;
  description: string;
  user: boolean;
  presets: PresetItem[];
}

export const listPresets = () => invoke<PresetPack[]>("list_presets");
/** Apply a preset. Returns the actions that were dispatched. */
export const applyPreset = (id: string, deck?: number) =>
  invoke<string[]>("apply_preset", { id, deck });
export const presetFolder = () => invoke<string>("preset_folder");

/**
 * Keep the rack as it stands as a preset.
 *
 * `deck` names which rack: a number for a deck's, null for the master's. A
 * deck chain is saved with the `{deck}` placeholder, so it can be recalled onto
 * any deck rather than only the one it came from.
 *
 * Returns the id it saved under. Saving twice under one name replaces rather
 * than duplicating — the second save is a correction.
 */
export async function saveRackPreset(name: string, deck: number | null): Promise<string> {
  return invoke<string>("save_rack_preset", { name, deck });
}

// ---------------------------------------------------------------- controllers

export interface ControlStatus {
  /** Every MIDI input the machine can see. */
  inputs: string[];
  open_port: string | null;
  open_mapping: string | null;
  /**
   * Why there are no inputs, when the reason is that MIDI itself is
   * unavailable rather than that nothing is plugged in. Two different
   * problems, and only one of them is fixed by plugging something in.
   */
  unavailable: string | null;
  keyboard: boolean;
  keyboard_name: string;
  /**
   * Where the connected controller says its own sockets go, when it says.
   * Absent for the great majority of controllers, whose arrangement is the
   * one djmanzo already assumes.
   */
  audio: AudioRouting | null;
  /**
   * HID devices, listed apart from MIDI ports because they are opened
   * differently: a HID mapping names byte offsets into a report, so it must be
   * chosen deliberately rather than matched by "whichever fits".
   */
  hid_inputs: HidDevice[];
  hid_unavailable: string | null;
  open_hid: string | null;
  open_hid_mapping: string | null;
}

export interface HidDevice {
  /** `2b73:0017`, the way lsusb and every manual write it. */
  id: string;
  name: string;
  /** What identifies this exact device when two identical ones are plugged in. */
  path: string;
}

/**
 * A controller's own output arrangement.
 *
 * Channels are numbered as they are printed on the back of the device —
 * from one — because that is the number a DJ is looking at while they plug
 * a cable in.
 */
export interface AudioRouting {
  master: [number, number];
  cue: [number, number] | null;
  booth: [number, number] | null;
  channels_needed: number;
  /**
   * Why the arrangement is not in force, when it is not: the open device has
   * fewer outputs than the mapping names, so djmanzo is using its usual
   * layout instead.
   */
  not_applied: string | null;
}

export interface MappingInfo {
  name: string;
  device: string;
  bindings: number;
  bundled: boolean;
}

export interface KeyBinding {
  /** The canonical chord: `shift+space`, `keyq`. */
  chord: string;
  label: string;
  group: string;
  /** Whether it undoes itself on release. */
  held: boolean;
  press: string | null;
  release: string | null;
}

export const controlStatus = () => invoke<ControlStatus>("control_status");
export const controlMappings = () => invoke<MappingInfo[]>("control_mappings");
export const keyboardKeys = () => invoke<KeyBinding[]>("keyboard_keys");
export const setKeyboardEnabled = (on: boolean) =>
  invoke<void>("set_keyboard_enabled", { on });
export const openController = (port: string, mapping?: string) =>
  invoke<void>("open_controller", { port, mapping: mapping ?? null });
export const closeController = () => invoke<void>("close_controller");
export const openHidController = (device: string, mapping: string) =>
  invoke<void>("open_hid_controller", { device, mapping });
export const closeHidController = () => invoke<void>("close_hid_controller");

// ------------------------------------------------------------ remote control

/**
 * The network control port: what M7's action boundary looks like from outside
 * the application. Off unless switched on, loopback unless told otherwise, and
 * a token is required the moment it faces the network.
 */
export interface RemoteStatus {
  running: boolean;
  /** Where it is listening, including the port the OS chose for port 0. */
  address: string | null;
  /** Whether a token is required. The token itself never comes back. */
  token_set: boolean;
  error: string | null;
  /**
   * The OSC port, when one is open. Loopback only — UDP has no handshake, so
   * there is nothing to authenticate with.
   */
  osc: string | null;
}

/** MIDI clock out: djmanzo as clock master for a drum machine or a light desk. */
export interface ClockStatus {
  running: boolean;
  port: string | null;
  error: string | null;
  /** The port djmanzo is following, when something else is the master. */
  following: string | null;
  /** That clock's tempo, once there are two pulses to compare. */
  external_bpm: number | null;
}

export interface MidiOutputs {
  ports: string[];
  /** Why there are none, when MIDI itself is the problem. */
  unavailable: string | null;
}

export const midiOutputs = () => invoke<MidiOutputs>("midi_outputs");
export const clockStatus = () => invoke<ClockStatus>("clock_status");
export const startClock = (port: string) =>
  invoke<ClockStatus>("start_clock", { port });
export const stopClock = () => invoke<ClockStatus>("stop_clock");
export const followClock = (port: string) =>
  invoke<ClockStatus>("follow_clock", { port });
export const unfollowClock = () => invoke<ClockStatus>("unfollow_clock");

export const remoteStatus = () => invoke<RemoteStatus>("remote_status");
export const startRemote = (address: string, token?: string) =>
  invoke<RemoteStatus>("start_remote", { address, token: token || null });
export const stopRemote = () => invoke<RemoteStatus>("stop_remote");
export const startOsc = (address: string) =>
  invoke<RemoteStatus>("start_osc", { address });
export const stopOsc = () => invoke<RemoteStatus>("stop_osc");

/* -- The room's own page --------------------------------------------------- */

/** One way a phone gets to the request page, and what it is good for. */
export interface WayIn {
  /** `"name"` — the same every venue. `"lan"` — certain, and only here. */
  kind: string;
  url: string;
  /** The honest sentence about which phones will manage it. */
  caveat: string;
  /** The QR code, as an inline SVG. */
  qr: string | null;
}

export interface AudienceStatus {
  running: boolean;
  /** Whether requests are being taken. A closed door still shows the list. */
  open: boolean;
  port: number;
  heading: string;
  language: string;
  show_playing: boolean;
  /** Most portable first, so the order is the recommendation. */
  ways_in: WayIn[];
  announcing: boolean;
  /** Why the local name is not being answered for. Usually blocked multicast. */
  announce_error: string | null;
  error: string | null;
  waiting: number;
}

/** One song the room asked for, however many people asked for it. */
export interface Ask {
  id: number;
  text: string;
  voices: number;
  first_asked: number;
  last_asked: number;
  standing: string;
}

export const audienceStatus = () => invoke<AudienceStatus>("audience_status");
export const audienceStart = (port?: number) =>
  invoke<AudienceStatus>("audience_start", { port: port ?? null });
export const audienceStop = () => invoke<AudienceStatus>("audience_stop");
export const audienceOpen = (open: boolean) =>
  invoke<AudienceStatus>("audience_open", { open });
export const audienceSettings = (settings: {
  heading?: string;
  language?: string;
  showPlaying?: boolean;
}) =>
  invoke<AudienceStatus>("audience_settings", {
    heading: settings.heading ?? null,
    language: settings.language ?? null,
    showPlaying: settings.showPlaying ?? null,
  });
export const audienceLanguages = () =>
  invoke<[string, string][]>("audience_languages");
export const audienceWaiting = () => invoke<Ask[]>("audience_waiting");
export const audienceAll = () => invoke<Ask[]>("audience_all");
export const audienceSettle = (id: number, standing: string) =>
  invoke<boolean>("audience_settle", { id, standing });
/**
 * Write a sheet of stickers to `path` and hand it to the operating system.
 *
 * Resolves to whether it opened. Written rather than shown in a new window:
 * `window.open` inside the webview returns something and opens nothing.
 */
export const audienceSheet = (kind: string, path: string, copies?: number) =>
  invoke<boolean>("audience_sheet", { kind, path, copies: copies ?? null });

/* -- what the room is doing ------------------------------------------------ */

/** What the sensors have made of the room. See `dj_assistant::room`. */
export interface RoomRead {
  /** Whether a reading arrived recently enough to call this live. */
  watching: boolean;
  recent: number;
  /** Whether there is enough to say anything at all. */
  enough: boolean;
  /** Worth saying, most important first. Empty means the room is carrying on. */
  notes: string[];
  /** Where the room disagrees with the night you set up. */
  disagreement: string | null;
  /** From the clock, not from a sensor. */
  hour: number | null;
  light: number | null;
  movement: number | null;
  loudness: number | null;
}

export const roomSaw = (reading: {
  light?: number;
  movement?: number;
  loudness?: number;
}) =>
  invoke<void>("room_saw", {
    light: reading.light ?? null,
    movement: reading.movement ?? null,
    loudness: reading.loudness ?? null,
  });
export const roomRead = () => invoke<RoomRead>("room_read");
export const roomForget = () => invoke<void>("room_forget");

/* -- finding a record from what you remember ------------------------------- */

/** One record whose words contain the phrase. */
export interface WordHit {
  track: LibraryTrack;
  /** The line it was found in, as the record has it. */
  line: string;
  line_number: number;
}

/** How much of the collection has been asked about. */
export interface WordsProgress {
  with_words: number;
  /** Asked at all, including the records with nothing to find. */
  asked: number;
  tracks: number;
}

/** What one fetch batch did. */
export interface Sweep {
  asked: number;
  found: number;
  left: number;
  /** The network refused; stop rather than grinding through the collection. */
  gave_up: boolean;
}

/** One record the assistant thinks a description might be. */
export interface Guess {
  artist: string;
  title: string;
  why: string | null;
  /** The matching record in your collection, when there is one. */
  owned: LibraryTrack | null;
}

/**
 * What djmanzo made of a hum.
 *
 * It narrows; it does not identify. See `dj_app::memory` for why.
 */
/** A record whose melody matched a hum, and where in it. */
export interface MelodyHit {
  track: LibraryTrack;
  /** Mean semitone error per point of the hum. Lower is better. */
  cost: number;
  /** Seconds into the record where the matching passage starts. */
  at_seconds: number;
}

export interface Hummed {
  /** Camelot, when there was enough pitch to tell. */
  key: string | null;
  tempo: number | null;
  seconds: number;
  near: LibraryTrack[];
  /** Records whose melody matches, best first. */
  melody: MelodyHit[];
  /** How much of the hum had a pitch in it, zero to one. */
  voiced: number;
}

export const wordsSearch = (phrase: string) =>
  invoke<WordHit[]>("words_search", { phrase });
export const wordsProgress = () => invoke<WordsProgress>("words_progress");
export const wordsFetch = () => invoke<Sweep>("words_fetch");
/** How many records can be searched by tune, and how many there are. */
export interface MelodyProgress {
  with_melody: number;
  tracks: number;
}

/** Make pitch contours for records that have none, a batch at a time. */
export const melodySweep = () => invoke<Sweep>("melody_sweep");
export const melodyProgress = () => invoke<MelodyProgress>("melody_progress");
export const guessFromDescription = (description: string) =>
  invoke<Guess[]>("guess_from_description", { description });
export const hum = (samples: number[], rate: number) =>
  invoke<Hummed>("hum", { samples, rate });
