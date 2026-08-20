/**
 * Level meters and progress bars, moved without a layout.
 *
 * # Why not `width`
 *
 * A meter fill written as `width: 62.7194%` changes the element's box, so the
 * browser lays out, paints and composites — sixty times a second, per meter,
 * and there are a dozen of them across four decks. `transform: scaleX()` moves
 * the same pixels without touching the box, which is the one property class the
 * compositor handles on its own.
 *
 * # Why the rounding
 *
 * Svelte skips a DOM write when the string it would set is identical to the
 * last one. `Math.min(peak, 1) * 100` almost never repeats, so almost every
 * frame writes. Rounded to `STEPS` divisions the string repeats constantly on
 * anything steady, and the write disappears entirely.
 *
 * 200 steps is half a percent of the bar's length — well under a pixel on any
 * meter this interface draws, so nothing visible is given up for it.
 *
 * There is deliberately no smoothing here. A level meter that lags is a lying
 * level meter; the quantisation only discards movement too small to see.
 */
const STEPS = 200;

/** A 0..=1 level as a scale factor, rounded so steady levels stop writing. */
export function fill(level: number): number {
  if (!Number.isFinite(level)) return 0;
  const clamped = Math.min(1, Math.max(0, level));
  return Math.round(clamped * STEPS) / STEPS;
}
