import type { Snippet } from "svelte";
import type { Face, PadRole } from "./faces";

/**
 * What a control looks like right now.
 *
 * Deliberately carries no audio. The live signal reaches a control through CSS
 * custom properties instead — see `audiovars.svelte.ts` — so that geometry
 * recomputes when the *control* changes rather than sixty times a second.
 */
export interface KnobState {
  value: number;
  min: number;
  max: number;
  normalized: number; // 0 to 1
  angle: number; // -135 to 135
  dragging: boolean;
  disabled: boolean;
  size: number;
  label?: string;
  /** §114: the curve its face draws, where the knob shapes the sound. */
  face?: Face;
  /**
   * §114: where the knob rests, 0..1 along its sweep — the filter's centre,
   * an EQ's unity. The arc fills from here, so a knob at rest shows none.
   * Absent is the bottom of the sweep.
   */
  origin?: number;
}

export interface FaderState {
  value: number;
  min: number;
  max: number;
  normalized: number; // 0 to 1
  dragging: boolean;
  disabled: boolean;
  width: number;
  height: number;
  orientation: "vertical" | "horizontal";
  label?: string;
  /** §114: where the fader rests, 0..1 — the pitch fader's zero is its middle. */
  origin?: number;
}

export interface PadState {
  active: boolean;
  pressed: boolean;
  disabled: boolean;
  width: number;
  height: number;
  label?: string;
  /** §114: what the pad does, which decides the colour it lights in. */
  does?: PadRole;
}

/**
 * The ThemeGrammar is a collection of rendering snippets.
 * A Theme module must export exactly these snippets.
 */
export interface ThemeGrammar {
  knob: Snippet<[KnobState]>;
  fader: Snippet<[FaderState]>;
  pad: Snippet<[PadState]>;
}
