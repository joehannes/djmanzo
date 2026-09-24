/**
 * §114: a control shaped like what it does.
 *
 * > i'd also like to see the forms and shapes of the controls and widgets to
 * > be individual and useful and resembling nature and functionality
 *
 * Three ideas, each one a question a DJ used to answer by reading a number:
 *
 * - **A tone knob draws the curve it puts on the record.** The HI knob's face
 *   is a high shelf, the MID's a bell, the LOW's a low shelf, and the filter's
 *   the slope it is cutting — each at its current setting, on the same
 *   20 Hz – 20 kHz axis the ear has. A killed low is a floor on the left of
 *   the LOW knob; a filter half-way down is a line that falls away on the
 *   right. The knob is read as *what is happening to the sound*, not as an
 *   angle to be converted into it.
 * - **A control that sits at a middle fills from the middle.** The filter is
 *   off at its centre and the pitch fader at zero, and an EQ is untouched at
 *   unity, a quarter of the way round. Filled from the bottom, each read as
 *   half on when it was off. Filled from where they rest, a glance says which
 *   way and how far.
 * - **Natural forms where the theme is a natural one.** The organic themes
 *   draw a knob as a river stone and a pad as a pebble, each stone its own
 *   because it is grown from the control's name — so the LOW knob is the same
 *   stone every night, which is what a hand learns — while the value is still
 *   drawn on a true circle over it, because the setting is never a thing a
 *   theme decides to omit or bend.
 *
 * The curves are the DSP's own arithmetic, not an illustration of it:
 * `dj_dsp::eq::ThreeBandEq` is three bands split by Linkwitz–Riley
 * crossovers, whose outputs are in phase, so a band's share of the sum is a
 * real weight and a gain on it is exactly that weight scaled; the sweep filter
 * is a second-order Butterworth, twelve decibels an octave past its corner.
 * A Rust test reads the numbers below out of this file, so the two cannot
 * drift apart.
 */

/** What a knob's face draws. */
export type Face = "eq-low" | "eq-mid" | "eq-high" | "filter";

/** What a pad does, which is what its colour says. */
export type PadRole = "play" | "cue" | "sync" | "eject";

/** `dj_dsp::eq::LOW_MID_HZ` and `MID_HIGH_HZ`: the isolator's crossovers. */
export const ISOLATOR_LOW_MID_HZ = 300;
export const ISOLATOR_MID_HIGH_HZ = 4_000;

/**
 * `dj_dsp::SweepFilter`: the dead zone around the centre, and where each side
 * ends — fully low-passed at 40 Hz, fully high-passed at 8 kHz, swept
 * exponentially from hearing's far end.
 */
export const SWEEP_DEAD_ZONE = 0.02;
export const SWEEP_LOW_PASS_FLOOR_HZ = 40;
export const SWEEP_HIGH_PASS_CEILING_HZ = 8_000;

/** Hearing's ends: the face's axis, as `dj_render::HEARING_HZ` is the lane's. */
export const LOWEST_HZ = 20;
export const HIGHEST_HZ = 20_000;

/**
 * What the face can show: a kill reads as the floor, and the EQ's +12 dB
 * (a gain of 4) as the ceiling.
 */
export const FLOOR_DB = -24;
export const CEILING_DB = 12;

/** Where the curve is drawn on the knob, in its 0–100 box. */
export const FACE = {
  left: 30,
  right: 70,
  /** The 0 dB line. */
  unity: 50,
  /** Box units per decibel: +12 dB is 6 up, a kill 12 down. */
  perDb: 0.5,
} as const;

/** How many points the curve is drawn through. */
const POINTS = 33;

/** A frequency along the face: `0` hearing's bottom, `1` its top. */
export function hzAt(along: number): number {
  return LOWEST_HZ * (HIGHEST_HZ / LOWEST_HZ) ** Math.min(1, Math.max(0, along));
}

/** An LR4 low-pass's magnitude: `1 / (1 + (f/fc)⁴)`. Its high-pass is `1 −` this. */
function lr4Low(hz: number, corner: number): number {
  return 1 / (1 + (hz / corner) ** 4);
}

/**
 * The three bands' shares of the signal at `hz`, summing to one.
 *
 * The isolator splits at 300 Hz, then splits what is above at 4 kHz. For one
 * LR4 crossover the two sides are in phase, so their magnitudes add; for two
 * crossovers three octaves apart the error is a fraction of a decibel, far
 * below what a face this size can show.
 */
export function bandShares(hz: number): { low: number; mid: number; high: number } {
  const low = lr4Low(hz, ISOLATOR_LOW_MID_HZ);
  const above = 1 - low;
  const mid = above * lr4Low(hz, ISOLATOR_MID_HIGH_HZ);
  return { low, mid, high: above - mid };
}

function toDb(magnitude: number): number {
  if (!(magnitude > 0)) return FLOOR_DB;
  return Math.min(CEILING_DB, Math.max(FLOOR_DB, 20 * Math.log10(magnitude)));
}

/**
 * One band's knob at `gain` (linear, 1 unity, 0 a kill), the other two left
 * at unity: what this knob alone is doing to the record at `hz`, in dB.
 */
export function eqResponseDb(band: "low" | "mid" | "high", gain: number, hz: number): number {
  const g = Number.isFinite(gain) ? Math.max(0, gain) : 1;
  const shares = bandShares(hz);
  return toDb(1 + (g - 1) * shares[band]);
}

/**
 * Where the sweep filter's corner is for a knob `position` (-1 fully
 * low-passed, +1 fully high-passed), or `null` inside the dead zone.
 */
