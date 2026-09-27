/**
 * §114: the platter's arithmetic — see `platter.ts`.
 */
import { describe, expect, it } from "vitest";

import { LABEL, RING_WANDER, RPM, RUN_OUT, grooves, growthRings, ringPath, turned } from "./platter";

describe("the platter", () => {
  /**
   * **It turns at 33⅓.** 1.8 seconds is one revolution, so a quarter of that
   * is a quarter turn; backwards is backwards; and a long set stays within
   * one turn rather than growing without end.
   */
  it("turns with the playhead at 33⅓ revolutions a minute", () => {
    expect(RPM).toBeCloseTo(33.333, 3);
    expect(turned(0)).toBe(0);
    expect(turned(0.45)).toBeCloseTo(90, 6);
    expect(turned(0.9)).toBeCloseTo(180, 6);
    expect(turned(1.8)).toBeCloseTo(0, 6);
    expect(turned(-0.45)).toBeCloseTo(270, 6);
    const late = turned(3 * 3600 + 0.45);
    expect(late).toBeGreaterThanOrEqual(0);
    expect(late).toBeLessThan(360);
    expect(late).toBeCloseTo(90, 3);
    expect(turned(Number.NaN)).toBe(0);
  });

  it("a record's grooves are evenly pitched between the label and the run-out", () => {
    const radii = grooves();
    expect(radii[0]).toBeGreaterThan(LABEL);
    expect(radii[radii.length - 1]).toBeLessThanOrEqual(RUN_OUT);
    const steps = new Set(radii.slice(1).map((r, i) => (r - radii[i]).toFixed(6)));
    expect(steps.size).toBe(1);
  });

  /**
   * **A log, not a record.** Growth rings go outward, stay on the playing
   * surface, are mostly narrow with some wide years, are the same log every
   * night for the same deck and a different log for the other.
   */
  it("growth rings are a log's own, and irregular as a log's are", () => {
    const one = growthRings("jog 1");
    expect(one.length).toBeGreaterThan(8);
    for (let i = 1; i < one.length; i++) expect(one[i]).toBeGreaterThan(one[i - 1]);
    expect(one[0]).toBeGreaterThan(LABEL);
    expect(one[one.length - 1]).toBeLessThanOrEqual(RUN_OUT);

    const widths = one.slice(1).map((r, i) => r - one[i]);
    const narrow = widths.filter((w) => w < 1.1 + 2.6 * 0.25).length;
    expect(narrow, "most years are hard").toBeGreaterThan(widths.length / 2);
    expect(Math.max(...widths) - Math.min(...widths), "and some are not").toBeGreaterThan(1);

    expect(growthRings("jog 1")).toEqual(one);
    expect(growthRings("jog 2")).not.toEqual(one);
  });

  /**
   * Each ring is out of round its own way, and never so far out that two
   * rings could cross.
   */
  it("a ring wanders from round, within bounds", () => {
    const radius = 30;
    const d = ringPath("jog 1", radius);
    const points = [...d.matchAll(/[ML] ([\d.]+) ([\d.]+)/g)].map((m) => [Number(m[1]), Number(m[2])]);
    expect(points.length).toBe(48);
    const distances = points.map(([x, y]) => Math.hypot(x - 50, y - 50));
    const spread = Math.max(...distances) - Math.min(...distances);
    expect(spread, "out of round").toBeGreaterThan(0.05);
    for (const r of distances) expect(Math.abs(r - radius)).toBeLessThanOrEqual(RING_WANDER + 0.01);
    expect(ringPath("jog 1", radius)).toBe(d);
  });
});
