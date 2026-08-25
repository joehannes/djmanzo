import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { ICONS, iconGlyph, normalizeIconName } from "./icons";

/** Every `.svelte` file under `ui/src`. */
function svelteFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) return svelteFiles(path);
    return path.endsWith(".svelte") ? [path] : [];
  });
}

/** Every icon name the interface asks for, by reading the interface. */
function iconsInUse(): { name: string; file: string }[] {
  const found: { name: string; file: string }[] = [];
  for (const file of svelteFiles(join(import.meta.dirname, ".."))) {
    const source = readFileSync(file, "utf8");
    for (const match of source.matchAll(/icon=["']([^"']+)["']/g)) {
      found.push({ name: match[1], file });
    }
  }
  return found;
}

describe("the icon set", () => {
  /**
   * **The defect this pins.** The icons came from a CDN, which a packaged
   * build cannot reach: the application's own CSP is `default-src 'self'`,
   * and a DJ at a venue has no internet. Every icon button rendered as an
   * empty square, and nothing in the build said so.
   *
   * Reading the components rather than listing names by hand is the point --
   * a new `icon="fa-solid fa-whatever"` that has no drawing fails here
   * instead of shipping as a blank button.
   */
  it("has a drawing for every icon the interface asks for", () => {
    const used = iconsInUse();
    expect(used.length).toBeGreaterThan(20);

    const missing = used
      .filter(({ name }) => !iconGlyph(name))
      .map(({ name, file }) => `${name} (${file.split("/src/")[1]})`);

    expect(missing, "icons with no drawing").toEqual([]);
  });

  it("accepts both the bare name and the Font Awesome form", () => {
    expect(normalizeIconName("gear")).toBe("gear");
    expect(normalizeIconName("fa-solid fa-gear")).toBe("gear");
    expect(normalizeIconName("fa-regular fa-circle-question")).toBe("circle-question");
    expect(iconGlyph("fa-solid fa-microphone")).toBe(ICONS.microphone);
  });

  it("says nothing for a name it does not have, rather than guessing", () => {
    expect(iconGlyph("fa-solid fa-not-a-real-icon")).toBeUndefined();
    expect(iconGlyph("")).toBeUndefined();
  });

  /**
   * A path that does not start with a move command draws nothing, silently.
   *
   * The magnitude check is deliberately weaker than "every coordinate is
   * inside the viewBox", because these paths mix absolute and relative
   * commands and a regex cannot tell `L20` from `l-8`: the negative numbers
   * in `M20 6v12l-8-6 8-6z` are legitimate deltas, not coordinates off the
   * grid. What it does catch is a number that could not be either on a
   * 24-unit grid -- a stray 240 from a dropped decimal point, say -- which is
   * the mistake that actually happens when hand-editing a path.
   */
  it("every drawing is a well-formed path at the grid's scale", () => {
    for (const [name, glyph] of Object.entries(ICONS)) {
      expect(glyph.d.startsWith("M"), `${name} does not begin with a move`).toBe(true);
      // The shortest honest glyph is a straight line -- `minus` is `M5 12h14`,
      // eight characters -- so the bound is only there to catch a stub.
      expect(glyph.d.length, `${name} is suspiciously short`).toBeGreaterThan(6);

      // SVG allows `.35` as well as `0.35`, and both appear in these paths.
      const numbers = (glyph.d.match(/-?(?:\d+(?:\.\d+)?|\.\d+)/g) ?? []).map(Number);
      const wild = numbers.filter((n) => Math.abs(n) > 26);
      expect(wild, `${name} has numbers off the grid's scale: ${wild}`).toEqual([]);
    }
  });

  it("no two names share a drawing by accident", () => {
    // Names Font Awesome itself treats as the same glyph, kept so existing
    // call sites resolve without being rewritten.
    const aliases = new Set(["cog", "th", "hand-paper"]);
    const seen = new Map<string, string>();
    for (const [name, glyph] of Object.entries(ICONS)) {
      if (aliases.has(name)) continue;
      const previous = seen.get(glyph.d);
      expect(previous, `${name} and ${previous} draw the same thing`).toBeUndefined();
      seen.set(glyph.d, name);
    }
  });
});
