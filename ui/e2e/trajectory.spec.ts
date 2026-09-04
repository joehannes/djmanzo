/**
 * §25's energy trajectory, on the overview.
 *
 * The section asks for the energy trajectory as a *curve* rather than as two
 * markers. The two markers — the breakdown band and the drop tick — are
 * thresholds over exactly this measurement, so drawing the line they came from
 * makes them legible as a consequence rather than as an assertion.
 *
 * On the overview and not the scrolling lane, deliberately: at the lane's zoom
 * about ten beats are visible, over which a trajectory is a straight line. The
 * arc of a record is a thing you see whole or not at all.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const OVERVIEW = ".deck.playing .overview";

test.describe("the energy trajectory", () => {
  test("is drawn as one line with a point per beat", async ({ page }) => {
    await openShell(page, "/");

    const points = await page.evaluate(() => {
      const line = document.querySelector(".deck.playing .overview .drive polyline");
      return line?.getAttribute("points")?.trim().split(/\s+/).length ?? 0;
    });
    // The fixture's sixteen levels, all of them.
    expect(points).toBe(16);
  });

  /**
   * The whole claim of a *trajectory*: the quiet beats sit lower than the loud
   * ones. A curve drawn flat, or upside down, is a decoration.
   */
  test("its quiet beats are drawn below its loud ones", async ({ page }) => {
    await openShell(page, "/");

    const ys = await page.evaluate(() => {
      const line = document.querySelector(".deck.playing .overview .drive polyline");
      return (line?.getAttribute("points") ?? "")
        .trim()
        .split(/\s+/)
        .map((pair) => Number(pair.split(",")[1]));
    });
    expect(ys.length).toBe(16);

    // The fixture's beats 5..10 are the breakdown. In SVG the y axis points
    // down, so quieter is a *larger* y.
    const loud = Math.max(ys[0], ys[1], ys[12], ys[13]);
    const quiet = Math.min(ys[6], ys[7], ys[8]);
    expect(quiet, "the breakdown was drawn as high as the drums").toBeGreaterThan(loud + 20);
  });

  /**
   * It has to be over the whole width the marks use, or the curve and the band
   * it was thresholded from would be talking about different places.
   */
  test("it spans the overview in the same coordinates as the marks", async ({
    page,
  }) => {
    await openShell(page, "/");

    const box = await page.evaluate(() => {
      const overview = document.querySelector(".deck.playing .overview");
      const svg = overview?.querySelector(".drive");
      if (!overview || !svg) return null;
      const o = overview.getBoundingClientRect();
      const s = svg.getBoundingClientRect();
      return { same: Math.abs(o.x - s.x) < 2 && Math.abs(o.width - s.width) < 2 };
    });
    expect(box?.same).toBe(true);
  });

  test("drawing it throws nothing", async ({ page }) => {
    await openShell(page, "/");
    await expect(page.locator(`${OVERVIEW} .drive`)).toHaveCount(1);
    expect(errorsThrown(page)).toEqual([]);
  });
});
