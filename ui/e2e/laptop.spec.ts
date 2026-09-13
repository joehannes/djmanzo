/**
 * §48's laptop mode, and the sentence it ends on.
 *
 * > The interface must explicitly prioritize: AUDIO > CONTROL > VISUAL EFFECTS.
 * > Never the reverse.
 *
 * The priority itself is `dj_app::thrift` and is held there by three tests:
 * nothing in the audio band is ever given up, nothing cheaper is kept while
 * something dearer goes, and a harder-pressed machine never pays for more. This
 * says the thing only a browser can — that a DJ can see it.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

async function openPerformance(page: Page) {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.locator(".spend-list")).toBeVisible();
}

test.describe("§48's laptop mode", () => {
  /**
   * **The load-bearing one: the sound is on the list, and it is never given
   * up.**
   *
   * §48's priority had no expression anywhere: the frame rate has been measured
   * for a long time and what consulted it was the theme pipeline alone, so a DJ
   * saw a tier name and nothing about what it cost them. The failure the
   * section forbids is the one that ends a set — a laptop under load that keeps
   * its glow and drops its audio — and the only way a DJ can trust it is not
   * happening is to be able to look.
   */
  test("the audio is named as the thing that never goes", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await openPerformance(page);

    const rows = page.locator(".spend-list li");
    await expect(rows).toHaveCount(7);

    // The audio row is there, is marked kept, and says why rather than just
    // claiming it. A row that only asserted would be the least believable line
    // on the page.
    const audio = page.locator('.spend-list li[data-spend="audio"]');
    await expect(audio).toHaveCount(1);
    await expect(audio).toHaveClass(/kept/);
    await expect(audio).toContainText("never allocates");

    // And the fixture is Eco, so something has actually gone — otherwise this
    // test would pass against a list that gives nothing up at all.
    await expect(rows.filter({ hasNotText: "Kept —" })).toHaveCount(4);
    expect(thrown).toEqual([]);
  });

  /**
   * **A row djmanzo does not give up says why, including the two that are
   * decisions.**
   *
   * Three of §48's seven are not savings: one is the audio claim, and two are
   * collisions with other sections that had to be decided somewhere. §25's
   * layers are a DJ's choice that §79 lets them lock, so djmanzo drops the
   * answering rather than the layers; and there are no expensive previews to
   * stop drawing. A list that left those out would read as §48 being done.
   */
  test("the rows it keeps carry their reason", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await openPerformance(page);

    const kept = page.locator(".spend-list li.kept");
    await expect(kept).toHaveCount(3);
    for (const row of await kept.all()) {
      await expect(row.locator(".spend-kept")).not.toBeEmpty();
    }
    await expect(kept.filter({ hasText: "visual layers" })).toContainText("§79");
    expect(thrown).toEqual([]);
  });
});
