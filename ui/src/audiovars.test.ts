import { describe, expect, it, beforeEach } from "vitest";
import { publishAudio, resetAudioVars } from "./audiovars.svelte";
import type { SessionContext } from "./api";

/**
 * A style target that records what reaches it.
 *
 * Counting the writes is the point: this module exists to make most frames
 * write nothing, and only a count can show that.
 */
function target() {
  const props = new Map<string, string>();
  let writes = 0;
  return {
    style: {
      setProperty(name: string, value: string) {
        props.set(name, value);
        writes += 1;
      },
    },
    get(name: string) {
      return props.get(name) ?? "";
    },
    get writes() {
      return writes;
    },
    reset() {
      writes = 0;
    },
  };
}

function context(loudness: number, bands: [number, number, number, number]): SessionContext {
  return { audio: { loudness, bands }, session: null };
}

describe("publishing the audio to CSS", () => {
  beforeEach(() => {
    resetAudioVars();
  });

  it("writes every band where a theme can read it", () => {
    const root = target();
    publishAudio(context(0.5, [1, 0.75, 0.5, 0.25]), root);
    expect(root.get("--audio-loudness")).toBe("0.5");
    expect(root.get("--audio-bass")).toBe("1");
    expect(root.get("--audio-treble")).toBe("0.25");
    // Energy falls back to loudness until M9 reads the room.
    expect(root.get("--audio-energy")).toBe("0.5");
    expect(root.get("--audio-hue")).toBe("130");
  });

  /**
   * The whole point of this module. A steady passage must stop writing, or the
   * style recalculation it was meant to avoid simply happens here instead.
   */
  it("stops writing when nothing has changed", () => {
    const root = target();
    const steady = context(0.5, [0.5, 0.5, 0.5, 0.5]);

    publishAudio(steady, root);
    expect(root.writes).toBeGreaterThan(0);

    root.reset();
    for (let i = 0; i < 60; i++) publishAudio(steady, root);
    expect(root.writes).toBe(0);
  });

  /** And a change below the quantisation step is not a change. */
  it("ignores movement too small to see", () => {
    const root = target();
    publishAudio(context(0.5, [0.5, 0.5, 0.5, 0.5]), root);
    root.reset();

    // 1/64 is about 0.0156, so this is well inside one step.
    publishAudio(context(0.502, [0.5, 0.5, 0.5, 0.5]), root);
    expect(root.writes).toBe(0);

    publishAudio(context(0.6, [0.5, 0.5, 0.5, 0.5]), root);
    expect(root.writes).toBeGreaterThan(0);
  });

  it("only writes the properties that moved", () => {
    const root = target();
    publishAudio(context(0.5, [0.5, 0.5, 0.5, 0.5]), root);
    root.reset();

    // Bass alone. Loudness, the other three bands, energy and hue all hold.
    publishAudio(context(0.5, [0.9, 0.5, 0.5, 0.5]), root);
    expect(root.writes).toBe(1);
  });

  it("survives a snapshot that has not arrived yet", () => {
    const root = target();
    expect(() => publishAudio(undefined, root)).not.toThrow();
    expect(root.get("--audio-bass")).toBe("0");
  });

  /**
   * A NaN from a wedged engine must not put `NaN` into a CSS declaration.
   *
   * Out-of-range but finite values clamp; non-finite ones read as *silence*
   * rather than as maximum. An infinity is a broken reading, and answering a
   * broken reading with a full-brightness interface is the wrong way round.
   */
  it("clamps anything that is not a sensible level", () => {
    const root = target();
    publishAudio(context(Number.NaN, [2, -1, Number.POSITIVE_INFINITY, 0.5]), root);
    expect(root.get("--audio-loudness")).toBe("0");
    expect(root.get("--audio-bass")).toBe("1");
    expect(root.get("--audio-low-mid")).toBe("0");
    expect(root.get("--audio-high-mid")).toBe("0");
  });

  it("prefers a real reading of the room once there is one", () => {
    const root = target();
    publishAudio(
      {
        audio: { loudness: 0.2, bands: [0.2, 0.2, 0.2, 0.2] },
        session: { phase: "peak", energy: 0.9, environment: { time_of_day: "night" } },
      },
      root,
    );
    expect(root.get("--audio-energy")).toBe("0.90625");
  });
});