export function filterCorner(position: number): { side: "low-pass" | "high-pass"; hz: number } | null {
  if (!Number.isFinite(position) || Math.abs(position) <= SWEEP_DEAD_ZONE) return null;
  const amount = Math.min(1, (Math.abs(position) - SWEEP_DEAD_ZONE) / (1 - SWEEP_DEAD_ZONE));
  if (position < 0) {
    return {
      side: "low-pass",
      hz: HIGHEST_HZ * (SWEEP_LOW_PASS_FLOOR_HZ / HIGHEST_HZ) ** amount,
    };
  }
  return {
    side: "high-pass",
    hz: LOWEST_HZ * (SWEEP_HIGH_PASS_CEILING_HZ / LOWEST_HZ) ** amount,
  };
}

/** The sweep filter at `position`, at `hz`, in dB: a Butterworth's 12 dB an octave. */
export function filterResponseDb(position: number, hz: number): number {
  const corner = filterCorner(position);
  if (!corner) return 0;
  const ratio = corner.side === "low-pass" ? hz / corner.hz : corner.hz / hz;
  return toDb(1 / Math.sqrt(1 + ratio ** 4));
}

/** What a face's knob is doing at `hz`, in dB, for the knob's own value. */
export function responseDb(face: Face, value: number, hz: number): number {
  switch (face) {
    case "eq-low":
      return eqResponseDb("low", value, hz);
    case "eq-mid":
      return eqResponseDb("mid", value, hz);
    case "eq-high":
      return eqResponseDb("high", value, hz);
    case "filter":
      return filterResponseDb(value, hz);
  }
}

/** The height a level is drawn at on the face. */
export function faceY(db: number): number {
  const clamped = Math.min(CEILING_DB, Math.max(FLOOR_DB, db));
  return FACE.unity - clamped * FACE.perDb;
}

/** The face's curve for a knob at `value`, as an SVG path in the 0–100 box. */
export function facePath(face: Face, value: number): string {
  const points: string[] = [];
  for (let i = 0; i < POINTS; i++) {
    const along = i / (POINTS - 1);
    const x = FACE.left + (FACE.right - FACE.left) * along;
    const y = faceY(responseDb(face, value, hzAt(along)));
    points.push(`${i === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`);
  }
  return points.join(" ");
}

/** The 0 dB line the curve is read against. */
export function unityPath(): string {
  return `M ${FACE.left} ${FACE.unity} H ${FACE.right}`;
}

/**
 * What each pad's light means, by what the pad does — the hardware's own code,
 * which a DJ's hand already knows: PLAY lit in go, CUE in the amber of *about
 * to*, SYNC in the colour of a thing chosen, and EJECT, the one press that
 * cannot be undone, always ringed in stop.
 */
export const PAD_LIGHT: Record<PadRole, string> = {
  play: "var(--success)",
  cue: "var(--warn)",
  sync: "var(--selected)",
  eject: "var(--danger)",
};

// -----------------------------------------------------------------------------
// Natural forms
// -----------------------------------------------------------------------------

/** FNV-1a: a name's own number, the same on every machine and every night. */
export function seedOf(name: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < name.length; i++) {
    hash ^= name.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

/** Mulberry32: a small, fast, repeatable sequence from one seed. */
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
 * The inner edge a stone may not come inside: the value track's outer edge
 * (radius 40, six wide) plus a margin, so the arc always lies on the stone.
 */
export const STONE_INNER = 44;
/** And the box's own edge, which it may not cross. */
export const STONE_OUTER = 49;

/**
 * A river stone's outline around the knob's centre, grown from `name`.
 *
 * Three low harmonics with random phases and falling amplitudes, which is what
 * water leaves on a stone: no corners, no symmetry, and never far from round.
 * The radius is held between [`STONE_INNER`] and [`STONE_OUTER`].
 */
export function stoneRadii(name: string, count = 64): number[] {
  const next = sequence(seedOf(name || "knob"));
  const harmonics = [2, 3, 5].map((order, index) => ({
    order,
    amplitude: (1.6 / (index + 1)) * (0.5 + next()),
    phase: next() * Math.PI * 2,
  }));
  const middle = (STONE_INNER + STONE_OUTER) / 2;
  const radii: number[] = [];
  for (let i = 0; i < count; i++) {
    const angle = (i / count) * Math.PI * 2;
    const swell = harmonics.reduce(
      (sum, h) => sum + h.amplitude * Math.cos(h.order * angle + h.phase),
      0,
    );
    radii.push(Math.min(STONE_OUTER, Math.max(STONE_INNER, middle + swell)));
  }
  return radii;
}

/** The stone as an SVG path around (50, 50). */
export function stonePath(name: string): string {
  const radii = stoneRadii(name);
  const points = radii.map((radius, i) => {
    const angle = (i / radii.length) * Math.PI * 2;
    return [50 + radius * Math.cos(angle), 50 + radius * Math.sin(angle)] as const;
  });
  return (
    points
      .map(([x, y], i) => `${i === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`)
      .join(" ") + " Z"
  );
}

/**
 * A pebble: Piet Hein's superellipse, `|x|ⁿ + |y|ⁿ = 1`, inset `inset` from
 * the box's edges. The shape between a circle and a square that water wears
 * a stone towards, and one that stays a pebble when a pad is stretched to its
 * cell.
 */
export function pebblePath(inset = 4, exponent = 4, count = 48): string {
  const half = 50 - inset;
  const points: string[] = [];
  for (let i = 0; i < count; i++) {
    const t = (i / count) * Math.PI * 2;
    const c = Math.cos(t);
    const s = Math.sin(t);
    const x = 50 + half * Math.sign(c) * Math.abs(c) ** (2 / exponent);
    const y = 50 + half * Math.sign(s) * Math.abs(s) ** (2 / exponent);
    points.push(`${i === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`);
  }
  return points.join(" ") + " Z";
}
