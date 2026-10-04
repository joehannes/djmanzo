/**
 * §122: the layer stack's drawing — one canvas, one WebGL context, each layer
 * a full-screen quad drawn in order with its blend. See `docs/VISUALS.md`.
 *
 * Every picture is djmanzo's own: a small fragment shader per kind, each
 * answering to one number (`u_value`, its signal, already followed by
 * `./looks`). They write premultiplied colour, so the three blends are three
 * blend functions and the canvas composites over K1's background beneath it
 * as any transparent picture does.
 *
 * What it must never do is take the words down. A shader that will not
 * compile leaves its layer out and is named; a context that is lost stops
 * drawing and leaves the canvas clear; no WebGL at all is `createStack`
 * returning null, and the caller draws nothing.
 *
 * WebGL 1 and GLSL ES 1.0, for the widest set of webviews and drivers.
 */
import type { Blend, DrawLayer, Kind, Rgb, Signals } from "./looks";

const VERTEX = `attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
  v_uv = a_pos * 0.5 + 0.5;
  gl_Position = vec4(a_pos, 0.0, 1.0);
}`;

const HEADER = `precision mediump float;
varying vec2 v_uv;
uniform float u_time;
uniform float u_value;
uniform float u_phase;
uniform float u_amount;
uniform vec2 u_res;
uniform vec3 u_c0;
uniform vec3 u_c1;
uniform vec3 u_c2;
uniform vec3 u_c3;
// Centred and square: y runs -1..1, x as far as the screen is wide.
vec2 centred() {
  vec2 p = v_uv * 2.0 - 1.0;
  p.x *= u_res.x / u_res.y;
  return p;
}
float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}
// Premultiplied, so the blend functions below are the whole of compositing.
vec4 paint(vec3 colour, float alpha) {
  float a = clamp(alpha, 0.0, 1.0);
  return vec4(colour * a, a);
}
`;

/**
 * Each kind's picture. The first line names it — what a test failing one
 * kind's compile looks for, and what a driver's error log shows.
 */
export const FRAGMENTS: Record<Kind, string> = {
  // A slow field of the record's colours, as bright as the room is loud.
  wash: `// djmanzo: wash
void main() {
  vec2 p = centred();
  float t = u_time * 0.05;
  float a = sin(p.x * 1.3 + t * 1.7) + sin(p.y * 1.1 - t * 1.3);
  float b = sin((p.x + p.y) * 0.9 + t) + cos(length(p) * 1.7 - t * 0.8);
  vec3 near = mix(u_c0, u_c1, 0.5 + 0.25 * a);
  vec3 far = mix(u_c2, u_c3, 0.5 + 0.25 * b);
  vec3 colour = mix(near, far, 0.5 + 0.25 * sin(t + p.x));
  gl_FragColor = paint(colour, u_amount * (0.15 + 0.85 * u_value));
}`,
  // Rings from the centre, moving out a step each beat, as strong as the signal.
  pulse: `// djmanzo: pulse
void main() {
  vec2 p = centred();
  float r = length(p);
  float ring = 0.0;
  for (int i = 0; i < 3; i++) {
    float at = (u_phase + float(i)) * 0.7;
    float width = 0.05 + 0.04 * float(i);
    ring += smoothstep(width, 0.0, abs(r - at)) * (1.0 - 0.3 * float(i));
  }
  vec3 colour = mix(u_c0, u_c1, clamp(r * 0.5, 0.0, 1.0));
  gl_FragColor = paint(colour, ring * u_value * u_amount);
}`,
  // Two sources of waves crossing, their crests lit by the signal.
  ripple: `// djmanzo: ripple
void main() {
  vec2 p = centred();
  float t = u_time;
  vec2 a = vec2(-0.9, -0.4) + 0.2 * vec2(sin(t * 0.13), cos(t * 0.11));
  vec2 b = vec2(0.9, 0.5) + 0.2 * vec2(cos(t * 0.17), sin(t * 0.07));
  float w = sin(length(p - a) * 9.0 - t * 1.5) + sin(length(p - b) * 8.0 - t * 1.2);
  float crest = smoothstep(0.9, 2.0, w);
  vec3 colour = mix(u_c2, u_c3, 0.5 + 0.5 * sin(t * 0.2 + p.y));
  gl_FragColor = paint(colour, crest * u_value * u_amount);
}`,
  // Points that flare and fade, each on its own clock; the signal how bright.
  sparks: `// djmanzo: sparks
void main() {
  vec2 p = v_uv * vec2(u_res.x / u_res.y, 1.0) * 14.0;
  vec2 cell = floor(p);
  vec2 f = fract(p) - 0.5;
  float h = hash(cell);
  vec2 offset = vec2(hash(cell + 1.7), hash(cell + 3.1)) - 0.5;
  float d = length(f - offset * 0.6);
  float flare = pow(fract(u_time * (0.3 + h * 0.7) + h * 7.0), 12.0);
  float spark = smoothstep(0.12, 0.0, d) * flare * step(0.55, h);
  vec3 colour = min(u_c1 * 1.4, vec3(1.0));
  gl_FragColor = paint(colour, spark * u_value * u_amount * 1.5);
}`,
  // A glow where the words are, as strong as the voice.
  halo: `// djmanzo: halo
void main() {
  vec2 p = v_uv * 2.0 - 1.0;
  float e = length(vec2(p.x * 0.75, p.y * 2.2));
  float glow = smoothstep(1.0, 0.2, e);
  gl_FragColor = paint(u_c0, glow * u_value * u_amount);
}`,
};

/** What one frame draws. */
export interface Frame {
  layers: DrawLayer[];
  signals: Signals;
  /** Where in the beat the most-heard deck is, 0..1. */
  phase: number;
  /** The record's four colours. */
  colours: Rgb[];
  /** Seconds, for the pictures that drift. */
  time: number;
}

