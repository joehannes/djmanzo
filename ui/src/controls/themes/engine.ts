import type { SessionContext } from "../../api";
import { performance, type ResolvedPerformance } from "../../performance.svelte";
import type { KnobState, FaderState, PadState } from "../grammar";

// The fundamental render instruction set for any SVG control
export interface SvgRenderState {
  paths: Array<{
    d: string;
    fill: string;
    stroke: string;
    strokeWidth: number;
    style?: string; // CSS inline styles (filters, transitions)
    transform?: string;
  }>;
  containerStyle: string;
}

// 1. Geometry Layer: Calculates the raw math for the shape
export type GeometryGenerator = (state: KnobState | FaderState | PadState) => SvgRenderState;

// 2. Behavior Layer: Modifies the Geometry dynamically based on context/audio
export type BehaviorModifier = (
  renderState: SvgRenderState,
  controlState: KnobState | FaderState | PadState,
  perf: ResolvedPerformance
) => SvgRenderState;

// 3. Effect Layer: GPU-accelerated post-processing and CSS magic
export type EffectProcessor = (
  renderState: SvgRenderState,
  controlState: KnobState | FaderState | PadState,
  perf: ResolvedPerformance
) => SvgRenderState;

export interface ThemePackage {
  id: string;
  name: string;
  category: "industrial" | "organic" | "cyber" | "minimalist";
  geometry: GeometryGenerator;
  behaviors: BehaviorModifier[];
  effects: EffectProcessor[];
}

export function executeThemePipeline(
  pkg: ThemePackage,
  state: KnobState | FaderState | PadState
): SvgRenderState {
  const perf = performance.resolved;
  
  // 1. Generate Base Geometry
  let renderState = pkg.geometry(state);

  // 2. Apply Behaviors (Skip heavy behaviors on Eco mode if needed, handled inside modifiers)
  for (const behavior of pkg.behaviors) {
    renderState = behavior(renderState, state, perf);
  }

  // 3. Apply Effects (Instantly skipped on Eco mode)
  if (perf !== "Eco") {
    for (const effect of pkg.effects) {
      renderState = effect(renderState, state, perf);
    }
  }

  return renderState;
}

// -----------------------------------------------------------------------------
// CORE GENERATORS
// -----------------------------------------------------------------------------

export const GeometryCircle: GeometryGenerator = (state) => {
  if ('angle' in state) {
    // Knob
    const r = 40;
    return {
      containerStyle: "",
      paths: [
        { d: `M 50 10 A ${r} ${r} 0 1 1 49.9 10 Z`, fill: "var(--panel)", stroke: "var(--border)", strokeWidth: 3 },
      ]
    };
  } else if ('orientation' in state) {
    // Fader
    return {
      containerStyle: "",
      paths: [
        { d: `M 10 10 h 20 v ${state.height - 20} h -20 Z`, fill: "var(--panel)", stroke: "var(--border)", strokeWidth: 2 }
      ]
    };
  } else {
    // Pad
    return {
      containerStyle: "",
      paths: [
        { d: `M 10 10 h ${state.width - 20} v ${state.height - 20} h -${state.width - 20} Z`, fill: "var(--panel)", stroke: "var(--border)", strokeWidth: 2 }
      ]
    };
  }
};

export const GeometryPolygon = (sides: number): GeometryGenerator => (state) => {
  if ('angle' in state) {
    let d = "";
    const r = 40;
    for (let i = 0; i < sides; i++) {
      const angle = (Math.PI * 2 * i) / sides - Math.PI / 2;
      const x = 50 + r * Math.cos(angle);
      const y = 50 + r * Math.sin(angle);
      d += i === 0 ? `M ${x} ${y} ` : `L ${x} ${y} `;
    }
    d += "Z";
  
    return {
      containerStyle: "",
      paths: [
        { d, fill: "var(--panel)", stroke: "var(--border)", strokeWidth: 2 }
      ]
    };
  }
  return GeometryCircle(state);
};
