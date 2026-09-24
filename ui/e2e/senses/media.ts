/**
 * Stand-ins for a camera pointed at a floor and a microphone in a booth.
 *
 * # Why these exist
 *
 * djmanzo reads a room through a lens and a diaphragm, and the machine it is
 * built and tested on has neither. Until these files existed, everything
 * between `getUserMedia` and `room_saw` — the constraints, the fallback when a
 * device is missing, the canvas readback, the analyser, the scaling of three
 * numbers — had never run at all: the Rust side was tested against synthetic
 * *series*, and the page was tested against a stub that never saw a pixel.
 *
 * Chromium will play a file as its camera and another as its microphone
 * (`--use-file-for-fake-video-capture`, `--use-file-for-fake-audio-capture`),
 * through the same capture path a real device takes. So the job here is to
 * make files worth pointing it at.
 *
 * # Why they are generated, not downloaded
 *
 * Real festival footage would be better evidence, and the first plan was to use
 * some. It is not reachable from where this is built — the network policy
 * refuses the openly licensed archives — and footage cut from a streaming site
 * could not be committed as a fixture anyway. So the scenes are drawn here,
 * from a seed, every time the suite runs: nothing binary is committed, and a
 * failure reproduces exactly.
 *
 * # What they are and are not
 *
 * They are a **dark club at its peak** (four sweeping beams, a strobe, three
 * rows of people bouncing on the beat), a **room that has emptied** (one slow
 * beam, nobody there), and a **beach party in daylight**. The sounds are a
 * four-on-the-floor mix heard through a microphone that is being pushed, a room
 * of low chatter, and somebody humming a tune.
 *
 * They are **not a claim about any real room**. A test that passes against
 * them proves the capture path runs and that the readings move the right way
 * when the scene does; it proves nothing about what a particular webcam makes
 * of a particular club, and nothing here should be read as if it did.
 */
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

/** Where the files are written. Outside the tree: they are regenerated, never committed. */
export const MEDIA_DIR = join(tmpdir(), "djmanzo-senses");

/** The picture: small, because `RoomSense` scales it to 64×48 anyway. */
export const WIDE = 320;
export const HIGH = 240;
export const FPS = 15;

/**
 * Eight seconds, looped by Chromium. Sixteen beats at 120 BPM, so the music
 * loops on a bar line and the footage's bounce loops with it.
 */
export const SECONDS = 8;
export const BPM = 120;
export const SAMPLE_RATE = 48_000;

export type Footage = "peak" | "empty" | "daylight";
export type Sound = "peak" | "quiet" | "hum";

export const footagePath = (scene: Footage) => join(MEDIA_DIR, `${scene}.y4m`);
export const soundPath = (kind: Sound) => join(MEDIA_DIR, `${kind}.wav`);

/** A small seeded generator (mulberry32), so every run draws the same night. */
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

type Rgb = [number, number, number];

interface Beam {
  x: number;
  colour: Rgb;
  centre: number;
  swing: number;
  period: number;
  phase: number;
  width: number;
  strength: number;
}

interface Dancer {
  x: number;
  y: number;
  r: number;
  lag: number;
  sway: number;
  shade: Rgb;
}

function beamsFor(scene: Footage): Beam[] {
  if (scene === "daylight") return [];
  const all: Beam[] = [
    { x: 0.2, colour: [1, 0.2, 0.8], centre: 0.3, swing: 0.5, period: 4, phase: 0, width: 0.09, strength: 0.4 },
    { x: 0.8, colour: [0.2, 0.9, 1], centre: -0.3, swing: 0.5, period: 4, phase: 2, width: 0.09, strength: 0.4 },
    { x: 0.4, colour: [1, 0.7, 0.2], centre: 0, swing: 0.7, period: 8, phase: 1, width: 0.07, strength: 0.35 },
    { x: 0.6, colour: [0.3, 1, 0.3], centre: 0, swing: 0.6, period: 2, phase: 0.5, width: 0.06, strength: 0.3 },
  ];
  // The emptied room keeps one light on a slow sweep: the rig is still running,
  // and a camera pointed at it should see very little change — not none.
  return scene === "empty"
    ? [{ ...all[2], strength: 0.25, swing: 0.3, period: 8 }]
    : all;
}