export interface Stack {
  /** Draw one frame, clearing what was there; nothing once the context is lost. */
  draw(frame: Frame): void;
  /** Kinds whose picture would not compile here, each with what the driver said. */
  failures(): string[];
  /** The context was lost: nothing more will be drawn. */
  lost(): boolean;
  dispose(): void;
}

interface Program {
  program: WebGLProgram;
  position: number;
  uniforms: Record<string, WebGLUniformLocation | null>;
}

const UNIFORMS = ["u_time", "u_value", "u_phase", "u_amount", "u_res", "u_c0", "u_c1", "u_c2", "u_c3"];

/**
 * A stack drawing into `canvas`, or null when this webview has no WebGL —
 * the caller then draws nothing, and K1's background is the screen.
 */
export function createStack(canvas: HTMLCanvasElement): Stack | null {
  let gl: WebGLRenderingContext | null = null;
  try {
    gl = canvas.getContext("webgl", { alpha: true, premultipliedAlpha: true, antialias: false });
  } catch {
    gl = null;
  }
  if (!gl) return null;
  const context = gl;

  let isLost = false;
  const onLost = (event: Event) => {
    event.preventDefault();
    isLost = true;
  };
  canvas.addEventListener("webglcontextlost", onLost);

  const quad = context.createBuffer();
  context.bindBuffer(context.ARRAY_BUFFER, quad);
  // One triangle covering the screen: no seam down the middle, three vertices.
  context.bufferData(context.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), context.STATIC_DRAW);

  const programs = new Map<Kind, Program | null>();
  const failed: string[] = [];

  function compile(type: number, source: string): WebGLShader | string {
    const shader = context.createShader(type);
    if (!shader) return "no shader could be made";
    context.shaderSource(shader, source);
    context.compileShader(shader);
    if (!context.getShaderParameter(shader, context.COMPILE_STATUS)) {
      const said = context.getShaderInfoLog(shader) ?? "";
      context.deleteShader(shader);
      return said.trim() || "it would not compile";
    }
    return shader;
  }

  /** The kind's program, made the first time it is drawn; null if it will not build here. */
  function program(kind: Kind): Program | null {
    if (programs.has(kind)) return programs.get(kind)!;
    let made: Program | null = null;
    const vertex = compile(context.VERTEX_SHADER, VERTEX);
    const fragment = compile(context.FRAGMENT_SHADER, HEADER + FRAGMENTS[kind]);
    if (typeof vertex === "string" || typeof fragment === "string") {
      failed.push(`${kind}: ${typeof fragment === "string" ? fragment : vertex}`);
    } else {
      const linked = context.createProgram();
      if (linked) {
        context.attachShader(linked, vertex);
        context.attachShader(linked, fragment);
        context.linkProgram(linked);
        if (context.getProgramParameter(linked, context.LINK_STATUS)) {
          made = {
            program: linked,
            position: context.getAttribLocation(linked, "a_pos"),
            uniforms: Object.fromEntries(UNIFORMS.map((u) => [u, context.getUniformLocation(linked, u)])),
          };
        } else {
          failed.push(`${kind}: ${context.getProgramInfoLog(linked)?.trim() || "it would not link"}`);
          context.deleteProgram(linked);
        }
      }
      context.deleteShader(vertex);
      context.deleteShader(fragment);
    }
    programs.set(kind, made);
    return made;
  }

  function blend(how: Blend) {
    switch (how) {
      case "over":
        context.blendFunc(context.ONE, context.ONE_MINUS_SRC_ALPHA);
        break;
      case "add":
        context.blendFunc(context.ONE, context.ONE);
        break;
      case "screen":
        // src + dst − src·dst.
        context.blendFunc(context.ONE_MINUS_DST_COLOR, context.ONE);
        break;
    }
  }

  return {
    draw(frame) {
      if (isLost || context.isContextLost()) {
        isLost = true;
        return;
      }
      const width = canvas.width;
      const height = canvas.height;
      context.viewport(0, 0, width, height);
      context.clearColor(0, 0, 0, 0);
      context.clear(context.COLOR_BUFFER_BIT);
      context.enable(context.BLEND);
      context.bindBuffer(context.ARRAY_BUFFER, quad);
      for (const layer of frame.layers) {
        const made = program(layer.kind);
        if (!made) continue;
        const colours = layer.colour === "record" ? frame.colours : [layer.colour, layer.colour, layer.colour, layer.colour];
        context.useProgram(made.program);
        context.enableVertexAttribArray(made.position);
        context.vertexAttribPointer(made.position, 2, context.FLOAT, false, 0, 0);
        const u = made.uniforms;
        context.uniform1f(u.u_time, frame.time);
        context.uniform1f(u.u_value, frame.signals[layer.signal]);
        context.uniform1f(u.u_phase, frame.phase);
        context.uniform1f(u.u_amount, layer.amount);
        context.uniform2f(u.u_res, width, Math.max(1, height));
        context.uniform3fv(u.u_c0, colours[0]);
        context.uniform3fv(u.u_c1, colours[1]);
        context.uniform3fv(u.u_c2, colours[2]);
        context.uniform3fv(u.u_c3, colours[3]);
        blend(layer.blend);
        context.drawArrays(context.TRIANGLES, 0, 3);
      }
    },
    failures: () => [...failed],
    lost: () => isLost,
    dispose() {
      canvas.removeEventListener("webglcontextlost", onLost);
      for (const made of programs.values()) if (made) context.deleteProgram(made.program);
      programs.clear();
      context.deleteBuffer(quad);
    },
  };
}
