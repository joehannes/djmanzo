import type { Snippet } from "svelte";

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
}

export interface PadState {
  active: boolean;
  pressed: boolean;
  disabled: boolean;
  width: number;
  height: number;
  label?: string;
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
