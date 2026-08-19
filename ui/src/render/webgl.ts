/**
 * The same scene, drawn with WebGL.
 *
 * This renderer exists to make [ADR-0009](../../../docs/adr/0009-the-living-interface.md)'s
 * central claim checkable rather than asserted: **the world model is not the
 * renderer's to define.** It consumes `scene.ts` unchanged — no river, deck,
 * loop or key appears anywhere below — and if a drawing decision ever leaks
 * into it, the two renderers will visibly disagree.
 *
 * # One draw call
 *
 * Every primitive is a quad. The fragment shader decides what the quad *is*
 * from a `kind` attribute — a gradient band, a solid bar, a disc, a triangle, a
 * ring segment — so the whole watershed is one instanced draw regardless of how
 * much is on screen. That is why the benchmark found WebGL flat across a
 * sixteen-fold range in shape count where Canvas 2D bends.
 *
 * A curve is the one thing a quad cannot be, so a stream is flattened into a
 * short strip of quads here. The scene still describes it as a curve, because
 * that is what it is; the flattening is this renderer's problem.
 */
import type { Paint, Primitive, Scene } from "./scene";

/** What a quad is, as the fragment shader understands it. */
const KIND_RECT = 0;
const KIND_DISC = 1;
const KIND_TRIANGLE = 2;
const KIND_RING = 3;

/** Floats per instance: rect(4) + colourTop(4) + colourBottom(4) + kind/args(4). */
const STRIDE = 16;

const VERTEX = `
attribute vec2 corner;
attribute vec4 rect;        // x, y, w, h in pixels
attribute vec4 tintTop;     // rgba
attribute vec4 tintBottom;  // rgba
attribute vec4 args;        // kind, from/horizontal, sweep, thickness
uniform vec2 viewport;
varying vec2 local;
varying vec4 cTop;
varying vec4 cBottom;
varying vec4 vArgs;
void main() {
  local = corner;
  cTop = tintTop;
  cBottom = tintBottom;
  vArgs = args;
  vec2 px = rect.xy + (corner * 0.5 + 0.5) * rect.zw;
  // Pixel space to clip space, with y down as the scene describes it.
  vec2 clip = vec2(px.x / viewport.x * 2.0 - 1.0, 1.0 - px.y / viewport.y * 2.0);
  gl_Position = vec4(clip, 0.0, 1.0);
}`;

const FRAGMENT = `
precision mediump float;
varying vec2 local;
varying vec4 cTop;
varying vec4 cBottom;
varying vec4 vArgs;

void main() {
  int kind = int(vArgs.x + 0.5);
  // Shade across the quad, which is what a band is. A flat primitive sends the
  // same colour twice and gets the same answer for free. args.y picks the
  // axis for a plain rect: the mouth fades along the river, everything else
  // fades down through the water.
  float along = (kind == ${KIND_RECT} && vArgs.y > 0.5) ? local.x : local.y;
  vec4 colour = mix(cTop, cBottom, along * 0.5 + 0.5);

  if (kind == ${KIND_DISC}) {
    if (dot(local, local) > 1.0) discard;
  } else if (kind == ${KIND_TRIANGLE}) {
    // Apex at the top, base at the bottom: a stone seen from upstream.
    if (abs(local.x) > (local.y * 0.5 + 0.5)) discard;
  } else if (kind == ${KIND_RING}) {
    float r = length(local);
    float inner = 1.0 - vArgs.w;
    if (r > 1.0 || r < inner) discard;
    // atan gives -PI..PI; the sweep is measured from the start angle clockwise in
    // screen space, which is why y is negated.
    float a = atan(-local.y, local.x);
    float d = mod(a - vArgs.y, 6.2831853);
    if (d > vArgs.z) discard;
  }

  gl_FragColor = vec4(colour.rgb * colour.a, colour.a);
}`;

