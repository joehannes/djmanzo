/**
 * §89's other half: comparing **appearance**, not only geometry.
 *
 * `regression.spec.ts` holds the layout rules in ten configurations and says,
 * at the end, exactly what it cannot do: *a theme that turned every panel the
 * same colour would pass every one of these*. This is that test.
 *
 * # Why not a pixel diff
 *
 * The reason `configurations.ts` gives, unchanged: CI installs its own
 * Chromium and this container has a different build, so font rasterisation
 * differs and a baseline captured in either place fails in the other for
 * reasons that have nothing to do with djmanzo. A suite re-blessed every run
 * has stopped being a test.
 *
 * Colour is not like that. `getComputedStyle` resolves `var()` and
 * `color-mix()` to numbers, and those numbers are the same on any machine — so
 * appearance *can* be regressed exactly, as long as what is regressed is the
 * design language's own relationships rather than a bitmap.
 *
 * # What is actually checked, and why it can fail
 *
 * §30's roles are **not** in a theme package. A package carries fourteen base
 * colours; the roles are derived from them in `app.css` — `--selected` is
 * `var(--accent-2)`, `--active` is `var(--accent)`, `--uncertain` is a
 * `color-mix` of the accent with the dim text. So the guarantee that two roles
 * stay distinguishable is a property of a *derivation meeting a palette*, and
 * it can collapse in one palette while holding in the other six. Nothing could
 * see that: Rust checks the pairs, vitest checks that components ask for
 * tokens, and `roles.spec.ts` checks one pair on one theme.
 *
 * Which pairs matter is Rust's judgement, read from `roles.json`, which
 * `cockpit::Role::must_differ_from` blesses. A pair added there fails here
 * until every shipped palette honours it.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import roles from "./roles.json" with { type: "json" };
import { listPaletteIds, paletteFor } from "../src/controls/themes/colors";
import { errorsThrown, openShell } from "./shell";

/**
 * How far apart two colours have to be to count as different.
 *
 * OKLab distance, which is perceptual: 0.02 is the edge of visible on a good
 * screen and 0.05 is unmistakable.
 *
 * 0.05 rather than something tighter for a reason about the room: a booth is
 * dark and a DJ is glancing, and two colours that are technically different
 * are not two colours that are *told apart* at arm's length in the twenty
 * seconds before a mix.
 *
 * **A floor, not a target, and the truth sits just above it**: the closest
 * pair djmanzo actually ships is `danger` against `success` in pkg-latin's
 * light palette, at **0.0563**. That is thirteen per cent of headroom, which
 * is narrow enough to be worth knowing — a new palette has very little room to
 * be careless in. `reports how much room the thresholds actually have` prints
 * the current figure; lower the number here whenever it drops, because a
 * ratchet drifted above reality holds nothing.
 */
const APART = 0.05;

/**
 * The surface ladder has to be a ladder.
 *
 * Its own floor, an order of magnitude below the roles', and deliberately:
 * depth is read as a nudge in lightness across a large area, which the eye
 * picks up far below the distance two small coloured marks need. The closest
 * step shipped is pkg-stemlab's light page against its panels at **0.0096**,
 * against a floor of 0.008 — see `keeps the surfaces a ladder`.
 */
const LADDER = ["--bg", "--panel", "--panel-raised"];

/**
 * Every shipped package, in both light and dark.
 *
 * From `listPaletteIds` rather than typed out: a palette added to the table
 * and left out of this list would be the one nobody checked.
 */
const PALETTES = listPaletteIds().flatMap((id) =>
  (["dark", "light"] as const).map((resolved) => ({ id, resolved })),
);

