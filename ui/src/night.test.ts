import { describe, expect, it } from "vitest";

import { ARC, BASIS, CERTAINTY, WARRANT, WHEN } from "./night";

/**
 * The words are typed as total maps, so a missing one is a type error rather
 * than an `undefined` in a panel. What a type cannot check is that the arc is
 * the *same* arc djmanzo reads a phase from, in the same order — a phase drawn
 * out of order would read as a night going backwards.
 */
describe("the night's vocabulary", () => {
  it("draws the arc in the order a night goes through it", () => {
    expect(ARC.map((step) => step.phase)).toEqual([
      "warm_up",
      "heat",
      "peak",
      "cooldown",
      "chill_out",
    ]);
  });

  it("names every step, once", () => {
    const labels = ARC.map((step) => step.label);
    expect(new Set(labels).size).toBe(ARC.length);
    expect(labels.every((label) => label.length > 0)).toBe(true);
  });

  /**
   * Every table is total over its union — enforced by the type, asserted here
   * so the count is visible when one of the unions grows.
   */
  it("has a word for every value djmanzo can send", () => {
    expect(Object.keys(BASIS)).toHaveLength(5);
    expect(Object.keys(CERTAINTY)).toHaveLength(3);
    expect(Object.keys(WARRANT)).toHaveLength(6);
    expect(Object.keys(WHEN)).toHaveLength(5);
    for (const table of [BASIS, CERTAINTY, WARRANT, WHEN]) {
      for (const [key, phrase] of Object.entries(table)) {
        expect(phrase, `${key} has no words`).not.toBe("");
      }
    }
  });

  /**
   * The two warrants that move something the room can hear have to read as
   * more than the ones that do not — this line is the only place a DJ learns
   * that an unclear night has stopped the mix.
   */
  it("says plainly which warrants touch what the room hears", () => {
    expect(WARRANT.stage).toContain("not touching anything the room hears");
    expect(WARRANT.act).toContain("May move");
    expect(WARRANT.mix).toContain("unasked");
  });
});
