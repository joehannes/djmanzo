/**
 * §114: the platter, shaped like what it does.
 *
 * > i'd also like to see the forms and shapes of the controls and widgets to
 * > be individual and useful and resembling nature and functionality
 *
 * **It turns with the record.** A turntable's platter goes round at 33⅓
 * revolutions a minute while the record plays, stops when it stops, runs
 * backwards when it does and faster when the pitch is up. Driven by the
 * playhead rather than a clock, so all of that follows without being asked
 * for: a paused deck's playhead does not move, a reversed one moves back, and
 * a scratch moves the record exactly as far as the hand did.
 *
 * **Vinyl and CDJ are different things, and look it.** In vinyl mode a hand
 * on the record stops it, so the record is drawn — grooves, and a mark that
 * goes round with them. In CDJ mode the platter never turns under power and a
 * hand only bends the tempo, so the platter stands still, ringed with the
 * ridges a CDJ's jog has at its edge, and only the position indicator in its
 * middle goes round, as a CDJ's jog display does.
 *
 * **Nature where the theme is a natural one.** The organic packages cut the
 * record from a log: growth rings rather than grooves, some years wide and
 * most narrow, each ring a little out of round, grown from the deck's own
 * name so deck 1 is the same log every night.
 *
 * Drawn in a 100 × 100 box about (50, 50).
 */

import { seedOf } from "./faces";

/** A turntable's speed for a twelve-inch record, in revolutions a minute. */
export const RPM = 100 / 3;

/** The edge of the record's label, which carries no grooves. */
export const LABEL = 17;
/** The edge of the playing surface, inside the platter's own rim. */
export const RUN_OUT = 46;

/**
 * How far round the record has gone, in degrees clockwise from the top,
 * `seconds` into it at 33⅓ — within one turn, so a set's worth of turning
 * stays a small number.
 */
export function turned(seconds: number): number {
  if (!Number.isFinite(seconds)) return 0;
  const degrees = ((seconds * RPM) / 60) * 360;
  return ((degrees % 360) + 360) % 360;
}

/** A record's grooves: evenly pitched, from the label out to the run-out. */
export function grooves(pitch = 2): number[] {
  const out: number[] = [];
  for (let radius = LABEL + pitch; radius <= RUN_OUT; radius += pitch) out.push(radius);
  return out;
}

/** Mulberry32, as `faces.ts` grows its stones with. */
function sequence(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
}

/**
 * A log's growth rings, from the label out, grown from `name`.
 *
 * A tree grows a narrow ring in a hard year and a wide one in a good one, and
 * most years are hard: the width is squared from an even draw, so narrow rings
 * outnumber wide ones as they do in a real cross-section.
 */
export function growthRings(name: string): number[] {
  const next = sequence(seedOf(name || "platter"));
  const out: number[] = [];
  let radius = LABEL;
  for (;;) {
    radius += 1.1 + 2.6 * next() ** 2;
    if (radius > RUN_OUT) return out;
    out.push(radius);
  }
}

/** How far a growth ring may stray from round, either way. */
export const RING_WANDER = 0.6;

/**
 * One growth ring as a closed path: never quite round, and each out of round
 * its own way, but never so far that it crosses the next.
 */
export function ringPath(name: string, radius: number, count = 48): string {
  const next = sequence(seedOf(`${name}:${radius.toFixed(2)}`));
  const waves = [2, 3].map((order, index) => ({
    order,
    amplitude: (RING_WANDER / (index + 1.5)) * next(),
    phase: next() * Math.PI * 2,
  }));
  const points: string[] = [];
  for (let i = 0; i < count; i++) {
    const angle = (i / count) * Math.PI * 2;
    const r =
      radius + waves.reduce((sum, w) => sum + w.amplitude * Math.cos(w.order * angle + w.phase), 0);
    const x = 50 + r * Math.cos(angle);
    const y = 50 + r * Math.sin(angle);
    points.push(`${i === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`);
  }
  return points.join(" ") + " Z";
}

/** The ridges round a CDJ's jog, as angles in degrees. */
export function ridges(count = 36): number[] {
  return Array.from({ length: count }, (_, i) => (i * 360) / count);
}
