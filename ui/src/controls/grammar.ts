import type { Snippet } from "svelte";
import type { SessionContext } from "../api";

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
  context: SessionContext;
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
  context: SessionContext;
}

export interface PadState {
  active: boolean;
  pressed: boolean;
  disabled: boolean;
  width: number;
  height: number;
  label?: string;
  context: SessionContext;
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
