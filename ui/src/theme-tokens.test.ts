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

  /**
   * **Every token a component asks for has a value somewhere.**
   *
   * The load-bearing one, and it found the defect it was written for: the
   * interface asked for `--muted` forty-seven times, `--ok` fourteen, `--line`
   * ten and `--control` eight, and not one of those had a value anywhere.
   *
   * Two failures, both invisible. Without a fallback the declaration is
   * invalid and the property inherits, so text asking to be dim came out the
   * ordinary colour. With one -- `var(--ok, #6a9955)` -- the literal wins and
   * that part of the interface draws a fixed hue on every palette, which is
   * precisely what the hex-literal test above exists to prevent, escaping
   * through the single exemption it grants.
   *
   * So a fallback does not excuse a token from this: the point is that the
   * token resolves.
   */
  it("every token a component asks for is defined somewhere", () => {
    const defined = new Set<string>();
    for (const file of PALETTE_FILES) {
      const source = readFileSync(join(SRC, file), "utf8");
      for (const match of source.matchAll(/--([a-z0-9-]+)\s*:/g)) {
        defined.add(match[1]);
      }
      // `colors.ts` writes them as quoted keys rather than declarations.
      for (const match of source.matchAll(/"--([a-z0-9-]+)"/g)) {
        defined.add(match[1]);
      }
    }

    // A token one component sets for another is provided, even though no
    // palette mentions it: `--jog-size` is a deck telling a jog wheel how big
    // to be, which is a contract between two components rather than a colour.
    // The rule being enforced is that *somebody* provides what is asked for.
    const files = svelteFiles(SRC);
    for (const file of files) {
      for (const match of readFileSync(file, "utf8").matchAll(
        /--([a-z0-9-]+)\s*[:=]/g,
      )) {
        defined.add(match[1]);
      }
    }

    const missing = new Map<string, string[]>();
    for (const file of files) {
      for (const match of readFileSync(file, "utf8").matchAll(
        /var\(\s*--([a-z0-9-]+)/g,
      )) {
        const token = match[1];
        if (defined.has(token)) continue;
        missing.set(token, [...(missing.get(token) ?? []), file.split("/src/")[1]]);
      }
    }

    const report = [...missing]
      .map(([token, files]) => `--${token} (${[...new Set(files)].join(", ")})`)
      .sort();
    expect(
      report,
      `these are asked for and never defined, so they inherit or fall back to ` +
        `a fixed hue:\n${report.join("\n")}`,
    ).toEqual([]);
  });

  /**
   * **§30's roles, where they are actually drawn.**
   *
   * The roles existed and nothing asked for them. Every control in the
   * interface painted its "this is on" state with `--accent` or `--accent-2`
   * directly, which renders identically today — `--active` *is* the accent —
   * and throws the meaning away: a palette that wanted "what the DJ chose" in
   * a different colour from the brand accent had no way to say so, and two
   * roles §30 declares as a pair that must differ were one colour in thirty
   * places.
   *
   * The rule is narrow, because it has to be. A CSS rule whose **selector**
   * names a state — `.active`, `.on`, `.selected`, `.chosen`, `.lit`,
   * `.picked`, `.running` — and which paints a fill, an edge or its text must
   * name a role. `--accent` on a heading underline or a focus ring is not a
   * state and is not this rule's business.
   */
  const STATE = /\.(active|on|selected|chosen|lit|picked|running)\b/;
  const ROLES = [
    "outgoing",
    "incoming",
    "selected",
    "active",
    "assistant",
    "uncertain",
    "audience",
    "success",
    "warn",
    "danger",
    "stem-vocal",
    "stem-drums",
    "stem-bass",
    "stem-other",
    "on-selected",
    "on-active",
    "on-accent",
  ];
  /** Tokens that are not hues: shape, ground and text, which carry no meaning. */
  const NEUTRAL = [
    "panel",
    "panel-raised",
    "bg",
    "surface",
    "surface-2",
    "sunken",
    "text",
    "text-dim",
    "muted",
    "fg",
    "border",
    "line",
    "edge",
    "control",
    "chip",
  ];

  it("a state is painted with the role it means, not with the accent", () => {
    const offenders: string[] = [];
    for (const file of svelteFiles(SRC)) {
      const source = readFileSync(file, "utf8");
      const style = source.slice(source.indexOf("<style>"));
      for (const rule of style.matchAll(
        /([^\n{}]*)\{([^{}]*(?:background|border-color|outline-color|\bcolor)\s*:[^{}]*)\}/g,
      )) {
        const [, selector, body] = rule;
        if (!STATE.test(selector)) continue;
        for (const paint of body.matchAll(
          /(background|border-color|outline-color|\bcolor)\s*:([^;]+)/g,
        )) {
          const [, property, value] = paint;
          // `transparent`, `inherit` and `currentColor` claim no hue at all.
          if (/^(\s*)(transparent|inherit|currentcolor|none)\s*$/i.test(value)) continue;
          const tokens = [...value.matchAll(/var\(\s*--([a-z0-9-]+)/g)].map((m) => m[1]);
          if (tokens.length === 0) {
            offenders.push(
              `${file.split("/src/")[1]}: ${selector.trim()} — ${property} names no token`,
            );
            continue;
          }
          if (tokens.some((t) => ROLES.includes(t) || NEUTRAL.includes(t))) continue;
          offenders.push(
            `${file.split("/src/")[1]}: ${selector.trim()} — ${property} uses ` +
              `${tokens.map((t) => `--${t}`).join(", ")} rather than a role`,
          );
        }
      }
    }
    expect(
      offenders.sort(),
      "these paint a state with a colour rather than with what the colour " +
        `means, so §30's roles cannot reach them:\n${offenders.sort().join("\n")}`,
    ).toEqual([]);
  });

  /**
   * **A filled state names what goes on top of the fill.**
   *
   * The bug this catches is the one the sweep found: every filled "on" button
   * paired its background with `--on-accent`, including the ones filled with
   * `--accent-2`. It read well on all seven palettes by luck rather than by
   * construction, and a palette that moved either accent would have discovered
   * that by shipping unreadable text.
   */
  it("a state filled with a role names the text colour that goes on it", () => {
    const PARTNERED = ["selected", "active"];
    const offenders: string[] = [];
    for (const file of svelteFiles(SRC)) {
      const source = readFileSync(file, "utf8");
      const style = source.slice(source.indexOf("<style>"));
      for (const rule of style.matchAll(/([^\n{}]*)\{([^{}]*)\}/g)) {
        const [, selector, body] = rule;
        if (!STATE.test(selector)) continue;
        const fill = body.match(/background\s*:\s*var\(\s*--(selected|active)\s*\)/);
        if (!fill || !PARTNERED.includes(fill[1])) continue;
        const partner = `--on-${fill[1]}`;
        if (body.includes(partner)) continue;
        offenders.push(
          `${file.split("/src/")[1]}: ${selector.trim()} — filled with ` +
            `--${fill[1]} and does not say ${partner}`,
        );
      }
    }
    expect(offenders.sort(), offenders.sort().join("\n")).toEqual([]);
  });

  it("the palette files are exempt, because that is where colours live", () => {
    for (const file of PALETTE_FILES) {
      const source = readFileSync(join(SRC, file), "utf8");
      expect(source).toMatch(/#[0-9a-fA-F]{3,8}\b/);
    }
  });
});
