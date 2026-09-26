/**
 * §121: a theme, worn whole or in parts.
 *
 * > the system is to be extended into package that can be applied partially:
 * > the theme; effect skinning/parts of the theme that are artistic and apply
 * > to specific parts functionally ...; interactive skinning (effects/partial
 * > skins/theme items that apply on specific events); any theme can be
 * > selected in the basic or more parts/fully
 *
 * A theme package already had its layers — a palette, the shape its
 * controls are cut in, behaviours that move them with the music, and effects
 * drawn over them (`engine.ts`) — but they only came as one. Here each layer
 * past the palette is a **part** the DJ can take from the chosen theme, from
 * another theme, or leave off:
 *
 * - **shapes** — the silhouette of every knob, fader, pad and jog wheel;
 * - **art** — the effects drawn over them: glow, a split of colour, a blend;
 * - **motion** — the behaviours that move them with the audio and the clock;
 * - **events** — a skin the window wears while something is happening that
 *   the DJ must not lose sight of: a recording running, the autopilot mixing,
 *   a record about to run out with nothing after it.
 *
 * The palette is always the chosen theme's. It is the one part a DJ picks a
 * theme *for* — the room it is meant for — and everything measured about a
 * theme (text on a panel, the rim of a region) is measured on it.
 *
 * **Basic, Parts, Full** are three places on the same dial: Basic is the
 * palette and nothing else, plain round controls standing still; Full is
 * every part from the chosen theme, as a theme was before; Parts is any mix
 * in between. They are not three code paths — `compose` is the only one.
 */
import { GeometryCircle, type ThemePackage } from "./engine";
import { themePackages } from "./packages";

export type PartName = "shapes" | "art" | "motion" | "events";

/**
 * Where a part comes from: the chosen theme, nowhere, or the theme with this
 * id.
 */
export type PartChoice = "base" | "none" | string;

export type Parts = Record<PartName, PartChoice>;

/** How much of the chosen theme is worn. */
export type Amount = "basic" | "parts" | "full";

/** The parts, in the order the picker lists them, each saying what it is. */
export const PARTS: { name: PartName; title: string; about: string; off: string }[] = [
  {
    name: "shapes",
    title: "Shapes",
    about: "The outline of every knob, fader, pad and jog wheel.",
    off: "Plain circles",
  },
  {
    name: "art",
    title: "Art",
    about: "What is drawn over the controls: a glow, split colour, a blend.",
    off: "None",
  },
  {
    name: "motion",
    title: "Motion",
    about: "The controls moving with the music and the clock.",
    off: "Still",
  },
  {
    name: "events",
    title: "Events",
    about: "The window's edge while a recording runs, the autopilot mixes, or a record runs out.",
    off: "None",
  },
];

/** Every part from the chosen theme: a theme as it always was. */
export const FULL: Parts = { shapes: "base", art: "base", motion: "base", events: "base" };

/** The palette and nothing else. */
export const BASIC: Parts = { shapes: "none", art: "none", motion: "none", events: "none" };

/**
 * Where "Parts" starts from when a DJ turns to it from Basic or Full: the
 * chosen theme's shapes and its events, without the art and the motion — the
 * theme recognisable, and nothing on the controls moving that is not the
 * music's own meters.
 */
export const SOME: Parts = { shapes: "base", art: "none", motion: "none", events: "base" };

const same = (a: Parts, b: Parts) => PARTS.every(({ name }) => a[name] === b[name]);

export function amountOf(parts: Parts): Amount {
  if (same(parts, FULL)) return "full";
  if (same(parts, BASIC)) return "basic";
  return "parts";
}

/**
 * How an event is drawn, by the kind of theme it comes from.
 *
 * - `line` — a thin rule round the window: the minimal themes.
 * - `glow` — a soft light from the edge in: the organic ones.
 * - `flash` — hazard bands along the top and bottom: industrial.
 * - `glitch` — a rule with a second colour torn off it: cyber.
 *
 * Only a record running out moves, and no faster than once a second; a
 * recording or the autopilot is held still, because they last all night and
 * a window that pulses for two hours is shouting.
 */
export type EventStyle = "line" | "glow" | "flash" | "glitch";

export const EVENT_STYLES: Record<EventStyle, string> = {
  line: "a thin line",
  glow: "a soft glow",
  flash: "hazard bands",
  glitch: "a torn edge",
};

export function eventStyleOf(pkg: ThemePackage): EventStyle {
  switch (pkg.category) {
    case "organic":
      return "glow";
    case "industrial":
      return "flash";
    case "cyber":
      return "glitch";
    default:
      return "line";
  }
}

/** A theme as worn: its package with the chosen parts, and its event skin. */
export interface Worn {
  pkg: ThemePackage;
  events: EventStyle | null;
}

/**
 * The chosen theme with each part taken from where the DJ said.
 *
 * The result keeps the chosen theme's id, name and setting, because those
 * are what the palette, the swatch and §31's adaptation key on: wearing
 * Industrial's hexagons on Organic's colours is still Organic.
 *
 * A part naming a theme that no longer exists comes from the chosen one,
 * rather than from nowhere: a saved choice outliving a theme should not strip
 * the controls bare.
 */
export function compose(
  base: ThemePackage,
  parts: Parts,
  all: readonly ThemePackage[] = themePackages,
): Worn {
  const from = (choice: PartChoice): ThemePackage | null =>
    choice === "none" ? null : choice === "base" ? base : (all.find((pkg) => pkg.id === choice) ?? base);
  const shapes = from(parts.shapes);
  const art = from(parts.art);
  const motion = from(parts.motion);
  const events = from(parts.events);
  const worn =
    shapes === base && art === base && motion === base
      ? base
      : {
          ...base,
          geometry: shapes?.geometry ?? GeometryCircle,
          effects: art?.effects ?? [],
          behaviors: motion?.behaviors ?? [],
        };
  return { pkg: worn, events: events ? eventStyleOf(events) : null };
}

/**
 * The themes worth offering for one part: the first theme of each distinct
 * kind, so the list is a choice between different things rather than twenty
 * names for the same circle. A theme with nothing in that part — no effects,
 * no behaviours — is what "off" already is, and is not listed.
 */
export function offered(part: PartName, all: readonly ThemePackage[] = themePackages): ThemePackage[] {
  const seen = new Set<unknown>();
  // Layers are functions; two themes with the same ones share them, so a
  // list of which is a fair name for what a theme adds.
  const known = new Map<unknown, number>();
  const which = (layers: readonly unknown[]): string | null =>
    layers.length === 0
      ? null
      : layers
          .map((layer) => {
            if (!known.has(layer)) known.set(layer, known.size);
            return known.get(layer);
          })
          .join(",");
  const kind = (pkg: ThemePackage): unknown => {
    switch (part) {
      case "shapes":
        return pkg.geometry;
      case "art":
        return which(pkg.effects);
      case "motion":
        return which(pkg.behaviors);
      case "events":
        return eventStyleOf(pkg);
    }
  };
  return all.filter((pkg) => {
    const k = kind(pkg);
    if (k === null || seen.has(k)) return false;
    seen.add(k);
    return true;
  });
}

/** A stored choice, read back defensively: anything unreadable is Full. */
export function readParts(stored: string | null): Parts {
  if (!stored) return { ...FULL };
  try {
    const value = JSON.parse(stored) as Record<string, unknown>;
    const parts = { ...FULL };
    for (const { name } of PARTS) {
      const choice = value?.[name];
      if (typeof choice === "string" && choice.length > 0) parts[name] = choice;
    }
    return parts;
  } catch {
    return { ...FULL };
  }
}
