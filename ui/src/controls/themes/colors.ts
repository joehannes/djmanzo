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
      "--on-accent": "#071021",
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
