/** Shapes for the island: organic coastlines from a seed, smoothed into SVG paths. */

/** A small deterministic hash: the same cell of sea always holds the same cay. */
export function hash(x: number, y: number, seed = 0): number {
  let h = (x * 374761393 + y * 668265263 + seed * 1442695041) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  h ^= h >>> 16;
  return (h >>> 0) / 4294967295;
}

/** Points around an ellipse, the radius wobbled by a few sines: a coastline. */
export function blob(cx: number, cy: number, rx: number, ry: number, seed: number, n = 56, rough = 0.16): [number, number][] {
  const pts: [number, number][] = [];
  const k = [hash(seed, 1), hash(seed, 2), hash(seed, 3), hash(seed, 4), hash(seed, 5), hash(seed, 6)];
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const wobble =
      1 +
      rough * (Math.sin(a * 2 + k[0] * 6) * 0.5 + Math.sin(a * 3 + k[1] * 6) * 0.35 + Math.sin(a * 5 + k[2] * 6) * 0.2 + Math.sin(a * 9 + k[3] * 6) * 0.08);
    pts.push([cx + Math.cos(a) * rx * wobble, cy + Math.sin(a) * ry * wobble]);
  }
  return pts;
}

/** A closed Catmull-Rom spline through the points, as cubic Béziers. */
export function smoothClosed(pts: [number, number][]): string {
  const n = pts.length;
  let d = `M${pts[0][0].toFixed(1)},${pts[0][1].toFixed(1)}`;
  for (let i = 0; i < n; i++) {
    const p0 = pts[(i - 1 + n) % n];
    const p1 = pts[i];
    const p2 = pts[(i + 1) % n];
    const p3 = pts[(i + 2) % n];
    const c1 = [p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6];
    const c2 = [p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6];
    d += `C${c1[0].toFixed(1)},${c1[1].toFixed(1)} ${c2[0].toFixed(1)},${c2[1].toFixed(1)} ${p2[0].toFixed(1)},${p2[1].toFixed(1)}`;
  }
  return d + "Z";
}

/** An open smooth path through the points (the river). */
export function smoothOpen(pts: [number, number][]): string {
  let d = `M${pts[0][0]},${pts[0][1]}`;
  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = pts[Math.max(0, i - 1)];
    const p1 = pts[i];
    const p2 = pts[i + 1];
    const p3 = pts[Math.min(pts.length - 1, i + 2)];
    const c1 = [p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6];
    const c2 = [p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6];
    d += ` C${c1[0]},${c1[1]} ${c2[0]},${c2[1]} ${p2[0]},${p2[1]}`;
  }
  return d;
}

export const blobPath = (cx: number, cy: number, rx: number, ry: number, seed: number, n?: number, rough?: number) => smoothClosed(blob(cx, cy, rx, ry, seed, n, rough));
