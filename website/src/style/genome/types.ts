/**
 * The style genome: what a kind of music looks, sounds, dresses and moves
 * like, as the website dresses itself in it (DIRECTIVE §133).
 *
 * A node describes a *scene*, never a person. Its roots and places are the
 * music's history, told as such; nothing here guesses who a visitor is from
 * where they are or what they like. Every node keeps the good parts of its
 * culture and offers healthy complements beside them — dance classes,
 * running clubs, morning raves, crate digging — and none of it celebrates
 * excess.
 *
 * Nodes form a tree: family → genre → subgenre. A child names only what it
 * changes, and `resolveStyle` walks from the root down, so "Electro swing"
 * is EDM's rig seen through Art Deco rather than a style of its own from
 * nothing — which is exactly what the owner asked for.
 */

/** The stages of the Signal Path a style dresses (WEBSITE.md). */
export type StageId = "silence" | "groove" | "needle" | "cue" | "confluence" | "voice" | "amp" | "cone" | "floor" | "afterglow";

/** How a style moves: its pulse, and the easing its motion uses. */
export interface Motion {
  /** Four on the floor, a swung beat, half time, the dembow's 3+3+2, a straight pop pulse, or none at all. */
  pulse: "four" | "swing" | "half" | "tresillo" | "straight" | "free";
  /** A CSS/GSAP easing curve: what the style's motion feels like arriving. */
  ease: string;
  /** 0 still … 1 restless: how much the painting moves when nothing is asked of it. */
  energy: number;
}

/** The painterly treatment (the Kuwahara pass and what sits over it). */
export interface Brush {
  /** Kuwahara radius at rest, in pixels at 1080p. */
  radius: number;
  /** Paper or film grain, 0 … 1. */
  grain: number;
  /** Which way strokes run. */
  flow: "radial" | "horizontal" | "vertical" | "swirl" | "cross";
}

export interface Palette {
  /** The night: the darkest colour, the background. */
  night: string;
  /** The day version of the background. */
  day: string;
  /** Text on the night. */
  ink: string;
  /** Two or three accents, the first the strongest. */
  accents: string[];
  /** The light sources: what glows. */
  glow: string;
}

/** A complement: something good to do beside the music, and why it fits. */
export interface Complement {
  what: string;
  why: string;
}

/** Everything a style carries. Every field is inherited unless a node changes it. */
export interface StyleTokens {
  id: string;
  name: string;
  /** The path from the family down, ids. */
  lineage: string[];
  sound: {
    /** Typical tempo range. */
    bpm: [number, number];
    /** Instruments and sounds that say "this one". */
    signature: string[];
    /** How a track is built. */
    structure: string;
  };
  /** Flyers, sleeves, signage: where its pictures come from. */
  visual: string[];
  /** Display and text type: Google Fonts families. */
  type: { display: string; text: string };
  palette: Palette;
  light: string[];
  decor: string[];
  materials: string[];
  fashion: string[];
  dance: string[];
  /** How the room feels. */
  mood: string[];
  /** Values and rituals worth keeping. */
  values: string[];
  /** Where it comes from, as history. */
  roots: { places: string[]; story: string };
  complements: Complement[];
  /** How its DJs mix: what a stage of the Signal Path should show of djmanzo for it. */
  craft: string[];
  /** The djmanzo theme package and skin that suit it (`ui/src/controls/themes/packages.ts`, `ui/src/skins/skins.ts`). */
  app: { theme: string; skin: string | null };
  motion: Motion;
  brush: Brush;
  /** The set piece each stage of the Signal Path becomes in this style. */
  scene: Record<StageId, string>;
  /** Search words for photographs of the scene (never of identifiable people without consent). */
  photos: string[];
  /** The tone the copy and the guide take. */
  voice: string;
  /** Where the facts above come from. */
  sources: string[];
}

/** Recursively optional, but arrays and tuples are replaced whole. */
export type Overrides<T> = {
  [K in keyof T]?: T[K] extends unknown[] ? T[K] : T[K] extends object ? Overrides<T[K]> : T[K];
};

export interface GenomeNode {
  id: string;
  name: string;
  /** The node it inherits from; null for the root. */
  parent: string | null;
  /** Other nodes it also belongs under, for finding it — inheritance follows `parent` only. */
  also?: string[];
  /** What it changes. The root sets everything. */
  set: Overrides<Omit<StyleTokens, "id" | "name" | "lineage">>;
}
