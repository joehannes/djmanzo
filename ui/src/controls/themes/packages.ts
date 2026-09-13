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
//
// Organised by the **room**, not by the aesthetic. A DJ picking a theme is not
// shopping for a look; they are somewhere, and the light in that somewhere
// decides what they can read. Three questions settle every choice below:
//
// - **How much light is in the room?** A screen tuned for a dark booth is
//   invisible on a terrace at six, and one tuned for daylight destroys the
//   night vision needed to find anything on the actual mixer.
// - **How far away are the eyes?** At a desk, a metre. In a booth, leaning back
//   with a hand on a platter, further -- so contrast matters more than density.
// - **How much can move?** Motion costs frames, and it competes for attention
//   with the one thing that must never be missed: which deck is playing.
//
// Every theme states its own answer in `when`, which the type requires, so a
// theme cannot ship without saying what it is for.

export const PkgDaylight: ThemePackage = {
  id: "pkg-daylight",
  name: "Daylight",
  category: "minimalist",
  setting: "daylight",
  when: "Preparing a set by a window, or an afternoon set outdoors.",
  geometry: GeometryCircle,
  // Nothing reactive at all. In bright light the eye is already working hard,
  // and a control that shimmers is one more thing competing with the sun.
  behaviors: [],
  effects: [],
};

export const PkgStudio: ThemePackage = {
  id: "pkg-studio",
  name: "Studio",
  category: "organic",
  setting: "home",
  when: "Long evenings at a desk. Easy on the eyes for hours at a time.",
  geometry: GeometryCircle,
  // Motion, but only the slow kind. Over a four-hour session anything faster
  // than a swell becomes something to look away from.
  behaviors: [TimeReactivePulse],
  effects: [],
};

export const PkgOrganic: ThemePackage = {
  id: "pkg-organic",
  name: "Organic Base",
  category: "organic",
  setting: "home",
  when: "The default. Calm, green, and readable in most rooms.",
  geometry: GeometryCircle,
  behaviors: [AudioReactiveStroke, TimeReactivePulse],
  effects: [], // Clean and smooth
};

export const PkgBooth: ThemePackage = {
  id: "pkg-booth",
  name: "Booth",
  category: "minimalist",
  setting: "booth",
  when: "Playing in the dark. Highest contrast, least motion, nothing to miss.",
  geometry: GeometryCircle,
  // Deliberately still. Mid-set the only question a screen has to answer at a
  // glance is which deck is playing and where the playhead is; a control that
  // pulses is a control that has to be looked at twice.
  behaviors: [],
  effects: [],
};

export const PkgIndustrial: ThemePackage = {
  id: "pkg-industrial",
  name: "Industrial Techno",
  category: "industrial",
  setting: "venue",
  when: "A dark room, hard music, and a screen that should look like the music.",
  geometry: GeometryPolygon(6), // Hexagons
  behaviors: [AudioReactiveStroke, AudioReactiveGlitch],
  effects: [ExclusionBlend],
};

export const PkgCyber: ThemePackage = {
  id: "pkg-cyber",
  name: "Cyber Trance",
  category: "cyber",
  setting: "venue",
  when: "Peak time. Everything moves; needs the machine to have it to spare.",
  geometry: GeometryCircle,
  behaviors: [AudioReactiveStroke, TimeReactivePulse],
  effects: [NeonGlow, ChromaticAberration],
};

export const PkgSunset: ThemePackage = {
  id: "pkg-sunset",
  name: "Sunset",
  category: "organic",
  setting: "venue",
  when: "Golden hour on a terrace, when the room is neither light nor dark.",
  geometry: GeometryCircle,
  behaviors: [TimeReactivePulse],
  effects: [],
};

/**
 * §32's Watershed Living.
 *
 * > The existing watershed metaphor should become a theme/world pack, not the
 * > only possible identity. It is a good visual language. It must not
 * > constrain DJs who do not want metaphors.
 *
 * Both halves of that are the design. The watershed was a switch in the status
 * strip beside the themes rather than one of them, which made it a mode — and
 * a mode is exactly the thing §32 says it should stop being. It is a theme
 * now: `dj_app::theme` marks this row as the one that wears the world, and
 * choosing it opens the watershed.
 *
 * **And only opens it.** The switch is still there and still closes it, and no
 * other theme touches it, so a DJ who wants the colours without the metaphor
 * has them and a DJ who wants the metaphor under Booth keeps it. A theme that
 * *enforced* its world would be the constraint §32 forbids in the same
 * sentence that asks for the pack.
 *
 * The controls themselves are deliberately quiet under it. Everything moving
 * on screen here should be the water; a knob that also pulsed would be a second
 * thing asking to be watched.
 */
