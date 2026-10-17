/**
 * Stand-ins for §34 / §36's other signals: footage and sound drawn to make
 * each one move, written beside `senses/media.ts`'s and for the same reasons
 * (nothing downloadable is reachable or committable; a seed makes a failure
 * reproduce).
 *
 * The club, the empty room and the beach in `senses/media.ts` were drawn for
 * light, movement and loudness. These are drawn for what those three cannot
 * show:
 *
 * - **sway** — the whole picture drifting one way, the way a camera pans or a
 *   floor moves as one: movement that is *together* and *even*;
 * - **scatter** — bright figures each going its own way across a dark room:
 *   movement that is *uneven* and *not together*;
 * - **cut** — a still room whose lights snap between dark and bright every
 *   second: the picture changing *as a whole*;
 * - **few** — two figures wandering a large dark room: little of the picture
 *   *occupied*, and what is, *not packed*;
 * - **crowd** — thirty figures milling in the middle of the same room: much
 *   of it *occupied*, and *packed together*;
 * - **beam** — a still dark room with one bright beam sweeping across it: the
 *   *lighting moving* while the *scene* stays as it was;
 * - **claps** — dense, sharp clicks at random moments over a quiet room, the
 *   way a room applauding sounds;
 * - **hats** — the same clicks, high and on a sixteenth-note grid at 128 BPM:
 *   just as sharp, perfectly regular, all treble;
 * - **bass** — a sub-bass tone and its octave and nothing else;
 * - **switch** — four seconds of that bass, then four of the hats: what one
 *   look hears must be that look's, not an echo of the last one.
 *
 * **Not a claim about any real room.** A test that passes against these
 * proves the capture path runs and that each measurement moves the way it
 * should when the thing it measures does — nothing about a real floor.
 */
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

export const MEDIA_DIR = join(tmpdir(), "djmanzo-floor");

export const WIDE = 320;
export const HIGH = 240;
export const FPS = 15;
export const SECONDS = 8;
export const SAMPLE_RATE = 48_000;

export type Footage = "sway" | "scatter" | "cut" | "few" | "crowd" | "beam";
export type Sound = "claps" | "hats" | "bass" | "switch";

export const footagePath = (scene: Footage) => join(MEDIA_DIR, `${scene}.y4m`);
export const soundPath = (kind: Sound) => join(MEDIA_DIR, `${kind}.wav`);

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

/**
 * A texture that repeats every {@link WIDE} pixels across, so a picture
 * drifting sideways wraps without a seam and keeps the same brightness —
 * which is what makes *sway* read as no scene change at all.
 */
function texture(x: number, y: number): number {
  const k = (2 * Math.PI) / WIDE;
  return (
    0.5 +
    0.22 * Math.sin(k * 7 * x + Math.sin(y * 0.05) * 2) +
    0.18 * Math.cos(k * 11 * x + y * 0.09) +
    0.08 * Math.sin(y * 0.21)
  );
}

interface Figure {
  x: number;
  y: number;
  dx: number;
  dy: number;
}

/** Luma of one frame, 0..1 per pixel. */
function draw(scene: Footage, t: number, out: Float32Array, figures: Figure[]) {
  if (scene === "sway") {
    // Sixty pixels a second: two of the measured 64×48 frame's pixels across
    // the sixth of a second movement is read over.
    const shift = 60 * t;
    for (let y = 0; y < HIGH; y++) {
      for (let x = 0; x < WIDE; x++) out[y * WIDE + x] = texture(x - shift, y);
    }
    return;
  }
  if (scene === "cut") {
    const bright = Math.floor(t) % 2 === 1;
    for (let y = 0; y < HIGH; y++) {
      for (let x = 0; x < WIDE; x++) {
        const base = texture(x, y);
        out[y * WIDE + x] = bright ? 0.55 + 0.4 * base : 0.05 + 0.2 * base;
      }
    }
    return;
  }
  if (scene === "beam") {
    // One beam, sixteen pixels wide, swinging across most of the room every
    // two seconds. As many pixels are lit in every frame, so the picture's
    // brightness as a whole never changes; only where the light is does.
    const centre = WIDE / 2 + (WIDE / 2 - 30) * Math.sin(Math.PI * t);
    for (let y = 0; y < HIGH; y++) {
      for (let x = 0; x < WIDE; x++) {
        out[y * WIDE + x] = Math.abs(x - centre) < 8 ? 0.95 : 0.03 + 0.1 * texture(x, y);
      }
    }
    return;
  }
  // Scatter, few and crowd: a dark, featureless room, and figures striped so
  // a camera can see them move, each on its own heading, bouncing off the
  // walls — of the room, or of the middle of it for the crowd.
  out.fill(0.08);
  const R = scene === "scatter" ? 22 : 16;
  const [left, top, right, bottom] =
    scene === "crowd" ? [60, 50, WIDE - 60, HIGH - 50] : [0, 0, WIDE, HIGH];
  for (const f of figures) {
    let x = f.x + f.dx * t;
    let y = f.y + f.dy * t;
    const spanX = right - left - 2 * R;
    const spanY = bottom - top - 2 * R;
    x = left + R + Math.abs(((((x - left - R) % (2 * spanX)) + 2 * spanX) % (2 * spanX)) - spanX);
    y = top + R + Math.abs(((((y - top - R) % (2 * spanY)) + 2 * spanY) % (2 * spanY)) - spanY);
    for (let py = Math.floor(y - R); py <= y + R; py++) {
      for (let px = Math.floor(x - R); px <= x + R; px++) {
        if (px < 0 || py < 0 || px >= WIDE || py >= HIGH) continue;
        const ox = px - x;
        const oy = py - y;
        if (ox * ox + oy * oy > R * R) continue;
        out[py * WIDE + px] = 0.45 + 0.4 * Math.sin(ox * 0.45) * Math.cos(oy * 0.35);
      }
    }
  }
}

