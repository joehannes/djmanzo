/**
 * Which rendering strategy the living interface should be built on.
 *
 * ADR-0004's benchmark established something specific and uncomfortable: when
 * WebKitGTK has no accelerated compositing, animating anything costs a **full
 * page repaint**, and the cost tracks page area rather than animated area. That
 * is why four scrolling waveform lanes cost barely more than one.
 *
 * A living interface — continuous organic motion across the whole surface — is
 * the workload that hits that wall hardest, so the strategy cannot be chosen by
 * taste. But the same finding suggests the answer might be counter-intuitive:
 * if the cost is *document invalidation* rather than fill rate, then replacing N
 * animating DOM layers with **one self-repainting canvas** should be cheaper on
 * the bad path, not dearer. A canvas dirties its own rect; it does not
 * invalidate layout.
 *
 * This measures that, with the same shape count and the same motion, three ways:
 *
 * 1. **DOM** — N elements moved by `transform`, which is what the interface does
 *    today.
 * 2. **Canvas 2D** — one canvas, N paths per frame. Needs no GPU, and therefore
 *    cannot silently fall back to software: it already is software.
 * 3. **WebGL** — one canvas, N instances. Reports the driver string, because a
 *    context that creates successfully and is then backed by llvmpipe or
 *    SwiftShader is precisely the silent failure ADR-0004 was written about.
 *
 * Triggered by `DJMANZO_RENDERBENCH` so it never runs in a normal session.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const MEASURE_MS = 6_000;
/** Layer promotion, shader compilation and first-paint land here. */
const WARMUP_MS = 1_500;

/**
 * Shapes drawn per frame.
 *
 * Chosen to represent an ecosystem view rather than a stress test: four deck
 * organisms, their flow lines, the confluence, and the level fields. A number
 * nobody would actually ship would answer a question nobody is asking.
 */
let SHAPES = 240;

function percentile(sorted: number[], p: number): number {
  if (sorted.length === 0) return 0;
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))];
}

/** Run `draw` every frame for the measurement window and report the timings. */
async function measure(label: string, draw: (t: number) => void): Promise<void> {
  const deltas: number[] = [];
  const start = performance.now();
  let previous = start;

  await new Promise<void>((resolve) => {
    const tick = () => {
      const now = performance.now();
      const delta = now - previous;
      previous = now;
      if (now - start > WARMUP_MS) deltas.push(delta);
      if (now - start > WARMUP_MS + MEASURE_MS) {
        resolve();
        return;
      }
      draw((now - start) / 1000);
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });

  deltas.sort((a, b) => a - b);
  const mean = deltas.reduce((a, b) => a + b, 0) / Math.max(deltas.length, 1);
  await invoke("report_bench", {
    label,
    fps: 1000 / mean,
    p50Ms: percentile(deltas, 0.5),
    p95Ms: percentile(deltas, 0.95),
    worstMs: deltas[deltas.length - 1] ?? 0,
  });
}

/** A full-window host for whichever surface is being measured. */
function stage(): HTMLDivElement {
  const el = document.createElement("div");
  el.style.cssText =
    "position:fixed;inset:0;z-index:9999;background:#0b0f14;overflow:hidden";
  document.body.appendChild(el);
  return el;
}

/**
 * The same motion in every scenario, so the three are comparable.
 *
 * Orbital rather than linear because it never leaves the viewport and never
 * settles — a scenario that drifts offscreen stops costing anything and reads
 * as a win that is not there.
 */
function position(i: number, t: number, w: number, h: number) {
  const phase = (i / SHAPES) * Math.PI * 2;
  const speed = 0.4 + (i % 7) * 0.05;
  return {
    x: w * (0.5 + 0.42 * Math.cos(phase + t * speed)),
    y: h * (0.5 + 0.42 * Math.sin(phase * 1.3 + t * speed)),
    r: 6 + 10 * (0.5 + 0.5 * Math.sin(phase + t)),
    hue: (i / SHAPES) * 360,
  };
}

// -- 1. DOM ----------------------------------------------------------------

async function benchDom(): Promise<void> {
  const host = stage();
  const nodes: HTMLDivElement[] = [];
  for (let i = 0; i < SHAPES; i += 1) {
    const el = document.createElement("div");
    el.style.cssText = `position:absolute;left:0;top:0;width:24px;height:24px;border-radius:50%;will-change:transform;background:hsl(${(i / SHAPES) * 360} 70% 55%)`;
    host.appendChild(el);
    nodes.push(el);
  }
  const w = host.clientWidth;
  const h = host.clientHeight;

  await measure(`DOM · ${SHAPES} transformed elements`, (t) => {
    for (let i = 0; i < SHAPES; i += 1) {
      const p = position(i, t, w, h);
      nodes[i].style.transform = `translate3d(${p.x}px, ${p.y}px, 0) scale(${p.r / 12})`;
    }
  });
  host.remove();
}

// -- 2. Canvas 2D ----------------------------------------------------------

async function benchCanvas2d(): Promise<void> {
  const host = stage();
  const canvas = document.createElement("canvas");
  const w = host.clientWidth;
  const h = host.clientHeight;
  canvas.width = w;
  canvas.height = h;
  canvas.style.cssText = "width:100%;height:100%;display:block";
  host.appendChild(canvas);
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    await invoke("report_bench", {
      label: "Canvas2D · UNAVAILABLE",
      fps: 0,
      p50Ms: 0,
      p95Ms: 0,
      worstMs: 0,
    });
    host.remove();
    return;
  }

  await measure(`Canvas2D · ${SHAPES} filled paths`, (t) => {
    ctx.clearRect(0, 0, w, h);
    for (let i = 0; i < SHAPES; i += 1) {
      const p = position(i, t, w, h);
      ctx.beginPath();
      ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
      ctx.fillStyle = `hsl(${p.hue} 70% 55%)`;
      ctx.fill();
    }
  });
  host.remove();
}

