/**
 * §114: a stem chip's line — see `stemPresence.ts`.
 */
import { describe, expect, it } from "vitest";

import { PRESENCE_FULL, presencePath } from "./stemPresence";

/** Four windows of a quarter of the record each, in the stem order vocal, drums, bass, other. */
const sections = [
  { at: 0, parts: [0, 0.6, 0.3, 0.1] },
  { at: 250, parts: [0.3, 0.2, 0.4, 0.1] },
  { at: 500, parts: [0.1, 0, 0, 0.9] },
  { at: 750, parts: null },
] as unknown as Parameters<typeof presencePath>[0];

/** Each column's left edge and height. */
const columns = (d: string) =>
  [...d.matchAll(/M ([\d.]+) 10 V ([\d.]+)/g)].map((m) => ({ x: Number(m[1]), height: 10 - Number(m[2]) }));

describe("a stem chip's presence", () => {
  it("draws one column a measured window, where the window is, as tall as the share", () => {
    const vocal = columns(presencePath(sections, 1000, 0));
    expect(vocal, "no voice in the first window, and none measured in the last").toEqual([
      { x: 25, height: 10 },
      { x: 50, height: 10 * (0.1 / PRESENCE_FULL) },
    ]);
    const bass = columns(presencePath(sections, 1000, 2));
    expect(bass.map((c) => c.x), "the bass drops out in the breakdown").toEqual([0, 25]);
  });

  it("each current draws its own line", () => {
    const lines = [0, 1, 2, 3].map((stem) => presencePath(sections, 1000, stem));
    expect(new Set(lines).size).toBe(4);
  });

  it("draws nothing with nothing to go on", () => {
    expect(presencePath([], 1000, 0)).toBe("");
    expect(presencePath(sections, 0, 0)).toBe("");
    expect(presencePath(sections.slice(0, 1), 1000, 0)).toBe("");
  });
});