/** HSL to RGB, because a shader wants channels and the world speaks in hues. */
function rgb(paint: Paint): [number, number, number, number] {
  const h = ((paint.hue % 360) + 360) % 360 / 60;
  const c = (1 - Math.abs(2 * paint.lightness - 1)) * paint.saturation;
  const x = c * (1 - Math.abs((h % 2) - 1));
  const m = paint.lightness - c / 2;
  let r = 0;
  let g = 0;
  let b = 0;
  if (h < 1) [r, g, b] = [c, x, 0];
  else if (h < 2) [r, g, b] = [x, c, 0];
  else if (h < 3) [r, g, b] = [0, c, x];
  else if (h < 4) [r, g, b] = [0, x, c];
  else if (h < 5) [r, g, b] = [x, 0, c];
  else [r, g, b] = [c, 0, x];
  return [r + m, g + m, b + m, paint.alpha];
}

export interface GlRenderer {
  draw(scene: Scene): void;
  dispose(): void;
  /** What the driver claims to be. Never trusted — see ADR-0009 §2. */
  driver: string;
}

/**
 * Set up a GL renderer, or return null when the context cannot be had.
 *
 * Null is not a failure to report loudly: the tier selector simply keeps
 * Canvas 2D, which says the same things.
 */
export function create(canvas: HTMLCanvasElement): GlRenderer | null {
  const gl = canvas.getContext("webgl", {
    antialias: true,
    alpha: true,
    premultipliedAlpha: true,
  }) as WebGLRenderingContext | null;
  if (!gl) return null;

  const instanced = gl.getExtension("ANGLE_instanced_arrays");
  if (!instanced) return null;

  const program = link(gl, VERTEX, FRAGMENT);
  if (!program) return null;

  const quad = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, quad);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([-1, -1, 1, -1, -1, 1, 1, -1, 1, 1, -1, 1]),
    gl.STATIC_DRAW,
  );

  const instances = gl.createBuffer();
  let data = new Float32Array(STRIDE * 256);

  gl.enable(gl.BLEND);
  // Premultiplied, because the shader multiplies in the fragment stage — the
  // alternative double-darkens every translucent band over its neighbour.
  gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

  const ext = gl.getExtension("WEBGL_debug_renderer_info");
  const driver = ext
    ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL))
    : String(gl.getParameter(gl.RENDERER));

  const attribute = (name: string, size: number, offset: number, divisor: number) => {
    const location = gl.getAttribLocation(program, name);
    if (location < 0) return;
    gl.enableVertexAttribArray(location);
    gl.vertexAttribPointer(location, size, gl.FLOAT, false, STRIDE * 4, offset * 4);
    instanced.vertexAttribDivisorANGLE(location, divisor);
  };

  return {
    driver,
    dispose() {
      gl.deleteBuffer(quad);
      gl.deleteBuffer(instances);
      gl.deleteProgram(program);
    },
    draw(scene: Scene) {
      const quads = flatten(scene.primitives);
      if (data.length < quads.length * STRIDE) {
        data = new Float32Array(quads.length * STRIDE * 2);
      }
      quads.forEach((q, i) => data.set(q, i * STRIDE));

      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.useProgram(program);
      gl.uniform2f(
        gl.getUniformLocation(program, "viewport"),
        canvas.width,
        canvas.height,
      );

      gl.bindBuffer(gl.ARRAY_BUFFER, quad);
      const corner = gl.getAttribLocation(program, "corner");
      gl.enableVertexAttribArray(corner);
      gl.vertexAttribPointer(corner, 2, gl.FLOAT, false, 0, 0);
      instanced.vertexAttribDivisorANGLE(corner, 0);

      gl.bindBuffer(gl.ARRAY_BUFFER, instances);
      gl.bufferData(gl.ARRAY_BUFFER, data.subarray(0, quads.length * STRIDE), gl.DYNAMIC_DRAW);
      attribute("rect", 4, 0, 1);
      attribute("tintTop", 4, 4, 1);
      attribute("tintBottom", 4, 8, 1);
      attribute("args", 4, 12, 1);

      instanced.drawArraysInstancedANGLE(gl.TRIANGLES, 0, 6, quads.length);
    },
  };
}

