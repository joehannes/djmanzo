import { describe, expect, it } from "vitest";

import { BINS, bands, clicks, flow, histogram, sceneChange } from "./floor";

const RATE = 48_000;

/** A small seeded generator, so a failure reproduces. */
function seeded(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** A two-thirds-of-a-second window, the analyser's longest. */
const WINDOW = 32768;

/** A sharp click: a few samples of noise, decaying fast. */
function click(into: Float32Array, at: number, random: () => number) {
  for (let i = 0; i < 96 && at + i < into.length; i++) {
    into[at + i] += (random() * 2 - 1) * 0.8 * Math.exp(-i / 20);
  }
}

describe("the room's sound in three bands", () => {
  const FFT = 32768;
  const spectrum = (loud: (hz: number) => boolean) => {
    const out = new Float32Array(FFT / 2).fill(-200);
    for (let bin = 0; bin < out.length; bin++) if (loud((bin * RATE) / FFT)) out[bin] = -40;
    return out;
  };

  it("puts a bass-heavy sound in the low band and nowhere else", () => {
    const read = bands(spectrum((hz) => hz > 40 && hz < 200), RATE);
    expect(read.low).toBeGreaterThan(0.5);
    expect(read.mid).toBe(0);
    expect(read.high).toBe(0);
  });

  it("puts a voice in the mids and a cymbal in the highs", () => {
    expect(bands(spectrum((hz) => hz > 300 && hz < 1800), RATE)).toMatchObject({ low: 0, high: 0 });
    const cymbal = bands(spectrum((hz) => hz > 5000 && hz < 10000), RATE);
    expect(cymbal.high).toBeGreaterThan(0.5);
    expect(cymbal.low).toBe(0);
  });

  it("reads a wider band as more, since a band is the energy in it", () => {
    const narrow = bands(spectrum((hz) => hz > 3000 && hz < 3500), RATE).high;
    const wide = bands(spectrum((hz) => hz > 3000 && hz < 9000), RATE).high;
    expect(wide).toBeGreaterThan(narrow);
  });

  it("reads each bin at its own frequency, from the spectrum's own length", () => {
    // 200 to 240 Hz is low; read as if the bins were twice as wide it would
    // be 400 to 480 Hz, a voice.
    const read = bands(spectrum((hz) => hz > 200 && hz < 240), RATE);
    expect(read.low).toBeGreaterThan(0.3);
    expect(read.mid).toBe(0);
  });
});

describe("applause-like clicks", () => {
  it("scores a room's worth of clapping, each on its own time", () => {
    const random = seeded(7);
    const sound = new Float32Array(WINDOW);
    for (let i = 0; i < sound.length; i++) sound[i] = (random() * 2 - 1) * 0.002;
    // A room's worth: about forty claps a second that stand apart, at random
    // moments (a real room makes more, and they blur into each other).
    let at = 0;
    while (at < WINDOW) {
      click(sound, Math.floor(at), random);
      at += (-Math.log(1 - random()) / 40) * RATE + 0.02 * RATE;
    }
    expect(clicks(sound, RATE)).toBeGreaterThan(0.4);
  });

  it("scores a hi-hat keeping time as nothing, however fast", () => {
    const random = seeded(9);
    const sound = new Float32Array(WINDOW);
    for (let i = 0; i < sound.length; i++) sound[i] = (random() * 2 - 1) * 0.002;
    // Sixteenths at 128 BPM: a click every 117 ms, exactly.
    const every = Math.round((60 / 128 / 4) * RATE);
    for (let at = 0; at < WINDOW; at += every) click(sound, at, random);
    expect(clicks(sound, RATE)).toBeLessThan(0.05);
  });

  it("finds no clicks in silence, steady noise or a tone", () => {
    const random = seeded(11);
    const noise = new Float32Array(WINDOW).map(() => (random() * 2 - 1) * 0.3);
    const tone = new Float32Array(WINDOW).map((_, i) => 0.5 * Math.sin((2 * Math.PI * 440 * i) / RATE));
    expect(clicks(new Float32Array(WINDOW), RATE)).toBe(0);
    expect(clicks(noise, RATE)).toBeLessThan(0.05);
    expect(clicks(tone, RATE)).toBe(0);
  });
});

describe("movement across the picture", () => {
  const W = 64;
  const H = 48;
  /** A picture with enough texture everywhere for every block to read. */
  const textured = () => {
    const random = seeded(3);
    // Blobs rather than pixel noise: a camera's picture is smooth at this
    // scale, and a block matcher is built for that.
    const out = new Float32Array(W * H);
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        out[y * W + x] = 128 + 60 * Math.sin(x * 0.7 + Math.sin(y * 0.4) * 3) + 40 * Math.cos(y * 0.9 + x * 0.2);
      }
    }
    for (let i = 0; i < out.length; i++) out[i] += random() * 4;
    return out;
  };
  /** `picture` with each pixel moved by `by(x, y)`, edges held. */
  const moved = (picture: Float32Array, by: (x: number, y: number) => [number, number]) => {
    const out = new Float32Array(W * H);
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        const [dx, dy] = by(x, y);
        const sx = Math.min(W - 1, Math.max(0, x - dx));
        const sy = Math.min(H - 1, Math.max(0, y - dy));
        out[y * W + x] = picture[sy * W + sx];
      }
    }
    return out;
  };

  it("reads a whole picture moving one way as moving together, evenly", () => {
    const before = textured();
    const read = flow(before, moved(before, () => [2, 0]), W, H);
    expect(read.together).toBeGreaterThan(0.9);
    expect(read.uneven).toBeLessThan(0.15);
  });

  it("reads halves moving opposite ways as uneven, and not together", () => {
    const before = textured();
    const read = flow(before, moved(before, (x) => (x < W / 2 ? [2, 0] : [-2, 0])), W, H);
    expect(read.together).toBeLessThan(0.2);
    expect(read.uneven).toBeGreaterThan(0.3);
  });

  it("reads a still picture as even, with nothing moving to be together", () => {
    const before = textured();
    expect(flow(before, before.slice(), W, H)).toEqual({ uneven: 0, together: null });
  });

  it("reads a dim, still picture through sensor noise as still", () => {
    // Barely enough texture to read, as in a dark room, and a camera's own
    // noise on both frames: every pixel up to eight levels off, independently.
    // A block of that fits a little better somewhere else by chance, and
    // only the margin a move must win by keeps it from reading as movement.
    const random = seeded(3);
    const dim = new Float32Array(W * H);
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        dim[y * W + x] = 100 + 6 * Math.sin(x * 0.7 + Math.sin(y * 0.4) * 3) + 3.6 * Math.cos(y * 0.9 + x * 0.2);
      }
    }
    const after = dim.map((v) => v + (random() * 2 - 1) * 8);
    const before = dim.map((v) => v + (random() * 2 - 1) * 8);
    const read = flow(before, after, W, H);
    expect(read.uneven).not.toBeNull();
    expect(read.together).toBeNull();
    expect(read.uneven).toBeLessThan(0.01);
  });

  it("reads nothing at all from a blank picture", () => {
    const blank = new Float32Array(W * H).fill(30);
    expect(flow(blank, blank.slice(), W, H)).toEqual({ uneven: null, together: null });
  });
});

describe("the picture changing as a whole", () => {
  it("is nothing for the same picture and everything for black after white", () => {
    const grey = histogram(new Float32Array(3072).map((_, i) => i % 256));
    expect(sceneChange(grey, grey)).toBeCloseTo(0, 6);
    const black = histogram(new Float32Array(3072).fill(5));
    const white = histogram(new Float32Array(3072).fill(250));
    expect(sceneChange(black, white)).toBeCloseTo(1, 6);
    expect(grey.length).toBe(BINS);
  });

  it("reads a room brightening by half as half changed", () => {
    const dark = new Float32Array(1000).fill(20);
    const half = dark.map((v, i) => (i < 500 ? 200 : v));
    expect(sceneChange(histogram(dark), histogram(half))).toBeCloseTo(0.5, 6);
  });
});
