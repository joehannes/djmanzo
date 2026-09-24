/**
 * §111: a song djmanzo does not have, and where to buy it.
 *
 * Which stores, in which order, and the addresses themselves are Rust's —
 * `dj_sources::stores`, tested there, and `stores.json`, blessed from it.
 * What a browser holds is that the links appear where a DJ meets a song
 * they do not have, that pressing one names the store and the song (never
 * an address) to Rust, and that the sentence about what buying covers is
 * always beside them.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const opened = (page: Page) =>
  page.evaluate(() => (window as unknown as { __opened?: Record<string, unknown>[] }).__opened ?? []);

test.describe("§111: finding a song to buy", () => {
  /**
   * **The load-bearing one: nothing in the collection matches, and the
   * stores are offered for the words searched.** Pressing one sends the
   * store's name and the song, and the address is built in Rust.
   */
  test("a search that finds nothing offers the stores for it", async ({ page }) => {
    await openShell(page, "/", {}, { library_search: [] });
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    const library = page.locator('.surface[data-surface="library"]');
    await library.getByPlaceholder(/search/i).first().fill("Bachata Rosa");
    await expect(library.getByText("Nothing matches")).toBeVisible();

    const buy = library.locator("[data-buy-links]");
    await expect(buy).toBeVisible();
    await expect(buy.locator("[data-store]").first()).toHaveAttribute("data-store", "bandcamp");
    await expect(buy).toContainText("performance licence");

    await buy.locator('[data-store="beatport"]').click();
    await expect.poll(() => opened(page)).toEqual([{ store: "beatport", artist: "", title: "Bachata Rosa" }]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A karaoke host is pointed at karaoke's stores first**: a singer asked
   * for a song nobody has, and licensed backing tracks are what the night
   * needs.
   */
  test("a singer's song nobody has offers karaoke's stores first", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Activities", exact: true }).click();
    await page.keyboard.press("F8");
    const singers = page.locator('.surface[data-surface="karaoke"]');
    await singers.getByRole("textbox", { name: "Singer's name" }).fill("Ben");
    await singers.getByRole("textbox", { name: "Song" }).fill("A song nobody has");
    await singers.getByRole("button", { name: "Add", exact: true }).click();

    const buy = singers.getByRole("region", { name: "Up next" }).locator("[data-buy-links]");
    await expect(buy.locator("[data-store]").first()).toHaveAttribute("data-store", "karaoke-version");
    await buy.locator('[data-store="karaoke-version"]').click();
    await expect
      .poll(() => opened(page))
      .toEqual([{ store: "karaoke-version", artist: "", title: "A song nobody has" }]);
  });
});

test.describe("§111: filing what was bought", () => {
  /**
   * **The downloads folder, switched on by the DJ, and what it filed.** The
   * rule that chooses the folder is Rust's (`dj_app::downloads::place`), and
   * the move was driven in the running application; what the settings hold
   * is that the switch sends both folders with it, and that every filing is
   * listed — the one that could not be moved saying why.
   */
  test("the settings switch filing on and list what was filed", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Settings", exact: true }).click();
    const block = page.locator("[data-downloads]");
    await block.scrollIntoViewIfNeeded();
    await expect(block.locator("[data-watch]")).toHaveText("/home/dj/Downloads");
    await expect(block.locator("[data-into]")).toHaveText("/home/dj/Music");

    const filed = block.getByRole("list", { name: "Filed lately" }).locator("li");
    await expect(filed).toHaveCount(3);
    await expect(filed.nth(0)).toContainText("House/Kerri Chandler/rising.wav");
    await expect(filed.nth(2)).toContainText("left where it was: Permission denied");

    await block.getByRole("checkbox").check();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __setDownloads?: unknown[] }).__setDownloads ?? []))
      .toEqual([{ watch: "/home/dj/Downloads", into: "/home/dj/Music", on: true }]);
    await expect(block.getByRole("checkbox")).toBeChecked();
    expect(errorsThrown(page)).toEqual([]);
  });
});
