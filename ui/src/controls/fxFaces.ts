/**
 * §114: an effect's knob draws what the effect does.
 *
 * > i'd also like to see the forms and shapes of the controls and widgets to
 * > be individual and useful and resembling nature and functionality
 *
 * The tone knobs draw the curve they put on the record (`faces.ts`). Each
 * effect in a rack has one knob of its own — feedback, spread, size, width,
 * grit, depth, sweep, bite — and a number on it says nothing about the sound.
 * So its face draws the sound, at the knob's setting:
 *
 * - **In time**, for the effects that are about when: the echo's repeats,
 *   each quieter by the feedback; the delay's two taps, pulled apart by the
 *   spread; the reverb's tail, a straight line on a decibel scale because a
 *   room's decay is exponential, shallower the bigger the room; the gate's
 *   chop over two of its periods; the crush's staircase through one cycle of
 *   a sine, coarser in level and in time together.
 * - **In frequency**, on the tone knobs' 20 Hz – 20 kHz axis, for the ones
 *   that are about which: the flanger's comb at the middle of its sweep, the
 *   phaser's notches at the top of it, the auto-filter's resonant low-pass at
 *   the middle of its travel.
 *
 * The arithmetic is `dj_dsp::fx`'s own, and a Rust test reads the numbers
 * below out of this file, so the picture cannot drift from the sound.
 */

import { FACE, FLOOR_DB, faceY, hzAt } from "./faces";

/** The effects with a face, by the name the rack knows them by. */
export type FxFace =
  | "fx-echo"
  | "fx-delay"
  | "fx-reverb"
  | "fx-gate"
  | "fx-crush"
  | "fx-flanger"
  | "fx-phaser"
  | "fx-filter";

/** The face for an effect, or none for an empty slot or an unknown name. */
export function fxFaceOf(kind: string): FxFace | undefined {
  const face = `fx-${kind}`;
  return (FX_FACES as readonly string[]).includes(face) ? (face as FxFace) : undefined;
}

export const FX_FACES = [
  "fx-echo",
  "fx-delay",
  "fx-reverb",
  "fx-gate",
  "fx-crush",
  "fx-flanger",
  "fx-phaser",
  "fx-filter",
] as const;

// `dj_dsp::fx`'s numbers. Each is read by a Rust test; keep the spelling
// `export const NAME = number;` on one line.
export const ECHO_FEEDBACK_MAX = 0.9;
export const DELAY_SPREAD_MAX = 0.5;
export const REVERB_FEEDBACK_MIN = 0.7;
export const REVERB_FEEDBACK_SPAN = 0.27;
/** The mean of the reverb's four comb lengths, in milliseconds. */
export const REVERB_COMB_MEAN_MS = 37.9;
export const GATE_DUTY_MIN = 0.1;
export const GATE_DUTY_SPAN = 0.8;
export const GATE_EDGE = 0.05;
export const CRUSH_BITS = 16;
export const CRUSH_BITS_SPAN = 13;
export const CRUSH_STRIDE_SPAN = 31;
export const FLANGER_FEEDBACK_MAX = 0.7;
export const FLANGER_SHORTEST_S = 0.001;
export const FLANGER_SPAN_S = 0.009;
export const PHASER_STAGES = 6;
export const PHASER_LOWEST = 0.05;
export const PHASER_TRAVEL = 0.75;
export const PHASER_REACH_MIN = 0.3;
export const AUTO_FILTER_LOW_HZ = 120;
export const AUTO_FILTER_RANGE = 60;
export const AUTO_FILTER_Q_MIN = 0.707;
export const AUTO_FILTER_Q_SPAN = 6;

/** The rate the frequency pictures are worked at. */
const RATE = 48_000;

/** The time pictures' box: silence on the floor, full level at the top. */
export const TIME = { floor: 62, top: 38 } as const;

const clamp01 = (value: number) => (Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0);
const x = (along: number) => FACE.left + (FACE.right - FACE.left) * along;
const level = (gain: number) => TIME.floor + (TIME.top - TIME.floor) * clamp01(gain);
const n = (value: number) => value.toFixed(2);

/** How many repeats the echo's face shows. */
export const ECHO_TAPS = 8;

/** The echo's repeats, loudest first: each the last times the feedback. */
export function echoTaps(amount: number): number[] {
  const feedback = clamp01(amount) * ECHO_FEEDBACK_MAX;
  return Array.from({ length: ECHO_TAPS }, (_, k) => feedback ** k);
}

/** The delay's two taps, in beats: the left early and the right late. */
export function delayTaps(amount: number): { left: number; right: number } {
  const spread = clamp01(amount) * DELAY_SPREAD_MAX;
  return { left: 1 - spread, right: 1 + spread };
}

