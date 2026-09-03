/**
 * The design system's colour rule, made executable.
 *
 * djmanzo ships three palettes in dark and light, and every one of them works
 * only if components ask for a *token* rather than naming a colour. The stems
 * panel named four — `#0ea5e9`, `#ef4444`, `#a855f7`, `#22c55e` — and so was
 * the one part of the interface that looked identical on all six.
 *
 * The rule this enforces is narrow on purpose:
 *
 * - **A hue must be a token.** A hex literal names a hue, and a hue that is
 *   right on the organic palette is wrong on the industrial one.
 * - **A translucent black or white overlay is fine.** It shades whatever is
 *   underneath, so it is correct on any palette by construction — which is
 *   why `rgba(0, 0, 0, 0.35)` is allowed and `#0ea5e9` is not.
 * - `var(--token, #fallback)` is fine: the token leads, the literal is only
 *   what happens if the sheet failed to load.
 */
import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const SRC = join(import.meta.dirname);

/** Where colours are *defined*, so literals are the point. */
const PALETTE_FILES = ["app.css", join("controls", "themes", "colors.ts")];

function svelteFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) return svelteFiles(path);
    return path.endsWith(".svelte") ? [path] : [];
  });
}

/** Strip what is allowed to contain a literal: comments and token fallbacks. */
function stripAllowed(source: string): string {
  return (
    source
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/^\s*\/\/.*$/gm, "")
      .replace(/var\(\s*--[a-zA-Z0-9-]+\s*,[^)]*\)/g, "")
      // Numeric HTML entities. `&#9679;` is a filled circle, and it is not a
      // colour -- but `#9679` matches the hex pattern below exactly, so a
      // component drawing a glyph that way was reported as ignoring the
      // palette. A false positive on a rule this blunt costs whoever hits it
      // an hour finding out the test is wrong rather than the code.
      .replace(/&#x?[0-9a-fA-F]+;/g, "")
  );
}

describe("colour tokens", () => {
  it("no component names a hue with a hex literal", () => {
    const offenders: string[] = [];
    for (const file of svelteFiles(SRC)) {
      const bare = stripAllowed(readFileSync(file, "utf8")).match(
        /#[0-9a-fA-F]{3,8}\b/g,
      );
      if (bare) offenders.push(`${file}: ${[...new Set(bare)].join(", ")}`);
    }
    expect(
      offenders,
      `these name a colour instead of asking for a token, so they ignore the ` +
        `palette:\n${offenders.join("\n")}`,
    ).toEqual([]);
  });

  it("every stem has a colour token, in the sheet and in the panel", () => {
    const css = readFileSync(join(SRC, "app.css"), "utf8");
    const panel = readFileSync(join(SRC, "Stems.svelte"), "utf8");
    for (const stem of ["vocal", "drums", "bass", "other"]) {
      expect(css, `--stem-${stem} is not defined`).toContain(`--stem-${stem}:`);
      expect(panel, `the panel does not use --stem-${stem}`).toContain(
        `var(--stem-${stem})`,
      );
    }
  });

  it("the four stem colours are four different tokens", () => {
    const css = readFileSync(join(SRC, "app.css"), "utf8");
    const values = ["vocal", "drums", "bass", "other"].map((stem) => {
      const match = css.match(new RegExp(`--stem-${stem}:\\s*([^;]+);`));
      expect(match, `--stem-${stem} has no value`).not.toBeNull();
      return match![1].trim();
    });
    expect(
      new Set(values).size,
      `two stems share a colour, so they cannot be told apart: ${values.join(", ")}`,
    ).toBe(4);
  });

  it("the palette files are exempt, because that is where colours live", () => {
    for (const file of PALETTE_FILES) {
      const source = readFileSync(join(SRC, file), "utf8");
      expect(source).toMatch(/#[0-9a-fA-F]{3,8}\b/);
    }
  });
});
