/**
 * Karaoke beta B6 (§129): skins — a third layer over a theme.
 *
 * Artwork drawn over the whole interface, faintly: 10–18 % opacity, never
 * taking a press (`pointer-events: none`), pure SVG with nothing to
 * download. Each pattern is a *mask* filled with the theme's own accent, so
 * a skin follows whatever theme is worn rather than fighting it. Off under
 * the system's high-contrast setting, fainter under reduced motion, and
 * never on the singers' screen, whose words must keep every bit of their
 * contrast.
 */

export interface Skin {
  id: string;
  name: string;
  about: string;
  /** The themes (package ids) it is offered with. */
  suits: string[];
  /** One tile of the pattern, an SVG drawn in black on transparent. */
  tile: string;
  /** The tile's size on screen. */
  size: string;
  /** How strongly it is drawn: 0.10–0.18. */
  opacity: number;
}

const svg = (w: number, h: number, body: string) =>
  `<svg xmlns='http://www.w3.org/2000/svg' width='${w}' height='${h}' viewBox='0 0 ${w} ${h}'>${body}</svg>`;

export const SKINS: Skin[] = [
  {
    id: "palms",
    name: "Palm shadows",
    about: "The shade of fronds across a beach bar's counter.",
    suits: ["pkg-caribbean", "pkg-sunset", "pkg-malecon", "pkg-rosa", "pkg-latin"],
    tile: svg(
      240,
      240,
      "<g fill='none' stroke='black' stroke-width='3' stroke-linecap='round'>" +
        "<path d='M40 220 Q70 120 120 60'/>" +
        [0, 1, 2, 3, 4, 5, 6]
          .map((i) => {
            const x = 52 + i * 11;
            const y = 190 - i * 20;
            return `<path d='M${x} ${y} q-34 -6 -52 14'/><path d='M${x} ${y} q30 -14 52 -4'/>`;
          })
          .join("") +
        "</g>",
    ),
    size: "320px",
    opacity: 0.12,
  },
  {
    id: "waves",
    name: "Waves",
    about: "Long low swells, the sea just past the screen.",
    suits: ["pkg-caribbean", "pkg-malecon", "pkg-biobay", "pkg-aurora", "pkg-sunset"],
    tile: svg(
      200,
      60,
      "<g fill='none' stroke='black' stroke-width='2.5'>" +
        "<path d='M0 20 Q25 8 50 20 T100 20 T150 20 T200 20'/>" +
        "<path d='M0 46 Q25 34 50 46 T100 46 T150 46 T200 46'/>" +
        "</g>",
    ),
    size: "220px 66px",
    opacity: 0.12,
  },
  {
    id: "papel",
    name: "Papel picado",
    about: "Cut-paper bunting strung across a fiesta.",
    suits: ["pkg-latin", "pkg-carnaval", "pkg-wedding", "pkg-festival", "pkg-rosa"],
    tile: svg(
      240,
      120,
      "<path d='M0 8 Q120 30 240 8' fill='none' stroke='black' stroke-width='2'/>" +
        [0, 1, 2, 3]
          .map((i) => {
            const x = 18 + i * 58;
            const y = 14 + Math.sin((i / 3) * Math.PI) * 9;
            return (
              `<path d='M${x} ${y} h38 v34 l-6 8 l-6 -8 l-7 8 l-6 -8 l-7 8 l-6 -8 z' fill='none' stroke='black' stroke-width='2.5'/>` +
              `<circle cx='${x + 19}' cy='${y + 16}' r='6' fill='none' stroke='black' stroke-width='2'/>`
            );
          })
          .join(""),
    ),
    size: "360px 200px",
    opacity: 0.12,
  },
  {
    id: "halftone",
    name: "Halftone",
    about: "A screen-printed gig poster's dots, fading across the screen.",
    suits: ["pkg-neon", "pkg-spotlight", "pkg-club", "pkg-festival", "pkg-carnaval"],
    tile: svg(
      48,
      48,
      "<g fill='black'><circle cx='12' cy='12' r='5'/><circle cx='36' cy='36' r='5'/>" +
        "<circle cx='36' cy='12' r='2.5'/><circle cx='12' cy='36' r='2.5'/></g>",
    ),
    size: "26px",
    opacity: 0.1,
  },
  {
    id: "trash-polka",
    name: "Trash polka",
    about: "Black and red brush strokes and halftone, a tattoo parlour's wall.",
    suits: ["pkg-industrial", "pkg-cyber", "pkg-carnaval", "pkg-neon"],
    tile: svg(
      360,
      260,
      "<g fill='black'>" +
        "<path d='M20 60 C90 20 170 90 250 40 C290 18 330 30 350 22 L352 34 C300 50 280 70 240 70 C160 110 90 50 24 78 Z'/>" +
        "<path d='M60 200 C120 170 200 220 300 180 L304 192 C210 236 130 186 64 214 Z'/>" +
        [0, 1, 2, 3, 4]
          .map((i) => `<circle cx='${230 + i * 22}' cy='${120 + i * 8}' r='${8 - i * 1.3}'/>`)
          .join("") +
        "</g>",
    ),
    size: "420px 300px",
    opacity: 0.16,
  },
  {
    id: "plankton",
    name: "Glowing plankton",
    about: "Specks of light in a dark bay, where the water was touched.",
    suits: ["pkg-biobay", "pkg-aurora", "pkg-velvet", "pkg-malecon"],
    tile: svg(
      180,
      180,
      "<g fill='black'>" +
        [
          [20, 30, 2.5],
          [70, 12, 1.5],
          [130, 40, 3],
          [160, 100, 1.8],
          [100, 90, 2.2],
          [40, 120, 3.2],
          [90, 160, 1.6],
          [150, 160, 2.4],
        ]
          .map(([x, y, r]) => `<circle cx='${x}' cy='${y}' r='${r}'/>`)
          .join("") +
        "</g>",
    ),
    size: "200px",
    opacity: 0.18,
  },
];

