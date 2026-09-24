/**
 * §116: where the rhythm strip's marks go. The pattern itself is Rust's
 * (`dj_analysis::rhythm`, held against a rendered drum machine); this holds
 * the placing.
 */
import { describe, expect, it } from "vitest";
import { FAINTEST, rhythmDots, type RhythmLine } from "./rhythm";

/** Four bars of four-on-the-floor with off-beat hats, a step every 1000 frames. */
const line: RhythmLine = {
  first_frame: 500,
  frames_per_step: 1_000,
  steps: Array.from({ length: 64 }, (_, i) => [
    i % 4 === 0 ? 255 : 0,
    i % 8 === 4 ? 200 : 20,
    i % 4 === 2 ? 180 : 0,
  ]),
};

describe("the rhythm strip", () => {
  it("puts each voice's marks on its steps, where the frames are", () => {
    const dots = rhythmDots(line, 0, 8_600, 100);
    const kicks = dots.filter((d) => d.voice === 0).map((d) => d.x);
    // Steps 0, 4 and 8 are at frames 500, 4500 and 8500: 5, 45 and 85 px.
    expect(kicks).toEqual([5, 45, 85]);
    expect(dots.filter((d) => d.voice === 2).map((d) => d.x)).toEqual([25, 65]);
    expect(dots.find((d) => d.voice === 0)?.strength).toBe(1);
  });

  it("leaves out the faint, and measures from where the stretch starts", () => {
    const dots = rhythmDots(line, 4_000, 9_000, 100);
    expect(dots.every((d) => d.strength * 255 >= FAINTEST)).toBe(true);
    // The snare's faint steps (20) are not marks; its hit on step 4 is.
    expect(dots.filter((d) => d.voice === 1).map((d) => d.x)).toEqual([5]);
  });

  it("steps aside when the steps crowd together", () => {
    expect(rhythmDots(line, 0, 64_000, 500)).toEqual([]);
    expect(rhythmDots(line, 0, 64_000, 300).length).toBeGreaterThan(0);
    expect(rhythmDots({ ...line, frames_per_step: 0 }, 0, 1_000, 100)).toEqual([]);
  });
});
