/**
 * Set Flow: a plan is a list of transitions, not a list of tracks.
 *
 * §20's third view asks for tracks "as a sequence", showing the energy curve,
 * the trajectories, the **transition links** and the **risk markers**. The
 * first two shipped with the set assembler; the second two did not, and their
 * absence is what made the panel a list of names with a line drawn over it.
 *
 * The thing a DJ reads a plan for is where it is going to be *difficult* —
 * which is a property of the seam between two records and of neither record on
 * its own. So the seam is drawn between the two rows it joins, and a seam that
 * needs a cut rather than a blend says so.
 *
 * The panel also moved. It was reachable only through the browser's folder
 * tree, three columns wide, which is not where a sequence is read. It is a
 * dockable surface now, along the bottom, which is the third time that move
 * has been the right one.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

async function planOpen(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Plan", exact: true }).click();
  await expect(page.locator('.surface[data-surface="plan"]')).toBeVisible();
}

test.describe("the set plan", () => {
  test("opens as a surface of its own, below the decks", async ({ page }) => {
    await planOpen(page);

    const plan = await page.locator('.surface[data-surface="plan"]').boundingBox();
    const deck = await page.locator(".deck").first().boundingBox();
    expect(
      plan!.y,
      "the plan is not below the decks. A sequence is read across, and a side " +
        "dock is 360 px wide",
    ).toBeGreaterThan(deck!.y + deck!.height - 1);
    expect(errorsThrown(page), "the plan threw while rendering").toEqual([]);
  });

  /** The browser no longer carries a second way in. */
  test("the browser's folder tree no longer offers it", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();

    await expect(
      page.locator('.surface[data-surface="library"]').getByRole("button", { name: "Plan", exact: true }),
      "the crate tree still has a Plan entry. The panel moved; it did not get copied",
    ).toHaveCount(0);
  });

  /**
   * **The seams, which are what a plan is made of.**
   *
   * Two records and one join between them, not three independent rows. A
   * three-record plan therefore draws two seams.
   */
  test("a seam is drawn between each pair of records", async ({ page }) => {
    await planOpen(page);
    await page.getByRole("button", { name: "Build the set" }).click();

    await expect(page.locator(".slots li:not(.seam)")).toHaveCount(3);
    await expect(
      page.locator(".slots li.seam"),
      "three records make two joins; the plan drew a different number",
    ).toHaveCount(2);
    await expect(page.locator(".slots li.seam").first()).toContainText("+3 BPM · 8A→9A");
  });

  /**
   * **A difficult seam is marked, and counted before it is read.**
   *
   * Twenty-five rows is more than anyone scans before deciding whether to keep
   * a plan, so the count goes at the top: two difficult joins is a plan to
   * play, eleven is a plan to rebuild.
   */
  test("a seam that needs a cut says so, and the count is at the top", async ({ page }) => {
    await planOpen(page);
    await page.getByRole("button", { name: "Build the set" }).click();

    const risky = page.locator(".slots li.seam.risky");
    await expect(risky, "the merengue-into-techno seam was not marked").toHaveCount(1);
    await expect(risky).toContainText("needs a cut");
    await expect(
      page.locator(".totals"),
      "the number of difficult joins is not stated above the list, so it can " +
        "only be found by reading every row",
    ).toContainText("1 seam needs a cut");
  });
});
