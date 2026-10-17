/**
 * Which djmanzo this is, in two senses that used to be one.
 *
 * - **The build**: the whole application, or the karaoke build for older Macs
 *   (`npm run build:karaoke`, which sets `VITE_EDITION`). Since §131 that is a
 *   platform target, starting at Eco for an older machine.
 * - **The edition the licence names** (§130/§131: `edition.svelte.ts`, from
 *   Rust's `crate::licence`): Hobbyist, Karaoke, Pro DJ… What it carries is
 *   Rust's to decide; this file only says where each opens and what it is
 *   called.
 *
 * Either is the same interface. What differs is where it opens — the karaoke
 * build and the Karaoke edition on the Karaoke activity, the Auto-Pilot
 * edition on Autopilot, the first time — and after that wherever its DJ left
 * it.
 */
export type Edition = "full" | "karaoke";

export const EDITION: Edition = import.meta.env.VITE_EDITION === "karaoke" ? "karaoke" : "full";

/** What this build is called where it introduces itself. */
export const PRODUCT = productName(EDITION);

export function productName(edition: Edition): string {
  return edition === "karaoke" ? "djmanzo Karaoke" : "djmanzo";
}

/**
 * The activity a run opens on when its DJ has never chosen one: Karaoke for
 * the karaoke build or the Karaoke edition, Autopilot for the Auto-Pilot
 * edition (`sold` is the licence's edition slug, `null` until Rust answers),
 * and none — the full cockpit — for everything else.
 */
export function firstActivity(
  edition: Edition,
  current: string | null | undefined,
  sold: string | null = null,
): string | null {
  if (current) return null;
  if (edition === "karaoke" || sold === "karaoke") return "karaoke";
  return sold === "autopilot" ? "autopilot" : null;
}
