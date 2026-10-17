/**
 * §34 / §36: the floor's other signals, measured where the room surface
 * already measures light, movement and loudness.
 *
 * Pure functions over a frame of luma or a window of sound, so each is tested
 * on its own (`floor.test.ts`) and through Chromium's stand-in camera and
 * microphone (`e2e/floor.spec.ts`). What crosses to Rust is the numbers they
 * return — never a frame, never a sample — and Rust places each against the
 * same room earlier tonight (`dj_assistant::floor`). Nothing here decides what
 * a number means, and nothing combines two of them: §36 forbids a crowd score,
 * and a sum of these would be one.
 *
 * Every scale below is the room's own. A band reading 0.6 is not "loud"; it is
 * 0.6 of this microphone's range, and only ever compared with itself.
 */
import { invoke } from "@tauri-apps/api/core";

/** The three bands, in hertz: below 250, 250 to 2k (voices), above 2k. */
export const BANDS = {
  low: [20, 250],
  mid: [250, 2000],
  high: [2000, 12000],
} as const;

/**
 * The band level, in decibels, that reads as zero. The analyser reports bins
 * relative to full scale; eighty decibels down is below any room a
 * microphone hears, so the range a night actually uses lands mid-scale.
 */
export const BAND_FLOOR_DB = -80;

/**
 * Each band's level on 0..1, from an analyser's frequency data in decibels.
 *
 * `spectrumDb` is `AnalyserNode.getFloatFrequencyData`'s array, whose bins
 * run from 0 Hz to half the sample rate. Power is summed across the band's bins — a band is the energy in it, not its
 * loudest bin — then put back into decibels and across 0..1 from
 * {@link BAND_FLOOR_DB} to 0 dB.
 */
export function bands(
  spectrumDb: Float32Array,
  sampleRate: number,
): { low: number; mid: number; high: number } {
  // An analyser gives half as many bins as its FFT is long, so the width of a
  // bin is read off the spectrum itself rather than passed beside it — two
  // numbers that must agree are two chances for them not to.
  const hzPerBin = sampleRate / (2 * spectrumDb.length);
  const level = ([from, to]: readonly [number, number]) => {
    let power = 0;
    const first = Math.max(1, Math.ceil(from / hzPerBin));
    const last = Math.min(spectrumDb.length - 1, Math.floor(to / hzPerBin));
    for (let bin = first; bin <= last; bin++) {
      const db = spectrumDb[bin];
      if (Number.isFinite(db)) power += 10 ** (db / 10);
    }
    const db = 10 * Math.log10(Math.max(power, 1e-12));
    return Math.min(1, Math.max(0, (db - BAND_FLOOR_DB) / -BAND_FLOOR_DB));
  };
  return { low: level(BANDS.low), mid: level(BANDS.mid), high: level(BANDS.high) };
}

/** How long one slice of sound is when looking for clicks: five milliseconds. */
export const CLICK_SLICE_S = 0.005;

/** How many times the window's typical slice a click must stand out by. */
export const CLICK_ABOVE = 6;

/** The shortest gap between two clicks: twenty milliseconds. */
export const CLICK_GAP_S = 0.02;

/** Clicks a second that read as the whole scale. A room clapping makes more. */
export const CLICKS_FULL = 25;

/**
 * Applause-like clicks, on 0..1: how many sharp clicks the window holds, and
 * how little they keep to a grid.
 *
 * A clap is a sharp broadband click and a room clapping is many of them, each
 * person on their own time. A hi-hat is just as sharp and keeps perfect time.
 * So the clicks are found — slices of the window's first difference (which
 * weighs the sharp, high part of the sound) standing well above the window's
 * own typical slice — and their rate is discounted by how regular they are:
 * the share of the gaps between them that sit at the commonest gap. A
 * metronome scores nothing however fast it ticks; a room clapping scores high.
 *
 * A fast, loose percussion part can look the same, and Rust's line for this
 * signal says so beside every reading.
 */
