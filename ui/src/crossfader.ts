/**
 * §114: the crossfader draws the law it mixes by.
 *
 * A crossfader's position is not what a DJ hears: its *curve* is. The engine
 * mixes by `dj_dsp::crossfader_gains` with the constant-power curve — each
 * side a quarter turn of sine and cosine, both at 71 % in the middle so two
 * unrelated records hold a steady loudness through a blend. Drawn under the
 * track, the two curves and the two points the thumb stands on say how much
 * of each side is in the room — which is why the thumb in the middle is not
 * "half of each", and why the last few millimetres at an end do most of the
 * work.
 *
 * `dj_dsp`'s own tests pin the curve to 0.707 in the middle and to exact
 * silence at the ends, and `crossfader.test.ts` pins this copy to the same
 * points.
 */

/** Each side's gain, `[left, right]`, for a position from -1 (hard left) to +1. */
export function sides(position: number): [number, number] {
  const p = Number.isFinite(position) ? Math.min(1, Math.max(-1, position)) : 0;
  const angle = ((p + 1) / 2) * (Math.PI / 2);
  return [Math.max(0, Math.cos(angle)), Math.max(0, Math.sin(angle))];
}

/** Where the curves are drawn, in the master strip's own units. */
export interface Plot {
  left: number;
  width: number;
  /** The top of the plot, where a side at full level is drawn. */
  top: number;
  height: number;
}

/** A gain's height in the plot: full at the top, silence at the bottom. */
export function gainY(plot: Plot, gain: number): number {
  return plot.top + plot.height * (1 - Math.min(1, Math.max(0, gain)));
}

/** One side's curve across the whole throw, as an SVG path. */
export function lawPath(side: 0 | 1, plot: Plot, points = 33): string {
  const out: string[] = [];
  for (let i = 0; i < points; i++) {
    const along = i / (points - 1);
    const x = plot.left + plot.width * along;
    const y = gainY(plot, sides(along * 2 - 1)[side]);
    out.push(`${i === 0 ? "M" : "L"} ${x.toFixed(1)} ${y.toFixed(1)}`);
  }
  return out.join(" ");
}

/** The two levels in words, for the slider's accessible value. */
export function sidesInWords(position: number): string {
  const [left, right] = sides(position);
  return `1 at ${Math.round(left * 100)}%, 2 at ${Math.round(right * 100)}%`;
}