/** How long the reverb's face shows, in seconds. */
export const REVERB_SECONDS = 2;
/** The bottom of the reverb's face, in decibels. */
export const REVERB_FLOOR_DB = -60;

/** The reverb's tail `seconds` after the sound, in dB: its combs' feedback per trip. */
export function reverbDb(amount: number, seconds: number): number {
  const feedback = REVERB_FEEDBACK_MIN + clamp01(amount) * REVERB_FEEDBACK_SPAN;
  const trips = (seconds * 1000) / REVERB_COMB_MEAN_MS;
  return Math.max(REVERB_FLOOR_DB, 20 * trips * Math.log10(feedback));
}

/** The gate's gain at `phase` through its period: the DSP's raised-cosine chop. */
export function gateGain(amount: number, phase: number): number {
  const duty = GATE_DUTY_MIN + clamp01(amount) * GATE_DUTY_SPAN;
  const p = phase - Math.floor(phase);
  if (p < duty - GATE_EDGE) return 1;
  if (p < duty) return 0.5 - 0.5 * Math.cos(Math.PI * (1 - (duty - p) / GATE_EDGE));
  if (p < 1 - GATE_EDGE) return 0;
  return 0.5 - 0.5 * Math.cos((Math.PI * (p - (1 - GATE_EDGE))) / GATE_EDGE);
}

/** How finely the crush's face draws a sine: this many samples a cycle. */
export const CRUSH_SAMPLES = 64;

/**
 * One cycle of a sine through the crush: held every `stride` samples and
 * rounded to `levels` steps, as the DSP does. Values in -1..1.
 */
export function crushed(amount: number): number[] {
  const a = clamp01(amount);
  const levels = 2 ** (CRUSH_BITS - a * CRUSH_BITS_SPAN);
  const stride = 1 + a * CRUSH_STRIDE_SPAN;
  const out: number[] = [];
  let held = stride;
  let hold = 0;
  for (let i = 0; i < CRUSH_SAMPLES; i++) {
    held += 1;
    if (held >= stride) {
      held -= stride;
      hold = Math.round(Math.sin((2 * Math.PI * i) / CRUSH_SAMPLES) * levels) / levels;
    }
    out.push(hold);
  }
  return out;
}

/** |H| of a complex ratio, from its parts. */
const magnitude = (re: number, im: number) => Math.hypot(re, im);

/**
 * The flanger at the middle of its sweep, at `hz`, in dB: the dry and the
 * delayed signal summed and halved, the delay fed back into itself.
 */
export function flangerDb(amount: number, hz: number): number {
  const feedback = clamp01(amount) * FLANGER_FEEDBACK_MAX;
  const delay = FLANGER_SHORTEST_S + FLANGER_SPAN_S * 0.5;
  const w = 2 * Math.PI * hz * delay;
  // T = e^{-jw} / (1 - g e^{-jw}); out = (1 + T) / 2
  const dRe = 1 - feedback * Math.cos(w);
  const dIm = feedback * Math.sin(w);
  const nRe = Math.cos(w);
  const nIm = -Math.sin(w);
  const d2 = dRe * dRe + dIm * dIm;
  const tRe = (nRe * dRe + nIm * dIm) / d2;
  const tIm = (nIm * dRe - nRe * dIm) / d2;
  return toDb(magnitude(1 + tRe, tIm) / 2);
}

/**
 * The phaser at the top of its sweep, at `hz`, in dB: six first-order
 * allpasses summed against the dry and halved, which is where the notches
 * come from.
 */
export function phaserDb(amount: number, hz: number): number {
  const t = PHASER_LOWEST + PHASER_TRAVEL * (PHASER_REACH_MIN + (1 - PHASER_REACH_MIN) * clamp01(amount));
  const c = (t - 1) / (t + 1);
  const w = (2 * Math.PI * hz) / RATE;
  // H(z) = (c + z^-1) / (1 + c z^-1), an allpass: its phase is all that moves.
  const nRe = c + Math.cos(w);
  const nIm = -Math.sin(w);
  const dRe = 1 + c * Math.cos(w);
  const dIm = -c * Math.sin(w);
  const phase = Math.atan2(nIm, nRe) - Math.atan2(dIm, dRe);
  const total = PHASER_STAGES * phase;
  return toDb(magnitude(1 + Math.cos(total), Math.sin(total)) / 2);
}

/** The auto-filter's corner at the middle of its travel. */
export const AUTO_FILTER_MIDDLE_HZ = AUTO_FILTER_LOW_HZ * Math.sqrt(AUTO_FILTER_RANGE);

