import {
  type ThemePackage,
  type BehaviorModifier,
  type EffectProcessor,
  GeometryCircle,
  GeometryPolygon,
  decorate,
} from "./engine";

export type { ThemePackage } from "./engine";

// -----------------------------------------------------------------------------
// A note on how these move
// -----------------------------------------------------------------------------
//
// None of the behaviours below read the audio. They emit *static* CSS that
// refers to the custom properties `audiovars.svelte.ts` writes on the root
// element once per snapshot.
//
// That is the whole performance story. The first version read `SessionContext`
// here and rewrote `stroke` and `stroke-width` on every path of every control
// sixty times a second — a few hundred animating DOM elements, which is the
// exact workload the living-interface benchmark measured collapsing to 18.6 fps
// on the no-GPU floor. Because the output no longer varies with the audio, the
// pipeline's result is stable between user gestures, Svelte writes nothing, and
// the motion happens in the style system off one property change.

// -----------------------------------------------------------------------------
// CORE BEHAVIORS
// -----------------------------------------------------------------------------

export const AudioReactiveStroke: BehaviorModifier = (render, _state, perf) => {
  if (perf === "Eco") return render;

  return decorate(render, (path) => ({
    ...path,
    stroke: "hsl(var(--audio-hue, 220) 80% 60%)",
    // `stroke-width` is an attribute, so the CSS custom property has to reach it
    // through the `stroke-width` *property*, which wins over the attribute.
    style: "stroke-width: calc(var(--sw, 2px) + var(--audio-energy, 0) * 4px); transition: stroke 0.1s linear;",
  }));
};

/**
 * Jitter on the hats.
 *
 * A container transform rather than a per-path one, and an amplitude rather
 * than a threshold: the animation always runs, and its size is
 * `var(--audio-treble)`, so it is invisible until there is treble and needs no
 * per-frame decision in JavaScript. The first version called `Math.random()`
 * once per path per frame, which meant new `transform` attributes on every path
 * of every control on screen.
 */
export const AudioReactiveGlitch: BehaviorModifier = (render, _state, perf) => {
  if (perf === "Eco") return render;

  return {
    ...render,
    containerStyle: `${render.containerStyle}; animation: djmanzo-glitch 0.12s steps(2, end) infinite; --glitch: calc(var(--audio-treble, 0) * 4px);`,
  };
};

/** Swelling on the kick. `transform` and nothing else, so the compositor has it. */
export const TimeReactivePulse: BehaviorModifier = (render, _state, perf) => {
  if (perf === "Eco") return render;

  return {
    ...render,
    containerStyle: `${render.containerStyle}; transform: scale(calc(1 + var(--audio-bass, 0) * 0.15)); transform-origin: center; transition: transform 0.05s ease-out;`,
  };
};

// -----------------------------------------------------------------------------
// CORE EFFECTS
// -----------------------------------------------------------------------------

export const NeonGlow: EffectProcessor = (render, _state, perf) => {
  if (perf === "Eco" || perf === "Balanced") return render; // Too heavy for Balanced

  return {
    ...render,
    containerStyle: `${render.containerStyle}; filter: drop-shadow(0 0 8px var(--accent-2));`,
  };
};

/**
 * An RGB fringe that widens with the treble.
 *
 * The fringes are always present and always the same paths; only their offset
 * moves, and it moves in CSS. Emitting them conditionally changed the path
 * count between frames, which is the most expensive thing a list can do.
 *
 * Only the body is fringed: a chromatic ghost of the value indicator is a knob
 * that appears to point three ways at once.
 */
export const ChromaticAberration: EffectProcessor = (render, _state, perf) => {
  if (perf !== "Ultra") return render;

  const fringes = render.paths
    .filter((path) => path.role !== "value")
    .flatMap((path) => [
      {
        ...path,
        stroke: "red",
        style: "mix-blend-mode: screen; translate: calc(var(--audio-treble, 0) * 5px) 0;",
      },
      {
        ...path,
        stroke: "cyan",
        style: "mix-blend-mode: screen; translate: calc(var(--audio-treble, 0) * -5px) 0;",
      },
    ]);

  return { ...render, paths: [...fringes, ...render.paths] };
};

export const ExclusionBlend: EffectProcessor = (render, _state, perf) => {
  if (perf === "Eco") return render;
  return {
    ...render,
    containerStyle: `${render.containerStyle}; mix-blend-mode: exclusion;`,
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
