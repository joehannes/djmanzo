/**
 * The audio, published to CSS instead of to the render pipeline.
 *
 * # Why this exists
 *
 * The living-interface benchmark in [ROADMAP.md](../../docs/ROADMAP.md) measured
 * the same motion three ways on the no-GPU floor and found DOM the worst by a
 * wide margin — 18.6 fps against Canvas 2D's 45.4 at 960 shapes — because the
 * cost is *document invalidation* rather than fill rate. One self-repainting
 * surface beats N animating layers.
 *
 * The first version of the SVG controls put `SessionContext` inside `KnobState`,
 * so every knob, fader and pad recomputed its whole path list sixty times a
 * second whether or not the DJ had touched anything, and wrote new `stroke` and
 * `stroke-width` attributes onto every path. Four decks of controls is a few
 * hundred animating elements: exactly the shape the measurement says collapses.
 *
 * So the audio does not enter the render pipeline at all. It is written here,
 * once per snapshot, as custom properties on the root element. Every control
 * refers to them from static CSS and inherits the change for free. Geometry then
 * recomputes only when the *control* changes — which is a human moving it, not a
 * clock.
 *
 * The properties, all 0..=1 unless noted:
 *
 * | property | meaning |
 * |---|---|
 * | `--audio-loudness` | overall level, RMS |
 * | `--audio-bass` … `--audio-treble` | the four bands |
 * | `--audio-energy` | how hard the room is going, or loudness until M9 |
 * | `--audio-hue` | a colour derived from energy, in degrees |
 */
import type { SessionContext } from "./api";

/**
 * Somewhere to write custom properties.
 *
 * Narrowed to the one method used so this module can be exercised without a
 * DOM — the alternative was a headless-browser dependency for six lines of
 * `setProperty`.
 */
export interface StyleTarget {
  style: { setProperty(name: string, value: string): void };
}

/**
 * Quantisation step.
 *
 * A style write whose value is identical to the last one still costs a style
 * recalculation, so the values are rounded before being written and skipped when
 * unchanged. 1/64 is finer than the eye can follow on a colour ramp and coarse
 * enough that a steady passage stops writing entirely.
 */
const STEP = 64;

const NAMES = [
  "--audio-loudness",
  "--audio-bass",
  "--audio-low-mid",
  "--audio-high-mid",
  "--audio-treble",
  "--audio-energy",
] as const;

let last: number[] = [];

function quantise(value: number): number {
  // Non-finite reads as silence rather than as maximum: an infinity means the
  // reading is broken, and answering that with a full-brightness interface is
  // the wrong way round.
  if (!Number.isFinite(value)) return 0;
  return Math.round(Math.min(1, Math.max(0, value)) * STEP) / STEP;
}

/**
 * Write the audio onto the root element.
 *
 * Call once per snapshot. Cheap and idempotent: unchanged properties are not
 * written.
 */
export function publishAudio(context: SessionContext | undefined, target?: StyleTarget) {
  const root = target ?? (typeof document === "undefined" ? null : document.documentElement);
  if (!root) return;

  const audio = context?.audio;
  const bands = audio?.bands ?? [0, 0, 0, 0];
  // `session` is null until M9 reads the room, so energy falls back to the
  // loudness that is actually measured. See `dj_core::context`.
  const energy = context?.session?.energy ?? audio?.loudness ?? 0;

  const values = [
    quantise(audio?.loudness ?? 0),
    quantise(bands[0]),
    quantise(bands[1]),
    quantise(bands[2]),
    quantise(bands[3]),
    quantise(energy),
  ];

  for (let i = 0; i < values.length; i++) {
    if (last[i] === values[i]) continue;
    root.style.setProperty(NAMES[i], String(values[i]));
    last[i] = values[i];
  }

  // Derived, so a theme can say `hsl(var(--audio-hue) 80% 60%)` without doing
  // arithmetic in every rule. Written on the same schedule as the rest.
  const hue = Math.round(220 - values[5] * 180);
  if (last[6] !== hue) {
    root.style.setProperty("--audio-hue", `${hue}`);
    last[6] = hue;
  }
}

/** For tests, which need each case to start from nothing written. */
export function resetAudioVars() {
  last = [];
}
