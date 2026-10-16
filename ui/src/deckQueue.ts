/**
 * §127: a playlist for each deck — the calls into `dj_app::deck_queue`.
 *
 * A file of its own rather than more of `api.ts`, so the karaoke beta's two
 * sessions never edit the same lines (docs/karaoke_beta_release.md §4).
 */
import { invoke } from "@tauri-apps/api/core";
import type { PlaylistEntry } from "./api";

/** A deck's queue, as Rust answers it. See `dj_app::deck_queue::DeckQueueDto`. */
export interface DeckQueue {
  deck: number;
  playlist: number | null;
  /** The playlist's name; null when none is chosen or it is gone. */
  name: string | null;
  entries: PlaylistEntry[];
  /** The record the queue last put on the deck, by its place in `entries`. */
  at: number | null;
  /** When the deck runs out, the next record is loaded and played. */
  advance: boolean;
  /** After the last, the first again. */
  repeat: boolean;
  /** In a shuffled order, every record once a round. */
  shuffle: boolean;
  /** The record that comes next, by its place in `entries`. */
  next: number | null;
  /** Why the list could not be read. */
  problem: string | null;
}

export const deckQueue = (deck: number) => invoke<DeckQueue>("deck_queue", { deck });

export const deckQueueSet = (
  deck: number,
  playlist: number | null,
  advance: boolean,
  repeat: boolean,
  shuffle: boolean,
) => invoke<DeckQueue>("deck_queue_set", { deck, playlist, advance, repeat, shuffle });

export const deckQueueLoad = (deck: number, index: number, play: boolean) =>
  invoke<DeckQueue>("deck_queue_load", { deck, index, play });
