/**
 * §121: a theme worn whole, as its colours alone, or in parts.
 *
 * `compose` is the one rule, so these hold what each setting of it must
 * give the controls: Full is a theme exactly as it was, Basic is plain still
 * circles, and Parts takes each layer from where it was told.
 */
import { describe, expect, it } from "vitest";
import { GeometryCircle } from "./engine";
import { PkgCyber, PkgIndustrial, PkgOrganic, PkgDaylight, themePackages } from "./packages";
import {
  BASIC,
  FULL,
  PARTS,
  SOME,
  amountOf,
  compose,
  eventStyleOf,
  offered,
  readParts,
} from "./parts";

describe("themes in parts", () => {
  /**
   * **The load-bearing one.** Each part comes from the theme it names and
   * from nowhere else, while the id -- what the palette keys on -- stays the
   * chosen theme's.
   */
  it("each part is taken from where it was told, and the colours stay the chosen theme's", () => {
    const worn = compose(PkgOrganic, {
      shapes: PkgIndustrial.id,
      art: PkgCyber.id,
      motion: "none",
      events: PkgIndustrial.id,
    });
    expect(worn.pkg.id).toBe(PkgOrganic.id);
    expect(worn.pkg.name).toBe(PkgOrganic.name);
    expect(worn.pkg.setting).toBe(PkgOrganic.setting);
    expect(worn.pkg.geometry).toBe(PkgIndustrial.geometry);
    expect(worn.pkg.effects).toEqual(PkgCyber.effects);
    expect(worn.pkg.behaviors).toEqual([]);
    expect(worn.events).toBe("flash");
    // The chosen theme is not changed by being worn in parts.
    expect(PkgOrganic.geometry).not.toBe(PkgIndustrial.geometry);
    expect(PkgOrganic.behaviors.length).toBeGreaterThan(0);
  });

  it("Full is every theme exactly as it was", () => {
    for (const pkg of themePackages) {
      const worn = compose(pkg, FULL);
      expect(worn.pkg, pkg.name).toBe(pkg);
      expect(worn.events).toBe(eventStyleOf(pkg));
    }
  });

  it("Basic is the colours alone: plain circles, standing still, no event skin", () => {
    for (const pkg of themePackages) {
      const worn = compose(pkg, BASIC);
      expect(worn.pkg.id).toBe(pkg.id);
      expect(worn.pkg.geometry).toBe(GeometryCircle);
      expect(worn.pkg.effects).toEqual([]);
      expect(worn.pkg.behaviors).toEqual([]);
      expect(worn.events).toBeNull();
    }
  });

  it("Basic, Parts and Full are read back from the parts, not stored beside them", () => {
    expect(amountOf(FULL)).toBe("full");
    expect(amountOf(BASIC)).toBe("basic");
    expect(amountOf(SOME)).toBe("parts");
    expect(amountOf({ ...FULL, art: "none" })).toBe("parts");
  });

  it("a part naming a theme that is gone comes from the chosen one", () => {
    const worn = compose(PkgCyber, { ...FULL, shapes: "pkg-gone", art: "pkg-gone" });
    expect(worn.pkg.geometry).toBe(PkgCyber.geometry);
    expect(worn.pkg.effects).toEqual(PkgCyber.effects);
  });

  it("a saved choice that cannot be read is Full, and one part saved keeps the rest Full", () => {
    expect(readParts(null)).toEqual(FULL);
    expect(readParts("not json")).toEqual(FULL);
    expect(readParts('{"art":"none","shapes":7}')).toEqual({ ...FULL, art: "none" });
  });

  /**
   * The list for a part offers different things, not twenty names for one
   * circle, and never a theme that brings nothing to that part.
   */
  it("each part offers one theme of each kind, and none that bring nothing", () => {
    const shapes = offered("shapes");
    expect(new Set(shapes.map((pkg) => pkg.geometry)).size).toBe(shapes.length);
    expect(shapes.length).toBeGreaterThanOrEqual(3);
    const art = offered("art");
    expect(art.every((pkg) => pkg.effects.length > 0)).toBe(true);
    expect(art.map((pkg) => pkg.id)).toEqual(expect.arrayContaining([PkgCyber.id, PkgIndustrial.id]));
    expect(offered("motion").every((pkg) => pkg.behaviors.length > 0)).toBe(true);
    expect(offered("events").map(eventStyleOf).sort()).toEqual(["flash", "glitch", "glow", "line"]);
    expect(PARTS.map((part) => part.name)).toEqual(["shapes", "art", "motion", "events"]);
    expect(eventStyleOf(PkgDaylight)).toBe("line");
  });
});
