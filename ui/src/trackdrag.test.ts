import { describe, expect, it } from "vitest";

import { TRACK, carriesTrack, carryTrack, droppedTrack } from "./trackdrag";

/** A DataTransfer stand-in: jsdom and node have none. */
function transfer() {
  const data = new Map<string, string>();
  return {
    get types() {
      return [...data.keys()];
    },
    setData: (type: string, value: string) => void data.set(type, value),
    getData: (type: string) => data.get(type) ?? "",
  };
}

const event = (dataTransfer: ReturnType<typeof transfer>) => ({ dataTransfer }) as unknown as DragEvent;

describe("a record carried by drag and drop", () => {
  it("arrives as it left, with only what is needed to load and name it", () => {
    const moving = event(transfer());
    carryTrack(moving, { id: "ab", path: "/m/a.mp3", title: "Vivir mi vida", artist: "Marc Anthony", extra: 1 } as never);
    expect(carriesTrack(moving)).toBe(true);
    expect(droppedTrack(moving)).toEqual({ id: "ab", path: "/m/a.mp3", title: "Vivir mi vida", artist: "Marc Anthony" });
  });

  it("anything else, or anything malformed, is not a record", () => {
    const other = transfer();
    other.setData("text/plain", "hello");
    expect(carriesTrack(event(other))).toBe(false);
    expect(droppedTrack(event(other))).toBeNull();
    for (const bad of ["{", "null", '{"id":"ab"}', '{"id":"ab","path":""}', '{"id":1,"path":"/a"}']) {
      const t = transfer();
      t.setData(TRACK, bad);
      expect(droppedTrack(event(t)), bad).toBeNull();
    }
  });
});
