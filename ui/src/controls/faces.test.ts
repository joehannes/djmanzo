/**
 * §114: controls shaped like what they do.
 *
 * The curves on the tone knobs are claims about the sound — "the low is
 * gone", "the filter is cutting above a kilohertz" — so they are held to the
 * DSP's arithmetic here, and the value layer is held to drawing them, filling
 * from where a control rests, and lighting a pad in what it does.
 */
import { describe, expect, it } from "vitest";
import {
  CEILING_DB,
  FLOOR_DB,
  PAD_LIGHT,
  STONE_INNER,
  STONE_OUTER,
  bandShares,
  eqResponseDb,
  facePath,
  filterCorner,
  filterResponseDb,
  hzAt,
  pebblePath,
  seedOf,
  stonePath,
  stoneRadii,
} from "./faces";
import { GeometryCircle, GeometryStone, type SvgPath } from "./themes/engine";
import type { FaderState, KnobState, PadState } from "./grammar";

const knob = (over: Partial<KnobState>): KnobState => ({
  value: 1,
  min: 0,
  max: 4,
  normalized: 0.25,
  angle: 0,
  dragging: false,
  disabled: false,
  size: 46,
  ...over,
});

const fader = (over: Partial<FaderState>): FaderState => ({
  value: 0,
  min: -1,
  max: 1,
  normalized: 0.5,
  dragging: false,
  disabled: false,
  width: 40,
  height: 140,
  orientation: "vertical",
  ...over,
});

const pad = (over: Partial<PadState>): PadState => ({
  active: false,
  pressed: false,
  disabled: false,
  width: 60,
  height: 40,
  ...over,
});

/** The y of every point of a path, in order. */
function heights(d: string): number[] {
  return [...d.matchAll(/[ML] [\d.]+ ([\d.]+)/g)].map((m) => Number(m[1]));
}

/** The last of a list (`Array.at` is past this project's TS lib). */
function last<T>(list: T[]): T {
  return list[list.length - 1];
}

/** The value paths that are arcs (the track and the fill). */
function arcs(paths: SvgPath[]): SvgPath[] {
  return paths.filter((p) => p.role === "value" && p.d.includes(" A "));
}

describe("the isolator's bands", () => {
  it("share the signal between them, each ruling its own octaves", () => {
    for (const hz of [20, 100, 300, 1_000, 4_000, 12_000, 20_000]) {
      const { low, mid, high } = bandShares(hz);
      expect(low + mid + high).toBeCloseTo(1, 9);
      expect(Math.min(low, mid, high)).toBeGreaterThanOrEqual(0);
    }
    expect(bandShares(40).low).toBeGreaterThan(0.99);
    expect(bandShares(1_100).mid).toBeGreaterThan(0.9);
    expect(bandShares(15_000).high).toBeGreaterThan(0.99);
    // At a crossover the two sides are level.
    expect(bandShares(300).low).toBeCloseTo(0.5, 1);
  });
});

describe("a tone knob's curve", () => {
  it("is flat at unity, whichever band", () => {
    for (const band of ["low", "mid", "high"] as const) {
      for (let i = 0; i <= 10; i++) expect(eqResponseDb(band, 1, hzAt(i / 10))).toBeCloseTo(0, 9);
    }
  });

  /** **A kill is a floor where the band is and nothing elsewhere.** */
  it("draws a killed band as the floor in its own octaves only", () => {
    expect(eqResponseDb("low", 0, 30)).toBe(FLOOR_DB);
    expect(eqResponseDb("low", 0, 15_000)).toBeCloseTo(0, 2);
    expect(eqResponseDb("high", 0, 15_000)).toBe(FLOOR_DB);
    expect(eqResponseDb("high", 0, 40)).toBeCloseTo(0, 2);
    expect(eqResponseDb("mid", 0, 1_100)).toBeLessThan(-18);
    expect(eqResponseDb("mid", 0, 20)).toBeCloseTo(0, 1);
    expect(eqResponseDb("mid", 0, 20_000)).toBeCloseTo(0, 1);
  });

  it("draws a boost up to the knob's +12 dB", () => {
    expect(eqResponseDb("low", 4, 25)).toBeCloseTo(CEILING_DB, 0);
    expect(eqResponseDb("high", 2, 18_000)).toBeCloseTo(6.02, 1);
  });

  it("puts the LOW knob's shelf on the left of its face and the HI knob's on the right", () => {
    const low = heights(facePath("eq-low", 0));
    const high = heights(facePath("eq-high", 0));
    // Lower on screen is a larger y.
    expect(low[0]).toBeGreaterThan(low[low.length - 1] + 10);
    expect(high[high.length - 1]).toBeGreaterThan(high[0] + 10);
    const mid = heights(facePath("eq-mid", 0));
    const deepest = mid.indexOf(Math.max(...mid));
    expect(deepest).toBeGreaterThan(mid.length * 0.4);
    expect(deepest).toBeLessThan(mid.length * 0.75);
  });
});

