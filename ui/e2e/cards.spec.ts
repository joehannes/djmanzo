/**
 * §20's second view: the collection as cards.
 *
 * "Use when album/artwork is valuable. **Each card should still be
 * operational.**" The second half is what this file is mostly about — a card
 * a DJ can look at but not act on is a worse table, and the operations are the
 * part a browser can actually check.
 *
 * The artwork itself cannot be checked here: `art://` is a Tauri scheme that
 * only exists inside the application, so every cover 404s in this harness and
 * every card falls back. That fallback is worth asserting for its own sake —
 * a hand-organised collection is mostly untagged files, so it is what most
 * cards will draw in a real booth too.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

async function openCards(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Browse", exact: true }).click();
  await page.getByRole("button", { name: "Cards", exact: true }).click();
}

test.describe("the card view", () => {
  /**
   * **A card per record, each carrying its operations.**
   *
   * Every card, not the hovered one: a booth is dark and a trackpad is small,
   * and a control that appears only when the pointer is already on it is a
   * control nobody finds.
   */
  test("every card is operational", async ({ page }) => {
    await openCards(page);

    const cards = page.locator(".cards .card");
    await expect(cards).toHaveCount(2);

    const first = cards.first();
    await expect(first).toContainText("Bachata Rosa");
    await expect(first).toContainText("Juan Luis Guerra");
    await expect(first).toContainText("124 BPM");
    await expect(first).toContainText("8A");

    // Set aside, more like this, favourite, and a button per deck.
    await expect(first.getByRole("button", { name: /Set aside/ })).toBeVisible();
    await expect(first.getByRole("button", { name: /Find records like/ })).toBeVisible();
    await expect(first.getByRole("button", { name: /Favourite/ })).toBeVisible();
    // Named for a screen reader, not "1": the visible text is a bare number,
    // which says nothing without the card around it.
    await expect(
      first.getByRole("button", { name: "Load Bachata Rosa onto deck 1" }),
    ).toBeVisible();
    await expect(
      first.getByRole("button", { name: "Load Bachata Rosa onto deck 2" }),
    ).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A record with no cover still says something about itself.**
   *
   * Not a placeholder icon repeated down the grid, which is a wall of one
   * shape. The key and the tempo are what a DJ would have read off the sleeve
   * anyway, and they differ per record — which is the whole job of a cover at
   * a glance.
   */
  test("a card with no artwork falls back to what djmanzo knows", async ({
    page,
  }) => {
    await openCards(page);
    const sleeves = page.locator(".cards .sleeve");

    await expect(sleeves.first()).toContainText("8A");
    await expect(sleeves.first()).toContainText("124");
    await expect(sleeves.nth(1)).toContainText("11B");
    await expect(sleeves.nth(1)).toContainText("138");
  });

  /**
   * **The star shows the rating djmanzo already holds.**
   *
   * §20 asks for "favorite" and djmanzo spells it as a five-star rating. A
   * separate flag would be a second opinion about one question — so the test
   * that matters is that the card reads the rating rather than a state of its
   * own.
   */
  test("a favourite is the rating, and is pressed when it is one", async ({
    page,
  }) => {
    await openCards(page);
    const cards = page.locator(".cards .card");

    await expect(
      cards.first().getByRole("button", { name: /Favourite/ }),
    ).toHaveAttribute("aria-pressed", "false");
    await expect(
      cards.nth(1).getByRole("button", { name: /Favourite/ }),
    ).toHaveAttribute("aria-pressed", "true");
  });

  /**
   * **Switching views does not change which records are shown.**
   *
   * §20 asks for "several representations of the same underlying collection".
   * A card grid that showed a different set from the table beside it would be
   * a second browser wearing the first one's name.
   */
  test("the cards are the same records as the table", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();

    const inTable = await page.locator("tbody tr td.title").allTextContents();
    await page.getByRole("button", { name: "Cards", exact: true }).click();
    const onCards = await page.locator(".cards .card .title").allTextContents();

    expect(onCards).toEqual(inTable);
    expect(onCards.length).toBeGreaterThan(0);
  });
});
