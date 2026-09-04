/**
 * The pair view, and the object it draws.
 *
 * §20's fourth view asks for two records side by side with the seam between
 * them; §68 asks for the transition to be an explicit object rather than an
 * answer that lives for one call. The two arrive together, because an object
 * nothing draws is a promise and a view with nothing behind it is a table.
 *
 * What is measured here is the part a type-check cannot see: that the panel
 * opens, that both records and the seam are actually on screen, that the
 * adjustments are refused until a transition is being held, and that pressing
 * one changes what is drawn. The arithmetic behind the adjustment is Rust's
 * and is tested there -- see `dj_app::transition`. What a browser can prove is
 * that the press reaches it and the answer comes back.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Open the shell with the pair view docked. */
async function pairOpen(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Pair", exact: true }).click();
  await expect(page.locator('.surface[data-surface="pair"]')).toBeVisible();
}

const PAIR = '.surface[data-surface="pair"]';

test.describe("the pair view", () => {
  /**
   * Nothing is held when djmanzo starts, and the panel says so in words that
   * tell a DJ what to do about it. An empty panel that merely looks broken is
   * the failure this asserts against.
   */
  test("opens saying what it needs, rather than empty", async ({ page }) => {
    await pairOpen(page);

    await expect(page.locator(`${PAIR} .empty`)).toContainText("Compare");
    expect(errorsThrown(page), "the pair view threw while rendering").toEqual([]);
  });

  /**
   * **Both records and the seam, which is the whole view.**
   *
   * A pair view that draws the two records and not what happens between them
   * is the browser at two rows. The deltas line is the thing a DJ reads first:
   * what the tempo does, what the keys do, and how well the two go together.
   */
  test("draws both records and the seam between them", async ({ page }) => {
    await pairOpen(page);
    await page.getByRole("button", { name: "Compare", exact: true }).click();

    await expect(page.locator(`${PAIR} .side`)).toHaveCount(2);
    await expect(page.locator(`${PAIR} .side h3`).first()).toHaveText("Bachata Rosa");
    await expect(page.locator(`${PAIR} .side h3`).nth(1)).toContainText("Ojal");

    const deltas = page.locator(`${PAIR} .deltas`);
    await expect(deltas).toContainText("BPM");
    await expect(
      deltas,
      "the seam does not say what the keys do, which is half of what a DJ is " +
        "comparing two records for",
    ).toContainText("8A → 9A");
    await expect(page.locator(`${PAIR} .when`)).toContainText("beats");
    await expect(
      page.locator(`${PAIR} .why li`).first(),
      "the transition does not say why it is where it is",
    ).toContainText("phrase start");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **An opinion is not a held transition, and the panel does not pretend.**
   *
   * Comparing plans without holding anything, so there is nothing to adjust
   * and the controls say so by being unavailable. A panel offering buttons
   * that quietly do nothing is how a DJ learns to distrust the whole surface.
   */
  test("the adjustments wait until a transition is being held", async ({ page }) => {
    await pairOpen(page);
    await page.getByRole("button", { name: "Compare", exact: true }).click();

    const move = page.getByRole("group", { name: "Move the mix point" });
    await expect(move.getByRole("button", { name: "+4" })).toBeDisabled();
    await expect(page.locator(`${PAIR} .hint`)).toContainText("Set it up");

    await page.getByRole("button", { name: "Set up", exact: true }).click();
    await expect(move.getByRole("button", { name: "+4" })).toBeEnabled();
    await expect(page.locator(`${PAIR} .hint`)).toHaveCount(0);
  });

  /**
   * **Pressing a length changes the transition that is drawn.**
   *
   * The round trip: the press goes to djmanzo, the answer comes back, and the
   * panel draws the answer rather than what it hoped for. A view that changed
   * its own state and never asked would look identical until the moment two
   * surfaces disagreed about the same mix.
   */
  test("shortening the mix redraws it from djmanzo's answer", async ({ page }) => {
    await pairOpen(page);
    await page.getByRole("button", { name: "Set up", exact: true }).click();
    await expect(page.locator(`${PAIR} .when`)).toContainText("over 32 beats");

    await page
      .getByRole("group", { name: "How long the mix runs" })
      .getByRole("button", { name: "8", exact: true })
      .click();

    await expect(page.locator(`${PAIR} .when`)).toContainText("over 8 beats");
    await expect(
      page.locator(`${PAIR} .edited`),
      "an adjusted transition does not say it has been adjusted, so a DJ " +
        "cannot tell djmanzo's proposal from their own change to it",
    ).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });
});
