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

  /**
   * **Hearing one back says where it went.**
   *
   * §68's object driving replay. What a browser can prove is the round trip:
   * the two numbers the panel holds reach Rust, and what comes back is put in
   * front of the DJ rather than swallowed — a render whose file nobody is told
   * about is a button that appears to do nothing.
   *
   * The arithmetic is Rust's and is tested there, against a set it builds
   * itself.
   */
  test("a mix can be heard again, and the file is named", async ({ page }) => {
    await openMixes(page);
    const row = page.locator('[data-surface="mixes"] li').last();

    await row.getByRole("button", { name: "hear it again" }).click();
    await expect(row.locator(".said")).toContainText("mix-at-214s.wav");

    // The two numbers that reached Rust are the mix's own, not the row's
    // position or a default.
    const asked = await page.evaluate(
      () => (window as unknown as { __renderMixArgs?: unknown }).__renderMixArgs,
    );
    expect(asked).toEqual({ at: 214, tookSeconds: 15.5 });
  });
});
