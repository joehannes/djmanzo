import { describe, expect, it } from "vitest";
import { themePackages } from "../controls/themes/packages";
import { SKINS, isSingersScreen, skinCss, skinsFor } from "./skins";

describe("skins (karaoke beta B6)", () => {
  /** Every skin is offered with real themes, and every Caribbean one has two or three. */
  it("is offered with themes that exist, two or three for the Caribbean ones", () => {
    const ids = new Set(themePackages.map((p) => p.id));
    for (const skin of SKINS) {
      for (const pkg of skin.suits) expect(ids.has(pkg), `${skin.id} suits ${pkg}`).toBe(true);
    }
    for (const pkg of ["pkg-caribbean", "pkg-malecon", "pkg-carnaval", "pkg-biobay", "pkg-rosa"]) {
      expect(skinsFor(pkg).length, pkg).toBeGreaterThanOrEqual(2);
      expect(skinsFor(pkg).length, pkg).toBeLessThanOrEqual(3);
    }
  });

  /** Faint, never taking a press, off under high contrast. */
  it("is faint, takes no press, and is off under high contrast", () => {
    for (const skin of SKINS) {
      expect(skin.opacity).toBeGreaterThanOrEqual(0.1);
      expect(skin.opacity).toBeLessThanOrEqual(0.18);
      expect(skin.tile.startsWith("<svg"), skin.id).toBe(true);
    }
    const css = skinCss();
    expect(css).toContain("pointer-events: none");
    expect(css).toMatch(/prefers-contrast: more[^{]*\{\s*html\[data-skin\] body::after \{ display: none; \}/);
  });

  it("knows the singers' screen", () => {
    expect(isSingersScreen("?panel=singers")).toBe(true);
    expect(isSingersScreen("?panel=library")).toBe(false);
    expect(isSingersScreen("")).toBe(false);
  });
});