export function clicks(samples: Float32Array, sampleRate: number): number {
  const slice = Math.max(1, Math.round(CLICK_SLICE_S * sampleRate));
  const count = Math.floor((samples.length - 1) / slice);
  if (count < 8) return 0;
  const energy = new Float64Array(count);
  for (let s = 0; s < count; s++) {
    let sum = 0;
    for (let i = s * slice + 1; i <= (s + 1) * slice; i++) {
      const d = samples[i] - samples[i - 1];
      sum += d * d;
    }
    energy[s] = sum / slice;
  }
  const sorted = Array.from(energy).sort((a, b) => a - b);
  const typical = sorted[Math.floor(sorted.length / 2)];
  // Silence and the hiss of a quiet input have no clicks in them, however
  // spiky their tiny numbers are.
  const threshold = Math.max(typical * CLICK_ABOVE, 1e-7);
  const gap = Math.max(1, Math.round(CLICK_GAP_S / CLICK_SLICE_S));
  const at: number[] = [];
  for (let s = 0; s < count; s++) {
    const e = energy[s];
    if (e < threshold) continue;
    if (s > 0 && energy[s - 1] > e) continue;
    if (s + 1 < count && energy[s + 1] > e) continue;
    if (at.length > 0 && s - at[at.length - 1] < gap) continue;
    at.push(s);
  }
  if (at.length < 4) return 0;
  const gaps = at.slice(1).map((s, i) => s - at[i]);
  const tally = new Map<number, number>();
  for (const g of gaps) tally.set(g, (tally.get(g) ?? 0) + 1);
  let commonest = 0;
  for (const [g] of tally) {
    const near = (tally.get(g - 1) ?? 0) + (tally.get(g) ?? 0) + (tally.get(g + 1) ?? 0);
    commonest = Math.max(commonest, near);
  }
  const regular = commonest / gaps.length;
  const seconds = (count * slice) / sampleRate;
  const rate = at.length / seconds;
  return Math.min(1, rate / CLICKS_FULL) * (1 - regular);
}

/** The side of one block of the picture, in pixels of the 64×48 frame. */
export const BLOCK = 8;

/** How far a block is searched for, each way, in pixels. */
export const REACH = 3;

/**
 * The least spread of brightness a block needs before its movement is read.
 * A blank wall moves nowhere a camera can see, and a vector read from one is
 * noise.
 */
export const TEXTURE = 20;

/**
 * Movement across the picture: how unevenly it moves, and how much of it
 * goes the same way.
 *
 * A coarse optical flow. The earlier frame is cut into {@link BLOCK}-pixel
 * blocks and each is looked for in the later one within {@link REACH} pixels;
 * where it fits best is how far that part of the picture moved. A block that
 * fits no better anywhere than where it was has not moved.
 *
 * - **uneven** is the variance of those movements across the picture, on
 *   0..1: everything still or everything moving alike is 0, parts moving
 *   every which way is high. §34's *optical-flow variance*.
 * - **together** is how much of the movement goes one way — the length of the
 *   blocks' summed movement over the sum of their lengths, on 0..1. §36's
 *   *cohesion*. `null` when fewer than three blocks moved: nothing moving is
 *   not moving together, or apart.
 *
 * Both `null` when fewer than four blocks have enough texture to read.
 */
export function flow(
  before: Float32Array,
  after: Float32Array,
  wide: number,
  high: number,
): { uneven: number | null; together: number | null } {
  const vectors: [number, number][] = [];
  const sad = (bx: number, by: number, dx: number, dy: number) => {
    let sum = 0;
    for (let y = 0; y < BLOCK; y++) {
      const row = (by + y) * wide;
      const moved = (by + y + dy) * wide + dx;
      for (let x = 0; x < BLOCK; x++) sum += Math.abs(after[moved + bx + x] - before[row + bx + x]);
    }
    return sum;
  };
  for (let by = 0; by + BLOCK <= high; by += BLOCK) {
    for (let bx = 0; bx + BLOCK <= wide; bx += BLOCK) {
      let mean = 0;
      for (let y = 0; y < BLOCK; y++) {
        for (let x = 0; x < BLOCK; x++) mean += before[(by + y) * wide + bx + x];
      }
      mean /= BLOCK * BLOCK;
      let spread = 0;
      for (let y = 0; y < BLOCK; y++) {
        for (let x = 0; x < BLOCK; x++) spread += (before[(by + y) * wide + bx + x] - mean) ** 2;
      }
      if (spread / (BLOCK * BLOCK) < TEXTURE) continue;
      const still = sad(bx, by, 0, 0);
      let best = still;
      let bestAt: [number, number] = [0, 0];
      for (let dy = -REACH; dy <= REACH; dy++) {
        if (by + dy < 0 || by + dy + BLOCK > high) continue;
        for (let dx = -REACH; dx <= REACH; dx++) {
          if (bx + dx < 0 || bx + dx + BLOCK > wide || (dx === 0 && dy === 0)) continue;
          const here = sad(bx, by, dx, dy);
          if (here < best) {
            best = here;
            bestAt = [dx, dy];
          }
        }
      }
      // Moved only where somewhere else fits clearly better than staying put:
      // a block of sensor noise fits a little better somewhere by chance.
      vectors.push(best < still * 0.8 ? bestAt : [0, 0]);
    }
  }
  if (vectors.length < 4) return { uneven: null, together: null };
  const mx = vectors.reduce((s, v) => s + v[0], 0) / vectors.length;
  const my = vectors.reduce((s, v) => s + v[1], 0) / vectors.length;
  const variance =
    vectors.reduce((s, v) => s + (v[0] - mx) ** 2 + (v[1] - my) ** 2, 0) / vectors.length;
  const uneven = Math.min(1, variance / (REACH * REACH));
  const moving = vectors.filter(([x, y]) => x !== 0 || y !== 0);
  if (moving.length < 3) return { uneven, together: null };
  const sx = moving.reduce((s, v) => s + v[0], 0);
  const sy = moving.reduce((s, v) => s + v[1], 0);
  const lengths = moving.reduce((s, v) => s + Math.hypot(v[0], v[1]), 0);
  return { uneven, together: Math.hypot(sx, sy) / lengths };
}

