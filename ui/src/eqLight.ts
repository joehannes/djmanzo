/**
 * §110, live: how brightly each of the waveform's three EQ parts is drawn.
 *
 * > reiterate over the (live) colors and interactive/reactiveness of the
 * > waveform
 *
 * The lane lays three images of every tile over each other — the low, mid
 * and high parts of the spectrum, `dj_render::EqPart` — and this is the
 * opacity of each, from the deck's own knobs: kill the low and the red heart
 * of the waveform fades to a ghost where it stood; sweep the filter down and
 * the violet edge goes first, then the greens, as the corner passes them.
 * The shape never moves, so a DJ reads *what they have taken out* of the
 * record, and the record itself, at once.
 *
 * An opacity, so the compositor does it: turning a knob redraws no tile.
 */

/** A killed part is a ghost, not gone: the DJ still sees what they cut. */
export const GHOST = 0.14;

/**
 * Where the three parts meet, in Hz: the edges of `dj_render`'s eight bands
 * nearest the isolator's 300 Hz and 4 kHz crossovers
 * (`SPECTRUM_EDGES_HZ[2]` and `[5]`). A Rust test reads these two numbers
 * out of this file, so the two cannot drift apart.
 */
export const LOW_MID_HZ = 266;
export const MID_HIGH_HZ = 3560;

/** Hearing's ends, `dj_render::HEARING_HZ`. */
const LOWEST_HZ = 20;
const HIGHEST_HZ = 20_000;

/**
 * The sweep filter's corners, from `dj_dsp::SweepFilter`: fully low-passed at
 * 40 Hz, fully high-passed at 8 kHz, and a dead zone around the centre.
 */
const DEAD_ZONE = 0.02;
const LOW_PASS_FLOOR_HZ = 40;
const HIGH_PASS_CEILING_HZ = 8_000;

export type Part = "low" | "mid" | "high";
export const PARTS: Part[] = ["low", "mid", "high"];

const RANGE: Record<Part, [number, number]> = {
  low: [LOWEST_HZ, LOW_MID_HZ],
  mid: [LOW_MID_HZ, MID_HIGH_HZ],
  high: [MID_HIGH_HZ, HIGHEST_HZ],
};

/** How much of a part's octaves lie below `hz`, 0..1. */
function below(part: Part, hz: number): number {
  const [low, high] = RANGE[part];
  const share = Math.log(hz / low) / Math.log(high / low);
  return Math.min(1, Math.max(0, share));
}

/**
 * How much of a part the filter lets through, 0..1: the share of its octaves
 * on the open side of the corner. `position` is the knob, -1 fully
 * low-passed, +1 fully high-passed, as `DeckState.filter` carries it.
 */
export function filterPasses(part: Part, position: number): number {
  if (!Number.isFinite(position) || Math.abs(position) <= DEAD_ZONE) return 1;
  const amount = Math.min(1, (Math.abs(position) - DEAD_ZONE) / (1 - DEAD_ZONE));
  if (position < 0) {
    const corner = HIGHEST_HZ * (LOW_PASS_FLOOR_HZ / HIGHEST_HZ) ** amount;
    return below(part, corner);
  }
  const corner = LOWEST_HZ * (HIGH_PASS_CEILING_HZ / LOWEST_HZ) ** amount;
  return 1 - below(part, corner);
}

/**
 * The opacity one part is drawn at.
 *
 * The EQ gain is linear, 1 at unity and 0 at a kill; a boost draws no
 * brighter than unity, because full colour is already what "all of it" looks
 * like and a brighter-than-true waveform would be a lie about the record.
 */
export function partOpacity(part: Part, gain: number, filter: number): number {
  const eq = Number.isFinite(gain) ? Math.min(1, Math.max(0, gain)) : 1;
  const through = eq * filterPasses(part, filter);
  return Math.round((GHOST + (1 - GHOST) * through) * 100) / 100;
}

/** All three, for one deck's knobs. */
export function partOpacities(deck: {
  eq_low: number;
  eq_mid: number;
  eq_high: number;
  filter: number;
}): Record<Part, number> {
  return {
    low: partOpacity("low", deck.eq_low, deck.filter),
    mid: partOpacity("mid", deck.eq_mid, deck.filter),
    high: partOpacity("high", deck.eq_high, deck.filter),
  };
}
