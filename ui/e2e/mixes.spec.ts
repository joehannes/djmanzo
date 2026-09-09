/**
 * The mixes tonight, as a surface.
 *
 * §67 says the session contains transitions; §68 asks for those transitions to
 * be explicit objects. `dj_app::mixes` derives them from the action log — that
 * derivation is tested in Rust, against a log it builds itself — so what is
 * left for a browser is whether a DJ can actually reach the answer and read it.
 *
 * Reached through the palette rather than a top-bar button, deliberately. The
 * handoff records that adding one destination button to that row wrapped it
 * onto another line and took forty pixels out of every deck at every window
 * height; a surface the palette already offers costs nothing.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/**
 * Open it the way a DJ does: the palette.
 *
 * The palette's ranking is Rust's, so the shared stub answers a fixed set that
 * proves each *kind* of entry runs the right way. Varying it here rather than
 * adding to it keeps that fixture about the palette — this is one answer
 * djmanzo really gives, for one query, which is exactly what the override is
 * for.
 */
async function openMixes(page: import("@playwright/test").Page) {
  await openShell(page, "/", {}, {
    palette: [
      {
        label: "Show Tonight's mixes",
        about: "How tonight's records were joined, and what kind of mix each was.",
        kind: "surface",
        run: "mixes",
      },
    ],
  });
  await page.keyboard.press("Control+k");
  await page.getByRole("button", { name: /Show Tonight's mixes/ }).first().click();
}

test.describe("tonight's mixes", () => {
  /**
   * **The night's mixes are on screen, newest first, as mixes.**
   *
   * Both records of each pair, not just the one going out: a mix is a pair,
   * and a panel showing half of it is answering a different question.
   */
  test("the panel lists what was mixed into what", async ({ page }) => {
    await openMixes(page);

    const panel = page.locator('[data-surface="mixes"]');
    await expect(panel).toBeVisible();

    const rows = panel.locator("li");
    await expect(rows).toHaveCount(2);

    // Newest first. The stub's later mix is the cut at 7:41.
    await expect(rows.first()).toContainText("cut");
    await expect(rows.last()).toContainText("blend");
    await expect(rows.last()).toContainText("Bachata Rosa");
    await expect(rows.last()).toContainText("Ojalá Que Llueva Café");
    await expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Beats where the record's tempo is known, seconds where it is not.**
   *
   * Not both, and not a zero. A mix whose record has left the library has an
   * unknown length in beats, and printing "0 beats" would be a confident wrong
   * answer about the one number a DJ would act on.
   */
  test("a length is in beats, or in seconds when there is no tempo", async ({
    page,
  }) => {
    await openMixes(page);
    const rows = page.locator('[data-surface="mixes"] li');

    await expect(rows.last()).toContainText("32 beats");
    await expect(rows.first()).toContainText("1 s");
    await expect(rows.first()).not.toContainText("beats");
    // And the deck it left is named even where the record it went to is not.
    await expect(rows.first()).toContainText("deck 1");
  });
});
