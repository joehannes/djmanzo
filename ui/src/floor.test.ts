import { describe, expect, it } from "vitest";

import {
  BINS,
  LEARNED_AFTER,
  bands,
  clicks,
  flow,
  histogram,
  learn,
  lighting,
  occupied,
  sceneChange,
  type Background,
} from "./floor";

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

describe("§34's occupancy and crowd density, as proxies", () => {
  const W = 64;
  const H = 48;
  /** A dark room with a little texture, the same every look. */
  const room = () => {
    const out = new Float32Array(W * H);
    for (let i = 0; i < out.length; i++) out[i] = 20 + 10 * Math.sin(i * 0.37);
    return out;
  };
  /** The room with bright squares of `side` pixels at each of `at`. */
  const withFigures = (at: [number, number][], side: number) => {
    const out = room();
    for (const [x0, y0] of at) {
      for (let y = y0; y < y0 + side && y < H; y++) {
        for (let x = x0; x < x0 + side && x < W; x++) out[y * W + x] = 160;
      }
    }
    return out;
  };
  const learned = (looks: number) => {
    let background: Background | null = null;
    for (let i = 0; i < looks; i++) background = learn(background, room());
    return background;
  };

  it("reads nothing until the room's look has been learned", () => {
    const frame = withFigures([[10, 10]], 6);
    expect(occupied(learned(LEARNED_AFTER - 1), frame, W, H)).toEqual({ occupancy: null, density: null });
    expect(occupied(null, frame, W, H)).toEqual({ occupancy: null, density: null });
    expect(occupied(learned(LEARNED_AFTER), room(), W, H).occupancy).toBe(0);
  });

  it("counts more of the picture occupied by many figures than by a few", () => {
    const background = learned(20);
    const few = occupied(background, withFigures([[4, 4], [50, 36]], 6), W, H);
    const packed: [number, number][] = [];
    for (let y = 12; y < 40; y += 7) for (let x = 8; x < 56; x += 7) packed.push([x, y]);
    const many = occupied(background, withFigures(packed, 6), W, H);
    expect(few.occupancy).toBeCloseTo((2 * 36) / (W * H), 5);
    expect(many.occupancy!).toBeGreaterThan(few.occupancy! * 5);
    // Packed together, the occupied blocks have occupied blocks beside them;
    // two figures across the room have only their own.
    expect(many.density!).toBeGreaterThan(0.5);
    expect(few.density!).toBeLessThan(0.25);
  });

  it("does not hold the first frame, with somebody in it, as the empty room", () => {
    // The camera is switched on with a figure in view, who then walks off.
    let background: Background | null = learn(null, withFigures([[29, 20]], 6));
    for (let i = 1; i < LEARNED_AFTER + 2; i++) background = learn(background, room());
    expect(occupied(background, room(), W, H).occupancy).toBe(0);
  });

  it("does not read specks scattered over the picture as a packed crowd", () => {
    const background = learned(20);
    const specks = room();
    // One pixel in every block over most of the picture: sensor noise, or
    // glitter, and nothing that fills a block.
    for (let y = 2; y < H - 4; y += 4) for (let x = 2; x < W - 4; x += 4) specks[y * W + x] = 200;
    const read = occupied(background, specks, W, H);
    expect(read.occupancy!).toBeGreaterThan(0);
    expect(read.density).toBeNull();
  });

  it("says nothing about how three blocks of nothing pack", () => {
    const background = learned(20);
    const tiny = occupied(background, withFigures([[30, 20]], 2), W, H);
    expect(tiny.occupancy!).toBeGreaterThan(0);
    expect(tiny.density).toBeNull();
  });

  it("lets a crowd that keeps moving stay out of the room's look, and lets a lamp in", () => {
    // A figure that moves every look never becomes the room.
    let background: Background | null = null;
    for (let i = 0; i < 200; i++) background = learn(background, withFigures([[(i * 9) % 58, 20]], 6));
    const moving = occupied(background, withFigures([[29, 20]], 6), W, H);
    expect(moving.occupancy!).toBeGreaterThan(0.005);
    // A light left on in one place does, given long enough.
    let lamp: Background | null = learned(20);
    for (let i = 0; i < 300; i++) lamp = learn(lamp, withFigures([[29, 20]], 6));
    expect(occupied(lamp, withFigures([[29, 20]], 6), W, H).occupancy).toBe(0);
  });
});

describe("lighting moving, as distinct from the scene changing", () => {
  const W = 64;
  const H = 48;
  /** A dark textured room with a bright vertical beam `wide` pixels across at `x`. */
  const beam = (x: number, wide = 3) => {
    const out = new Float32Array(W * H);
    for (let y = 0; y < H; y++) {
      for (let px = 0; px < W; px++) {
        out[y * W + px] = px >= x && px < x + wide ? 240 : 15 + 10 * Math.sin(px * 0.5 + y * 0.3);
      }
    }
    return out;
  };

  it("follows a beam sweeping across the room, which does not change the scene", () => {
    const moved = lighting(beam(10), beam(30), W, H);
    expect(moved!).toBeGreaterThan(0.9);
    expect(sceneChange(histogram(beam(10)), histogram(beam(30)))).toBeLessThan(0.05);
    expect(lighting(beam(10), beam(12), W, H)!).toBeCloseTo(2 / (Math.hypot(W, H) * 0.1), 5);
  });

  it("does not follow a single bright speck", () => {
    const speck = (x: number) => {
      const out = new Float32Array(W * H).fill(30);
      out[20 * W + x] = 250;
      out[20 * W + x + 1] = 250;
      return out;
    };
    expect(lighting(speck(10), speck(40), W, H)).toBeNull();
  });

  it("reads a still light as still, and has nothing to follow in the dark", () => {
    expect(lighting(beam(20), beam(20), W, H)).toBe(0);
    const dark = new Float32Array(W * H).fill(30);
    expect(lighting(dark, beam(20), W, H)).toBeNull();
    // The lights snapping on change the scene and leave nothing to follow
    // from: the two readings are about different things.
    const bright = new Float32Array(W * H).fill(150);
    expect(sceneChange(histogram(dark), histogram(bright))).toBeGreaterThan(0.9);
    expect(lighting(dark, bright, W, H)).toBeNull();
  });
});
