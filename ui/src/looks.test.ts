import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import type { Look } from "./api";
import {
  BLENDS,
  FALL,
  FALLBACK,
  follow,
  KINDS,
  palette,
  parseColour,
  phase,
  RISE,
  SIGNALS,
  signals,
  SILENT,
  validate,
  type DeckForSignals,
  type Hearing,
} from "./looks";

function deck(number: number, over: Partial<DeckForSignals> = {}): DeckForSignals {
  return {
    number,
    playing: true,
    position_seconds: 10,
    rate: 1,
    volume: 1,
    crossfader_assign: number === 1 ? "left" : "right",
    reversed: false,
    stem_levels: [0, 0, 0, 0],
    beat_phase: 0,
    effective_bpm: 120,
    ...over,
  };
}

function hearing(over: Partial<Hearing> = {}): Hearing {
  return {
    decks: [deck(1), deck(2, { playing: false })],
    crossfader: -1,
    audio: { loudness: 0.5, bands: [0.1, 0.2, 0.3, 0.4] },
    voices: [],
    ...over,
  };
}

const layer = (kind: string, signal = "loudness", blend = "over", colour = "record", amount = 0.5) => ({
  kind,
  signal,
  colour,
  amount,
  blend,
});

describe("§122: a look's layers, checked before they are drawn", () => {
  it("keeps every layer it can draw, in order", () => {
    const look: Look = {
      name: "Test",
      description: "",
      layers: [layer("wash"), layer("pulse", "stem.drums", "add"), layer("halo", "voice", "screen", "#ff8000")],
    };
    const { layers, dropped } = validate(look);
    expect(dropped).toEqual([]);
    expect(layers.map((l) => l.kind)).toEqual(["wash", "pulse", "halo"]);
    expect(layers[2].colour).toEqual([1, 128 / 255, 0]);
  });

  it("leaves out what it cannot draw, says which and why, and keeps the rest", () => {
    const look: Look = {
      name: "Odd",
      description: "",
      layers: [
        layer("lasers"),
        layer("wash", "crowd"),
        layer("wash", "loudness", "multiply"),
        layer("wash", "loudness", "over", "sort of blue"),
        layer("halo", "voice"),
      ],
    };
    const { layers, dropped } = validate(look);
    expect(layers.map((l) => l.kind)).toEqual(["halo"]);
    expect(dropped).toHaveLength(4);
    expect(dropped[0]).toContain("Odd, layer 1");
    expect(dropped[0]).toContain("lasers");
    expect(dropped[1]).toContain("crowd");
    expect(dropped[2]).toContain("multiply");
    expect(dropped[3]).toContain("sort of blue");
  });

  it("holds an amount to 0..1", () => {
    const look: Look = { name: "Loud", description: "", layers: [layer("wash", "loudness", "over", "record", 7)] };
    expect(validate(look).layers[0].amount).toBe(1);
    look.layers[0].amount = Number.NaN;
    expect(validate(look).layers[0].amount).toBe(0);
  });

  /**
   * The built-in looks are checked against Rust's copy of these lists by
   * `dj_app::looks`' own test; this holds the two copies to each other, so
   * a look Rust passes is one this interface draws.
   */
  it("knows the same kinds, signals and blends as dj_app::looks", () => {
    const rust = readFileSync(new URL("../../crates/dj-app/src/looks.rs", import.meta.url), "utf8");
    const list = (name: string) => {
      const found = new RegExp(`const ${name}: \\[&str; \\d+\\] = \\[([^\\]]*)\\]`, "s").exec(rust);
      expect(found, name).not.toBeNull();
      return [...found![1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
    };
    expect(list("KINDS")).toEqual([...KINDS]);
    expect(list("SIGNALS")).toEqual([...SIGNALS]);
    expect(list("BLENDS")).toEqual([...BLENDS]);
  });
});

describe("§122: what the layers answer to", () => {
  it("reads the master's loudness and bands as they are", () => {
    const s = signals(hearing());
    expect(s.loudness).toBe(0.5);
    expect([s["band.bass"], s["band.low_mid"], s["band.high_mid"], s["band.treble"]]).toEqual([0.1, 0.2, 0.3, 0.4]);
  });

  it("weighs each deck's stems by its share of what is heard", () => {
    const loud = deck(1, { stem_levels: [0.2, 0.8, 0.4, 0.1] });
    expect(signals(hearing({ decks: [loud] }))["stem.drums"]).toBeCloseTo(0.8);
    // Half the fader, half the light.
    expect(signals(hearing({ decks: [{ ...loud, volume: 0.5 }] }))["stem.drums"]).toBeCloseTo(0.4);
    // A record cued under a closed fader, or on the far side of the crossfader, lights nothing.
    expect(signals(hearing({ decks: [{ ...loud, volume: 0 }] }))["stem.drums"]).toBe(0);
    expect(signals(hearing({ decks: [loud], crossfader: 1 }))["stem.drums"]).toBe(0);
    // Nor does a paused one.
    expect(signals(hearing({ decks: [{ ...loud, playing: false }] }))["stem.drums"]).toBe(0);
  });

  it("adds two decks' stems as the room hears both, and never past full", () => {
    const one = deck(1, { stem_levels: [0, 0.3, 0, 0], crossfader_assign: "thru" });
    const two = deck(2, { stem_levels: [0, 0.4, 0, 0.9], crossfader_assign: "thru" });
    const s = signals(hearing({ decks: [one, two] }));
    expect(s["stem.drums"]).toBeCloseTo(0.7);
    const three = deck(3, { stem_levels: [0, 0, 0, 0.9], crossfader_assign: "thru" });
    expect(signals(hearing({ decks: [two, three] }))["stem.other"]).toBe(1);
  });

  it("keeps the stems in their order: vocal, drums, bass, other", () => {
    const s = signals(hearing({ decks: [deck(1, { stem_levels: [0.1, 0.2, 0.3, 0.4] })] }));
    expect([s["stem.vocal"], s["stem.drums"], s["stem.bass"], s["stem.other"]]).toEqual([0.1, 0.2, 0.3, 0.4]);
  });

  it("the beat is struck on the beat and falls away through it, from the most-heard deck", () => {
    const on = signals(hearing({ decks: [deck(1, { beat_phase: 0 })] })).beat;
    const half = signals(hearing({ decks: [deck(1, { beat_phase: 0.5 })] })).beat;
    const late = signals(hearing({ decks: [deck(1, { beat_phase: 0.9 })] })).beat;
    expect(on).toBe(1);
    expect(half).toBeLessThan(0.2);
    expect(late).toBeLessThan(half);

    // Two decks: the one more of the room hears is the one in time.
    const quiet = deck(1, { beat_phase: 0.5, volume: 0.3, crossfader_assign: "thru" });
    const loud = deck(2, { beat_phase: 0.25, volume: 0.9, crossfader_assign: "thru" });
    expect(phase(hearing({ decks: [quiet, loud] }))).toBe(0.25);
  });

  it("with no grid there is no beat", () => {
    const s = signals(hearing({ decks: [deck(1, { effective_bpm: null, beat_phase: 0 })] }));
    expect(s.beat).toBe(0);
    expect(phase(hearing({ decks: [deck(1, { effective_bpm: null, beat_phase: 0.4 })] }))).toBe(0);
  });

  it("the voice is the loudest singer's microphone", () => {
    expect(signals(hearing({ voices: [0.2, 0.7, 0.1] })).voice).toBe(0.7);
    expect(signals(hearing({ voices: [] })).voice).toBe(0);
  });
});

describe("§122: followed as a light follows sound", () => {
  const loud = { ...SILENT, loudness: 1 };

  it("rises at once and lets go slowly", () => {
    const up = follow(SILENT, loud, RISE);
    expect(up.loudness).toBeGreaterThan(0.6);
    const down = follow(loud, SILENT, RISE);
    expect(down.loudness).toBeGreaterThan(0.85);
    expect(follow(loud, SILENT, FALL).loudness).toBeCloseTo(Math.exp(-1), 5);
  });

  it("looks the same at 30 frames a second as at 120", () => {
    let fast = loud;
    for (let i = 0; i < 4; i += 1) fast = follow(fast, SILENT, 1 / 120);
    const slow = follow(loud, SILENT, 1 / 30);
    expect(fast.loudness).toBeCloseTo(slow.loudness, 6);
  });

  it("leaves the beat its own shape", () => {
    expect(follow(SILENT, { ...SILENT, beat: 1 }, 0.001).beat).toBe(1);
  });
});

describe("§122: the record's colours", () => {
  it("reads the colours as djmanzo writes them", () => {
    expect(parseColour("rgb(255 0 51)")).toEqual([1, 0, 0.2]);
    expect(parseColour("rgb(255, 0, 51)")).toEqual([1, 0, 0.2]);
    expect(parseColour("#f00")).toEqual([1, 0, 0]);
    expect(parseColour("rgb(300 0 0)")).toBeNull();
    expect(parseColour("blue")).toBeNull();
  });

  it("makes four of however many there are, and djmanzo's own with none", () => {
    expect(palette([])).toEqual(FALLBACK);
    const two = palette([{ colour: "rgb(255 0 0)" }, { colour: "rgb(0 0 255)" }]);
    expect(two).toEqual([
      [1, 0, 0],
      [0, 0, 1],
      [1, 0, 0],
      [0, 0, 1],
    ]);
  });
});
