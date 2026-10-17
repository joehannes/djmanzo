/**
 * Karaoke beta B3: a colour per singer in a duet, from the theme's own.
 *
 * The theme's four colours that can carry a voice — its accent, its second
 * accent, its warning and its danger hues — are close in some themes (an
 * amber theme's accent and warning are a few degrees apart), so the order is
 * not fixed: the first microphone keeps the accent (the voice's colour
 * everywhere else), and each next one takes the token farthest in hue from
 * those already taken. Pure, so it is tested without a theme.
 */

/** The tokens a microphone may wear, the voice's own first. */
export const MIC_TOKENS = ["--accent", "--accent-2", "--warn", "--danger"] as const;

/** A colour's hue in degrees, from `#rgb`, `#rrggbb` or `rgb(…)`; `null` for anything else. */
export function hueOf(colour: string): number | null {
  const text = colour.trim();
  let rgb: number[] | null = null;
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(text);
  if (hex) {
    const h = hex[1].length === 3 ? [...hex[1]].map((c) => c + c).join("") : hex[1];
    rgb = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
  } else {
    const fn = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)/i.exec(text);
    if (fn) rgb = [fn[1], fn[2], fn[3]].map((v) => Number(v) / 255);
  }
  if (!rgb) return null;
  const [r, g, b] = rgb;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  if (d === 0) return 0;
  let h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  h *= 60;
  return h < 0 ? h + 360 : h;
}

const apart = (a: number, b: number) => {
  const d = Math.abs(a - b) % 360;
  return Math.min(d, 360 - d);
};

/**
 * The order the tokens are worn in: indexes into `colours` (the tokens'
 * values, in `MIC_TOKENS`' order), the first always first, each next the
 * farthest in hue from those taken. A colour that cannot be read keeps its
 * place at the end.
 */
export function spread(colours: string[]): number[] {
  if (colours.length === 0) return [];
  const hues = colours.map(hueOf);
  const taken = [0];
  const left = colours.map((_, i) => i).slice(1);
  while (left.length > 0) {
    let best = 0;
    let bestGap = -1;
    left.forEach((candidate, at) => {
      const hue = hues[candidate];
      if (hue === null) return;
      const gap = Math.min(...taken.map((t) => (hues[t] === null ? 180 : apart(hue, hues[t]!))));
      if (gap > bestGap) {
        bestGap = gap;
        best = at;
      }
    });
    taken.push(left.splice(best, 1)[0]);
  }
  return taken;
}