/** sRGB 0..255 to linear. */
function linear(channel: number): number {
  const c = channel / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/**
 * `rgb(r, g, b)` as OKLab.
 *
 * Björn Ottosson's matrices. Written out here rather than pulled in as a
 * dependency: it is twelve constants, and a colour library is a licence
 * question (ADR-0002) for arithmetic that fits on a screen.
 */
function oklab(css: string): [number, number, number] {
  const found = css.match(/-?[\d.]+/g);
  if (!found || found.length < 3) throw new Error(`not a colour: ${css}`);
  const [r, g, b] = found.slice(0, 3).map((n) => linear(Number(n)));

  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);

  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

function apart(one: string, two: string): number {
  const [l1, a1, b1] = oklab(one);
  const [l2, a2, b2] = oklab(two);
  return Math.hypot(l1 - l2, a1 - a2, b1 - b2);
}

/** WCAG relative luminance, from a rendered `rgb()`. */
function luminance(css: string): number {
  const found = css.match(/-?[\d.]+/g);
  if (!found || found.length < 3) throw new Error(`not a colour: ${css}`);
  const [r, g, b] = found.slice(0, 3).map((n) => linear(Number(n)));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(one: string, two: string): number {
  const a = luminance(one);
  const b = luminance(two);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

/**
 * Wear a palette, and read back what the document actually resolves.
 *
 * The fourteen base colours are written onto the root the way
 * `applyPackagePalette` writes them, and then the browser is asked what every
 * *role* came out as. That middle step is the whole test: the roles are
 * derived in `app.css`, so nothing outside a browser knows what they are.
 */
async function wearing(
  page: Page,
  id: string,
  resolved: "dark" | "light",
  tokens: string[],
): Promise<Record<string, string>> {
  const palette = paletteFor(id, resolved);
  return page.evaluate(
    ({ palette, resolved, tokens }) => {
      const root = document.documentElement;
      root.setAttribute("data-theme", resolved);
      for (const [key, value] of Object.entries(palette)) {
        root.style.setProperty(key, value as string);
      }
      // A probe rather than the root itself: `color-mix()` resolves against a
      // real element, and reading a custom property off the root hands back
      // the unresolved text -- `color-mix(in oklab, var(--accent) 35%, ...)`
      // as a string, which compares equal to every other unresolved mix and
      // would make this test pass by saying nothing.
      const probe = document.createElement("span");
      probe.style.position = "absolute";
      probe.style.opacity = "0";
      document.body.append(probe);
      const out: Record<string, string> = {};
      for (const token of tokens) {
        probe.style.color = `var(${token})`;
        out[token] = getComputedStyle(probe).color;
      }
      probe.remove();
      return out;
    },
    { palette, resolved, tokens },
  );
}

const ROLE_TOKENS = roles.map((role) => `--${role.token}`);
const ALL_TOKENS = [...ROLE_TOKENS, ...LADDER, "--text", "--text-dim"];

test.describe("§89's appearance", () => {
  /**
   * What the margins actually are, printed rather than asserted.
   *
   * The thresholds above are floors, and a floor is only honest while somebody
   * knows how far above it the truth sits. This reports the worst pair, the
   * worst surface step and the worst contrast across every shipped palette, so
   * the numbers in this file can be checked rather than taken.
   */
  test("reports how much room the thresholds actually have", async ({ page }) => {
    await openShell(page, "/");
    let worstPair = { what: "", distance: Infinity };
    let worstStep = { what: "", distance: Infinity };
    let worstText = { what: "", ratio: Infinity };

    for (const { id, resolved } of PALETTES) {
      const worn = await wearing(page, id, resolved, ALL_TOKENS);
      for (const role of roles) {
        for (const other of role.must_differ_from) {
          if (role.token >= other) continue;
          const distance = apart(worn[`--${role.token}`], worn[`--${other}`]);
          if (distance < worstPair.distance) {
            worstPair = { what: `${id}/${resolved} ${role.token}·${other}`, distance };
          }
        }
      }
      for (let n = 1; n < LADDER.length; n += 1) {
        const distance = apart(worn[LADDER[n - 1]], worn[LADDER[n]]);
        if (distance < worstStep.distance) {
          worstStep = { what: `${id}/${resolved} ${LADDER[n - 1]}·${LADDER[n]}`, distance };
        }
      }
      for (const token of ["--text", "--text-dim"]) {
        const ratio = contrast(worn[token], worn["--panel"]);
        if (ratio < worstText.ratio) {
          worstText = { what: `${id}/${resolved} ${token}`, ratio };
        }
      }
    }

    console.log(
      `\n  worst role pair:    ${worstPair.distance.toFixed(4)}  (${worstPair.what}), floor ${APART}` +
        `\n  worst surface step: ${worstStep.distance.toFixed(4)}  (${worstStep.what}), floor 0.008` +
        `\n  worst text:         ${worstText.ratio.toFixed(2)}:1 (${worstText.what})\n`,
    );
  });

  /**
   * **The load-bearing one: no shipped palette collapses a distinction §30
   * says costs something.**
   *
   * Every pair, every palette, light and dark. The failure this exists to
   * catch is a theme where `--selected` and `--active` land on the same hue —
   * which renders as a browser where the thing you picked and the thing that
   * is running look identical, in exactly the moment that matters.
   */
  for (const { id, resolved } of PALETTES) {
    test(`${id} in ${resolved} keeps §30's roles apart`, async ({ page }) => {
      await openShell(page, "/");
      const worn = await wearing(page, id, resolved, ALL_TOKENS);

      const collapsed: string[] = [];
      for (const role of roles) {
        for (const other of role.must_differ_from) {
          // Each pair once. `must_differ_from` is symmetric, so reporting both
          // directions would double every failure and halve how much of the
          // list a reader gets through.
          if (role.token >= other) continue;
          const distance = apart(worn[`--${role.token}`], worn[`--${other}`]);
          if (distance < APART) {
            collapsed.push(
              `${role.token} and ${other} are ${distance.toFixed(3)} apart ` +
                `(${worn[`--${role.token}`]} vs ${worn[`--${other}`]})`,
            );
          }
        }
      }

      expect(
        collapsed,
        `${id}/${resolved} makes distinctions §30 says a DJ has to see, and ` +
          `they came out the same colour:\n  ${collapsed.join("\n  ")}`,
      ).toEqual([]);
      expect(errorsThrown(page)).toEqual([]);
    });
  }

  /**
   * **The surfaces stay a ladder.**
   *
   * `--bg`, `--panel` and `--panel-raised` are how depth is expressed without
   * shadows, and a theme that flattened them is the one `regression.spec.ts`
   * names as the thing it cannot see: every layout rule would still pass, and
   * the interface would be one slab.
   *
   * A lower floor than the roles, deliberately, for the reason `LADDER` gives.
   * This is the assertion that found `pkg-daylight` drawing its page and its
   * panels in exactly the same white — 0.0000 apart, in the one theme meant
   * for working outdoors, where a washed-out hairline border is the *first*
   * thing to go.
   */
  for (const { id, resolved } of PALETTES) {
    test(`${id} in ${resolved} keeps the surfaces a ladder`, async ({ page }) => {
      await openShell(page, "/");
      const worn = await wearing(page, id, resolved, ALL_TOKENS);

      const steps: string[] = [];
      for (let n = 1; n < LADDER.length; n += 1) {
        const distance = apart(worn[LADDER[n - 1]], worn[LADDER[n]]);
        if (distance < 0.008) {
          steps.push(
            `${LADDER[n - 1]} and ${LADDER[n]} are ${distance.toFixed(4)} apart`,
          );
        }
      }
      expect(
        steps,
        `${id}/${resolved} draws its surfaces as one slab:\n  ${steps.join("\n  ")}`,
      ).toEqual([]);
    });
  }

  /**
   * **Text is readable on the surface it sits on.**
   *
   * WCAG AA for body text is 4.5:1, and this holds the ordinary text to it and
   * the dim text to the 3:1 a large or secondary label gets — measured on what
   * the browser *rendered* rather than on the token table, because a component
   * can ask for the right token and a palette can still ship a pair nobody
   * checked together.
   *
   * §33 is why the dim text is in here at all: it is the colour djmanzo says
   * most of its own explaining in. It has more room than it looks: the worst
   * shipped is pkg-industrial's light dim text at **5.98:1**, which clears AA
   * for body text let alone the 3:1 asked of it here.
   */
  for (const { id, resolved } of PALETTES) {
    test(`${id} in ${resolved} is readable`, async ({ page }) => {
      await openShell(page, "/");
      const worn = await wearing(page, id, resolved, ALL_TOKENS);

      const text = contrast(worn["--text"], worn["--panel"]);
      expect(
        text,
        `${id}/${resolved}: ordinary text on a panel is ${text.toFixed(2)}:1`,
      ).toBeGreaterThanOrEqual(4.5);

      const dim = contrast(worn["--text-dim"], worn["--panel"]);
      expect(
        dim,
        `${id}/${resolved}: dim text on a panel is ${dim.toFixed(2)}:1, and it ` +
          `is what djmanzo does most of its explaining in`,
      ).toBeGreaterThanOrEqual(3);
    });
  }
});
