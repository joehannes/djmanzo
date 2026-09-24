import { describe, expect, it } from "vitest";

import { comingUp, distance, framesPerBeat, happenings } from "./ahead";
import type { EnergyTrajectory, RecordChange } from "./api";

/** Windows of 32 beats, 96 000 frames a beat: a bar is 384 000 frames. */
const trajectory: EnergyTrajectory = {
  sections: [
    { at: 0, energy: 0.9, low: 0.9, parts: null, strikes: null },
    { at: 3_072_000, energy: 1, low: 1, parts: null, strikes: null },
  ],
  beats_per_section: 32,
  breakdowns: [{ from: 6_144_000, to: 9_216_000 }],
  drops: [9_216_000],
};
const BAR = 384_000;

const changes: RecordChange[] = [
  { at: 6_144_000, until: null, kind: "leaves", stem: 1 },
  { at: 6_144_000, until: 6_144_000, kind: "settles", stem: null },
  { at: 9_216_000, until: null, kind: "enters", stem: 0 },
  { at: 9_216_000, until: 9_216_000, kind: "rises", stem: null },
];

describe("§116: what is coming, in bars", () => {
  it("counts a beat from the trajectory's own windows", () => {
    expect(framesPerBeat(trajectory)).toBe(96_000);
    expect(framesPerBeat({ ...trajectory, sections: trajectory.sections.slice(0, 1) })).toBeNull();
  });

  it("says a drop rather than a rise, and a breakdown rather than a settle, at the same place", () => {
    const said = happenings(changes, trajectory).map((thing) => thing.says);
    expect(said).toContain("Drop");
    expect(said).toContain("Breakdown");
    expect(said).not.toContain("Builds");
    expect(said).not.toContain("Settles");
    expect(said).toContain("Vocals in");
    expect(said).toContain("Drums out");
  });

  it("says what is ahead, nearest first, in whole bars", () => {
    const ahead = comingUp(6_144_000 - 8 * BAR, changes, trajectory);
    expect(ahead[0].bars).toBe(8);
    expect(ahead.map((thing) => thing.says).slice(0, 2).sort()).toEqual(["Breakdown", "Drums out"]);
    expect(ahead.every((thing) => thing.at > 6_144_000 - 8 * BAR)).toBe(true);
  });

  it("counts the last bar in beats", () => {
    const [next] = comingUp(9_216_000 - 3 * 96_000, changes, trajectory);
    expect(distance(next)).toBe("3 beats");
    expect(distance({ bars: 1, beats: 5 })).toBe("1 bar");
    expect(distance({ bars: 16, beats: 64 })).toBe("16 bars");
  });

  it("says nothing past the horizon or behind the playhead", () => {
    expect(comingUp(0, changes, trajectory, () => true, 8)).toEqual([]);
    expect(comingUp(9_216_000, changes, trajectory)).toEqual([]);
  });

  it("keeps quiet about a layer the DJ switched off", () => {
    const ahead = comingUp(6_144_000 - BAR, changes, trajectory, (layer) => layer !== "stems");
    expect(ahead.some((thing) => thing.layer === "stems")).toBe(false);
    expect(ahead.length).toBeGreaterThan(0);
  });
});
