import { describe, expect, it } from "vitest";
import { fill } from "./meter";

describe("meter fills", () => {
  it("passes a level through as a scale factor", () => {
    expect(fill(0)).toBe(0);
    expect(fill(1)).toBe(1);
    expect(fill(0.5)).toBe(0.5);
  });

  it("clamps a level that is off the scale", () => {
    expect(fill(1.4)).toBe(1);
    expect(fill(-0.2)).toBe(0);
  });

  /** A NaN peak must not put `NaN` into a transform. */
  it("reads a broken level as silence", () => {
    expect(fill(Number.NaN)).toBe(0);
    expect(fill(Number.POSITIVE_INFINITY)).toBe(0);
  });

  /**
   * The reason this function exists. Svelte skips the DOM write when the value
   * it would set is unchanged, and a raw peak almost never repeats — so a
   * steady level has to round to the same number for the write to disappear.
   */
  it("gives the same answer for movement too small to see", () => {
    expect(fill(0.5)).toBe(fill(0.5004));
    expect(fill(0.5)).toBe(fill(0.4996));
  });

  /** And still resolves anything a person could actually see. */
  it("still separates levels that differ visibly", () => {
    expect(fill(0.5)).not.toBe(fill(0.51));
    // Half a percent of the bar: under a pixel on any meter drawn here.
    expect(fill(0.5) - fill(0.495)).toBeCloseTo(0.005, 10);
  });

  it("keeps the ends exact, so a meter reads empty and full", () => {
    // Rounding that drifted at the extremes would leave a hairline of fill on
    // silence, or a gap at the top of a clipping meter.
    expect(fill(0.0001)).toBe(0);
    expect(fill(0.9999)).toBe(1);
  });
});
