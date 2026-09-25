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
   * **§121: the bar wraps only when it has to.** The device group held 26rem
   * while it showed nothing but the device's name, so the rest of the bar
   * went to a second line past an empty gap -- and a composition's density
   * tipped it one way or the other, which read as a bar that changed for no
   * reason. Closed, the group is as wide as what it shows; open (the device
   * pickers, in Settings), it takes the room they need.
   */
  test("the device group is as wide as what it shows, and the bar is one row when that fits", async ({ page }) => {
    await openShell(page, "/", {}, SLIM);
    const group = page.locator(".topbar .device");
    const spare = () =>
      group.evaluate((el) => {
        const kids = [...el.children].map((kid) => kid.getBoundingClientRect());
        const used = kids.reduce((sum, box) => sum + box.width, 0);
        const gap = parseFloat(getComputedStyle(el).columnGap) || 0;
        return el.getBoundingClientRect().width - used - gap * Math.max(0, kids.length - 1);
      });
    expect(await spare(), "the closed device group reserves room it does not use").toBeLessThan(2);
    const tops = await page.evaluate(() =>
      [".brand", ".device", ".status", ".go"].map((s) =>
        Math.round(document.querySelector(`.topbar ${s}`)!.getBoundingClientRect().top),
      ),
    );
    expect(Math.max(...tops) - Math.min(...tops), `not one row: ${tops}`).toBeLessThanOrEqual(10);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **And a proposal arriving does not tip it onto a second line.** The
   * quiet proposer's sentence is cut to the room its line has; it used to
   * count its whole length towards the bar's, so on a window wide enough for
   * one row the bar went to two the moment the assistant said something, and
   * back when it stopped.
   */
  test("a proposal arriving leaves the bar on one row", async ({ page }) => {
    // After the shell, which sets its own size.
    await openShell(page, "/", {}, SLIM);
    await page.setViewportSize({ width: 1600, height: 900 });
    const rows = () =>
      page.evaluate(() => {
        const tops = [".brand", ".device", ".status", ".go"].map((s) =>
          Math.round(document.querySelector(`.topbar ${s}`)!.getBoundingClientRect().top),
        );
        return Math.max(...tops) - Math.min(...tops);
      });
    expect(await rows(), "not one row before anything was said").toBeLessThanOrEqual(10);
    const height = await page.locator(".topbar").evaluate((el) => el.getBoundingClientRect().height);
    await page.evaluate(() => {
      const win = window as unknown as { __lastState?: Record<string, unknown>; __emit?: (next: unknown) => void };
      win.__emit?.({
        ...win.__lastState,
        whisper: {
          kind: "tempo",
          says: "Decks 1 and 2 are 3.4% apart in tempo, and the next phrase lands in eight bars. Sync deck 2?",
          offer: "Sync deck 2",
          run: "deck 2 sync",
          urgent: false,
        },
      });
    });
    await expect(page.locator(".whisper .says")).toHaveCount(1);
    expect(await rows(), "a proposal put the bar on two rows").toBeLessThanOrEqual(10);
    // Cut to the room there is, and there is room: a sentence squeezed to
    // nothing would keep the bar on one row by saying nothing.
    const said = await page.locator(".whisper .says").evaluate((el) => el.getBoundingClientRect().width);
    expect(said, "the proposal was given no room to be read").toBeGreaterThan(120);
    expect(await page.locator(".topbar").evaluate((el) => el.getBoundingClientRect().height)).toBeCloseTo(height, 0);
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
