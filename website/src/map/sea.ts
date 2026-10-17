/**
 * The sea, drawn on a canvas under the island: endless in every direction,
 * shallower toward land, glinting by day and — like Vieques' bioluminescent
 * bay — glowing at night where the visitor's pointer stirs it.
 */

import { hexToRgb, mix } from "../lib/colour";

export interface SeaState {
  w: number;
  h: number;
  x: number;
  y: number;
  z: number;
  t: number;
  /** 0 night … 1 day. */
  day: number;
  /** The genre's (or island's) background and accent colours. */
  deep: string;
  accent: string;
  /** Bass energy now, 0–1, so the waves breathe with the music. */
  pulse: number;
  /** Recent pointer positions, in world units, with their age in seconds. */
  trail: { x: number; y: number; age: number }[];
}

const DAY_DEEP = "#0b6e99";
const DAY_SHALLOW = "#46c6d8";

export function drawSea(ctx: CanvasRenderingContext2D, s: SeaState) {
  const { w, h, z } = s;
  const night = 1 - s.day;
  const deep = mix(mix("#020c18", s.deep, 0.45), DAY_DEEP, s.day);
  const shallow = mix(mix("#05304a", s.deep, 0.3), DAY_SHALLOW, s.day);

  // Depth: shallow near the island (world origin), deep far away.
  const ox = w / 2 - s.x * z;
  const oy = h / 2 - s.y * z;
  const g = ctx.createRadialGradient(ox, oy, 200 * z, ox, oy, 2600 * z);
  g.addColorStop(0, shallow);
  g.addColorStop(1, deep);
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, w, h);

  // Waves: short strokes on a world-anchored grid, so they move with the map.
  const spacing = 150;
  const left = s.x - w / 2 / z;
  const top = s.y - h / 2 / z;
  const right = s.x + w / 2 / z;
  const bottom = s.y + h / 2 / z;
  const step = z < 0.2 ? spacing * 3 : z < 0.4 ? spacing * 2 : spacing;
  const [r, gg, b] = hexToRgb(mix("#9fd8ff", "#ffffff", s.day));
  ctx.lineWidth = Math.max(1, 2 * z);
  ctx.lineCap = "round";
  const breathe = 1 + s.pulse * 0.6;
  for (let wx = Math.floor(left / step) * step; wx < right + step; wx += step) {
    for (let wy = Math.floor(top / step) * step; wy < bottom + step; wy += step) {
      const k = Math.sin(wx * 0.013 + wy * 0.021) * 0.5 + 0.5;
      const phase = s.t * 0.6 + k * 6.28;
      const ox2 = Math.sin(phase) * 18;
      const len = (26 + 18 * k) * breathe;
      const a = (0.1 + 0.12 * Math.sin(phase * 0.7)) * (0.5 + s.day * 0.6);
      if (a <= 0.02) continue;
      const sx = (wx + ox2 - s.x) * z + w / 2;
      const sy = (wy + k * 40 - s.y) * z + h / 2;
      ctx.strokeStyle = `rgba(${r},${gg},${b},${a.toFixed(3)})`;
      ctx.beginPath();
      ctx.arc(sx, sy, len * z, Math.PI * 1.15, Math.PI * 1.85);
      ctx.stroke();
    }
  }

  // Night: plankton — faint everywhere, bright where the pointer passed.
  if (night > 0.05) {
    const [pr, pg, pb] = hexToRgb(mix("#5ef2d6", s.accent, 0.25));
    const cell = 90;
    for (let wx = Math.floor(left / cell) * cell; wx < right; wx += cell) {
      for (let wy = Math.floor(top / cell) * cell; wy < bottom; wy += cell) {
        const k = Math.abs(Math.sin(wx * 12.9898 + wy * 78.233) * 43758.5453) % 1;
        if (k > 0.35) continue;
        const tw = Math.sin(s.t * (1 + k * 3) + k * 40) * 0.5 + 0.5;
        const a = night * tw * 0.35 * (0.5 + s.pulse);
        const sx = (wx + k * cell - s.x) * z + w / 2;
        const sy = (wy + ((k * 7) % 1) * cell - s.y) * z + h / 2;
        ctx.fillStyle = `rgba(${pr},${pg},${pb},${a.toFixed(3)})`;
        ctx.fillRect(sx, sy, 2, 2);
      }
    }
    for (const p of s.trail) {
      const life = Math.max(0, 1 - p.age / 2.5);
      if (life <= 0) continue;
      const sx = (p.x - s.x) * z + w / 2;
      const sy = (p.y - s.y) * z + h / 2;
      const rad = (6 + (1 - life) * 22) * Math.max(0.6, z);
      const glow = ctx.createRadialGradient(sx, sy, 0, sx, sy, rad);
      glow.addColorStop(0, `rgba(${pr},${pg},${pb},${(0.55 * life * night).toFixed(3)})`);
      glow.addColorStop(1, `rgba(${pr},${pg},${pb},0)`);
      ctx.fillStyle = glow;
      ctx.beginPath();
      ctx.arc(sx, sy, rad, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  // Day: the sun's glitter, off to the upper left of the screen.
  if (s.day > 0.05) {
    for (let i = 0; i < 40; i++) {
      const k = (i * 0.618) % 1;
      const sx = ((k * 997 + s.t * 7) % (w * 0.6)) + w * 0.05;
      const sy = ((i * 53.3) % (h * 0.5)) + h * 0.05;
      const a = s.day * 0.25 * (Math.sin(s.t * 3 + i) * 0.5 + 0.5);
      ctx.fillStyle = `rgba(255,255,240,${a.toFixed(3)})`;
      ctx.fillRect(sx, sy, 3, 1);
    }
  }
}
