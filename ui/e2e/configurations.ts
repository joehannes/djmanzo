/**
 * §89's ten workspace configurations, as data.
 *
 * > Create visual regression coverage for major workspace configurations.
 *
 * # What is regressed here, and what is not
 *
 * **Not pixels.** CI installs its own Chromium on `ubuntu-latest`; this
 * container has one pre-installed at a different build. Font rasterisation and
 * subpixel positioning differ between them, so a baseline captured in either
 * place fails in the other for reasons that have nothing to do with djmanzo.
 * A screenshot suite that has to be re-blessed on every run is a suite people
 * stop reading, and one that is re-blessed automatically has stopped being a
 * test.
 *
 * **What is regressed is the thing that actually breaks.** Twice now the
 * controls a DJ performs a mix with have drifted below the fold, and both times
 * a human found it with a screenshot — see `budget.spec.ts`. That budget
 * measures *one* arrangement. §89's contribution is that the same rules have to
 * hold in all ten: every performing control on the first screen, nothing
 * painting outside its deck, no sideways scroll, and nothing thrown while
 * rendering. Those are geometry, they fail for the right reason, and they do
 * not care which machine drew the font.
 *
 * # Two of §89's ten are not arrangements
 *
 * *Watershed* is a surface and *High Contrast* is a theme. They are in the list
 * because §89 puts them there, and they are configurations of the interface
 * even though neither is a deck layout — which is the useful reading: §89 asks
 * for coverage of the ways the interface is *configured*, not only of the ways
 * the decks are arranged.
 */
import type { Density, Focus, SurfacePlacement } from "../src/api";

/** One arrangement, and how to reach it. */
export type Configuration = {
  name: string;
  /** The window it is judged at. Some of these exist *because* of a size. */
  window: { width: number; height: number };
  decks: number;
  density: Density;
  focus: Focus;
  /** Surfaces open, as a saved workspace would name them. */
  surfaces: SurfacePlacement[];
  /** The theme package, when the configuration is about one. */
  theme: string;
};

/** A docked surface, with the fields a workspace carries. */
function dock(
  surface: string,
  where: SurfacePlacement["dock"],
  order = 0,
): SurfacePlacement {
  return { surface, dock: where, order, size: null, collapsed: false, pinned: false };
}

/**
 * The ten, in §89's own order.
 *
 * Sizes are the ones each configuration is *about*: a compact laptop is a
 * height, not a preference, and four decks need width or they are two decks
 * drawn badly.
 */
export const CONFIGURATIONS: Configuration[] = [
  {
    name: "Classic 2 deck",
    window: { width: 1280, height: 800 },
    decks: 2,
    density: "standard",
    focus: "performing",
    surfaces: [],
    theme: "",
  },
  {
    name: "Pro 2 deck",
    window: { width: 1600, height: 1000 },
    decks: 2,
    density: "pro-dense",
    focus: "performing",
    surfaces: [dock("next", "right"), dock("booth", "bottom")],
    theme: "",
  },
  {
    name: "4 deck",
    window: { width: 1600, height: 1000 },
    decks: 4,
    density: "compact",
    focus: "performing",
    surfaces: [],
    theme: "",
  },
  {
    // The one that has caught real defects: least height, most to fit.
    name: "Compact laptop",
    window: { width: 1280, height: 720 },
    decks: 2,
    density: "ultra-dense",
    focus: "performing",
    surfaces: [],
    theme: "",
  },
  {
    name: "Preparation",
    window: { width: 1440, height: 900 },
    decks: 2,
    density: "standard",
    focus: "preparing",
    surfaces: [dock("library", "bottom"), dock("prepare", "right")],
    theme: "",
  },
  {
    name: "Practice",
    window: { width: 1440, height: 900 },
    decks: 2,
    density: "standard",
    focus: "learning",
    surfaces: [dock("practice", "bottom"), dock("pair", "bottom", 1)],
    theme: "pkg-studio",
  },
  {
    name: "Autopilot",
    window: { width: 1440, height: 900 },
    decks: 2,
    density: "standard",
    focus: "supervising",
    surfaces: [dock("booth", "bottom"), dock("assistant", "right")],
    theme: "",
  },
  {
    name: "Club",
    window: { width: 1600, height: 1000 },
    decks: 4,
    density: "compact",
    focus: "performing",
    surfaces: [dock("night", "right"), dock("athand", "right", 1)],
    theme: "pkg-booth",
  },
  {
    name: "Watershed",
    window: { width: 1440, height: 900 },
    decks: 2,
    density: "standard",
    focus: "planning",
    surfaces: [dock("plan", "bottom")],
    theme: "",
  },
  {
    name: "High Contrast",
    window: { width: 1280, height: 800 },
    decks: 2,
    density: "standard",
    focus: "performing",
    surfaces: [],
    theme: "pkg-booth",
  },
];