/** The auto-filter at the middle of its travel, at `hz`, in dB: an RBJ low-pass whose Q is the bite. */
export function autoFilterDb(amount: number, hz: number): number {
  const q = AUTO_FILTER_Q_MIN + clamp01(amount) * AUTO_FILTER_Q_SPAN;
  const w0 = (2 * Math.PI * AUTO_FILTER_MIDDLE_HZ) / RATE;
  const alpha = Math.sin(w0) / (2 * q);
  const cos0 = Math.cos(w0);
  const b0 = (1 - cos0) / 2;
  const b1 = 1 - cos0;
  const b2 = b0;
  const a0 = 1 + alpha;
  const a1 = -2 * cos0;
  const a2 = 1 - alpha;
  const w = (2 * Math.PI * hz) / RATE;
  const num = [b0 + b1 * Math.cos(w) + b2 * Math.cos(2 * w), -(b1 * Math.sin(w) + b2 * Math.sin(2 * w))];
  const den = [a0 + a1 * Math.cos(w) + a2 * Math.cos(2 * w), -(a1 * Math.sin(w) + a2 * Math.sin(2 * w))];
  return toDb(magnitude(num[0], num[1]) / magnitude(den[0], den[1]));
}

function toDb(gain: number): number {
  if (!(gain > 0)) return FLOOR_DB;
  return 20 * Math.log10(gain);
}

/** How many points a frequency picture is drawn through: enough for a comb's teeth. */
export const FX_POINTS = 97;

function frequencyPath(response: (hz: number) => number): string {
  const points: string[] = [];
  for (let i = 0; i < FX_POINTS; i++) {
    const along = i / (FX_POINTS - 1);
    points.push(`${i === 0 ? "M" : "L"} ${n(x(along))} ${n(faceY(response(hzAt(along))))}`);
  }
  return points.join(" ");
}

/** An effect's face at its knob's `amount`, as an SVG path in the knob's 0–100 box. */
export function fxFacePath(face: FxFace, amount: number): string {
  switch (face) {
    case "fx-echo":
      return echoTaps(amount)
        .map((gain, k) => ({ gain, at: (k + 0.5) / ECHO_TAPS }))
        .filter(({ gain }) => gain >= 0.02)
        .map(({ gain, at }) => `M ${n(x(at))} ${TIME.floor} V ${n(level(gain))}`)
        .join(" ");
    case "fx-delay": {
      const { left, right } = delayTaps(amount);
      // Two beats across the face: the taps sit either side of the middle.
      return [left, right].map((beats) => `M ${n(x(beats / 2))} ${TIME.floor} V ${TIME.top}`).join(" ");
    }
    case "fx-reverb": {
      const points: string[] = [];
      for (let i = 0; i <= 32; i++) {
        const along = i / 32;
        const db = reverbDb(amount, along * REVERB_SECONDS);
        points.push(`${i === 0 ? "M" : "L"} ${n(x(along))} ${n(level(1 - db / REVERB_FLOOR_DB))}`);
      }
      return points.join(" ");
    }
    case "fx-gate": {
      const points: string[] = [];
      const steps = 80;
      for (let i = 0; i <= steps; i++) {
        const along = i / steps;
        points.push(`${i === 0 ? "M" : "L"} ${n(x(along))} ${n(level(gateGain(amount, along * 2)))}`);
      }
      return points.join(" ");
    }
    case "fx-crush": {
      const middle = (TIME.floor + TIME.top) / 2;
      const half = (TIME.floor - TIME.top) / 2;
      const values = crushed(amount);
      const points: string[] = [];
      values.forEach((value, i) => {
        const y = n(middle - value * half);
        const from = n(x(i / values.length));
        const to = n(x((i + 1) / values.length));
        points.push(`${i === 0 ? "M" : "L"} ${from} ${y} L ${to} ${y}`);
      });
      return points.join(" ");
    }
    case "fx-flanger":
      return frequencyPath((hz) => flangerDb(amount, hz));
    case "fx-phaser":
      return frequencyPath((hz) => phaserDb(amount, hz));
    case "fx-filter":
      return frequencyPath((hz) => autoFilterDb(amount, hz));
  }
}

/** The line an effect's face is read against: silence for time, 0 dB for frequency. */
export function fxReferencePath(face: FxFace): string {
  switch (face) {
    case "fx-flanger":
    case "fx-phaser":
    case "fx-filter":
      return `M ${FACE.left} ${FACE.unity} H ${FACE.right}`;
    case "fx-crush":
      return `M ${FACE.left} ${(TIME.floor + TIME.top) / 2} H ${FACE.right}`;
    default:
      return `M ${FACE.left} ${TIME.floor} H ${FACE.right}`;
  }
}