// -- 3. WebGL --------------------------------------------------------------

const VERTEX = `
attribute vec2 corner;
attribute vec3 instance;   // x, y, radius
attribute vec3 tint;
uniform vec2 viewport;
varying vec3 colour;
varying vec2 local;
void main() {
  local = corner;
  colour = tint;
  vec2 px = instance.xy + corner * instance.z;
  gl_Position = vec4((px / viewport) * 2.0 - 1.0, 0.0, 1.0);
}`;

const FRAGMENT = `
precision mediump float;
varying vec3 colour;
varying vec2 local;
void main() {
  // A disc, so the shape matches the other two scenarios rather than being a
  // cheaper quad and flattering the result.
  if (dot(local, local) > 1.0) discard;
  gl_FragColor = vec4(colour, 1.0);
}`;

function compile(gl: WebGLRenderingContext, kind: number, source: string): WebGLShader | null {
  const shader = gl.createShader(kind);
  if (!shader) return null;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  return gl.getShaderParameter(shader, gl.COMPILE_STATUS) ? shader : null;
}

/**
 * The driver actually behind the context.
 *
 * A context that creates successfully and is then backed by llvmpipe or
 * SwiftShader is the silent failure ADR-0004 exists to fear, and the only way to
 * see it is to ask.
 */
function driverOf(gl: WebGLRenderingContext): string {
  const ext = gl.getExtension("WEBGL_debug_renderer_info");
  if (!ext) return String(gl.getParameter(gl.RENDERER));
  return String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL));
}

