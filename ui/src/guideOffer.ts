/**
 * Karaoke beta B7: the guide vocal, proposed for a song new to the singer up
 * next — decided in Rust (`dj_app::guide_offer`). Session B's own call, kept
 * out of `api.ts` (docs/karaoke_beta_release.md §4).
 */
import { invoke } from "@tauri-apps/api/core";

export interface GuideOffer {
  /** The host's line, in the up-next card. */
  line: string;
}

/** The deck's voice level a guide sets: a quarter, about −12 dB. */
export const GUIDE_LEVEL = 0.25;

/** Whether a deck's voice level is the guide's. */
export const isGuide = (voice: number | null | undefined) => Math.abs((voice ?? 1) - GUIDE_LEVEL) < 0.05;

/** The offer for the singer up next, or none; anything but an offer is none. */
export async function karaokeGuideOffer(): Promise<GuideOffer | null> {
  const offer = await invoke<unknown>("karaoke_guide_offer");
  const line = (offer as { line?: unknown } | null)?.line;
  return typeof line === "string" && line !== "" ? { line } : null;
}
