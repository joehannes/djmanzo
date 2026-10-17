import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { NODES, children, lineage, resolveStyle } from "../src/style/genome/index";
import { merge, resolveWith } from "../src/style/genome/resolve";
import type { GenomeNode } from "../src/style/genome/types";

const STAGES = ["silence", "groove", "needle", "cue", "confluence", "voice", "amp", "cone", "floor", "afterglow"];

describe("a style inherits from its family", () => {
  it("electro swing is EDM's rig through Art Deco", () => {
    const edm = resolveStyle("edm");
    const swing = resolveStyle("electroswing");
    expect(lineage("electroswing")).toEqual(["music", "edm", "electroswing"]);
    // Changed: the room, the clothes, the dances, the feel of the beat.
    expect(swing.decor).toContain("gramophones");
    expect(swing.fashion).not.toEqual(edm.fashion);
    expect(swing.dance).toContain("Lindy Hop");
    expect(swing.motion.pulse).toBe("swing");
    expect(swing.palette.accents[0]).not.toBe(edm.palette.accents[0]);
    // Kept from the rave: one neon accent, and the drop-led structure's family.
    expect(swing.palette.accents).toContain(edm.palette.accents[0]);
    expect(swing.scene.cone).toMatch(/gramophone horn/);
    expect(edm.scene.cone).toMatch(/line-array/);
  });

  it("what a genre does not change, it inherits", () => {
    const edm = resolveStyle("edm");
    const trance = resolveStyle("trance");
    expect(trance.fashion).toEqual(edm.fashion);
    expect(trance.light).toEqual(edm.light);
    expect(trance.values).toEqual(edm.values);
    expect(trance.sound.bpm).toEqual([132, 140]);
    // A field set partly merges with the rest of the family's.
    expect(trance.palette.day).toBe(edm.palette.day);
    expect(trance.palette.night).toBe("#060716");
  });

  it("a list is replaced whole, never appended to", () => {
    expect(merge({ a: [1, 2], b: { c: 1, d: 2 } }, { a: [3], b: { c: 9 } })).toEqual({ a: [3], b: { c: 9, d: 2 } });
  });

  it("every style is complete, down to a scene for every stage", () => {
    for (const id of Object.keys(NODES)) {
      const s = resolveStyle(id);
      for (const st of STAGES) expect(s.scene[st as keyof typeof s.scene], `${id} ${st}`).toBeTruthy();
      expect(s.palette.accents.length, id).toBeGreaterThan(0);
      expect(s.complements.length, id).toBeGreaterThan(0);
      expect(s.sound.bpm[0], id).toBeLessThanOrEqual(s.sound.bpm[1]);
    }
  });

  it("an unknown style and a loop are refused", () => {
    expect(() => resolveStyle("polka-metal")).toThrow(/No style/);
    const loop: Record<string, GenomeNode> = { a: { id: "a", name: "A", parent: "b", set: {} }, b: { id: "b", name: "B", parent: "a", set: {} } };
    expect(() => resolveWith(loop, "a")).toThrow(/itself/);
  });

  it("a family lists its genres, electro swing under the jazz age too", () => {
    expect(children("latin")).toContain("bachata");
    expect(children("jazzage")).toContain("electroswing");
  });
});

describe("the djmanzo looks it names are real", () => {
  const packages = readFileSync(new URL("../../ui/src/controls/themes/packages.ts", import.meta.url), "utf8");
  const skins = readFileSync(new URL("../../ui/src/skins/skins.ts", import.meta.url), "utf8");
  const themeId = new Map([...packages.matchAll(/id: "(pkg-[a-z-]+)",\s*name: "([^"]+)"/g)].map((m) => [m[2], m[1]]));
  const skinSuits = new Map(
    [...skins.matchAll(/name: "([^"]+)",[\s\S]*?suits: \[([^\]]*)\]/g)].map((m) => [m[1], [...m[2].matchAll(/"([^"]+)"/g)].map((x) => x[1])]),
  );

  it("reads the application's lists", () => {
    expect(themeId.size).toBeGreaterThan(20);
    expect(skinSuits.size).toBe(6);
  });

  it("every style's theme ships, and its skin is one the application wears with that theme", () => {
    for (const id of Object.keys(NODES)) {
      const { theme, skin } = resolveStyle(id).app;
      expect(themeId.has(theme), `${id}: theme "${theme}"`).toBe(true);
      if (skin) expect(skinSuits.get(skin), `${id}: skin "${skin}" with "${theme}"`).toContain(themeId.get(theme));
    }
  });
});
