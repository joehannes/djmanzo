/**
 * §17 at the peak, as the owner chose (17 October): when djmanzo reads the
 * night as its peak, between mixes, each deck's effects fold opens and the
 * stem chips grow a little (`peak.svelte.ts`). Never during a mix, never on
 * a peak nothing agreed with, never closing anything, and a fold the DJ
 * closes stays closed for the night.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** The night as djmanzo reads it, with `playing` decks audible. */
async function night(page: Page, phase: string | null, certainty: string, playing: number[]) {
  await page.evaluate(
    ({ phase, certainty, playing }) => {
      const win = window as unknown as {
        __lastState?: { decks: Record<string, unknown>[]; context: Record<string, unknown> };
        __emit?: (next: unknown) => void;
      };
      const state = win.__lastState;
      if (!state) throw new Error("no state");
      const decks = state.decks.map((deck, i) => ({ ...deck, playing: playing.includes(i + 1), volume: 1 }));
      const session = phase
        ? {
            phase,
            energy: 0.9,
            environment: { time_of_day: "night", indoors: true },
            certainty,
            basis: "measured",
            drift: null,
          }
        : null;
      win.__emit?.({ ...state, decks, context: { ...state.context, session } });
    },
    { phase, certainty, playing },
  );
}

const fold = (page: Page, deck: number) => page.locator(`section.deck[data-deck="${deck}"] details.rack-fold`);
const chipHeight = (page: Page) =>
  page.locator('section.deck[data-deck="1"] .stem-chip').first().evaluate((el) => el.getBoundingClientRect().height);

test.describe("§17: stems and effects at the peak", () => {
  test("open between mixes at a peak sure enough, and stay as the DJ leaves them", async ({ page }) => {
    test.setTimeout(90_000);
    await openShell(page, "/");
    await page.setViewportSize({ width: 1400, height: 1000 });
    await expect(fold(page, 1)).not.toHaveAttribute("open", /.*/);
    const resting = await chipHeight(page);

    // A peak nothing agreed with: nothing moves.
    await night(page, "peak", "unsure", [1]);
    await page.waitForTimeout(300);
    await expect(fold(page, 1)).not.toHaveAttribute("open", /.*/);

    // A peak, but mid-mix: still nothing.
    await night(page, "peak", "fair", [1, 2]);
    await page.waitForTimeout(300);
    await expect(fold(page, 1)).not.toHaveAttribute("open", /.*/);

    // The mix over: the folds open, the chips grow.
    await night(page, "peak", "fair", [2]);
    await expect(fold(page, 1)).toHaveAttribute("open", "");
    await expect(fold(page, 2)).toHaveAttribute("open", "");
    await expect.poll(() => chipHeight(page)).toBeGreaterThan(resting + 2);

    // The DJ closes deck 2's: it stays closed for the night.
    await fold(page, 2).locator("summary").click();
    await expect(fold(page, 2)).not.toHaveAttribute("open", /.*/);
    await night(page, "peak", "sure", [1]);
    await page.waitForTimeout(300);
    await expect(fold(page, 2)).not.toHaveAttribute("open", /.*/);
    // Even once the deck is drawn again: made slim, then full size.
    const deck2 = page.locator('section.deck[data-deck="2"]');
    // (The header's controls show on hover; dispatched, they run their own handlers.)
    await deck2.locator('button[aria-label="Make deck 2 slim"]').dispatchEvent("click");
    await expect(deck2.locator(".deck-body")).toHaveCount(0);
    await deck2.locator('button:has-text("Full size")').dispatchEvent("click");
    await expect(fold(page, 2)).toHaveCount(1);
    await expect(fold(page, 2)).not.toHaveAttribute("open", /.*/);
    await expect(fold(page, 1)).toHaveAttribute("open", "");

    // The peak eases: the chips go back, and nothing is closed by djmanzo.
    await night(page, "cooldown", "fair", [1]);
    await expect.poll(() => chipHeight(page)).toBeLessThan(resting + 1);
    await expect(fold(page, 1)).toHaveAttribute("open", "");
    expect(errorsThrown(page)).toEqual([]);
  });
});
