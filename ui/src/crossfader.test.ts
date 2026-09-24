/**
 * §114: the crossfader's drawn law is the engine's.
 *
 * `dj_dsp::crossfader_gains` with `CrossfaderCurve::Smooth`, the curve the
 * engine mixes by: constant power, 1/√2 each in the middle, exact silence at
 * the far end.
 */
import { describe, expect, it } from "vitest";
import { gainY, lawPath, sides, sidesInWords } from "./crossfader";

describe("the crossfader's law", () => {
  it("is constant power: both at 71 % in the middle, power summing to one", () => {
    const [left, right] = sides(0);
    expect(left).toBeCloseTo(Math.SQRT1_2, 6);
    expect(right).toBeCloseTo(Math.SQRT1_2, 6);
    for (const p of [-1, -0.6, -0.1, 0.3, 0.9, 1]) {
      const [l, r] = sides(p);
      expect(l * l + r * r).toBeCloseTo(1, 6);
    }
  });

  it("silences the far side exactly at each end, never below zero", () => {
    expect(sides(-1)).toEqual([1, 0]);
    const [l, r] = sides(1);
    expect(l).toBe(Math.max(0, Math.cos(Math.PI / 2)));
    expect(l).toBeGreaterThanOrEqual(0);
    expect(l).toBeLessThan(1e-9);
    expect(r).toBe(1);
    expect(sides(Number.NaN)).toEqual(sides(0));
    expect(sides(7)).toEqual(sides(1));
  });

  it("draws side 1 falling and side 2 rising across the throw", () => {
    const plot = { left: 0, width: 100, top: 10, height: 20 };
    const heights = (d: string) => [...d.matchAll(/[ML] [\d.]+ ([\d.]+)/g)].map((m) => Number(m[1]));
    const one = heights(lawPath(0, plot));
    const two = heights(lawPath(1, plot));
    expect(one[0]).toBe(10);
    expect(one[one.length - 1]).toBe(30);
    expect(two[0]).toBe(30);
    expect(two[two.length - 1]).toBe(10);
    // They cross in the middle, above half-way: 71 %, not 50.
    expect(one[16]).toBeCloseTo(two[16], 1);
    expect(one[16]).toBeLessThan(gainY(plot, 0.5));
  });

  it("says both levels in words", () => {
    expect(sidesInWords(0)).toBe("1 at 71%, 2 at 71%");
    expect(sidesInWords(-1)).toBe("1 at 100%, 2 at 0%");
  });
});
