import { performance, type ResolvedPerformance } from "../../performance.svelte";
import type { KnobState, FaderState, PadState } from "../grammar";

// The fundamental render instruction set for any SVG control
export interface SvgRenderState {
  paths: SvgPath[];
  containerStyle: string;
}

export interface SvgPath {
  d: string;
  fill: string;
  stroke: string;
  strokeWidth: number;
  /** CSS inline styles — filters, transitions. */
  style?: string;
  transform?: string;
  /**
   * What this path is for.
   *
   * `"value"` marks the layer that says where the control is set. Behaviours
   * and effects must leave those alone: a theme is allowed to restyle a
   * control's body all it likes, and is never allowed to recolour the one part
   * a DJ reads the setting from. Without this, `AudioReactiveStroke` repainted
   * the indicator to match the body and the knob became unreadable again by a
   * different route.
   */
  role?: "body" | "value";
}

/** Map only the paths a theme may restyle. */
export function decorate(
  render: SvgRenderState,
  paint: (path: SvgPath) => SvgPath,
): SvgRenderState {
  return {
    ...render,
    paths: render.paths.map((path) => (path.role === "value" ? path : paint(path))),
  };
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
//
// Every generator has one job before it has any other: draw where the control
// is set. The first version drew a bare circle for a knob and a bare rectangle
// for a fader, ignoring `normalized` entirely — so after `Deck.svelte` replaced
// its HTML range inputs with these, a DJ could not see where the volume, EQ,
// filter or pitch were. A control that cannot show its own value is not a
// control. Everything below therefore emits the position first, and the theme's
// behaviours and effects decorate it afterwards.

/** Sweep of a knob's arc, in degrees, centred at the top. Hardware convention. */
const KNOB_SWEEP = 270;

/** Everything is drawn in a 0–100 box and scaled by the renderer's viewBox. */
const BOX = 100;

function polar(cx: number, cy: number, r: number, degrees: number) {
  const radians = ((degrees - 90) * Math.PI) / 180;
  return { x: cx + r * Math.cos(radians), y: cy + r * Math.sin(radians) };
}

/** An SVG arc between two angles on a circle. */
function arc(cx: number, cy: number, r: number, from: number, to: number): string {
  const start = polar(cx, cy, r, from);
  const end = polar(cx, cy, r, to);
  const large = Math.abs(to - from) > 180 ? 1 : 0;
  const sweep = to >= from ? 1 : 0;
  return `M ${start.x.toFixed(2)} ${start.y.toFixed(2)} A ${r} ${r} 0 ${large} ${sweep} ${end.x.toFixed(2)} ${end.y.toFixed(2)}`;
}

function isKnob(state: KnobState | FaderState | PadState): state is KnobState {
  return "angle" in state;
}

function isFader(state: KnobState | FaderState | PadState): state is FaderState {
  return "orientation" in state;
}

/**
 * Knob, fader and pad share one shape family per theme, so the value layer is
 * written once here and the theme only chooses the outline.
 *
 * `body` is the theme's own silhouette — a circle, a hexagon, whatever. The
 * indicator drawn over it is the same in every theme, because "where is this
 * set" must not be a thing a theme can decide to omit.
 */
function withValue(
  state: KnobState | FaderState | PadState,
  body: SvgRenderState,
): SvgRenderState {
  const paths = [...body.paths];

  if (isKnob(state)) {
    const centre = BOX / 2;
    const radius = 40;
    const from = -KNOB_SWEEP / 2;
    const to = KNOB_SWEEP / 2;
    const at = from + KNOB_SWEEP * clamp01(state.normalized);

    // The unfilled track, so the range is visible even at zero.
    paths.push({
      d: arc(centre, centre, radius, from, to),
      fill: "none",
      stroke: "var(--panel-raised)",
      strokeWidth: 6,
      role: "value",
    });
    // How far along it is. Skipped at the very bottom, where an arc of zero
    // length renders as a stray dot in some engines.
    if (at - from > 0.5) {
      paths.push({
        d: arc(centre, centre, radius, from, at),
        fill: "none",
        stroke: "var(--accent-2)",
        strokeWidth: 6,
        role: "value",
      });
    }
    // The pointer. A knob is read by where it points long before it is read by
    // how much of its arc is filled.
    const inner = polar(centre, centre, radius - 14, at);
    const outer = polar(centre, centre, radius + 2, at);
    paths.push({
      d: `M ${inner.x.toFixed(2)} ${inner.y.toFixed(2)} L ${outer.x.toFixed(2)} ${outer.y.toFixed(2)}`,
      fill: "none",
      stroke: "var(--text)",
      strokeWidth: 4,
      role: "value",
    });
    return { ...body, paths };
  }

  if (isFader(state)) {
    const along = clamp01(state.normalized);
    if (state.orientation === "vertical") {
      // Zero at the bottom, which is where a channel fader's zero is.
      const top = 10 + (BOX - 20) * (1 - along);
      paths.push({
        d: `M 40 ${top.toFixed(2)} H 60 V 90 H 40 Z`,
        fill: "var(--accent-2)",
        stroke: "none",
        strokeWidth: 0,
        role: "value",
      });
      paths.push({
        d: `M 26 ${(top - 5).toFixed(2)} H 74 V ${(top + 5).toFixed(2)} H 26 Z`,
        fill: "var(--text)",
        stroke: "var(--border)",
        strokeWidth: 2,
        role: "value",
      });
    } else {
      const right = 10 + (BOX - 20) * along;
      paths.push({
        d: `M 10 40 H ${right.toFixed(2)} V 60 H 10 Z`,
        fill: "var(--accent-2)",
        stroke: "none",
        strokeWidth: 0,
        role: "value",
      });
      paths.push({
        d: `M ${(right - 5).toFixed(2)} 26 H ${(right + 5).toFixed(2)} V 74 H ${(right - 5).toFixed(2)} Z`,
        fill: "var(--text)",
        stroke: "var(--border)",
        strokeWidth: 2,
        role: "value",
      });
    }
    return { ...body, paths };
  }

  // A pad has two states rather than a position, and they are read by fill.
  if (state.active || state.pressed) {
    paths.push({
      d: "M 4 4 H 96 V 96 H 4 Z",
      fill: state.pressed ? "var(--accent)" : "var(--accent-2)",
      stroke: "none",
      strokeWidth: 0,
      role: "value",
    });
  }
  return { ...body, paths };
}

function clamp01(value: number): number {
  return Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0;
}

/** The outline every theme starts from, before its own silhouette. */
function frame(state: KnobState | FaderState | PadState): SvgRenderState {
  if (isKnob(state)) {
    return {
      containerStyle: "",
      paths: [
        {
          d: arc(BOX / 2, BOX / 2, 46, -180, 179.9) + " Z",
          fill: "var(--panel)",
          stroke: "var(--border)",
          strokeWidth: 2,
        },
      ],
    };
  }
  // A fader is a slot the handle runs along; a pad is the whole surface. They
  // are not the same rectangle, and drawing a pad with the fader's slot is how
  // the transport pads ended up as thin horizontal bars.
  const d = isFader(state)
    ? state.orientation === "vertical"
      ? "M 38 8 H 62 V 92 H 38 Z"
      : "M 8 38 H 92 V 62 H 8 Z"
    : "M 4 4 H 96 V 96 H 4 Z";

  return {
    containerStyle: "",
    paths: [{ d, fill: "var(--panel)", stroke: "var(--border)", strokeWidth: 2 }],
  };
}

export const GeometryCircle: GeometryGenerator = (state) => withValue(state, frame(state));

export const GeometryPolygon =
  (sides: number): GeometryGenerator =>
  (state) => {
    if (!isKnob(state)) return GeometryCircle(state);

    const centre = BOX / 2;
    const radius = 46;
    let d = "";
    for (let i = 0; i < sides; i++) {
      const angle = (Math.PI * 2 * i) / sides - Math.PI / 2;
      const x = centre + radius * Math.cos(angle);
      const y = centre + radius * Math.sin(angle);
      d += `${i === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)} `;
    }
    d += "Z";

    return withValue(state, {
      containerStyle: "",
      paths: [{ d, fill: "var(--panel)", stroke: "var(--border)", strokeWidth: 2 }],
    });
  };