const clamp8 = (v: number) => Math.max(0, Math.min(255, Math.round(v * 255)));

/** One scene as YUV4MPEG2, grey (chroma at 128): only luma is measured. */
export function writeFootage(scene: Footage): string {
  const random = seeded(scene === "scatter" ? 21 : scene === "few" ? 25 : scene === "crowd" ? 27 : 23);
  const count = scene === "few" ? 2 : scene === "crowd" ? 30 : 9;
  const figures: Figure[] = Array.from({ length: count }, () => {
    const heading = random() * 2 * Math.PI;
    const speed = 50 + random() * 30;
    return {
      x: 30 + random() * (WIDE - 60),
      y: 30 + random() * (HIGH - 60),
      dx: Math.cos(heading) * speed,
      dy: Math.sin(heading) * speed,
    };
  });
  const frames = SECONDS * FPS;
  const header = Buffer.from(`YUV4MPEG2 W${WIDE} H${HIGH} F${FPS}:1 Ip A1:1 C420jpeg\n`);
  const marker = Buffer.from("FRAME\n");
  const luma = WIDE * HIGH;
  const chroma = (WIDE / 2) * (HIGH / 2);
  const out = Buffer.alloc(header.length + frames * (marker.length + luma + 2 * chroma), 128);
  header.copy(out, 0);
  const picture = new Float32Array(luma);
  let at = header.length;
  for (let f = 0; f < frames; f++) {
    draw(scene, f / FPS, picture, figures);
    marker.copy(out, at);
    at += marker.length;
    for (let p = 0; p < luma; p++) out[at + p] = clamp8(picture[p]);
    at += luma + 2 * chroma;
  }
  mkdirSync(MEDIA_DIR, { recursive: true });
  writeFileSync(footagePath(scene), out);
  return footagePath(scene);
}

/** A clap: a few milliseconds of noise, sharp at the front, gone fast. */
function clap(into: Float32Array, at: number, random: () => number, level: number, highOnly: boolean) {
  let last = 0;
  for (let i = 0; i < 240 && at + i < into.length; i++) {
    const noise = random() * 2 - 1;
    // A hi-hat is the same click with its body removed: a first difference
    // keeps only the top of the spectrum.
    const sample = highOnly ? noise - last : noise;
    last = noise;
    into[at + i] += sample * level * Math.exp(-i / 40);
  }
}

function sound(kind: Sound): Float32Array {
  const n = SECONDS * SAMPLE_RATE;
  if (kind === "switch") {
    const bass = sound("bass");
    const hats = sound("hats");
    return bass.map((v, i) => (i < n / 2 ? v : hats[i]));
  }
  const random = seeded(kind === "claps" ? 31 : kind === "hats" ? 37 : 41);
  const out = new Float32Array(n);
  if (kind === "bass") {
    for (let i = 0; i < n; i++) {
      const t = i / SAMPLE_RATE;
      out[i] = 0.3 * Math.sin(2 * Math.PI * 55 * t) + 0.15 * Math.sin(2 * Math.PI * 110 * t);
    }
    return out;
  }
  for (let i = 0; i < n; i++) out[i] = (random() * 2 - 1) * 0.003;
  if (kind === "hats") {
    const every = Math.round((60 / 128 / 4) * SAMPLE_RATE);
    for (let at = 0; at < n; at += every) clap(out, at, random, 0.5, true);
    return out;
  }
  // Claps: about forty that stand apart each second, at random moments.
  let at = 0;
  while (at < n) {
    clap(out, Math.floor(at), random, 0.4 + 0.3 * random(), false);
    at += (-Math.log(1 - random()) / 40) * SAMPLE_RATE + 0.02 * SAMPLE_RATE;
  }
  return out;
}

/** One sound as 16-bit mono PCM WAV. */
export function writeSound(kind: Sound): string {
  const samples = sound(kind);
  const n = samples.length;
  const out = Buffer.alloc(44 + n * 2);
  out.write("RIFF", 0);
  out.writeUInt32LE(36 + n * 2, 4);
  out.write("WAVE", 8);
  out.write("fmt ", 12);
  out.writeUInt32LE(16, 16);
  out.writeUInt16LE(1, 20);
  out.writeUInt16LE(1, 22);
  out.writeUInt32LE(SAMPLE_RATE, 24);
  out.writeUInt32LE(SAMPLE_RATE * 2, 28);
  out.writeUInt16LE(2, 32);
  out.writeUInt16LE(16, 34);
  out.write("data", 36);
  out.writeUInt32LE(n * 2, 40);
  for (let i = 0; i < n; i++) {
    out.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round(samples[i] * 32767))), 44 + i * 2);
  }
  mkdirSync(MEDIA_DIR, { recursive: true });
  writeFileSync(soundPath(kind), out);
  return soundPath(kind);
}

/** Every file, written once per run; a file already there is reused. */
export function ensureMedia() {
  for (const scene of ["sway", "scatter", "cut", "few", "crowd", "beam"] as const) {
    if (!existsSync(footagePath(scene))) writeFootage(scene);
  }
  for (const kind of ["claps", "hats", "bass", "switch"] as const) {
    if (!existsSync(soundPath(kind))) writeSound(kind);
  }
}