describe("the filter's curve", () => {
  it("is flat inside the dead zone", () => {
    expect(filterCorner(0)).toBeNull();
    expect(filterCorner(0.02)).toBeNull();
    expect(filterResponseDb(0.01, 5_000)).toBe(0);
  });

  /** The corners are `dj_dsp::SweepFilter`'s: 40 Hz at the bottom, 8 kHz at the top. */
  it("sweeps to the DSP's corners", () => {
    expect(filterCorner(-1)).toEqual({ side: "low-pass", hz: 40 });
    expect(filterCorner(1)!.side).toBe("high-pass");
    expect(filterCorner(1)!.hz).toBeCloseTo(8_000, 6);
    const half = filterCorner(-0.5)!.hz;
    expect(half).toBeGreaterThan(500);
    expect(half).toBeLessThan(2_000);
  });

  it("is a Butterworth: 3 dB down at the corner and 12 dB an octave past it", () => {
    const { hz } = filterCorner(-0.5)!;
    expect(filterResponseDb(-0.5, hz)).toBeCloseTo(-3.01, 1);
    expect(filterResponseDb(-0.5, hz * 4)).toBeCloseTo(-24, 0);
    expect(filterResponseDb(-0.5, 20)).toBeCloseTo(0, 1);
    const high = filterCorner(0.5)!.hz;
    expect(filterResponseDb(0.5, high)).toBeCloseTo(-3.01, 1);
    expect(filterResponseDb(0.5, 20_000)).toBeCloseTo(0, 1);
  });

  it("falls away on the side it cuts", () => {
    const lowPass = heights(facePath("filter", -0.6));
    const highPass = heights(facePath("filter", 0.6));
    expect(lowPass[lowPass.length - 1]).toBeGreaterThan(lowPass[0] + 5);
    expect(highPass[0]).toBeGreaterThan(highPass[highPass.length - 1] + 5);
    for (let i = 1; i < lowPass.length; i++) expect(lowPass[i]).toBeGreaterThanOrEqual(lowPass[i - 1]);
  });
});

