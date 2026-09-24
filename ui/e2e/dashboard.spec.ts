/**
 * §117: the dashboard, instead of toolbars.
 *
 * > minimalise the use of toolbars/space ... so i can use every space for my
 * > focused activity widget ... instead of toolbards let there be an central
 * > dashboard view with easy nav buttons for packages/presets/activities/
 * > overview ... even different dashboards per purpose/activity
 *
 * What is on the dashboard and in which order is Rust's
 * (`dj_app::dashboard::build`, arranged by use and tested there), delivered as
 * `dashboards.json`. What the browser holds: with the toolbars off the top of
 * the window is one slim row and the decks start higher; `0` calls the
 * dashboard up and puts it away; a tile does what its key does; an activity
 * has its own dashboard; and the toolbars come back from Settings.
 */
import { expect, test, type Page } from "@playwright/test";

import boards from "./dashboards.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const SLIM = { interface_settings: { toolbars: false, uses: {} } };
const board = (page: Page) => page.locator("[data-dashboard]");
const surface = (page: Page, name: string) => page.locator(`.surface[data-surface="${name}"]`);

/** Where the top bar ends, which is where the decks begin. */
async function topbarBottom(page: Page): Promise<number> {
  const box = await page.locator(".topbar").boundingBox();
  return (box?.y ?? 0) + (box?.height ?? 0);
}

test.describe("§117: the dashboard", () => {
  /**
   * **The load-bearing one: with the toolbars off, the decks get the room.**
   * The panel buttons and the stage pickers are gone from the top, what is
   * read from across a booth stays (REC, Mark, SAFE), and the decks start at
   * least a row higher than they do under the toolbars.
   */
  test("without the toolbars the top is one slim row and the decks start higher", async ({ page }) => {
    await openShell(page, "/");
    const full = await topbarBottom(page);

    await openShell(page, "/", {}, SLIM);
    await expect(page.getByRole("navigation", { name: "Panels" })).toHaveCount(0);
    await expect(page.getByRole("group", { name: "Stage" })).toHaveCount(0);
    await expect(page.getByRole("navigation", { name: "Where you are" })).toBeVisible();
    await expect(page.getByRole("group", { name: "This set" })).toBeVisible();
    const slim = await topbarBottom(page);
    expect(full - slim, `the toolbars' room was not given back (${full} → ${slim})`).toBeGreaterThanOrEqual(30);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * `0` calls the dashboard up with every section Rust built, in order; a
   * tile runs what its key would — Library opens the library — and the
   * dashboard goes; `0` again, and Escape, put it away.
   */
  test("0 calls it up, a tile does what its key does, and 0 or Escape put it away", async ({ page }) => {
    await openShell(page, "/", {}, SLIM);
    await page.keyboard.press("0");
    await expect(board(page)).toBeVisible();
    await expect(board(page).locator("section h3")).toHaveText(boards[""].sections.map((s) => s.title));

    await board(page).locator('[data-tile="surface:library"]').click();
    await expect(board(page)).toHaveCount(0);
    await expect(surface(page, "library")).toBeVisible();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __usedTiles?: string[] }).__usedTiles ?? []))
      .toContain("surface:library");

    await page.keyboard.press("0");
    await expect(board(page)).toBeVisible();
    await page.keyboard.press("0");
    await expect(board(page)).toHaveCount(0);
    await page.keyboard.press("0");
    await page.keyboard.press("Escape");
    await expect(board(page)).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A dashboard per activity**: in Karaoke it leads with what Karaoke is
   * for, and the slim row says where the DJ is, with its digit.
   */
  test("an activity has its own dashboard, and the slim row says which", async ({ page }) => {
    await openShell(page, "/", {}, SLIM);
    await page.keyboard.press("8");
    await expect(page.locator("[data-where]")).toContainText("8");
    await expect(page.locator("[data-where]")).toContainText("Karaoke");
    await page.keyboard.press("0");
    await expect(board(page).locator("section h3").first()).toHaveText("For Karaoke");
    for (const tile of boards.karaoke.sections[0].tiles) {
      await expect(board(page).locator("section").first().locator(`[data-tile="${tile.id}"]`)).toBeVisible();
    }
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The toolbars come back from Settings, reached from the dashboard. */
  test("the toolbars come back from Settings", async ({ page }) => {
    await openShell(page, "/", {}, SLIM);
    await page.getByRole("button", { name: /Dashboard/ }).click();
    await board(page).locator('[data-tile="surface:settings"]').click();
    await page.getByRole("checkbox", { name: /Show the toolbars/ }).check();
    await expect(page.getByRole("navigation", { name: "Panels" })).toBeVisible();
    await expect(page.getByRole("navigation", { name: "Where you are" })).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});