async function benchWebgl(): Promise<void> {
  const host = stage();
  const canvas = document.createElement("canvas");
  const w = host.clientWidth;
  const h = host.clientHeight;
  canvas.width = w;
  canvas.height = h;
  canvas.style.cssText = "width:100%;height:100%;display:block";
  host.appendChild(canvas);

  const gl = (canvas.getContext("webgl", { antialias: false, alpha: false }) ??
    canvas.getContext("experimental-webgl")) as WebGLRenderingContext | null;
  if (!gl) {
    await invoke("report_bench", {
      label: "WebGL · NO CONTEXT",
      fps: 0,
      p50Ms: 0,
      p95Ms: 0,
      worstMs: 0,
    });
    host.remove();
    return;
  }

  const driver = driverOf(gl);
  const instanced = gl.getExtension("ANGLE_instanced_arrays");
  const vs = compile(gl, gl.VERTEX_SHADER, VERTEX);
  const fs = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT);
  const program = gl.createProgram();
  if (!vs || !fs || !program || !instanced) {
    await invoke("report_bench", {
      label: `WebGL · UNUSABLE (${driver}${instanced ? "" : ", no instancing"})`,
      fps: 0,
      p50Ms: 0,
      p95Ms: 0,
      worstMs: 0,
    });
    host.remove();
    return;
  }
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  gl.useProgram(program);

  const quad = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, quad);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([-1, -1, 1, -1, -1, 1, 1, -1, 1, 1, -1, 1]),
    gl.STATIC_DRAW,
  );
  const cornerLoc = gl.getAttribLocation(program, "corner");
  gl.enableVertexAttribArray(cornerLoc);
  gl.vertexAttribPointer(cornerLoc, 2, gl.FLOAT, false, 0, 0);

  // Colour never changes, so it is uploaded once.
  const tints = new Float32Array(SHAPES * 3);
  for (let i = 0; i < SHAPES; i += 1) {
    const hue = (i / SHAPES) * 6;
    tints[i * 3] = Math.abs(((hue + 0) % 6) - 3) - 1;
    tints[i * 3 + 1] = 2 - Math.abs(((hue + 4) % 6) - 3);
    tints[i * 3 + 2] = 2 - Math.abs(((hue + 2) % 6) - 3);
  }
  const tintBuf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, tintBuf);
  gl.bufferData(gl.ARRAY_BUFFER, tints, gl.STATIC_DRAW);
  const tintLoc = gl.getAttribLocation(program, "tint");
  gl.enableVertexAttribArray(tintLoc);
  gl.vertexAttribPointer(tintLoc, 3, gl.FLOAT, false, 0, 0);
  instanced.vertexAttribDivisorANGLE(tintLoc, 1);

  const instances = new Float32Array(SHAPES * 3);
  const instanceBuf = gl.createBuffer();
  const instanceLoc = gl.getAttribLocation(program, "instance");
  gl.uniform2f(gl.getUniformLocation(program, "viewport"), w, h);
  gl.viewport(0, 0, w, h);
  gl.clearColor(0.04, 0.06, 0.08, 1);

  await measure(`WebGL · ${SHAPES} instances · ${driver}`, (t) => {
    for (let i = 0; i < SHAPES; i += 1) {
      const p = position(i, t, w, h);
      instances[i * 3] = p.x;
      instances[i * 3 + 1] = h - p.y;
      instances[i * 3 + 2] = p.r;
    }
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.bindBuffer(gl.ARRAY_BUFFER, instanceBuf);
    gl.bufferData(gl.ARRAY_BUFFER, instances, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(instanceLoc);
    gl.vertexAttribPointer(instanceLoc, 3, gl.FLOAT, false, 0, 0);
    instanced.vertexAttribDivisorANGLE(instanceLoc, 1);
    instanced.drawArraysInstancedANGLE(gl.TRIANGLES, 0, 6, SHAPES);
  });
  host.remove();
}

export function armRenderBenchmark(): void {
  void listen<number>("renderbench", async (event) => {
    try {
      if (event.payload > 0) SHAPES = event.payload;
      // A still frame first. Everything after is read against this rather than
      // against 60 fps in the abstract -- a headless X server has its own
      // ceiling, and ADR-0004's table only made sense with the idle row in it.
      await measure("idle (nothing animating)", () => {});
      await benchDom();
      await benchCanvas2d();
      await benchWebgl();
    } catch (e) {
      await invoke("report_bench", {
        label: `FAILED: ${String(e)}`,
        fps: 0,
        p50Ms: 0,
        p95Ms: 0,
        worstMs: 0,
      });
    }
  });
}
