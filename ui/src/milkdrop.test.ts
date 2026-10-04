import { describe, expect, it } from "vitest";

import { RATE, SAMPLES, TONES, tones, withSyncWasm } from "./milkdrop";

/** How strong `hz` is in a block of Butterchurn's bytes (the Goertzel filter). */
function strength(block: Uint8Array, hz: number): number {
  const k = (2 * Math.PI * hz) / RATE;
  const coefficient = 2 * Math.cos(k);
  let previous = 0;
  let before = 0;
  for (const byte of block) {
    const sample = (byte - 128) / 127;
    const now = sample + coefficient * previous - before;
    before = previous;
    previous = now;
  }
  return Math.sqrt(previous * previous + before * before - coefficient * previous * before) / (block.length / 2);
}

/** Butterchurn's own split, at the rate it assumes with no audio context. */
const BUTTERCHURN = { bass: [20, 320], mid: [320, 2800], treble: [2800, 11025] } as const;

describe("§122: the sound MilkDrop is given, made from the bands", () => {
  it("plays each band as a tone in Butterchurn's own bass, mid and treble", () => {
    const [bass, lowMid, highMid, treble] = TONES;
    expect(bass).toBeGreaterThan(BUTTERCHURN.bass[0]);
    expect(bass).toBeLessThan(BUTTERCHURN.bass[1]);
    for (const mid of [lowMid, highMid]) {
      expect(mid).toBeGreaterThan(BUTTERCHURN.mid[0]);
      expect(mid).toBeLessThan(BUTTERCHURN.mid[1]);
    }
    expect(treble).toBeGreaterThan(BUTTERCHURN.treble[0]);
    expect(treble).toBeLessThan(BUTTERCHURN.treble[1]);
  });

  it("makes each tone as loud as its band, and only its band", () => {
    const block = tones([0.8, 0, 0, 0.4], [0, 0, 0, 0], 0);
    expect(block).toHaveLength(SAMPLES);
    const [bass, lowMid, highMid, treble] = TONES.map((hz) => strength(block, hz));
    // A quarter each at full level, so the sum never clips.
    expect(bass).toBeCloseTo(0.8 / 4, 1);
    expect(treble).toBeCloseTo(0.4 / 4, 1);
    expect(lowMid).toBeLessThan(0.02);
    expect(highMid).toBeLessThan(0.02);
    expect(bass / treble).toBeCloseTo(2, 0);
  });

  it("is silence when the room is, and never clips when it is full", () => {
    expect(new Set(tones([0, 0, 0, 0], [0, 0, 0, 0], 0))).toEqual(new Set([128]));
    const full = tones([1, 1, 1, 1], [0, 0, 0, 0], 0);
    expect(Math.min(...full)).toBeGreaterThanOrEqual(1);
    expect(Math.max(...full)).toBeLessThanOrEqual(255);
    // A band with no reading is silent, not a full swing.
    expect(new Set(tones([Number.NaN, 0, 0, 0], [0, 0, 0, 0], 0))).toEqual(new Set([128]));
    // Past full is full.
    expect(tones([7, 0, 0, 0], [0, 0, 0, 0], 0)).toEqual(tones([1, 0, 0, 0], [0, 0, 0, 0], 0));
  });

  it("carries each tone on from the block before, rather than restarting it", () => {
    const phases = [0, 0, 0, 0];
    const seconds = SAMPLES / RATE;
    const first = Array.from(tones([0.8, 0, 0, 0], phases, seconds));
    const second = Array.from(tones([0.8, 0, 0, 0], phases, seconds));
    // A block played straight after the last one continues it: the tone at
    // the second block's first sample is where the first block left off.
    const expected = Math.round(128 + 127 * (0.8 / 4) * Math.sin((2 * Math.PI * TONES[0] * SAMPLES) / RATE));
    expect(Math.abs(second[0] - expected)).toBeLessThanOrEqual(1);
    expect(second).not.toEqual(first);
  });
});

/** The smallest WebAssembly there is: a module with nothing in it. */
const EMPTY = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]);

describe("§122: WebAssembly compiled where WebKitGTK will compile it", () => {
  /**
   * **As WebKitGTK behaves: asynchronous compiling that never settles.** A
   * preset loaded inside `withSyncWasm` still gets its module, because the
   * compile is done synchronously — and the browser's own calls are put back
   * afterwards, even when the work throws.
   */
  it("gets a module where the asynchronous calls never settle", async () => {
    const compile = WebAssembly.compile;
    const instantiate = WebAssembly.instantiate;
    const never = () => new Promise<never>(() => {});
    WebAssembly.compile = never;
    WebAssembly.instantiate = never as unknown as typeof WebAssembly.instantiate;
    try {
      const [module, built] = await withSyncWasm(async () => [
        await WebAssembly.compile(EMPTY),
        (await WebAssembly.instantiate(EMPTY)) as unknown as { instance: WebAssembly.Instance },
      ]);
      expect(module).toBeInstanceOf(WebAssembly.Module);
      expect(built.instance).toBeInstanceOf(WebAssembly.Instance);
      expect(WebAssembly.compile).toBe(never);
      await expect(withSyncWasm(() => Promise.reject(new Error("a preset it would not take")))).rejects.toThrow();
      expect(WebAssembly.compile).toBe(never);
    } finally {
      WebAssembly.compile = compile;
      WebAssembly.instantiate = instantiate;
    }
  });

  /** As Chromium behaves: a synchronous compile refused, the asynchronous one used. */
  it("falls back to the asynchronous call where a synchronous one is refused", async () => {
    const Module = WebAssembly.Module;
    const compile = WebAssembly.compile;
    const asked: string[] = [];
    WebAssembly.Module = function () {
      throw new RangeError("over the main thread's limit");
    } as unknown as typeof WebAssembly.Module;
    WebAssembly.compile = (bytes: BufferSource) => {
      asked.push("async");
      return compile(bytes);
    };
    try {
      await withSyncWasm(() => WebAssembly.compile(EMPTY));
      expect(asked).toEqual(["async"]);
      // Anything else wrong is said, not retried.
      WebAssembly.Module = function () {
        throw new TypeError("not WebAssembly");
      } as unknown as typeof WebAssembly.Module;
      await expect(withSyncWasm(() => WebAssembly.compile(EMPTY))).rejects.toThrow("not WebAssembly");
      expect(asked).toEqual(["async"]);
    } finally {
      WebAssembly.Module = Module;
      WebAssembly.compile = compile;
    }
  });
});