/** How many brightness bins a picture is summarised in. */
export const BINS = 16;

/** A frame's brightness, as the share of its pixels in each of {@link BINS} bins. */
export function histogram(luma: Float32Array): Float32Array {
  const out = new Float32Array(BINS);
  for (const v of luma) out[Math.min(BINS - 1, Math.max(0, Math.floor((v / 256) * BINS)))] += 1;
  for (let i = 0; i < BINS; i++) out[i] /= Math.max(1, luma.length);
  return out;
}

/**
 * How much the picture changed as a whole between two looks, on 0..1: the
 * share of the picture's brightness that is not where it was. The same
 * picture is 0; a black frame after a white one is 1. A light sweeping across
 * the room changes it too, and Rust's line says so.
 */
export function sceneChange(before: Float32Array, after: Float32Array): number {
  let shared = 0;
  for (let i = 0; i < BINS; i++) shared += Math.min(before[i], after[i]);
  return Math.min(1, Math.max(0, 1 - shared));
}

/**
 * How far a pixel's brightness must sit from the room's learned look before it
 * counts as something in the room, on the 0..255 luma scale. Sensor noise and
 * the slow drift of a room's light stay well inside it.
 */
export const OCCUPIED_BY = 24;

/** The share of the difference a look moves the learned look by, once settled. */
export const LEARN = 0.02;

/** How many looks the room's look needs before anything is read against it. */
export const LEARNED_AFTER = 6;

/** The side of one block when reading how the occupied parts pack, in pixels. */
export const PACK = 4;

/** How many blocks each way count as beside a block. */
export const PACK_REACH = 2;

/** The room's look, learned from the looks so far. */
export interface Background {
  luma: Float32Array;
  looks: number;
}

/**
 * Learn the room's look from one more frame.
 *
 * A running average of the frames: each look moves it {@link LEARN} of the way
 * towards what the camera sees now, so a crowd that keeps moving never settles
 * into it and a lamp switched on an hour ago does. For the first looks it is
 * the plain average of everything seen, so the first frame — which may well
 * have people in it — is not held as the empty room for minutes.
 */
export function learn(background: Background | null, frame: Float32Array): Background {
  if (!background || background.luma.length !== frame.length) {
    return { luma: Float32Array.from(frame), looks: 1 };
  }
  const share = Math.max(LEARN, 1 / (background.looks + 1));
  const luma = background.luma.map((v, i) => v + (frame[i] - v) * share);
  return { luma, looks: background.looks + 1 };
}

/**
 * How much of the picture is occupied, and how tightly what occupies it packs.
 *
 * - **occupancy** is the share of the picture's pixels more than
 *   {@link OCCUPIED_BY} from the room's learned look, on 0..1. §34's
 *   *occupancy*, as a proxy: it counts whatever is not the usual room, people
 *   and a light thrown on a wall alike, and a person standing still long
 *   enough becomes the usual room.
 * - **density** is how closely those pixels sit together: the picture is cut
 *   into {@link PACK}-pixel blocks, a block is occupied when a quarter of it
 *   is, and for each occupied block the share of the blocks within
 *   {@link PACK_REACH} of it that are occupied too, averaged. §34's *crowd
 *   density*, as a proxy: a few figures apart read low, a packed floor high,
 *   and one person filling the lens as high as a packed floor.
 *
 * Both `null` until the look has been learned from {@link LEARNED_AFTER}
 * frames; density `null` too when fewer than three blocks are occupied, since
 * how nothing packs is not a reading.
 */