/** Every primitive as one or more quads, in the layout the shader expects. */
function flatten(primitives: Primitive[]): number[][] {
  const out: number[][] = [];
  const push = (
    x: number,
    y: number,
    w: number,
    h: number,
    top: Paint,
    bottom: Paint,
    kind: number,
    from = 0,
    sweep = 0,
    thickness = 0,
  ) => {
    out.push([x, y, w, h, ...rgb(top), ...rgb(bottom), kind, from, sweep, thickness]);
  };

  for (const p of primitives) {
    switch (p.kind) {
      case "band":
        push(
          p.x, p.y, p.w, p.h,
          p.top, p.bottom ?? p.top,
          KIND_RECT,
          p.horizontal ? 1 : 0,
        );
        break;
      case "bar":
        // A dash pattern is not worth a shader branch for the one dashed thing
        // in the scene: it is drawn as a run of short quads instead.
        if (p.dashed) {
          for (let x = p.x; x < p.x + p.w; x += 7) {
            push(x, p.y, 3, Math.max(1, p.h), p.paint, p.paint, KIND_RECT);
          }
        } else {
          push(p.x, p.y, p.w, Math.max(1, p.h), p.paint, p.paint, KIND_RECT);
        }
        break;
      case "mark":
        push(
          p.x,
          p.y,
          p.w,
          p.h,
          p.paint,
          p.paint,
          p.shape === "disc" ? KIND_DISC : KIND_TRIANGLE,
        );
        break;
      case "whirl": {
        const radius = p.w / 2;
        push(
          p.x,
          p.y,
          p.w,
          p.h,
          p.paint,
          p.paint,
          KIND_RING,
          p.from,
          p.sweep,
          // The ring's thickness as a fraction of its radius, which is what the
          // shader compares against.
          Math.min(1, p.thickness / radius),
        );
        break;
      }
      case "stream": {
        // A curve is the one thing a quad cannot be, so it is flattened here —
        // the scene still describes it as a curve, because that is what it is.
        //
        // One thin *column* per pixel of width rather than a segment per step.
        // The curve is a function of x here (x increases monotonically), so a
        // column is exact where a chain of axis-aligned segments is visibly
        // stair-stepped — which the first attempt was, badly, despite a comment
        // claiming otherwise.
        // A column every few pixels, not every pixel. The curve is shallow
        // enough that three is indistinguishable from one, and one per pixel
        // put over a thousand quads a frame into the buffer for two streams.
        const STEP = 3;
        const columns = Math.max(1, Math.ceil(Math.abs(p.toX - p.x) / STEP));
        for (let i = 0; i < columns; i += 1) {
          const t = i / columns;
          const point = bezier(p.x, p.y, p.toX * 0.5, p.y, p.toX * 0.7, p.toY, p.toX, p.toY, t);
          push(
            p.x + (p.toX - p.x) * t,
            point[1] - p.thickness / 2,
            // A hair wider than the step, so neighbouring columns overlap
            // rather than leaving seams at fractional device ratios.
            STEP + 0.5,
            p.thickness,
            p.paint,
            p.paint,
            KIND_RECT,
          );
        }
        break;
      }
    }
  }
  return out;
}

function bezier(
  x0: number, y0: number,
  x1: number, y1: number,
  x2: number, y2: number,
  x3: number, y3: number,
  t: number,
): [number, number] {
  const u = 1 - t;
  const a = u * u * u;
  const b = 3 * u * u * t;
  const c = 3 * u * t * t;
  const d = t * t * t;
  return [a * x0 + b * x1 + c * x2 + d * x3, a * y0 + b * y1 + c * y2 + d * y3];
}

function link(gl: WebGLRenderingContext, vertex: string, fragment: string): WebGLProgram | null {
  const compile = (kind: number, source: string) => {
    const shader = gl.createShader(kind);
    if (!shader) return null;
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      console.error("shader", gl.getShaderInfoLog(shader));
      return null;
    }
    return shader;
  };
  const vs = compile(gl.VERTEX_SHADER, vertex);
  const fs = compile(gl.FRAGMENT_SHADER, fragment);
  const program = gl.createProgram();
  if (!vs || !fs || !program) return null;
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.error("link", gl.getProgramInfoLog(program));
    return null;
  }
  return program;
}