function crowdFor(scene: Footage): Dancer[] {
  if (scene === "empty") return [];
  const random = seeded(scene === "peak" ? 7 : 11);
  const rows =
    scene === "peak"
      ? [
          { y: 0.62, r: 5, count: 16 },
          { y: 0.75, r: 8, count: 12 },
          { y: 0.9, r: 12, count: 8 },
        ]
      : [
          { y: 0.7, r: 6, count: 10 },
          { y: 0.85, r: 10, count: 7 },
        ];
  const people: Dancer[] = [];
  for (const row of rows) {
    for (let i = 0; i < row.count; i++) {
      people.push({
        x: (i + 0.5 + (random() - 0.5) * 0.6) / row.count,
        y: row.y + (random() - 0.5) * 0.04,
        r: row.r * (0.85 + random() * 0.3),
        // Nobody in a crowd is exactly on the beat.
        lag: (random() - 0.5) * 0.2,
        sway: 0.5 + random(),
        shade:
          scene === "daylight"
            ? [0.3 + random() * 0.15, 0.24 + random() * 0.1, 0.22 + random() * 0.1]
            : [0.2 + random() * 0.1, 0.18 + random() * 0.1, 0.22 + random() * 0.1],
      });
    }
  }
  return people;
}

/** How much light each beam puts on one point. */
function beamLight(beams: Beam[], t: number, px: number, py: number): Rgb {
  const out: Rgb = [0, 0, 0];
  for (const beam of beams) {
    const angle =
      beam.centre + beam.swing * Math.sin((2 * Math.PI * (t + beam.phase)) / beam.period);
    const ox = beam.x * WIDE;
    const at = Math.atan2(px - ox, py + 10);
    const off = (at - angle) / beam.width;
    // Haze: a beam is brightest near the rig and thins towards the floor.
    const k = beam.strength * Math.exp(-off * off) * (1 - 0.45 * (py / HIGH));
    out[0] += beam.colour[0] * k;
    out[1] += beam.colour[1] * k;
    out[2] += beam.colour[2] * k;
  }
  return out;
}

function background(scene: Footage, y: number): Rgb {
  if (scene === "daylight") {
    const v = y / HIGH;
    if (v < 0.45) {
      const s = v / 0.45;
      return [0.55 + 0.3 * s, 0.72 + 0.18 * s, 0.95];
    }
    if (v < 0.55) return [0.2, 0.45, 0.6];
    return [0.86, 0.76, 0.56];
  }
  const base = 0.03 + 0.02 * (y / HIGH);
  return [base * 0.9, base * 0.8, base * 1.4];
}

/** Draw one frame into `rgb`, which is WIDE×HIGH×3 floats. */
function drawFrame(scene: Footage, t: number, rgb: Float32Array, beams: Beam[], crowd: Dancer[]) {
  for (let y = 0; y < HIGH; y++) {
    const sky = background(scene, y);
    for (let x = 0; x < WIDE; x++) {
      const light = beamLight(beams, t, x, y);
      const i = (y * WIDE + x) * 3;
      rgb[i] = sky[0] + light[0];
      rgb[i + 1] = sky[1] + light[1];
      rgb[i + 2] = sky[2] + light[2];
    }
  }

  // People, back row first, so the front row stands in front of it.
  const beat = (t * BPM) / 60;
  for (const person of crowd) {
    const phase = beat + person.lag;
    const bounce = person.r * 0.9 * Math.abs(Math.sin(Math.PI * phase));
    const sway = person.r * 0.4 * Math.sin((Math.PI * phase) / 2) * person.sway;
    const cx = person.x * WIDE + sway;
    const cy = person.y * HIGH - bounce;
    const lit = beamLight(beams, t, cx, cy);
    const colour: Rgb = [
      person.shade[0] + lit[0] * 0.8,
      person.shade[1] + lit[1] * 0.8,
      person.shade[2] + lit[2] * 0.8,
    ];
    const r = person.r;
    // A head and a body; the body runs off the bottom of the frame.
    const x0 = Math.max(0, Math.floor(cx - r * 1.3));
    const x1 = Math.min(WIDE - 1, Math.ceil(cx + r * 1.3));
    const y0 = Math.max(0, Math.floor(cy - r));
    const y1 = Math.min(HIGH - 1, Math.ceil(cy + r * 5));
    for (let y = y0; y <= y1; y++) {
      for (let x = x0; x <= x1; x++) {
        const dx = x - cx;
        const dy = y - cy;
        const head = dx * dx + dy * dy <= r * r;
        const by = y - (cy + r * 2.4);
        const body = (dx * dx) / (1.3 * r * 1.3 * r) + (by * by) / (1.6 * r * 1.6 * r) <= 1;
        if (!head && !body) continue;
        const i = (y * WIDE + x) * 3;
        rgb[i] = colour[0];
        rgb[i + 1] = colour[1];
        rgb[i + 2] = colour[2];
      }
    }
  }

  // The strobe: one frame, twice a loop, on the peak only.
  if (scene === "peak") {
    const frame = Math.round(t * FPS);
    if (frame === Math.round(3.5 * FPS) || frame === Math.round(7.5 * FPS)) {
      for (let i = 0; i < rgb.length; i++) rgb[i] += 0.7;
    }
  }
}

