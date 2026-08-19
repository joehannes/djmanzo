/**
 * The world, and which renderer draws it.
 *
 * The world itself is built in Rust (`dj_world`); this is the interface's view
 * of it plus the tier selection, which per
 * [ADR-0009](../../docs/adr/0009-the-living-interface.md) is decided by
 * **measurement, never by feature detection**.
 *
 * That is not a preference. The rendering benchmark found WebKitGTK reporting
 * its renderer as "Apple GPU" on a headless Linux container with no GPU at all,
 * so `WEBGL_debug_renderer_info` is not merely unreliable here — it is
 * misleading. The frame probe is the only honest detector.
 */
import { invoke } from "@tauri-apps/api/core";

/** The shape family a thing belongs to. Deliberately few. */
export type Form = "Flow" | "Eddy" | "Marker" | "Field";

/**
 * Whether a thing bears weight.
 *
 * `Trunk` is anything a DJ clicks, drags or aims at: rigid, always where it was
 * last time, and a real focusable element. `Foliage` reports state and may move
 * within bounds. Nothing in nature asks you to stand on something swaying.
 */
export type Bearing = "Trunk" | "Foliage";

/** How two keys behave where their waters meet. */
export type Confluence = "Unknown" | "Same" | "Blend" | "Seam";

export interface Tint {
  /** 0..360, the Camelot wheel. Meaningless when saturation is zero. */
  hue: number;
  /** Certainty. Pale is unsure. */
  saturation: number;
  /** Energy and level. */
  lightness: number;
}

/** How far a thing may stray from where it rests. */
export interface Excursion {
  /** Fraction of its own radius. Its centre never moves. */
  drift: number;
  scale: number;
}

/** How alive a thing is. Rates and depths, never positions. */
export interface Vitality {
  /** The music's tempo. Everything pulses on this, not on wall clock. */
  pulse_bpm: number;
  /** Where in the beat the crest is, 0..1. Synced decks share it. */
  phase: number;
  depth: number;
  agitation: number;
  /** Uncertainty. You do not navigate water you cannot see through. */
  turbidity: number;
  excursion: Excursion;
}

export interface Entity {
  /** The ADR-0008 widget name, e.g. `deck.river`. */
  name: string;
  index: number;
  form: Form;
  bearing: Bearing;
  tint: Tint;
  vitality: Vitality;
  along: number;
  extent: number;
  /** What it stands for, so a still frame is legible and a reader can say it. */
  reading: string;
}

/**
 * The one thing allowed to move in the corner of a DJ's eye.
 *
 * Peripheral attention is close to a single channel: three things claiming it
 * means none of them arrive. Everything not holding it still shows its state as
 * static form — losing the alarm is losing the motion, not the information.
 */
export type Alarm =
  | "Dropouts"
  | { RunningOut: { deck: number } }
  | "Limiting"
  | { EndingSoon: { deck: number } };

/**
 * How two rivers stand relative to each other in time.
 *
 * Three states because they are **three different actions**: locked is nothing
 * to do, an offset is a nudge, and a slide is the pitch fader. A DJ reading
 * "out of sync" learns only that something is wrong.
 */
export type Beating =
  | "Unknown"
  | "Locked"
  | { Offset: { beats: number } }
  | { Sliding: { bpm_difference: number } };

export interface World {
  entities: Entity[];
  confluence: Confluence;
  strain: number;
  alarm: Alarm | null;
  beating: Beating;
}

export const emptyWorld = (): World => ({
  entities: [],
  confluence: "Unknown",
  strain: 0,
  alarm: null,
  beating: "Unknown",
});

/** Which deck an alarm is about, when it is about one. */
export function alarmDeck(alarm: Alarm | null): number | null {
  if (alarm == null || typeof alarm === "string") return null;
  return "RunningOut" in alarm ? alarm.RunningOut.deck : alarm.EndingSoon.deck;
}

/** What the living interface should draw, right now. */
export const getWorld = () => invoke<World>("world");

/**
 * How richly the world is being drawn.
 *
 * The tiers say the same things with different fidelity — never different
 * content. Tier 3 may say it more beautifully; it must not say it more
 * completely, or beauty has become load-bearing.
 */
export type Tier = "still" | "living";

/**
 * Whether the viewer has asked for less motion.
 *
 * A hard requirement rather than a courtesy, and also the best test in the
 * system: if a still frame does not tell a DJ the state of their mix, the design
 * is leaning on animation to say something shape should have said.
 */
export function prefersStillness(): boolean {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

/**
 * Which tier the world should be drawn at.
 *
 * ADR-0009: **selected by measurement, never by feature detection.** The
 * rendering benchmark caught WebKitGTK reporting its renderer as "Apple GPU" on
 * a machine with no GPU, so asking the platform what it can do returns an
 * answer that is not merely unreliable but misleading. Asking how fast the last
 * second actually was cannot lie.
 *
 * Demotion is not a failure state. Tier 0 is a complete, legible interface —
 * it says the same things, in form and colour and position instead of motion —
 * so a machine that cannot animate gets a working mixer rather than a broken
 * pretty one.
 */
export function tierFor(fps: number | null, reducedMotion: boolean): Tier {
  if (reducedMotion) return "still";
  // Null means the probe has not reported distress, which is the healthy case.
  if (fps == null) return "living";
  return fps >= DEMOTE_BELOW_FPS ? "living" : "still";
}

/**
 * Below this sustained frame rate the world stops animating.
 *
 * Well under the probe's own warning threshold: a warning is worth showing at
 * 40 fps, but the animation is still doing its job there. Below 24 the pulse no
 * longer reads as a pulse — it reads as stutter, which is worse than stillness
 * because it says something is wrong with the *music*.
 */
export const DEMOTE_BELOW_FPS = 24;

/** Turn a world tint into something CSS and canvas both understand. */
export function css(tint: Tint, alpha = 1): string {
  const s = Math.round(tint.saturation * 100);
  const l = Math.round(tint.lightness * 100);
  return `hsl(${tint.hue.toFixed(1)} ${s}% ${l}% / ${alpha})`;
}

/**
 * Where in the beat **the room** is, now.
 *
 * Two corrections turn the engine's phase into the room's. The world is read
 * more slowly than a frame, so `secondsSince` advances it; and the audio has
 * yet to travel through the output chain, so `latencyMs` pulls it back.
 *
 * The second correction is the one nobody makes. At 128 BPM one beat is 469 ms,
 * so a 20 ms error is 4% of a beat — small, but visible as a crest sitting
 * ahead of the kick, and an interface whose pulse is visibly early is one a DJ
 * stops trusting for phase. `output_latency_ms` is already published in the
 * master snapshot, so being right costs one subtraction.
 *
 * Both decks take the same correction, so two decks in sync still draw one
 * crest — which is the property that actually has to hold. See
 * docs/VISUAL-TECH.md §5.1b.
 */
export function phaseAt(vitality: Vitality, secondsSince: number, latencyMs = 0): number {
  if (!(vitality.pulse_bpm > 0)) return 0;
  const beats = (vitality.pulse_bpm / 60) * (secondsSince - latencyMs / 1000);
  // Not `%`: a negative phase is early in the previous beat, not a negative one.
  return (((vitality.phase + beats) % 1) + 1) % 1;
}
