import type { SessionContext } from "../../api";
import {
  type ThemePackage,
  type BehaviorModifier,
  type EffectProcessor,
  GeometryCircle,
  GeometryPolygon,
  decorate,
} from "./engine";

export type { ThemePackage } from "./engine";

/**
 * How hard the room is going, as far as anything actually knows.
 *
 * `session` is null until M9 reads the room, so a theme keyed to energy falls
 * back to the loudness it can measure. That keeps the interface moving to the
 * music without claiming to know anything about the night — see
 * `dj_core::context`.
 */
function energyOf(context: SessionContext): number {
  return context.session?.energy ?? context.audio.loudness;
}

// -----------------------------------------------------------------------------
// CORE BEHAVIORS
// -----------------------------------------------------------------------------

export const AudioReactiveStroke: BehaviorModifier = (render, state, perf) => {
  if (perf === "Eco") return render;

  const energy = energyOf(state.context);

  return decorate(render, (path) => ({
    ...path,
    stroke: `hsl(${220 - energy * 180}, 80%, 60%)`,
    strokeWidth: path.strokeWidth + energy * 4,
    style: "transition: stroke 0.1s linear, stroke-width 0.1s linear;",
  }));
};

export const AudioReactiveGlitch: BehaviorModifier = (render, state, perf) => {
  if (perf === "Eco") return render;
  
  const treble = state.context.audio.bands[3];
  if (treble < 0.3) return render; // Only glitch on loud hi-hats/cymbals

  return decorate(render, (path) => ({
    ...path,
    transform: `translate(${(Math.random() - 0.5) * 10}px, ${(Math.random() - 0.5) * 10}px)`,
  }));
};

export const TimeReactivePulse: BehaviorModifier = (render, state, perf) => {
  if (perf === "Eco") return render;
  
  const bass = state.context.audio.bands[0];
  
  // Use CSS transform scale based on bass for hardware accelerated pulsing
  return {
    ...render,
    containerStyle: `${render.containerStyle}; transform: scale(${1 + bass * 0.15}); transform-origin: center; transition: transform 0.05s ease-out;`
  };
};

// -----------------------------------------------------------------------------
// CORE EFFECTS
// -----------------------------------------------------------------------------

export const NeonGlow: EffectProcessor = (render, _state, perf) => {
  if (perf === "Eco" || perf === "Balanced") return render; // Too heavy for Balanced
  
  return {
    ...render,
    containerStyle: `${render.containerStyle}; filter: drop-shadow(0 0 8px var(--accent-2));`
  };
};

export const ChromaticAberration: EffectProcessor = (render, state, perf) => {
  if (perf !== "Ultra") return render;
  
  const treble = state.context.audio.bands[3];
  if (treble < 0.2) return render;

  // Fringes behind the real paths, offset either way. Only the body is
  // fringed: a chromatic ghost of the value indicator is a knob that appears
  // to be pointing three ways at once.
  const fringes = render.paths
    .filter((path) => path.role !== "value")
    .flatMap((path) => [
      {
        ...path,
        stroke: "red",
        style: "mix-blend-mode: screen;",
        transform: `translate(${treble * 5}px, 0)`,
      },
      {
        ...path,
        stroke: "cyan",
        style: "mix-blend-mode: screen;",
        transform: `translate(${-treble * 5}px, 0)`,
      },
    ]);

  return { ...render, paths: [...fringes, ...render.paths] };
};

export const ExclusionBlend: EffectProcessor = (render, _state, perf) => {
  if (perf === "Eco") return render;
  return {
    ...render,
    containerStyle: `${render.containerStyle}; mix-blend-mode: exclusion;`
  };
};

// -----------------------------------------------------------------------------
// CURATED PACKAGES
// -----------------------------------------------------------------------------

export const PkgOrganic: ThemePackage = {
  id: "pkg-organic",
  name: "Organic Base",
  category: "organic",
  geometry: GeometryCircle,
  behaviors: [AudioReactiveStroke, TimeReactivePulse],
  effects: [] // Clean and smooth
};

export const PkgIndustrial: ThemePackage = {
  id: "pkg-industrial",
  name: "Industrial Techno",
  category: "industrial",
  geometry: GeometryPolygon(6), // Hexagons
  behaviors: [AudioReactiveStroke, AudioReactiveGlitch],
  effects: [ExclusionBlend]
};

export const PkgCyber: ThemePackage = {
  id: "pkg-cyber",
  name: "Cyber Trance",
  category: "cyber",
  geometry: GeometryCircle,
  behaviors: [AudioReactiveStroke, TimeReactivePulse],
  effects: [NeonGlow, ChromaticAberration]
};

// The global registry of all installed packages
export const themePackages = [PkgOrganic, PkgIndustrial, PkgCyber];
