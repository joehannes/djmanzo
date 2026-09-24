/**
 * §114: controls shaped like what they do.
 *
 * > i'd also like to see the forms and shapes of the controls and widgets to
 * > be individual and useful and resembling nature and functionality
 *
 * The arithmetic is `src/controls/faces.test.ts`'s. What a browser holds is
 * that it reaches the deck: the tone knobs draw their curves at the deck's
 * settings, the controls that rest in the middle fill from it, the transport
 * pads light in what they do, and the default theme's knobs are stones.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const deck = (page: Page) => page.locator('.deck[data-deck="1"]');

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

/** Every path's `d` inside a control. */
const paths = (page: Page, selector: string) =>
  page.locator(selector).evaluate((el) =>
    [...el.querySelectorAll("svg path")].map((p) => ({
      d: p.getAttribute("d") ?? "",
      stroke: p.getAttribute("stroke") ?? "",
      fill: p.getAttribute("fill") ?? "",
    })),
  );

/** The face: the one value path drawn through 33 points. */
function faceHeights(all: { d: string }[]): number[] {
  const face = all.find((p) => (p.d.match(/L /g) ?? []).length === 32);
  if (!face) throw new Error("no face on this knob");
  return [...face.d.matchAll(/[ML] [\d.]+ ([\d.]+)/g)].map((m) => Number(m[1]));
}

/** How many filled arcs (other than the track) a knob has. */
const fills = (all: { d: string; stroke: string }[]) =>
  all.filter((p) => p.d.includes(" A ") && p.stroke === "var(--knob-value)").length;

test.describe("§114: shapes that say what a control does", () => {
  /**
   * **The load-bearing one: a tone knob draws what it is doing to the
   * record.** The LOW knob killed is a floor on the left of its face and
   * level on the right; boosted it rises on the left; the HI knob's shelf is
   * on the right.
   */
  test("each EQ knob's face is its curve at the deck's setting", async ({ page }) => {
    await openShell(page, "/");
    const low = '.deck[data-deck="1"] .band[data-band="eq_low"]';
    const high = '.deck[data-deck="1"] .band[data-band="eq_high"]';

    await emitDeck(page, { eq_low: 0, eq_high: 1 });
    await expect.poll(async () => faceHeights(await paths(page, low))[0]).toBeGreaterThan(58);
    let heights = faceHeights(await paths(page, low));
    expect(heights.at(-1)).toBeCloseTo(50, 0);
    // At unity the HI knob is flat, and shows no arc.
    const flat = faceHeights(await paths(page, high));
    expect(Math.max(...flat) - Math.min(...flat)).toBeLessThan(0.01);
    expect(fills(await paths(page, high))).toBe(0);

    await emitDeck(page, { eq_low: 4, eq_high: 0 });
    await expect.poll(async () => faceHeights(await paths(page, low))[0]).toBeLessThan(45);
    heights = faceHeights(await paths(page, high));
    expect(heights.at(-1)).toBeGreaterThan(58);
    expect(heights[0]).toBeCloseTo(50, 0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The filter is off at its centre, and its face falls on the side it cuts. */
  test("the filter fills from its centre and draws its slope", async ({ page }) => {
    await openShell(page, "/");
    const filter = '.deck[data-deck="1"] .control[data-filter]';
    await emitDeck(page, { filter: 0 });
    await expect.poll(async () => fills(await paths(page, filter))).toBe(0);

    await emitDeck(page, { filter: -0.6 });
    await expect(deck(page).locator(".control[data-filter]")).toHaveAttribute("data-filter", "low-pass");
    expect(fills(await paths(page, filter))).toBe(1);
    const lowPass = faceHeights(await paths(page, filter));
    expect(lowPass.at(-1)!).toBeGreaterThan(lowPass[0] + 5);

    await emitDeck(page, { filter: 0.6 });
    await expect(deck(page).locator(".control[data-filter]")).toHaveAttribute("data-filter", "high-pass");
    const highPass = faceHeights(await paths(page, filter));
    expect(highPass[0]).toBeGreaterThan(highPass.at(-1)! + 5);
  });

  /** The pitch fader's zero is its middle: untouched, nothing is filled. */
  test("the pitch fader fills from zero", async ({ page }) => {
    await openShell(page, "/");
    const pitch = deck(page).locator('.fader-wrap:has([aria-label="Pitch"])');
    const fill = () =>
      pitch.evaluate((el) =>
        [...el.querySelectorAll("svg path")]
          .find((p) => p.getAttribute("fill") === "var(--accent-2)")
          ?.getAttribute("d"),
      );
    await emitDeck(page, { pitch: 0 });
    await expect.poll(fill).toBe("M 40 50.00 H 60 V 50.00 H 40 Z");
    await emitDeck(page, { pitch: 0.08 });
    await expect.poll(fill).toBe("M 40 30.00 H 60 V 50.00 H 40 Z");
    await emitDeck(page, { pitch: -0.08 });
    await expect.poll(fill).toBe("M 40 50.00 H 60 V 70.00 H 40 Z");
  });

  /** PLAY lights in go when playing; EJECT is ringed in stop at rest. */
  test("the transport pads light in what they do", async ({ page }) => {
    await openShell(page, "/");
    const play = deck(page).locator('.transport [data-does="play"]');
    const eject = deck(page).locator('.transport [data-does="eject"]');
    const last = (loc: typeof play) =>
      loc.evaluate((el) => {
        const all = [...el.querySelectorAll("svg path")];
        const top = all[all.length - 1];
        return { fill: top.getAttribute("fill"), stroke: top.getAttribute("stroke"), count: all.length };
      });

    await emitDeck(page, { loaded: true, playing: false });
    await expect.poll(async () => (await last(play)).count).toBe(1);
    await emitDeck(page, { loaded: true, playing: true });
    await expect.poll(async () => (await last(play)).fill).toBe("var(--success)");
    expect((await last(eject)).stroke).toBe("var(--danger)");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Nature where the theme is a natural one.** The default theme is an
   * organic one, and its knobs are stones — each its own, the value still on
   * a circle — while Booth, a plain one, keeps the circle.
   */
  test("the organic theme's knobs are stones, each its own", async ({ page }) => {
    await openShell(page, "/");
    const body = (band: string) =>
      page
        .locator(`.deck[data-deck="1"] .band[data-band="${band}"] svg path`)
        .first()
        .getAttribute("d");
    const low = await body("eq_low");
    const mid = await body("eq_mid");
    // A stone is drawn through many points and closed; a circle is an arc.
    expect((low!.match(/L /g) ?? []).length).toBeGreaterThan(40);
    expect(low).not.toBe(mid);
    // The same stone on the other deck: it is the knob's, not the deck's.
    const other = await page
      .locator('.deck[data-deck="2"] .band[data-band="eq_low"] svg path')
      .first()
      .getAttribute("d");
    expect(other).toBe(low);

    if (!(await page.locator(".switcher .menu").isVisible())) {
      await page.locator(".switcher button.icon").click();
    }
    await page
      .locator(".switcher .theme")
      .filter({ has: page.locator(".name", { hasText: /^Booth$/ }) })
      .click();
    await expect.poll(() => body("eq_low")).toContain(" A ");
    expect(errorsThrown(page)).toEqual([]);
  });
});
