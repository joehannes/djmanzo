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
  loudness_lufs: number | null;
  /** True once the track has everything sync and harmonic mixing need. */
  analysed: boolean;
  play_count: number;
  /** 0..=5, when the DJ has rated it. */
  rating: number | null;
  /** `#rrggbb`, when the DJ has coloured it. */
  colour: string | null;
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

export const exportSession = (session: string, path: string) =>
  invoke<number>("export_session", { session, path });

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
