/**
 * Karaoke beta B2: the room's emoji, from Rust (`dj_app::reactions`).
 * Session B's own calls, kept out of `api.ts` (docs/karaoke_beta_release.md §4).
 */
import { invoke } from "@tauri-apps/api/core";

export interface RoomReaction {
  /** Increasing; the screen asks for what is newer than it drew. */
  id: number;
  emoji: string;
}

export interface RoomReactions {
  /** Whether the host has them on. */
  on: boolean;
  reactions: RoomReaction[];
}

export const roomReactions = (since: number) => invoke<RoomReactions>("room_reactions", { since });
export const roomReactionsSet = (on: boolean) => invoke<RoomReactions>("room_reactions_set", { on });