describe("the value layer", () => {
  /** **A control at rest shows no fill, and a cut runs the other way from a boost.** */
  it("fills a knob's arc from where it rests", () => {
    const resting = GeometryCircle(knob({ value: 1, normalized: 0.25, origin: 0.25 }));
    // The track alone: no fill arc at unity.
    expect(arcs(resting.paths)).toHaveLength(1);

    const boost = arcs(GeometryCircle(knob({ value: 2, normalized: 0.5, origin: 0.25 })).paths)[1];
    const cut = arcs(GeometryCircle(knob({ value: 0.5, normalized: 0.125, origin: 0.25 })).paths)[1];
    // Both start at the rest, and sweep in opposite directions.
    expect(boost.d.split(" A ")[0]).toBe(cut.d.split(" A ")[0]);
    expect(boost.d).toMatch(/ 0 0 1 [\d.]+ [\d.]+$/);
    expect(cut.d).toMatch(/ 0 0 0 [\d.]+ [\d.]+$/);

    // Without a rest the fill starts at the bottom of the sweep, as before.
    const plain = arcs(GeometryCircle(knob({ value: 1, normalized: 0.25 })).paths);
    expect(plain).toHaveLength(2);
    expect(plain[1].d.split(" A ")[0]).toBe(plain[0].d.split(" A ")[0]);
  });

  it("draws the face only on a knob that has one, in the knob's own colour", () => {
    const bare = GeometryCircle(knob({})).paths;
    const faced = GeometryCircle(knob({ face: "eq-low", value: 0 })).paths;
    expect(faced.length).toBe(bare.length + 2);
    const curve = faced.find((p) => p.d === facePath("eq-low", 0));
    expect(curve?.stroke).toBe("var(--knob-value)");
    expect(curve?.role).toBe("value");
  });

  it("fills a centred fader from its middle, with the rest marked", () => {
    const up = GeometryCircle(fader({ normalized: 0.75, origin: 0.5 })).paths;
    const fill = up.find((p) => p.fill === "var(--accent-2)")!;
    // From 30 (three quarters up) to 50 (the middle): the zero, not the bottom.
    expect(fill.d).toBe("M 40 30.00 H 60 V 50.00 H 40 Z");
    const down = GeometryCircle(fader({ normalized: 0.25, origin: 0.5 })).paths;
    expect(down.find((p) => p.fill === "var(--accent-2)")!.d).toBe("M 40 50.00 H 60 V 70.00 H 40 Z");
    expect(up.some((p) => p.stroke === "var(--text-dim)" && p.d.includes("H 36"))).toBe(true);
    // A plain fader still fills from the bottom and has no mark.
    const plain = GeometryCircle(fader({ normalized: 0.75 })).paths;
    expect(plain.find((p) => p.fill === "var(--accent-2)")!.d).toBe("M 40 30.00 H 60 V 90.00 H 40 Z");
    expect(plain.some((p) => p.stroke === "var(--text-dim)")).toBe(false);
  });

  /** **A pad lights in what it does**: go, about to, chosen, stop. */
  it("lights a pad in its function's colour, and rings EJECT at rest", () => {
    const playing = last(GeometryCircle(pad({ does: "play", active: true })).paths);
    expect(playing.fill).toBe(PAD_LIGHT.play);
    expect(playing.fill).toBe("var(--success)");
    expect(playing.style).toContain("fill-opacity");
    expect(last(GeometryCircle(pad({ does: "cue", pressed: true })).paths).fill).toBe("var(--warn)");
    expect(last(GeometryCircle(pad({ does: "sync", active: true })).paths).fill).toBe("var(--selected)");
    const eject = last(GeometryCircle(pad({ does: "eject" })).paths);
    expect(eject.stroke).toBe("var(--danger)");
    expect(eject.fill).toBe("none");
    // A pad that does not say keeps the accent it always had.
    expect(last(GeometryCircle(pad({ active: true })).paths).fill).toBe("var(--accent-2)");
    // And an idle PLAY is unlit.
    expect(GeometryCircle(pad({ does: "play" })).paths).toHaveLength(1);
  });
});

describe("natural forms", () => {
  it("grows each knob its own stone, the same one every time", () => {
    expect(stonePath("LOW")).toBe(stonePath("LOW"));
    expect(stonePath("LOW")).not.toBe(stonePath("MID"));
    expect(seedOf("HI")).not.toBe(seedOf("MID"));
    for (const name of ["HI", "MID", "LOW", "Filter", ""]) {
      const radii = stoneRadii(name);
      expect(Math.min(...radii)).toBeGreaterThanOrEqual(STONE_INNER);
      expect(Math.max(...radii)).toBeLessThanOrEqual(STONE_OUTER);
      // A stone, not a circle.
      expect(Math.max(...radii) - Math.min(...radii)).toBeGreaterThan(1);
    }
  });

  /** The stone is the body; the value is still drawn on a true circle over it. */
  it("keeps the value on a circle over the stone", () => {
    const stone = GeometryStone(knob({ label: "LOW", normalized: 0.5 }));
    const circle = GeometryCircle(knob({ label: "LOW", normalized: 0.5 }));
    expect(stone.paths[0].d).toBe(stonePath("LOW"));
    expect(stone.paths.slice(1)).toEqual(circle.paths.slice(1));
    // The track (radius 40, six wide) lies inside the stone everywhere.
    expect(STONE_INNER).toBeGreaterThan(40 + 3);
  });

  it("draws a pad as a pebble and fills it in its own outline", () => {
    const pebble = pebblePath();
    const points = [...pebble.matchAll(/[ML] ([\d.]+) ([\d.]+)/g)].map((m) => [Number(m[1]), Number(m[2])]);
    for (const [x, y] of points) {
      expect(x).toBeGreaterThanOrEqual(4);
      expect(x).toBeLessThanOrEqual(96);
      expect(y).toBeGreaterThanOrEqual(4);
      expect(y).toBeLessThanOrEqual(96);
    }
    // Rounder than the box: no point sits in a corner.
    expect(points.some(([x, y]) => x < 8 && y < 8)).toBe(false);
    const lit = GeometryStone(pad({ does: "play", active: true })).paths;
    expect(lit[0].d).toBe(pebble);
    expect(last(lit).d).toBe(pebble);
  });
});
