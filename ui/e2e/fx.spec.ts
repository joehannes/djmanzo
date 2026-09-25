/**
 * §120: the effects rack as three units side by side, with knobs.
 *
 * > the rest of the faders/knobs/controls seem to take a lot of space ... fx
 * > also and design seems poor/redundant ... please improve
 *
 * Each slot was a dropdown beside a slider the width of the deck, and a loaded
 * slot grew a second slider and a row of beat lengths under it: three effects
 * took nine rows. Now each slot is a unit — switch and effect on top, wet and
 * the effect's own control as knobs, the lengths under them — and the three sit
 * in one row across a deck.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const dispatched = (page: Page) =>
  page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []);

const ECHO = {
  slot: 1,
  kind: "echo",
  enabled: true,
  wet: 0.4,
  beats: 0.5,
  amount: 0.5,
  amount_label: "feedback",
  timed: true,
  post_fader: false,
};

/** Deliver a frame with deck 1's rack as given, as the engine would. */
async function rack(page: Page, slots: unknown[]) {
  await page.evaluate((slots) => {
    const win = window as unknown as {
      __lastState: { decks: { fx: unknown[] }[] };
      __emit: (next: unknown) => void;
    };
    const next = structuredClone(win.__lastState);
    next.decks[0].fx = slots;
    win.__emit(next);
  }, slots);
}

const deck = (page: Page) => page.locator("section.deck[data-deck]").first();

test.describe("§120: the effects rack", () => {
  /**
   * **The load-bearing one: three loaded effects in one row, each with its
   * own knobs.** The units' tops line up, and each holds its wet and its
   * effect's control.
   */
  test("three effects sit side by side as units with knobs", async ({ page }) => {
    await openShell(page, "/");
    await rack(page, [
      ECHO,
      { ...ECHO, slot: 2, kind: "gate", amount_label: "duty" },
      { ...ECHO, slot: 3, kind: "crush", amount_label: "grit", timed: false },
    ]);
    const units = deck(page).locator("[data-fx-slot]");
    await expect(units).toHaveCount(3);
    const tops = await units.evaluateAll((els) => els.map((el) => Math.round(el.getBoundingClientRect().top)));
    expect(new Set(tops).size, `the units are on different rows: ${tops.join(", ")}`).toBe(1);
    await expect(units.nth(0).getByRole("slider", { name: "Slot 1 wet" })).toBeVisible();
    await expect(units.nth(0).getByRole("slider", { name: "Slot 1 feedback" })).toBeVisible();
    await expect(units.nth(2).getByRole("slider", { name: "Slot 3 grit" })).toBeVisible();
    // An effect with no time in it offers no lengths.
    await expect(units.nth(2).locator(".lengths")).toHaveCount(0);
    // And an effect's lengths stay inside its unit rather than running out
    // past its edge (the first version's did, seen in WebKitGTK).
    const inside = await units.nth(0).evaluate((unit) => {
      const edge = unit.getBoundingClientRect().right;
      return [...unit.querySelectorAll(".lengths button")].every(
        (b) => b.getBoundingClientRect().right <= edge + 0.5,
      );
    });
    expect(inside, "a beat length runs past the unit's edge").toBe(true);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A small drag on wet is a small change, sent as the rack's own action. */
  test("wet turns gradually", async ({ page }) => {
    await openShell(page, "/");
    await rack(page, [ECHO, { ...ECHO, slot: 2, kind: "none", enabled: false }, { ...ECHO, slot: 3, kind: "none", enabled: false }]);
    const knob = deck(page).getByRole("slider", { name: "Slot 1 wet" });
    // The rack is below the fold of the deck's scrolling body, as it is for
    // a DJ: scroll to it, then turn it.
    await knob.scrollIntoViewIfNeeded();
    const box = await knob.boundingBox();
    if (!box) throw new Error("the wet knob is not drawn");
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;
    const before = (await dispatched(page)).length;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x, y - 10, { steps: 4 });
    await page.mouse.up();
    const sent = (await dispatched(page)).slice(before);
    const wets = sent.filter((a) => a.startsWith("deck 1 fx 1 wet ")).map((a) => Number(a.split(" ")[5]));
    expect(wets.length).toBeGreaterThan(0);
    expect(wets[wets.length - 1]).toBeCloseTo(0.5, 1);
    expect(sent.filter((a) => !a.startsWith("deck 1 fx 1 wet "))).toEqual([]);
  });
});
