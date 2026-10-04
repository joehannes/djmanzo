/**
 * §122: MilkDrop as the stack's main layer — Butterchurn (MIT) drawing a
 * preset, fed the room as djmanzo already hears it. See `docs/VISUALS.md`
 * step 4.
 *
 * Butterchurn expects sound: a block of samples it takes the spectrum of, to
 * find its own bass, mid and treble. djmanzo has no sound in the webview — the
 * audio is Rust's — but it has the master's four bands sixty times a second.
 * So the sound it is given is **made from the bands**: a tone in each,
 * as loud as the band, which lands in Butterchurn's bass (20–320 Hz), mid
 * (320–2800 Hz) and treble (2800–11025 Hz) as the room's would.
 *
 * It runs with `onlyUseWASM`: a preset's equations are compiled to
 * WebAssembly, never evaluated as JavaScript, which the content rules forbid.
 * Butterchurn is loaded only when a look has a MilkDrop layer.
 */
import type { Visualizer } from "butterchurn";

/** How many samples Butterchurn takes the spectrum of, at the rate it assumes with no audio context. */
export const SAMPLES = 1024;
export const RATE = 44_100;
/**
 * The tone each band is played as: bass and low mid in Butterchurn's bass and
 * mid, high mid in its mid, treble in its treble.
 */
export const TONES = [100, 600, 2_000, 6_000] as const;

/**
 * One block of sound made from the bands, as the unsigned bytes Butterchurn
 * reads (128 is silence). `phases` carries each tone on from the block
 * before, so the sound is continuous rather than restarting every frame; it
 * is advanced by `seconds`.
 */
export function tones(
  bands: readonly number[],
  phases: number[],
  seconds: number,
  into: Uint8Array = new Uint8Array(SAMPLES),
): Uint8Array {
  // A band with no reading is silent: NaN would become a byte of 0, a full
  // swing rather than nothing.
  const levels = TONES.map((_, i) => {
    const band = bands[i] ?? 0;
    return Number.isFinite(band) ? Math.min(1, Math.max(0, band)) : 0;
  });
  // Four tones at full level sum to 1, and no more: never clipped.
  const scale = 1 / TONES.length;
  for (let n = 0; n < into.length; n += 1) {
    let sample = 0;
    for (let i = 0; i < TONES.length; i += 1) {
      sample += levels[i] * scale * Math.sin(phases[i] + (2 * Math.PI * TONES[i] * n) / RATE);
    }
    into[n] = Math.round(128 + 127 * sample);
  }
  for (let i = 0; i < TONES.length; i += 1) {
    phases[i] = (phases[i] + 2 * Math.PI * TONES[i] * seconds) % (2 * Math.PI);
  }
  return into;
}

/**
 * Whether an `OffscreenCanvas` here can have WebGL 2. Butterchurn draws into
 * one whenever the browser has the class — and WebKitGTK, djmanzo's webview on
 * Linux, has the class but gives it no WebGL 2 context, so Butterchurn failed
 * there at its first framebuffer while every browser test passed. Found by
 * driving it (HANDOFF.md).
 */
function offscreenHasWebgl2(): boolean {
  if (typeof OffscreenCanvas === "undefined") return false;
  try {
    return new OffscreenCanvas(1, 1).getContext("webgl2") !== null;
  } catch {
    return false;
  }
}

/**
 * Run `work` with WebAssembly compiled synchronously where the browser allows.
 *
 * In WebKitGTK — djmanzo's Linux webview — the asynchronous
 * `WebAssembly.instantiate` never settled, not even for an empty module,
 * while `new WebAssembly.Module` compiled at once under the same content
 * rules. Butterchurn's two loaders compile asynchronously, so its first
 * preset waited forever, with no error to name. Chromium, the other way
 * round, refuses a synchronous compile of more than a few kilobytes on the
 * main thread with a `RangeError`, and that refusal falls back to the
 * asynchronous call it allows. Found by driving it (HANDOFF.md).
 *
 * Only while a preset loads, and put back afterwards.
 */
export async function withSyncWasm<T>(work: () => Promise<T>): Promise<T> {
  const compile = WebAssembly.compile;
  const instantiate = WebAssembly.instantiate;
  const refusedHere = (error: unknown) => error instanceof RangeError;
  WebAssembly.compile = (bytes: BufferSource) => {
    try {
      return Promise.resolve(new WebAssembly.Module(bytes));
    } catch (error) {
      return refusedHere(error) ? compile(bytes) : Promise.reject(error);
    }
  };
  WebAssembly.instantiate = ((source: BufferSource | WebAssembly.Module, imports?: WebAssembly.Imports) => {
    try {
      if (source instanceof WebAssembly.Module) return Promise.resolve(new WebAssembly.Instance(source, imports));
      const module = new WebAssembly.Module(source);
      return Promise.resolve({ module, instance: new WebAssembly.Instance(module, imports) });
    } catch (error) {
      return refusedHere(error)
        ? (instantiate as (s: unknown, i?: WebAssembly.Imports) => Promise<unknown>)(source, imports)
        : Promise.reject(error);
    }
  }) as typeof WebAssembly.instantiate;
  try {
    return await work();
  } finally {
    WebAssembly.compile = compile;
    WebAssembly.instantiate = instantiate;
  }
}

export interface Milkdrop {
  /** What Butterchurn draws into: the stack takes it as a picture. */
  canvas: HTMLCanvasElement;
  /** Draw a frame, the room's bands as its sound. */
  render(bands: readonly number[], seconds: number): void;
  resize(width: number, height: number): void;
}

/**
 * Butterchurn, drawing `preset` at `width` × `height`.
 *
 * Throws — and the caller leaves the layer out and says why — when it cannot:
 * no WebGL 2, WebAssembly refused, or a preset its compiler will not take.
 */
export async function createMilkdrop(preset: object, width: number, height: number): Promise<Milkdrop> {
  const { default: butterchurn } = await import("butterchurn");
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  // Where an offscreen canvas cannot have WebGL 2, Butterchurn is not shown
  // the class while it is made, and draws into an ordinary canvas instead —
  // the only place it looks for one.
  const hide = typeof OffscreenCanvas !== "undefined" && !offscreenHasWebgl2();
  const offscreen = globalThis.OffscreenCanvas;
  let visualizer: Visualizer;
  try {
    if (hide) (globalThis as { OffscreenCanvas?: unknown }).OffscreenCanvas = undefined;
    // No audio context: Butterchurn is handed its samples, below.
    visualizer = butterchurn.createVisualizer(null, canvas, {
      width,
      height,
      pixelRatio: 1,
      textureRatio: 1,
      onlyUseWASM: true,
    });
  } finally {
    if (hide) globalThis.OffscreenCanvas = offscreen;
  }
  await withSyncWasm(() => visualizer.loadPreset(preset, 0));
  const phases = TONES.map(() => 0);
  const block = new Uint8Array(SAMPLES);
  return {
    canvas,
    render(bands, seconds) {
      tones(bands, phases, seconds, block);
      visualizer.render({
        audioLevels: { timeByteArray: block, timeByteArrayL: block, timeByteArrayR: block },
        elapsedTime: seconds,
      });
    },
    resize(w, h) {
      if (canvas.width === w && canvas.height === h) return;
      canvas.width = w;
      canvas.height = h;
      visualizer.setRendererSize(w, h);
    },
  };
}
