/**
 * §128: the guided questions for a song a singer cannot name — the call into
 * `dj_app::find_song`. Its own file, as the karaoke beta's sessions keep
 * theirs (docs/karaoke_beta_release.md §4).
 */
import { invoke } from "@tauri-apps/api/core";
import type { LibraryTrack } from "./api";

export type Pace = "slow" | "medium" | "fast";
export type Voice = "man" | "woman" | "duet" | "group";

/** What a singer remembers. See `dj_app::find_song::Remembered`. */
export interface Remembered {
  decade?: number | null;
  pace?: Pace | null;
  voice?: Voice | null;
  language?: string | null;
  genre?: string | null;
  like?: string | null;
  heard?: string | null;
  words?: string | null;
}

export interface Found {
  near: { track: LibraryTrack; because: string[] }[];
  /** An answer the collection can check was given. */
  checked: boolean;
  /** Every answer as one sentence, for the assistant. */
  description: string;
}

export const findSong = (remembered: Remembered) => invoke<Found>("find_song", { remembered });
