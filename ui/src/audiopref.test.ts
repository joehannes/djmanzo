import { describe, expect, it } from "vitest";
import {
  deviceMissing,
  deviceToOpen,
  readAudioPreference,
  writeAudioPreference,
} from "./audiopref";

/** A store that behaves, and one that does not. */
function store(): Storage {
  const map = new Map<string, string>();
  return {
    getItem: (k: string) => map.get(k) ?? null,
    setItem: (k: string, v: string) => void map.set(k, v),
    removeItem: (k: string) => void map.delete(k),
    clear: () => map.clear(),
    key: () => null,
    length: 0,
  } as unknown as Storage;
}

function hostile(): Storage {
  return {
    getItem: () => {
      throw new Error("blocked");
    },
    setItem: () => {
      throw new Error("full");
    },
  } as unknown as Storage;
}

const devices = [
  { id: "builtin", is_default: true },
  { id: "interface", is_default: false },
];

describe("remembering a device", () => {
  it("round-trips a choice", () => {
    const s = store();
    writeAudioPreference({ device: "interface", cue: "builtin", bufferFrames: 512 }, s);
    expect(readAudioPreference(s)).toEqual({
      device: "interface",
      cue: "builtin",
      bufferFrames: 512,
    });
  });

  it("has sensible defaults when nothing was ever stored", () => {
    expect(readAudioPreference(store())).toEqual({
      device: null,
      cue: null,
      bufferFrames: 256,
    });
  });

  /**
   * A half-written or hand-edited value reaching the audio backend as a buffer
   * size is a crash, not a typo — so it is validated rather than trusted.
   */
  it("refuses a buffer size the interface does not offer", () => {
    const s = store();
    s.setItem("djmanzo.audio", JSON.stringify({ bufferFrames: 999 }));
    expect(readAudioPreference(s).bufferFrames).toBe(256);
  });

  it("survives a stored value that is not JSON at all", () => {
    const s = store();
    s.setItem("djmanzo.audio", "{ half a writ");
    expect(readAudioPreference(s)).toEqual({
      device: null,
      cue: null,
      bufferFrames: 256,
    });
  });

  /**
   * A DJ whose preference does not stick has a small annoyance. One whose
   * application will not start has a problem.
   */
  it("survives a store that throws in both directions", () => {
    expect(() =>
      writeAudioPreference({ device: "a", cue: null, bufferFrames: 256 }, hostile()),
    ).not.toThrow();
    expect(readAudioPreference(hostile())).toEqual({
      device: null,
      cue: null,
      bufferFrames: 256,
    });
  });

  it("works with no store at all", () => {
    expect(readAudioPreference(null).bufferFrames).toBe(256);
    expect(() => writeAudioPreference(readAudioPreference(null), null)).not.toThrow();
  });
});

describe("choosing what to open", () => {
  it("opens what was remembered when it is still there", () => {
    expect(deviceToOpen("interface", devices)).toBe("interface");
  });

  /**
   * Hardware moves. A DJ who played through a controller last night and opened
   * the laptop on a train has a stored id that no longer exists.
   */
  it("falls back to the default when the remembered device is gone", () => {
    expect(deviceToOpen("a-controller-left-at-the-venue", devices)).toBe("builtin");
    expect(deviceMissing("a-controller-left-at-the-venue", devices)).toBe(true);
  });

  it("takes the first device when nothing is marked default", () => {
    expect(deviceToOpen(null, [{ id: "only", is_default: false }])).toBe("only");
  });

  it("has nothing to open when there is nothing at all", () => {
    expect(deviceToOpen("anything", [])).toBe(null);
  });

  /** Not remembering anything is not the same as a missing device. */
  it("does not call a first launch a missing device", () => {
    expect(deviceMissing(null, devices)).toBe(false);
  });
});