const clamp8 = (v: number) => Math.max(0, Math.min(255, Math.round(v)));

/** Write one scene as YUV4MPEG2, 4:2:0, full range (what `C420jpeg` means). */
export function writeFootage(scene: Footage): string {
  const beams = beamsFor(scene);
  const crowd = crowdFor(scene);
  const frames = SECONDS * FPS;
  const header = Buffer.from(`YUV4MPEG2 W${WIDE} H${HIGH} F${FPS}:1 Ip A1:1 C420jpeg\n`);
  const marker = Buffer.from("FRAME\n");
  const lumaSize = WIDE * HIGH;
  const chromaSize = (WIDE / 2) * (HIGH / 2);
  const frameSize = marker.length + lumaSize + 2 * chromaSize;
  const out = Buffer.alloc(header.length + frames * frameSize);
  header.copy(out, 0);

  const rgb = new Float32Array(WIDE * HIGH * 3);
  let at = header.length;
  for (let f = 0; f < frames; f++) {
    drawFrame(scene, f / FPS, rgb, beams, crowd);
    marker.copy(out, at);
    at += marker.length;
    const yAt = at;
    const uAt = at + lumaSize;
    const vAt = uAt + chromaSize;
    for (let p = 0; p < lumaSize; p++) {
      const r = Math.min(1, Math.max(0, rgb[p * 3])) * 255;
      const g = Math.min(1, Math.max(0, rgb[p * 3 + 1])) * 255;
      const b = Math.min(1, Math.max(0, rgb[p * 3 + 2])) * 255;
      out[yAt + p] = clamp8(0.299 * r + 0.587 * g + 0.114 * b);
    }
    for (let cy = 0; cy < HIGH / 2; cy++) {
      for (let cx = 0; cx < WIDE / 2; cx++) {
        let r = 0;
        let g = 0;
        let b = 0;
        for (const [dx, dy] of [
          [0, 0],
          [1, 0],
          [0, 1],
          [1, 1],
        ]) {
          const p = ((cy * 2 + dy) * WIDE + cx * 2 + dx) * 3;
          r += Math.min(1, Math.max(0, rgb[p])) * 255;
          g += Math.min(1, Math.max(0, rgb[p + 1])) * 255;
          b += Math.min(1, Math.max(0, rgb[p + 2])) * 255;
        }
        r /= 4;
        g /= 4;
        b /= 4;
        const c = cy * (WIDE / 2) + cx;
        out[uAt + c] = clamp8(-0.168736 * r - 0.331264 * g + 0.5 * b + 128);
        out[vAt + c] = clamp8(0.5 * r - 0.418688 * g - 0.081312 * b + 128);
      }
    }
    at += lumaSize + 2 * chromaSize;
  }
  mkdirSync(MEDIA_DIR, { recursive: true });
  const path = footagePath(scene);
  writeFileSync(path, out);
  return path;
}

/** Scale a buffer to a target RMS, so each sound's loudness is a stated number. */
function toRms(samples: Float32Array, rms: number) {
  let squares = 0;
  for (const s of samples) squares += s * s;
  const now = Math.sqrt(squares / samples.length);
  const gain = now > 0 ? rms / now : 0;
  for (let i = 0; i < samples.length; i++) samples[i] *= gain;
}

/**
 * The target loudness of each sound, as RMS in dBFS.
 *
 * Exported so the test states its expectation against the number the file was
 * made to, rather than against a second copy of it.
 */
export const LOUDNESS_DBFS: Record<Sound, number> = {
  // A microphone in a booth during the peak is being pushed hard. It is then
  // soft-clipped, which moves the RMS a little; the test allows for that.
  peak: -10,
  quiet: -42,
  hum: -20,
};

