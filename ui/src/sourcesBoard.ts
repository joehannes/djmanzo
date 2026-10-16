/**
 * §128: every source djmanzo can use, by what it is for — the call into
 * `dj_app::sources_board`. Its own file, as the karaoke beta's sessions keep
 * theirs (docs/karaoke_beta_release.md §4).
 */
import { invoke } from "@tauri-apps/api/core";

export type SourceJob = "search" | "play" | "buy" | "lyrics" | "similar" | "imagery";
export type SourceReady = "ready" | "needs_setup" | "partner_gated" | "off";

/** One source doing one job. See `dj_app::sources_board::Row`. */
export interface SourceRow {
  job: SourceJob;
  name: string;
  what: string;
  ready: SourceReady;
  detail: string;
  /** Where it is set up: "sources", "assistant", or null when there is nothing to set. */
  setup: "sources" | "assistant" | null;
}

export interface SourcesBoard {
  jobs: { job: SourceJob; title: string; rows: SourceRow[] }[];
}

export const sourcesBoard = () => invoke<SourcesBoard>("sources_board");
