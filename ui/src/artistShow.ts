/**
 * §122 step 5: the artist's photographs, one at a time — each held a while,
 * then faded out and the next faded in, and a new artist's set faded in once
 * the last one has faded out. See `docs/VISUALS.md`.
 *
 * Kept apart from the drawing so it can be held by tests: the frame loop in
 * `./Visuals.svelte` calls `step` and draws `current` at `fade`, and the
 * credit beside the picture is the current one's, as visible as it is.
 * Changed in place, because it runs thirty times a second.
 */

/** Seconds each photograph is held. */
export const HOLD = 12;
/** Seconds a photograph takes to fade out, or in. */
export const FADE = 1.5;

export interface Show<P> {
  pictures: P[];
  index: number;
  /** How much of the current photograph shows, 0..1. */
  fade: number;
  /** Seconds the current photograph has been fully shown. */
  held: number;
  /** A new artist's set, shown once the current photograph has faded out. */
  next: P[] | null;
}

export function start<P>(): Show<P> {
  return { pictures: [], index: 0, fade: 0, held: 0, next: null };
}

/** A new artist's photographs, or none: shown after the current fades out. */
export function offer<P>(show: Show<P>, pictures: P[]): void {
  show.next = pictures;
}

/** The photograph showing now, if any. */
export function current<P>(show: Show<P>): P | null {
  return show.pictures[show.index] ?? null;
}

/** `seconds` later. */
export function step<P>(show: Show<P>, seconds: number): void {
  const going = show.next !== null || (show.pictures.length > 1 && show.held >= HOLD);
  if (going) {
    show.fade = Math.max(0, show.fade - seconds / FADE);
    if (show.fade > 0 && show.pictures.length > 0) return;
    if (show.next !== null) {
      show.pictures = show.next;
      show.index = 0;
      show.next = null;
    } else {
      show.index = (show.index + 1) % show.pictures.length;
    }
    show.fade = 0;
    show.held = 0;
    return;
  }
  if (show.pictures.length === 0) return;
  if (show.fade < 1) {
    show.fade = Math.min(1, show.fade + seconds / FADE);
  } else {
    show.held += seconds;
  }
}
