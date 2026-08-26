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
      "--accent-2": "#28b485",
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
      "--accent-2": "#057a5f",
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
      "--warn": "#e3b23c",
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
      "--warn": "#95610a",
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
      "--warn": "#ffd23f",
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
      "--accent-2": "#8a3c00",
      "--warn": "#8a6100",
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
      "--accent-2": "#a16207",
      "--warn": "#9a6700",
      "--danger": "#b02a2a",
      "--on-accent": "#fffaf6",
      "--scrim": "#fff4eccc",
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
