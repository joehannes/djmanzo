import { describe, expect, it } from "vitest";

import type { MelodyLine } from "./api";
import { LEAP_SEMITONES, melodyPaths } from "./melody";

const colours: [number, number, number][] = Array.from({ length: 256 }, (_, i) => [i, 0, 255 - i]);

/** Ten points a second at 48 kHz: 4 800 frames a point. */
const line = (points: ([number, number] | null)[]): MelodyLine => ({
  frames_per_point: 4_800,
  points,
  colours,
  range: [70, 700],
});

/** The y of every point of every path, in order. */
const ys = (paths: { d: string }[]) =>
  paths.flatMap((path) => [...path.d.matchAll(/[ML]([\d.]+) ([\d.]+)/g)].map((m) => Number(m[2])));

describe("§116: the melody line", () => {
  it("draws a higher note higher, by semitones", () => {
    const paths = melodyPaths(line([[220, 80], [330, 80]]), 0, 9_600, 100, 100);
    expect(paths).toHaveLength(1);
    const [low, high] = ys(paths);
    expect(high).toBeLessThan(low);
    // A fifth is seven of the forty semitones between 70 and 700 Hz.
    const lane = 100 * (1 - 2 * 0.12);
    expect(low - high).toBeCloseTo((lane * 12 * Math.log2(1.5)) / (12 * Math.log2(10)), 0);
  });

  it("breaks where nothing was pitched", () => {
    const paths = melodyPaths(line([[220, 80], [230, 80], null, [240, 80], [250, 80]]), 0, 30_000, 100, 100);
    expect(paths).toHaveLength(2);
  });

  it("breaks at a leap no melody makes in a tenth of a second", () => {
    const octaveAndMore = 220 * 2 ** ((LEAP_SEMITONES + 2) / 12);
    const paths = melodyPaths(line([[220, 80], [225, 80], [octaveAndMore, 90], [octaveAndMore, 90]]), 0, 30_000, 100, 100);
    expect(paths).toHaveLength(2);
  });

  it("colours each run by its own step, joined where the colour changes", () => {
    const paths = melodyPaths(line([[220, 80], [230, 80], [240, 81], [250, 81]]), 0, 30_000, 100, 100);
    expect(paths.map((path) => path.colour)).toEqual(["rgb(80, 0, 175)", "rgb(81, 0, 174)"]);
    // Joined: the second run starts where the first ended.
    const runs = paths[0].d.split(" L");
    const end = runs[runs.length - 1];
    expect(paths[1].d.startsWith(`M${end}`)).toBe(true);
  });

  it("draws only the stretch asked for, from its left edge", () => {
    const points: [number, number][] = Array.from({ length: 100 }, () => [220, 80]);
    const [path] = melodyPaths(line(points), 48_000, 96_000, 480, 100);
    const xs = [...path.d.matchAll(/[ML]([\d.]+) /g)].map((m) => Number(m[1]));
    expect(Math.min(...xs)).toBe(0);
    expect(Math.max(...xs)).toBe(100);
  });
});
