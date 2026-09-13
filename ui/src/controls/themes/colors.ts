export type ResolvedTheme = "dark" | "light";

type Palette = {
  [key: string]: string;
};

const palettes: Record<string, { dark: Palette; light: Palette }> = {
  "pkg-organic": {
    dark: {
      "--bg": "#08100b",
      "--panel": "#0f1612",
      "--panel-raised": "#162018",
      "--panel-hover": "#20302a",
      "--border": "#223028",
      "--border-strong": "#2f3b33",
      "--text": "#e9f8f1",
      "--text-dim": "#98a79b",
      "--accent": "#66d6a6",
      "--accent-2": "#28b460",
      "--warn": "#f6bd4a",
      "--danger": "#ef6b6b",
      "--on-accent": "#062014",
      "--scrim": "#08100b88",
    },
    light: {
      "--bg": "#fbfbf8",
      "--panel": "#ffffff",
      "--panel-raised": "#f3f6f2",
      "--panel-hover": "#e9efe9",
      "--border": "#dfe8e1",
      "--border-strong": "#c9d6cd",
      "--text": "#0f1b16",
      "--text-dim": "#4e665c",
      "--accent": "#0f7b57",
      "--accent-2": "#055e7a",
      "--warn": "#b46907",
      "--danger": "#b91c1c",
      "--on-accent": "#ffffff",
      "--scrim": "#ffffffcc",
    },
  },
  // Bright rooms. The dark variant is not black: outdoors, a black panel
  // becomes a mirror and you read your own face instead of the waveform.
  // Contrast is pushed hard in both, because sunlight eats the middle of any
  // range and leaves only the ends.
  "pkg-daylight": {
    dark: {
      "--bg": "#1c1f24",
      "--panel": "#262a31",
      "--panel-raised": "#31363f",
      "--panel-hover": "#3d434e",
      "--border": "#454c58",
      "--border-strong": "#5d6674",
      "--text": "#ffffff",
      "--text-dim": "#c2c9d4",
      "--accent": "#ffd23f",
      "--accent-2": "#4cc9f0",
      "--warn": "#ff9f1c",
      "--danger": "#ff5c5c",
      "--on-accent": "#1c1f24",
      "--scrim": "#1c1f24aa",
    },
    light: {
      "--bg": "#ffffff",
      "--panel": "#ffffff",
      "--panel-raised": "#eef1f5",
      "--panel-hover": "#dfe4ec",
      "--border": "#b9c2ce",
      "--border-strong": "#8e99a8",
      // Near-black rather than black: pure black on pure white shimmers under
      // sunlight, which is the one place this theme is meant to work.
      "--text": "#101318",
      "--text-dim": "#3c4653",
      "--accent": "#0b5cd6",
      "--accent-2": "#00695c",
      "--warn": "#a14b00",
      "--danger": "#a41414",
      "--on-accent": "#ffffff",
      "--scrim": "#ffffffdd",
    },
  },
  // Hours at a desk. Warm, low-saturation, and the contrast pulled *down* from
  // Daylight on purpose -- maximum contrast is what you want for ten seconds
  // and what tires the eye over four hours.
  "pkg-studio": {
    dark: {
      "--bg": "#14120f",
      "--panel": "#1c1a16",
      "--panel-raised": "#26231e",
      "--panel-hover": "#332f28",
      "--border": "#38332b",
      "--border-strong": "#4b453a",
      "--text": "#ede6da",
      "--text-dim": "#a49b8c",
      "--accent": "#e0a458",
      "--accent-2": "#8fb996",
      "--warn": "#e3bd3c",
      "--danger": "#d9694f",
      "--on-accent": "#14120f",
      "--scrim": "#14120f99",
    },
    light: {
      "--bg": "#faf6ef",
      "--panel": "#fffdf9",
      "--panel-raised": "#f2ece1",
      "--panel-hover": "#e8e0d1",
      "--border": "#ddd3c2",
      "--border-strong": "#c3b7a2",
      "--text": "#241f18",
      "--text-dim": "#5f5648",
      "--accent": "#9a5b16",
      "--accent-2": "#3f6b4d",
      "--warn": "#957d0a",
      "--danger": "#a63a25",
      "--on-accent": "#fffdf9",
      "--scrim": "#faf6efcc",
    },
  },
  // A dark booth. The darkest palette here, and the only one that goes to true
  // black -- in a room with no light behind the screen, black costs nothing and
  // buys every bit of contrast for what matters.
  //
  // The accent is amber rather than blue or green: blue light is the worst for
  // night vision, and a booth is exactly where night vision is the point.
  "pkg-booth": {
    dark: {
      "--bg": "#000000",
      "--panel": "#0a0a0a",
      "--panel-raised": "#141414",
      "--panel-hover": "#1f1f1f",
      "--border": "#2a2a2a",
      "--border-strong": "#454545",
      "--text": "#f2f2f2",
      "--text-dim": "#9a9a9a",
      "--accent": "#ffb020",
      "--accent-2": "#ff7a1a",
      "--warn": "#ffdf3f",
      "--danger": "#ff4d4d",
      "--on-accent": "#000000",
      "--scrim": "#000000cc",
    },
    // Offered rather than useful. A booth theme in light mode is a
    // contradiction, and someone will still pick it -- so it is a plain,
    // legible grey rather than a joke.
    light: {
      "--bg": "#f4f4f4",
      "--panel": "#ffffff",
      "--panel-raised": "#ebebeb",
      "--panel-hover": "#dedede",
      "--border": "#c8c8c8",
      "--border-strong": "#a4a4a4",
      "--text": "#111111",
      "--text-dim": "#4a4a4a",
      "--accent": "#a15c00",
      "--accent-2": "#00508a",
      "--warn": "#5f6b00",
      "--danger": "#a11111",
      "--on-accent": "#ffffff",
      "--scrim": "#f4f4f4cc",
    },
  },
  // Golden hour: a room that is neither lit nor dark, which is the hardest
  // case. Mid-tone panels, so the interface does not glare against a bright
  // sky or vanish against a dim one.
  "pkg-sunset": {
    dark: {
      "--bg": "#160f14",
      "--panel": "#20151b",
      "--panel-raised": "#2c1d24",
      "--panel-hover": "#3a262f",
      "--border": "#3d2a33",
      "--border-strong": "#553a45",
      "--text": "#fbeee6",
      "--text-dim": "#b79c98",
      "--accent": "#ff8c42",
      "--accent-2": "#ffcb69",
      "--warn": "#ffb703",
      "--danger": "#e5484d",
      "--on-accent": "#160f14",
      "--scrim": "#160f1499",
    },
    light: {
      "--bg": "#fff4ec",
      "--panel": "#fffaf6",
      "--panel-raised": "#ffe8d8",
      "--panel-hover": "#ffdcc6",
      "--border": "#f0cdb6",
      "--border-strong": "#d9a687",
      "--text": "#2a1710",
      "--text-dim": "#6b4a3a",
      "--accent": "#c2410c",
      "--accent-2": "#0f6b6b",
      "--warn": "#9a6700",
      "--danger": "#b02a2a",
      "--on-accent": "#fffaf6",
      "--scrim": "#fff4eccc",
    },
  },
  // §32's Watershed Living. Slate and river blue, keyed to the water the
  // renderer actually draws: `render/scene.ts` paints the body at hue 210-220,
  // the murk at 30 and a seam at 40. A palette that fought those would put the
  // chrome and the metaphor in two different rooms, which is the one thing a
  // theme *about* the metaphor cannot do.
  //
  // The accents are deliberately the coolest in the set. Everything warm on
  // screen under this theme is the watershed saying something — turbidity,
  // strain, a key seam — and chrome that borrowed the same warmth would make
  // those three readings compete with the furniture.
  "pkg-watershed": {
    dark: {
      "--bg": "#070d13",
      "--panel": "#0d151d",
      "--panel-raised": "#141f2a",
      "--panel-hover": "#1d2c3a",
      "--border": "#1f2f3d",
      "--border-strong": "#2e4356",
      "--text": "#e6f1fa",
      "--text-dim": "#93a8ba",
      "--accent": "#5cb4e8",
      "--accent-2": "#2f7ac9",
      "--warn": "#e9a23b",
      "--danger": "#ea5f60",
      "--on-accent": "#04131d",
      "--scrim": "#070d1399",
    },
    light: {
      "--bg": "#f2f7fb",
      "--panel": "#ffffff",
      "--panel-raised": "#e8f1f8",
      "--panel-hover": "#dbe9f4",
      "--border": "#cbdeeb",
      "--border-strong": "#a4c2d8",
      "--text": "#0a1922",
      "--text-dim": "#4a5560",
      "--accent": "#12658f",
      "--accent-2": "#0f7361",
      "--warn": "#8a5a05",
      "--danger": "#b02a2a",
      "--on-accent": "#ffffff",
      "--scrim": "#f2f7fbcc",
    },
  },
  "pkg-industrial": {
    dark: {
      "--bg": "#0b0e12",
      "--panel": "#121417",
      "--panel-raised": "#1a1c20",
      "--panel-hover": "#26282d",
      "--border": "#2b2f34",
      "--border-strong": "#3b3f45",
      "--text": "#e6e9ee",
      "--text-dim": "#8f98a3",
      "--accent": "#9aa7ff",
      "--accent-2": "#6ee7d8",
      "--warn": "#fbbf24",
      "--danger": "#f87171",
      "--on-accent": "#0b0e12",
      "--scrim": "#0b0e1288",
    },
    light: {
      "--bg": "#f6f7f9",
      "--panel": "#ffffff",
      "--panel-raised": "#f0f3f7",
      "--panel-hover": "#e6eaef",
      "--border": "#d4dbe3",
      "--border-strong": "#bfcad6",
      "--text": "#121417",
      "--text-dim": "#5b6472",
      "--accent": "#3343ff",
      "--accent-2": "#0f766e",
      "--warn": "#b45309",
      "--danger": "#c43131",
      "--on-accent": "#ffffff",
      "--scrim": "#ffffffaa",
    },
  },
  "pkg-cyber": {
    dark: {
      "--bg": "#05060a",
      "--panel": "#0c0f14",
      "--panel-raised": "#13161b",
      "--panel-hover": "#1b1f26",
      "--border": "#21252c",
      "--border-strong": "#31363d",
      "--text": "#eaf6ff",
      "--text-dim": "#9fb6d0",
      "--accent": "#7c5cff",
      "--accent-2": "#00ffd1",
      "--warn": "#fdb02f",
      "--danger": "#ff6b6b",
      // Darkened from #071021, which put a button's own label at 4.37:1
      // against this accent -- under AA, and not something a palette author
      // can see. `themes.test.ts` computes the ratio.
      "--on-accent": "#03060d",
      "--scrim": "#05060a66",
    },
    light: {
      "--bg": "#f7f9fb",
      "--panel": "#ffffff",
      "--panel-raised": "#f5f7fb",
      "--panel-hover": "#eef3fb",
      "--border": "#dbe6f0",
      "--border-strong": "#c7d8e6",
      "--text": "#071021",
      "--text-dim": "#4b6076",
      "--accent": "#5b3bff",
      "--accent-2": "#0bbfa3",
      "--warn": "#b76a09",
      "--danger": "#b33a3a",
      "--on-accent": "#ffffff",
      "--scrim": "#ffffffcc",
    },
  },
  /* §32's Club. A dark room that plays everything.
   *
   * Deliberately not Industrial Techno's near-black: that theme is a harder,
   * narrower room, and a DJ who asked for a club theme should not be handed
   * hard music's. Blue-violet rather than a hue with a genre attached. */
  "pkg-club": {
    dark: {
      "--bg": "#0a0a14",
      "--panel": "#121320",
      "--panel-raised": "#1b1c2e",
      "--panel-hover": "#272844",
      "--border": "#2a2b45",
      "--border-strong": "#3c3e60",
      "--text": "#eceafd",
      "--text-dim": "#9a99b8",
      "--accent": "#8b7bff",
      "--accent-2": "#41d4f2",
      "--warn": "#ffbc42",
      "--danger": "#ff5470",
      "--on-accent": "#0a0a14",
      "--scrim": "#0a0a14cc",
    },
    light: {
      "--bg": "#f7f6fb",
      "--panel": "#ffffff",
      "--panel-raised": "#efedf8",
      "--panel-hover": "#e3e0f3",
      "--border": "#dcd9ee",
      "--border-strong": "#b9b4d8",
      "--text": "#14132a",
      "--text-dim": "#4f4c70",
      "--accent": "#4a34c7",
      "--accent-2": "#0a6f8a",
      "--warn": "#9a5f00",
      "--danger": "#b3123a",
      "--on-accent": "#ffffff",
      "--scrim": "#f7f6fbcc",
    },
  },
  /* §32's Festival. Outdoors, large, and usually in daylight.
   *
   * Daylight already covers the readable half, which is why this is a gap
   * rather than a hole; what it does not cover is a main stage at sunset. So
   * this is the warm, high-chroma one, and its light variant is pushed as hard
   * as Daylight's because sun eats the middle of any range. */
  "pkg-festival": {
    dark: {
      "--bg": "#141018",
      "--panel": "#1e1823",
      "--panel-raised": "#2b2231",
      "--panel-hover": "#3a2e42",
      "--border": "#3d3147",
      "--border-strong": "#574561",
      "--text": "#fff6ee",
      "--text-dim": "#c6b3bd",
      "--accent": "#ff8a3d",
      "--accent-2": "#38e0c8",
      "--warn": "#ffd447",
      "--danger": "#ff4d6d",
      "--on-accent": "#180f06",
      "--scrim": "#141018cc",
    },
    light: {
      "--bg": "#fffaf4",
      "--panel": "#ffffff",
      "--panel-raised": "#fff0e2",
      "--panel-hover": "#ffe3cc",
      "--border": "#f0d9c4",
      "--border-strong": "#cdae95",
      "--text": "#241611",
      "--text-dim": "#6a4d3e",
      "--accent": "#a63b00",
      "--accent-2": "#00695c",
      "--warn": "#8a5a00",
      "--danger": "#b3123a",
      "--on-accent": "#ffffff",
      "--scrim": "#fffaf4cc",
    },
  },
  /* §32's Caribbean. Sea and sand, and usually a light room.
   *
   * The light variant is the one that matters: a beach bar at four in the
   * afternoon is the setting, and a theme for it that is only usable after
   * dark would be a theme for somewhere else. */
  "pkg-caribbean": {
    dark: {
      "--bg": "#04161a",
      "--panel": "#0a2027",
      "--panel-raised": "#0f2d36",
      "--panel-hover": "#164049",
      "--border": "#17414b",
      "--border-strong": "#256a75",
      "--text": "#e8fbfa",
      "--text-dim": "#8fb5b6",
      "--accent": "#2fe0c0",
      "--accent-2": "#ffd166",
      "--warn": "#ffa62b",
      "--danger": "#ff6b6b",
      "--on-accent": "#04161a",
      "--scrim": "#04161acc",
    },
    light: {
      "--bg": "#f6fdfc",
      "--panel": "#ffffff",
      "--panel-raised": "#e6f6f3",
      "--panel-hover": "#d3ede9",
      "--border": "#cbe7e2",
      "--border-strong": "#93c7bf",
      "--text": "#062024",
      "--text-dim": "#4a5a5c",
      "--accent": "#00695f",
      "--accent-2": "#5b3fa0",
      "--warn": "#9a5f00",
      "--danger": "#b3123a",
      "--on-accent": "#ffffff",
      "--scrim": "#f6fdfccc",
    },
  },
  /* §32's Latin. The colours of the music rather than of a room.
   *
   * Warm reds and gold, kept dark enough for a venue because most Latin nights
   * are one. The secondary accent goes green rather than deeper into the reds:
   * four warm colours that a DJ has to tell apart are four colours they
   * cannot. */
  "pkg-latin": {
    dark: {
      "--bg": "#170a0c",
      "--panel": "#221012",
      "--panel-raised": "#2f171a",
      "--panel-hover": "#402024",
      "--border": "#432124",
      "--border-strong": "#65343a",
      "--text": "#fff0ec",
      "--text-dim": "#c4a39e",
      "--accent": "#ff7a5c",
      "--accent-2": "#8fd9c4",
      "--warn": "#ffb03a",
      "--danger": "#ff4f6a",
      "--on-accent": "#170a0c",
      "--scrim": "#170a0ccc",
    },
    light: {
      "--bg": "#fff8f5",
      "--panel": "#ffffff",
      "--panel-raised": "#ffeae3",
      "--panel-hover": "#ffd9cd",
      "--border": "#f4d3c8",
      "--border-strong": "#d0a294",
      "--text": "#2a0f0c",
      "--text-dim": "#70453c",
      "--accent": "#a8301b",
      "--accent-2": "#00695f",
      "--warn": "#8a5a00",
      "--danger": "#b3123a",
      "--on-accent": "#ffffff",
      "--scrim": "#fff8f5cc",
    },
  },
  /* §32's Wedding. A room with tablecloths and uplighters.
   *
   * Low chroma and warm neutral, because the screen is visible to the guests
   * and nothing about a first dance should look like a nightclub. Its warning
   * colour goes olive rather than amber: the accent is already warm, and two
   * warm colours side by side is the failure the §30 test below exists for. */
  "pkg-wedding": {
    dark: {
      "--bg": "#14110e",
      "--panel": "#1e1a16",
      "--panel-raised": "#2a241e",
      "--panel-hover": "#382f27",
      "--border": "#3a322a",
      "--border-strong": "#584c40",
      "--text": "#f7f1e8",
      "--text-dim": "#bdb0a0",
      "--accent": "#e6c98a",
      "--accent-2": "#a8b8d8",
      "--warn": "#e0a33a",
      "--danger": "#e06b6b",
      "--on-accent": "#14110e",
      "--scrim": "#14110ecc",
    },
    light: {
      "--bg": "#fdfbf7",
      "--panel": "#ffffff",
      "--panel-raised": "#f6f1e8",
      "--panel-hover": "#ece5d8",
      "--border": "#e6ddcd",
      "--border-strong": "#c0b39c",
      "--text": "#20190f",
      "--text-dim": "#5d5344",
      "--accent": "#7a4d17",
      "--accent-2": "#3c5480",
      "--warn": "#5f6b00",
      "--danger": "#b3123a",
      "--on-accent": "#ffffff",
      "--scrim": "#fdfbf7cc",
    },
  },
  /* §32's Lounge. Low light, slow music, a screen that should recede.
   *
   * The dimmest accents of the set and the smallest spread between panel
   * steps. A lounge screen that announced itself would be doing the opposite
   * of its job. */
  "pkg-lounge": {
    dark: {
      "--bg": "#0d0e12",
      "--panel": "#15161c",
      "--panel-raised": "#1d1f27",
      "--panel-hover": "#282b35",
      "--border": "#282a33",
      "--border-strong": "#3c404c",
      "--text": "#e6e4ee",
      "--text-dim": "#989aa8",
      "--accent": "#b08fd8",
      "--accent-2": "#6fb6c9",
      "--warn": "#d9a441",
      "--danger": "#d96a7a",
      "--on-accent": "#0d0e12",
      "--scrim": "#0d0e12cc",
    },
    light: {
      "--bg": "#faf9fb",
      "--panel": "#ffffff",
      "--panel-raised": "#f1eff5",
      "--panel-hover": "#e6e3ed",
      "--border": "#e0dde7",
      "--border-strong": "#b6b1c4",
      "--text": "#1a1822",
      "--text-dim": "#57536a",
      "--accent": "#5d3f92",
      "--accent-2": "#1a5f73",
      "--warn": "#8a5a00",
      "--danger": "#b3123a",
      "--on-accent": "#ffffff",
      "--scrim": "#faf9fbcc",
    },
  },
  /* §32's Scratch. The screen is not where a turntablist is looking.
   *
   * Near-monochrome, so the few things that carry colour -- the platter's
   * marker, the cue, an armed deck -- are the only things that do. §7's
   * arrangement and §74's rail are what a scratch DJ actually needs from the
   * screen and both already ship; this is the colours, which were the part
   * that was missing. */
  "pkg-scratch": {
    dark: {
      "--bg": "#0b0b0b",
      "--panel": "#141414",
      "--panel-raised": "#1e1e1e",
      "--panel-hover": "#2a2a2a",
      "--border": "#2c2c2c",
      "--border-strong": "#454545",
      "--text": "#f2f2f2",
      "--text-dim": "#9d9d9d",
      "--accent": "#ff3b30",
      "--accent-2": "#4dd2ff",
      "--warn": "#ffcc00",
      "--danger": "#ff6b8a",
      "--on-accent": "#0b0b0b",
      "--scrim": "#0b0b0bcc",
    },
    light: {
      "--bg": "#fafafa",
      "--panel": "#ffffff",
      "--panel-raised": "#f0f0f0",
      "--panel-hover": "#e2e2e2",
      "--border": "#dcdcdc",
      "--border-strong": "#a8a8a8",
      "--text": "#111111",
      "--text-dim": "#4d4d4d",
      "--accent": "#c1121f",
      "--accent-2": "#005f87",
      "--warn": "#7a5a00",
      "--danger": "#a3123a",
      "--on-accent": "#ffffff",
      "--scrim": "#fafafacc",
    },
  },
  /* §32's Stem Lab, and the one with a real constraint behind it.
   *
   * The four stem colours are §57's roles -- vocal, drums, bass and other --
   * and they are `--accent`, `--warn`, `--danger` and `--accent-2`. A palette
   * for working on stems has to be built around those four rather than
   * choosing them, so everything else here is grey and gets out of their way,
   * and the four are further apart than in any other palette that ships. */
  "pkg-stemlab": {
    dark: {
      "--bg": "#0c0e10",
      "--panel": "#14171a",
      "--panel-raised": "#1d2126",
      "--panel-hover": "#282d33",
      "--border": "#282d33",
      "--border-strong": "#3e454e",
      "--text": "#eef1f4",
      "--text-dim": "#98a1ab",
      "--accent": "#5ad2ff",
      "--accent-2": "#b388ff",
      "--warn": "#ffc247",
      "--danger": "#ff7a6b",
      "--on-accent": "#0c0e10",
      "--scrim": "#0c0e10cc",
    },
    light: {
      "--bg": "#fbfcfd",
      "--panel": "#ffffff",
      "--panel-raised": "#eff2f5",
      "--panel-hover": "#e2e7ec",
      "--border": "#dde3e9",
      "--border-strong": "#b0bac4",
      "--text": "#0f1418",
      "--text-dim": "#4c5661",
      "--accent": "#005f87",
      "--accent-2": "#5b2fb3",
      "--warn": "#8a5a00",
      "--danger": "#b3123a",
      "--on-accent": "#ffffff",
      "--scrim": "#fbfcfdcc",
    },
  },
};

export function applyPackagePalette(pkgId: string, resolved: ResolvedTheme) {
  const root = document.documentElement;
  const set = palettes[pkgId] ?? palettes["pkg-organic"];
  const palette = resolved === "light" ? set.light : set.dark;
  for (const [k, v] of Object.entries(palette)) {
    root.style.setProperty(k, v);
  }
}

export function listPaletteIds(): string[] {
  return Object.keys(palettes);
}

/**
 * One theme's colours, without touching the document.
 *
 * Exported so the palettes can be *checked* -- contrast ratios, missing
 * tokens, a dark block pasted into the light slot -- none of which is
 * observable through `applyPackagePalette`, because that writes to the DOM and
 * returns nothing. It is also what lets the picker draw a swatch of a theme it
 * has not applied.
 */
export function paletteFor(pkgId: string, resolved: ResolvedTheme): Palette {
  const set = palettes[pkgId] ?? palettes["pkg-organic"];
  return resolved === "light" ? set.light : set.dark;
}
