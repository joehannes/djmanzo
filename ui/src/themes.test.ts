/**
 * Every theme has to be a whole theme.
 *
 * Two ways a theme can be half-finished, and neither of them looks like a bug
 * from inside the code:
 *
 * - **A package with no palette.** `applyPackagePalette` falls back to the
 *   organic one, so picking the new theme silently gives you a different
 *   theme's colours. Nothing throws; the picker just appears not to work.
 * - **A palette missing a token.** Custom properties are inherited, so an
 *   absent `--danger` keeps whatever the previously applied theme set — the
 *   interface then wears one colour from a palette nobody selected, and only
 *   on the second theme change.
 *
 * Both are caught here rather than by looking, because both are invisible
 * until the exact sequence that exposes them.
 */
import { describe, expect, it } from "vitest";
import { themePackages } from "./controls/themes/packages";
import { listPaletteIds, paletteFor } from "./controls/themes/colors";

/** The tokens the interface actually reads. */
const REQUIRED = [
  "--bg",
  "--panel",
  "--panel-raised",
  "--panel-hover",
  "--border",
  "--border-strong",
  "--text",
  "--text-dim",
  "--accent",
  "--accent-2",
  "--warn",
  "--danger",
  "--on-accent",
  "--scrim",
];

describe("themes", () => {
  it("every package has a palette of its own", () => {
    const palettes = new Set(listPaletteIds());
    for (const pkg of themePackages) {
      expect(
        palettes.has(pkg.id),
        `${pkg.name} has no palette, so choosing it silently shows another theme's colours`,
      ).toBe(true);
    }
  });

  it("every palette defines every token, in both light and dark", () => {
    for (const pkg of themePackages) {
      for (const resolved of ["dark", "light"] as const) {
        const palette = paletteFor(pkg.id, resolved);
        for (const token of REQUIRED) {
          expect(
            palette[token],
            `${pkg.name} (${resolved}) has no ${token}, so it would keep the previous theme's`,
          ).toBeTruthy();
        }
      }
    }
  });

  it("every theme says where it is for and when to use it", () => {
    for (const pkg of themePackages) {
      expect(pkg.setting, `${pkg.name} names no setting`).toBeTruthy();
      expect(
        pkg.when.length,
        `${pkg.name} does not say when to use it, so the picker cannot explain it`,
      ).toBeGreaterThan(20);
    }
  });

  it("the settings a DJ actually works in are all covered", () => {
    const covered = new Set(themePackages.map((pkg) => pkg.setting));
    for (const setting of ["daylight", "home", "booth", "venue"] as const) {
      expect(
        covered.has(setting),
        `nothing to pick for ${setting}`,
      ).toBe(true);
    }
  });

  /**
   * **A dark theme has to be darker than its own light one.**
   *
   * The cheapest possible mistake when adding a palette is pasting the light
   * block into the dark slot, and it produces a theme that is not obviously
   * wrong in a diff — two plausible sets of hex, in the right shape, in the
   * wrong order. It is obvious the moment the app is opened in a dark room,
   * which is the worst time to find out.
   */
  it("a dark palette is darker than the light one it ships with", () => {
    const luminance = (hex: string): number => {
      const value = hex.replace("#", "").slice(0, 6);
      const full =
        value.length === 3
          ? value
              .split("")
              .map((c) => c + c)
              .join("")
          : value;
      const [r, g, b] = [0, 2, 4].map((i) => parseInt(full.slice(i, i + 2), 16));
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    for (const pkg of themePackages) {
      const dark = luminance(paletteFor(pkg.id, "dark")["--bg"]);
      const light = luminance(paletteFor(pkg.id, "light")["--bg"]);
      expect(
        dark,
        `${pkg.name}'s dark background (${dark.toFixed(0)}) is not darker than its light one (${light.toFixed(0)})`,
      ).toBeLessThan(light);
    }
  });

  /**
   * **Text has to be readable on the panel it sits on.**
   *
   * WCAG AA for body text is 4.5:1. This checks the two pairings that carry
   * almost everything on screen — text on a panel, and the accent's own
   * foreground on the accent — because a palette that fails them is unusable
   * rather than merely ugly, and a palette author cannot see a ratio.
   */
  it("body text and accent labels clear 4.5:1 against their backgrounds", () => {
    const channel = (value: number): number => {
      const v = value / 255;
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
    };
    const relative = (hex: string): number => {
      const value = hex.replace("#", "").slice(0, 6);
      const [r, g, b] = [0, 2, 4].map((i) =>
        channel(parseInt(value.slice(i, i + 2), 16)),
      );
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const ratio = (a: string, b: string): number => {
      const [hi, lo] = [relative(a), relative(b)].sort((x, y) => y - x);
      return (hi + 0.05) / (lo + 0.05);
    };
    for (const pkg of themePackages) {
      for (const resolved of ["dark", "light"] as const) {
        const p = paletteFor(pkg.id, resolved);
        expect(
          ratio(p["--text"], p["--panel"]),
          `${pkg.name} (${resolved}): body text on a panel`,
        ).toBeGreaterThanOrEqual(4.5);
        expect(
          ratio(p["--on-accent"], p["--accent"]),
          `${pkg.name} (${resolved}): a label on an accent-filled button`,
        ).toBeGreaterThanOrEqual(4.5);
      }
    }
  });
});
