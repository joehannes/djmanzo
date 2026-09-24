import { describe, expect, it } from "vitest";

import { GHOST, filterPasses, partOpacities, partOpacity } from "./eqLight";

const unity = { eq_low: 1, eq_mid: 1, eq_high: 1, filter: 0 };

describe("§110, live: each EQ part as bright as its knob", () => {
  it("draws every part whole at unity", () => {
    expect(partOpacities(unity)).toEqual({ low: 1, mid: 1, high: 1 });
  });

  it("leaves a killed part a ghost, not gone", () => {
    expect(partOpacities({ ...unity, eq_low: 0 })).toEqual({ low: GHOST, mid: 1, high: 1 });
    expect(GHOST).toBeGreaterThan(0);
  });

  it("draws a boost no brighter than unity", () => {
    expect(partOpacity("mid", 4, 0)).toBe(1);
  });

  it("dims with the knob, not in a step", () => {
    const half = partOpacity("high", 0.5, 0);
    expect(half).toBeGreaterThan(GHOST);
    expect(half).toBeLessThan(1);
  });

  it("takes the high first and the low last as the low-pass comes down", () => {
    // A quarter of the way: the corner is still well above the mids.
    expect(filterPasses("low", -0.25)).toBe(1);
    expect(filterPasses("high", -0.25)).toBeLessThan(1);
    // All the way: 40 Hz — the high and mid gone, a sliver of the low left.
    expect(filterPasses("high", -1)).toBe(0);
    expect(filterPasses("mid", -1)).toBe(0);
    expect(filterPasses("low", -1)).toBeGreaterThan(0);
  });

  it("takes the low first as the high-pass goes up", () => {
    expect(filterPasses("low", 0.5)).toBeLessThan(filterPasses("mid", 0.5));
    expect(filterPasses("high", 0.5)).toBe(1);
    expect(filterPasses("low", 1)).toBe(0);
  });

  it("treats the dead zone and nonsense as open", () => {
    expect(filterPasses("high", -0.01)).toBe(1);
    expect(filterPasses("high", Number.NaN)).toBe(1);
    expect(partOpacity("low", Number.NaN, 0)).toBe(1);
  });
});
