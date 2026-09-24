/**
 * §113: functional colour — a control wears the colour of what it does.
 *
 * > i also want more themes ... and the themes shall be functional as of
 * > color codes or usage parts, meaningful colors and gradients
 *
 * The palettes' contrast and the pairs that must stay apart are held by
 * `themes.test.ts` and by `dj_app::cockpit`'s tests. What a browser holds is
 * that the meanings reach the controls: each EQ knob's arc in its band's
 * colour (the waveform's colour for the same frequencies), the filter knob
 * in the colour of the side it keeps, the level meter's colour placed by
 * loudness — and that the new themes are there to wear.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const deck = (page: Page) => page.locator('.deck[data-deck="1"]');

/** The computed stroke of a knob's filled arc, and of a token, as rgb(). */
async function arcStroke(page: Page, selector: string) {
  return page.locator(selector).evaluate((wrapper) => {
    const paths = [...wrapper.querySelectorAll("path")] as SVGPathElement[];
    // The value arc is the second arc drawn, after the unfilled track.
    const strokes = paths.map((path) => getComputedStyle(path).stroke);
    return strokes;
  });
}

const token = (page: Page, name: string) =>
  page.evaluate((name) => {
    const probe = document.createElement("span");
    probe.style.color = `var(${name})`;
    document.body.append(probe);
    const value = getComputedStyle(probe).color;
    probe.remove();
    return value;
  }, name);

async function emitDeck(page: Page, change: Record<string, unknown>) {
  await page.evaluate((change) => {
    const win = window as unknown as {
      __lastState?: { decks: Record<string, unknown>[] };
      __emit?: (next: unknown) => void;
    };
    const state = win.__lastState;
    if (!state) throw new Error("no state");
    win.__emit?.({ ...state, decks: state.decks.map((d, i) => (i === 0 ? { ...d, ...change } : d)) });
  }, change);
}

test.describe("§113: functional colour", () => {
  /**
   * **The load-bearing one: each EQ knob's arc is its band's colour**, three
   * different colours, and the same three the waveform draws those
   * frequencies in.
   */
  test("each EQ knob wears its band's colour", async ({ page }) => {
    await openShell(page, "/");
    // A boost, so each knob has an arc: §114 fills from unity, and a knob at
    // rest has none.
    await emitDeck(page, { eq_low: 1.5, eq_mid: 1.5, eq_high: 1.5 });
    const bands = { eq_low: "--band-low", eq_mid: "--band-mid", eq_high: "--band-high" } as const;
    const seen: string[] = [];
    for (const [band, name] of Object.entries(bands)) {
      const strokes = await arcStroke(page, `.deck[data-deck="1"] .band[data-band="${band}"]`);
      const colour = await token(page, name);
      expect(strokes, `the ${band} knob does not wear ${name}`).toContain(colour);
      seen.push(colour);
    }
    expect(new Set(seen).size, "two bands share a colour").toBe(3);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The filter knob in the colour of the side of the record it keeps. */
  test("the filter knob wears the colour of what it leaves", async ({ page }) => {
    await openShell(page, "/");
    const filter = '.deck[data-deck="1"] .control[data-filter]';
    await emitDeck(page, { filter: -0.5 });
    await expect(deck(page).locator(".control[data-filter]")).toHaveAttribute("data-filter", "low-pass");
    expect(await arcStroke(page, filter)).toContain(await token(page, "--band-low"));
    await emitDeck(page, { filter: 0.5 });
    await expect(deck(page).locator(".control[data-filter]")).toHaveAttribute("data-filter", "high-pass");
    expect(await arcStroke(page, filter)).toContain(await token(page, "--band-high"));
  });

  /**
   * **A level's colour is where it is.** The gradient is the whole meter and
   * the cover shrinks from the right, so a quiet signal shows only the calm
   * start of it.
   */
  test("the level meter uncovers a gradient placed by loudness", async ({ page }) => {
    await openShell(page, "/");
    const meter = deck(page).locator(".meter");
    expect(await meter.evaluate((el) => getComputedStyle(el).backgroundImage)).toContain("linear-gradient");
    await emitDeck(page, { peak: 0.25 });
    await expect
      .poll(() => deck(page).locator("[data-meter-cover]").evaluate((el) => (el as HTMLElement).style.scale))
      .toBe("0.75 1");
  });

  /** And the three themes built on it are there to wear, from the switcher. */
  test("the functional themes are offered and wear their own ground", async ({ page }) => {
    await openShell(page, "/");
    for (const [name, ground] of [
      ["Spectrum", "rgb(13, 13, 15)"],
      ["Signal", "rgb(7, 11, 20)"],
      ["Aurora", "rgb(10, 11, 26)"],
    ]) {
      if (!(await page.locator(".switcher .menu").isVisible())) {
        await page.locator(".switcher button.icon").click();
      }
      await page
        .locator(".switcher .theme")
        .filter({ has: page.locator(".name", { hasText: new RegExp(`^${name}$`) }) })
        .click();
      await expect
        .poll(() => page.evaluate(() => getComputedStyle(document.body).backgroundColor))
        .toBe(ground);
    }
    expect(errorsThrown(page)).toEqual([]);
  });
});