function peakMix(n: number, random: () => number): Float32Array {
  const out = new Float32Array(n);
  const beat = 60 / BPM;
  let kickPhase = 0;
  let bandLow = 0;
  let bandHigh = 0;
  let lastNoise = 0;
  for (let i = 0; i < n; i++) {
    const t = i / SAMPLE_RATE;
    const into = t % beat;
    // Kick: a falling sine, on every beat.
    const f = 45 + 75 * Math.exp(-into / 0.03);
    kickPhase += (2 * Math.PI * f) / SAMPLE_RATE;
    const kick = Math.sin(kickPhase) * Math.exp(-into / 0.12);
    // Bass on the off-beat, three harmonics of A1.
    const off = into - beat / 2;
    const bassEnv = off >= 0 ? Math.exp(-off / 0.15) : 0;
    const bass =
      bassEnv *
      (Math.sin(2 * Math.PI * 55 * t) +
        0.5 * Math.sin(2 * Math.PI * 110 * t) +
        0.3 * Math.sin(2 * Math.PI * 165 * t));
    // Hats: a differenced noise burst on the off-beat.
    const white = random() * 2 - 1;
    const hat = off >= 0 ? (white - lastNoise) * Math.exp(-off / 0.02) : 0;
    lastNoise = white;
    // A pad, so there is something between the kick and the hat.
    const pad =
      0.08 *
      (Math.sin(2 * Math.PI * 220 * t) +
        Math.sin(2 * Math.PI * 261.63 * t) +
        Math.sin(2 * Math.PI * 329.63 * t));
    // The crowd: band-passed noise, swelling once a loop.
    bandLow += 0.3 * (white - bandLow);
    bandHigh += 0.03 * (white - bandHigh);
    const roar = (bandLow - bandHigh) * (0.6 + 0.4 * Math.sin((2 * Math.PI * t) / SECONDS));
    out[i] = 0.9 * kick + 0.45 * bass + 0.25 * hat + pad + 0.35 * roar;
  }
  toRms(out, 10 ** (LOUDNESS_DBFS.peak / 20));
  // A pushed microphone: soft-clipped rather than hard, as a preamp would.
  for (let i = 0; i < n; i++) out[i] = Math.tanh(1.2 * out[i]) / Math.tanh(1.2);
  return out;
}

function chatter(n: number, random: () => number): Float32Array {
  const out = new Float32Array(n);
  let low = 0;
  let high = 0;
  for (let i = 0; i < n; i++) {
    const t = i / SAMPLE_RATE;
    const white = random() * 2 - 1;
    low += 0.2 * (white - low);
    high += 0.02 * (white - high);
    // Syllables: a few a second, from several people at once.
    const syllables = 0.5 + 0.5 * Math.abs(Math.sin(2 * Math.PI * 2.1 * t) * Math.sin(2 * Math.PI * 3.3 * t + 1));
    out[i] = (low - high) * syllables;
  }
  toRms(out, 10 ** (LOUDNESS_DBFS.quiet / 20));
  return out;
}

/** The tune: A minor pentatonic, one note a beat, hummed with some vibrato. */
export const HUMMED_HZ = [220, 261.63, 293.66, 329.63, 392, 329.63, 293.66, 261.63];

function hum(n: number, random: () => number): Float32Array {
  const out = new Float32Array(n);
  const beat = 60 / BPM;
  let phase = 0;
  for (let i = 0; i < n; i++) {
    const t = i / SAMPLE_RATE;
    const note = Math.floor(t / beat);
    const into = t - note * beat;
    const hz = HUMMED_HZ[note % HUMMED_HZ.length];
    const vibrato = 1 + 0.004 * Math.sin(2 * Math.PI * 5.5 * t);
    phase += (2 * Math.PI * hz * vibrato) / SAMPLE_RATE;
    // Attack, hold, and a breath between notes.
    const env = Math.min(1, into / 0.03) * (into < beat * 0.88 ? 1 : Math.max(0, 1 - (into - beat * 0.88) / 0.05));
    let voice = 0;
    for (let h = 1; h <= 6; h++) voice += Math.sin(h * phase) / h ** 1.5;
    out[i] = env * voice + 0.01 * (random() * 2 - 1);
  }
  toRms(out, 10 ** (LOUDNESS_DBFS.hum / 20));
  return out;
}

/** Write one sound as 16-bit mono PCM WAV. */
export function writeSound(kind: Sound): string {
  const n = SECONDS * SAMPLE_RATE;
  const random = seeded(kind === "peak" ? 3 : kind === "quiet" ? 5 : 9);
  const samples = kind === "peak" ? peakMix(n, random) : kind === "quiet" ? chatter(n, random) : hum(n, random);
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
  const path = soundPath(kind);
  writeFileSync(path, out);
  return path;
}

/** Every file, written once per run; a file already there is reused. */
export function ensureMedia() {
  for (const scene of ["peak", "empty", "daylight"] as const) {
    if (!existsSync(footagePath(scene))) writeFootage(scene);
  }
  for (const kind of ["peak", "quiet", "hum"] as const) {
    if (!existsSync(soundPath(kind))) writeSound(kind);
  }
}