export function occupied(
  background: Background | null,
  frame: Float32Array,
  wide: number,
  high: number,
): { occupancy: number | null; density: number | null } {
  if (!background || background.looks < LEARNED_AFTER || background.luma.length !== frame.length) {
    return { occupancy: null, density: null };
  }
  const mask = new Uint8Array(frame.length);
  let count = 0;
  for (let i = 0; i < frame.length; i++) {
    if (Math.abs(frame[i] - background.luma[i]) > OCCUPIED_BY) {
      mask[i] = 1;
      count += 1;
    }
  }
  const occupancy = count / frame.length;
  const across = Math.floor(wide / PACK);
  const down = Math.floor(high / PACK);
  const blocks = new Uint8Array(across * down);
  const taken: number[] = [];
  for (let by = 0; by < down; by++) {
    for (let bx = 0; bx < across; bx++) {
      let n = 0;
      for (let y = 0; y < PACK; y++) {
        for (let x = 0; x < PACK; x++) n += mask[(by * PACK + y) * wide + bx * PACK + x];
      }
      if (n * 4 >= PACK * PACK) {
        blocks[by * across + bx] = 1;
        taken.push(by * across + bx);
      }
    }
  }
  if (taken.length < 3) return { occupancy, density: null };
  let sum = 0;
  for (const at of taken) {
    const bx = at % across;
    const by = Math.floor(at / across);
    let near = 0;
    let cells = 0;
    for (let dy = -PACK_REACH; dy <= PACK_REACH; dy++) {
      for (let dx = -PACK_REACH; dx <= PACK_REACH; dx++) {
        const x = bx + dx;
        const y = by + dy;
        if ((dx === 0 && dy === 0) || x < 0 || y < 0 || x >= across || y >= down) continue;
        cells += 1;
        near += blocks[y * across + x];
      }
    }
    sum += cells > 0 ? near / cells : 0;
  }
  return { occupancy, density: sum / taken.length };
}

/** The luma, on 0..255, at and above which a pixel is part of the lighting. */
export const BRIGHT = 200;

/** The fewest bright pixels that make a light worth following. */
export const BRIGHT_LEAST = 3;

/** The share of the picture's diagonal a light must cross to read as the whole scale. */
export const LIGHT_FULL = 0.1;

/**
 * How far the lighting moved between two frames, on 0..1: the distance the
 * centre of the picture's brightest pixels (at or above {@link BRIGHT}) moved,
 * against {@link LIGHT_FULL} of the diagonal.
 *
 * Distinct from {@link sceneChange} on purpose. A beam sweeping across a dark
 * room moves its light a long way and leaves the picture's brightness exactly
 * as it was, so the scene has not changed and the lighting has; the lights
 * snapping on change the scene and move nothing. `null` when either frame has
 * fewer than {@link BRIGHT_LEAST} bright pixels — there is no light to follow.
 *
 * A centre: two beams sweeping opposite ways cancel, and anything bright that
 * moves — a white shirt in a dark room — moves it too. Rust's line says so.
 */
export function lighting(
  before: Float32Array,
  after: Float32Array,
  wide: number,
  high: number,
): number | null {
  const centre = (luma: Float32Array): [number, number] | null => {
    let n = 0;
    let sx = 0;
    let sy = 0;
    for (let i = 0; i < luma.length; i++) {
      if (luma[i] < BRIGHT) continue;
      n += 1;
      sx += i % wide;
      sy += Math.floor(i / wide);
    }
    return n >= BRIGHT_LEAST ? [sx / n, sy / n] : null;
  };
  const from = centre(before);
  const to = centre(after);
  if (!from || !to) return null;
  const moved = Math.hypot(to[0] - from[0], to[1] - from[1]);
  return Math.min(1, moved / (Math.hypot(wide, high) * LIGHT_FULL));
}

/** What one look measured. Every field optional: a microphone alone sees no picture. */
export interface FloorMeasured {
  low?: number;
  mid?: number;
  high?: number;
  applause?: number;
  uneven?: number;
  together?: number;
  scene?: number;
  occupancy?: number;
  density?: number;
  lighting?: number;
  /** The microphone's loudness, the number the room surface already read. */
  heard?: number;
}

/** One signal's line, as Rust says it. */
export interface FloorLine {
  signal: string;
  title: string;
  now: number | null;
  /** `lowest` … `highest`, or null before it can be placed. */
  against: string | null;
  notable: boolean;
  confidence: "not_yet" | "early" | "settled";
  looks: number;
  says: string;
  measures: string;
}

export interface FloorRead {
  watching: boolean;
  lines: FloorLine[];
}

export const floorSaw = (measured: FloorMeasured) =>
  invoke<void>("floor_saw", {
    measured: {
      low: measured.low ?? null,
      mid: measured.mid ?? null,
      high: measured.high ?? null,
      applause: measured.applause ?? null,
      uneven: measured.uneven ?? null,
      together: measured.together ?? null,
      scene: measured.scene ?? null,
      occupancy: measured.occupancy ?? null,
      density: measured.density ?? null,
      lighting: measured.lighting ?? null,
      heard: measured.heard ?? null,
    },
  });

export const floorRead = () => invoke<FloorRead | null>("floor_read");