export const PkgWatershed: ThemePackage = {
  id: "pkg-watershed",
  name: "Watershed Living",
  category: "organic",
  setting: "venue",
  when: "The mix drawn as moving water. Choosing it opens the watershed.",
  geometry: GeometryCircle,
  behaviors: [TimeReactivePulse],
  effects: [],
};

/**
 * Every installed theme, in the order the picker lists them.
 *
 * Grouped by setting rather than sorted, so the list reads as a walk through a
 * day: desk, evening, booth, room.
 */

// -----------------------------------------------------------------------------
// §32's remaining eight
// -----------------------------------------------------------------------------
//
// The list §32 gives is a list of *rooms and nights*, not of aesthetics, which
// is why each of these says `when` in terms of where a DJ is standing. Three of
// them had a reason recorded for not shipping and two of those reasons still
// stand and are unchanged: High Contrast is an override the stylesheet already
// applies over every theme, and Minimal is a density §5 already fits to the
// window. The rest were "a palette somebody has to design", which is what these
// are.

export const PkgClub: ThemePackage = {
  id: "pkg-club",
  name: "Club",
  category: "minimalist",
  setting: "venue",
  when: "A dark room that plays everything, rather than one genre loudly.",
  geometry: GeometryCircle,
  // Motion, but not Cyber's. A club screen is read in glances between mixes.
  behaviors: [AudioReactiveStroke],
  effects: [],
};

export const PkgFestival: ThemePackage = {
  id: "pkg-festival",
  name: "Festival",
  category: "organic",
  setting: "daylight",
  when: "A main stage, outdoors, with the sun still on the screen.",
  geometry: GeometryCircle,
  // Nothing reactive, for Daylight's reason: in bright light the eye is
  // already working, and a control that shimmers competes with the sun.
  behaviors: [],
  effects: [],
};

export const PkgCaribbean: ThemePackage = {
  id: "pkg-caribbean",
  name: "Caribbean",
  category: "organic",
  setting: "daylight",
  when: "A beach bar in the afternoon, where the screen is in the open.",
  geometry: GeometryCircle,
  behaviors: [TimeReactivePulse],
  effects: [],
};

export const PkgLatin: ThemePackage = {
  id: "pkg-latin",
  name: "Latin",
  category: "organic",
  setting: "venue",
  when: "A Latin night: warm, dark enough for a room, and never clinical.",
  geometry: GeometryCircle,
  behaviors: [AudioReactiveStroke, TimeReactivePulse],
  effects: [],
};

export const PkgWedding: ThemePackage = {
  id: "pkg-wedding",
  name: "Wedding",
  category: "minimalist",
  setting: "venue",
  when: "A room with tablecloths, where the guests can see your screen.",
  geometry: GeometryCircle,
  // Still. A screen visible to a hundred guests during a first dance should
  // not be the thing moving in the corner of the photographs.
  behaviors: [],
  effects: [],
};

export const PkgLounge: ThemePackage = {
  id: "pkg-lounge",
  name: "Lounge",
  category: "minimalist",
  setting: "venue",
  when: "Low light and slow music, where the screen should recede.",
  geometry: GeometryCircle,
  behaviors: [TimeReactivePulse],
  effects: [],
};

export const PkgScratch: ThemePackage = {
  id: "pkg-scratch",
  name: "Scratch",
  category: "minimalist",
  setting: "booth",
  when: "Hands on the records, where the screen is glanced at and not read.",
  geometry: GeometryCircle,
  // Deliberately still, and for a stronger reason than Booth's: a turntablist
  // looks up for a fraction of a second, and anything that moves costs part of
  // that fraction.
  behaviors: [],
  effects: [],
};

export const PkgStemLab: ThemePackage = {
  id: "pkg-stemlab",
  name: "Stem Lab",
  category: "minimalist",
  setting: "home",
  when: "Working on the four parts of a record, where telling them apart is the job.",
  geometry: GeometryCircle,
  behaviors: [TimeReactivePulse],
  effects: [],
};

export const themePackages = [
  PkgDaylight,
  PkgStudio,
  PkgOrganic,
  PkgBooth,
  PkgSunset,
  PkgIndustrial,
  PkgCyber,
  PkgWatershed,
  PkgClub,
  PkgFestival,
  PkgCaribbean,
  PkgLatin,
  PkgWedding,
  PkgLounge,
  PkgScratch,
  PkgStemLab,
];
