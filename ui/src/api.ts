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

export interface DeckState {
  number: number;
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
  /** Deliberate transposition in semitones, for harmonic mixing. */
  key_shift: number;
}

export interface MasterState {
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
}

export interface Snapshot {
  decks: DeckState[];
  master: MasterState;
}

export const listDevices = () => invoke<Device[]>("list_devices");

export const openDevice = (deviceId: string | null, bufferFrames: number) =>
  invoke<ActiveDevice>("open_device", {
    deviceId,
    bufferFrames,
  });

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
 */
export function tileUrl(
  deck: number,
  width: number,
  height: number,
  startFrame: number,
  framesPerPixel: number,
): string {
  const zoomMilli = Math.round(framesPerPixel * 1000);
  const start = Math.round(startFrame);
  const path = `tile/${deck}/${width}/${height}/${start}/${zoomMilli}`;
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

/** Read state once, so a freshly-mounted UI can paint without waiting. */
export const getSnapshot = () => invoke<Snapshot>("get_snapshot");

export const onSnapshot = (handler: (snapshot: Snapshot) => void): Promise<UnlistenFn> =>
  listen<Snapshot>("snapshot", (event) => handler(event.payload));

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
