/**
 * Which djmanzo this is: the whole application, or the karaoke build for
 * older Macs (`npm run build:karaoke`, which sets `VITE_EDITION`).
 *
 * The karaoke build is the same interface. What differs is where it opens:
 * on the Karaoke activity the first time, and after that wherever its DJ
 * left it.
 */
export type Edition = "full" | "karaoke";

export const EDITION: Edition = import.meta.env.VITE_EDITION === "karaoke" ? "karaoke" : "full";

/** What this build is called where it introduces itself. */
export const PRODUCT = productName(EDITION);

export function productName(edition: Edition): string {
  return edition === "karaoke" ? "djmanzo Karaoke" : "djmanzo";
}

/**
 * The activity a build opens on when its DJ has never chosen one: the
 * karaoke build's is Karaoke, the whole application's is none (the full
 * cockpit).
 */
export function firstActivity(edition: Edition, current: string | null | undefined): string | null {
  if (current) return null;
  return edition === "karaoke" ? "karaoke" : null;
}
