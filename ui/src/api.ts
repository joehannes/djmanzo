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
}

export interface MasterState {
  crossfader: number;
  gain_db: number;
  peak_left: number;
  peak_right: number;
  sample_rate: number;
  xruns: number;
  cpu_load: number;
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