/** The skins offered with a theme. */
export function skinsFor(pkg: string): Skin[] {
  return SKINS.filter((skin) => skin.suits.includes(pkg));
}

/** A tile as a CSS `url()`. */
function url(tile: string): string {
  return `url("data:image/svg+xml,${encodeURIComponent(tile)}")`;
}

/** The stylesheet every skin is drawn by, keyed on `html[data-skin]`. */
export function skinCss(): string {
  const base = `
html[data-skin] body::after {
  content: "";
  position: fixed;
  inset: 0;
  z-index: 2147483000;
  pointer-events: none;
  background-color: var(--accent);
  -webkit-mask-repeat: repeat;
  mask-repeat: repeat;
  opacity: var(--skin-opacity, 0.14);
}
@media (prefers-reduced-motion: reduce) {
  html[data-skin] body::after { opacity: calc(var(--skin-opacity, 0.14) * 0.6); }
}
@media (prefers-contrast: more), (forced-colors: active) {
  html[data-skin] body::after { display: none; }
}`;
  const each = SKINS.map(
    (skin) => `
html[data-skin="${skin.id}"] body::after {
  -webkit-mask-image: ${url(skin.tile)};
  mask-image: ${url(skin.tile)};
  -webkit-mask-size: ${skin.size};
  mask-size: ${skin.size};
  --skin-opacity: ${skin.opacity};
}`,
  ).join("");
  return base + each;
}

/** Put the stylesheet in the page, once. */
export function installSkins(doc: Document): void {
  if (doc.getElementById("djmanzo-skins")) return;
  const style = doc.createElement("style");
  style.id = "djmanzo-skins";
  style.textContent = skinCss();
  doc.head.appendChild(style);
}

/** Whether this window is the singers' screen, where no skin is drawn. */
export function isSingersScreen(search: string): boolean {
  return new URLSearchParams(search).get("panel") === "singers";
}
