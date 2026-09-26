/**
 * §121: dashboards that are dashboards.
 *
 * > i meant the dashboard to be a dashboard not only like the which-key
 * > shortcut widget ... (prepare dashboard, live dashboard, messing
 * > around/experimental dashboard, my music dashbaord, my presskit dashboard,
 * > social dashboard) ... all these dashboards can hold widgets, info,
 * > controls ... they should replace (apart from the ubuquitous main bar ...
 * > music flow/ctrl/autopilot ... disaster ctrl) the main view entirely
 *
 * The boards are `dj_app::boards`', delivered as `boards.json`; this holds
 * that one takes the decks' place, keeps the music in reach, and gives the
 * decks back as they were.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";
import boards from "./boards.json" with { type: "json" };

const dispatched = (page: Page) =>
  page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []);

async function shell(page: Page) {
  await openShell(page, "/");
  await page.setViewportSize({ width: 1400, height: 860 });
}

const board = (page: Page) => page.locator("section.board");

/** Put a board up from the bar's one button and its menu. */
async function pick(page: Page, title: string) {
  await page.getByRole("button", { name: "Dashboards", exact: true }).click();
  await page.getByRole("menu", { name: "Dashboards" }).getByRole("menuitemradio", { name: title }).click();
}

test.describe("§121: the boards", () => {
  /**
   * **The load-bearing one.** A board takes the place of the decks and the
   * docks — they are not drawn behind it — shows its widgets, keeps the
   * decks in brief under the bar, and "Decks" gives the cockpit back with the
   * panel that was open still open.
   */
  test("a board replaces the decks, keeps them in brief, and gives them back", async ({ page }) => {
    await shell(page);
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();

    await page.getByRole("button", { name: "Dashboards", exact: true }).click();
    const menu = page.getByRole("menu", { name: "Dashboards" });
    // Every board, and the decks.
    await expect(menu.getByRole("menuitemradio")).toHaveCount(boards.length + 1);
    await expect(menu.getByRole("menuitemradio", { name: "Decks" })).toHaveAttribute("aria-checked", "true");
    await menu.getByRole("menuitemradio", { name: "Live" }).click();
    await expect(menu).toHaveCount(0);
    await expect(board(page)).toHaveAttribute("data-board", "live");
    await expect(page.locator(".cockpit")).toHaveCount(0);
    await expect(page.locator(".deck-slot")).toHaveCount(0);
    const live = boards.find((b) => b.slug === "live")!;
    for (const widget of live.widgets) {
      await expect(board(page).locator(`[data-widget="${widget.name}"]`)).toHaveCount(1);
    }
    // The music, in brief, at the top of the board.
    const strip = board(page).locator(".board-head").getByRole("group", { name: "The decks in brief" });
    await expect(strip.getByRole("button", { name: /^(Play|Pause) deck 1$/ })).toBeVisible();
    await expect(strip.getByRole("button", { name: /Autopilot/ })).toBeVisible();
    await page.getByRole("button", { name: "Dashboards", exact: true }).click();
    await expect(page.getByRole("menuitemradio", { name: "Live" })).toHaveAttribute("aria-checked", "true");
    await page.getByRole("menuitemradio", { name: "Decks" }).click();
    await expect(board(page)).toHaveCount(0);
    await expect(page.locator(".deck-slot").first()).toBeVisible();
    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The strip's controls send what the decks' own send. */
  test("the decks in brief play a deck and hand the mix to the autopilot", async ({ page }) => {
    await shell(page);
    await pick(page, "Prepare");
    const strip = board(page).getByRole("group", { name: "The decks in brief" });
    // The fixture has deck 1 playing and deck 2 stopped: each button says
    // what it will do.
    await expect(strip.getByRole("button", { name: "Pause deck 1" })).toHaveAttribute("aria-pressed", "true");
    await strip.getByRole("button", { name: "Play deck 2" }).click();
    await strip.getByRole("button", { name: /Autopilot/ }).click();
    await expect.poll(() => dispatched(page)).toEqual(
      expect.arrayContaining(["deck 2 play_pause", "automix on"]),
    );
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Two keys each: Space b and a letter up, Space b d back. */
  test("Space b and a letter puts a board up, and Space b d goes back to the decks", async ({ page }) => {
    await shell(page);
    await page.locator("body").click({ position: { x: 700, y: 120 } });
    await page.keyboard.press("Space");
    await page.keyboard.press("b");
    await page.keyboard.press("m");
    await expect(board(page)).toHaveAttribute("data-board", "music");
    await expect(board(page).locator('[data-widget="library"]')).toBeVisible();
    await page.keyboard.press("Space");
    await page.keyboard.press("b");
    await page.keyboard.press("d");
    await expect(board(page)).toHaveCount(0);
    await expect(page.locator(".deck-slot").first()).toBeVisible();
  });

  /**
   * **The press kit like a desktop.** A folder opens that part of the kit
   * beside it: the kit turns to its fields and brings the heading into view.
   */
  test("a press-kit folder opens that part of the kit", async ({ page }) => {
    await shell(page);
    await pick(page, "Press kit");
    const desktop = board(page).getByRole("list", { name: "Press kit folders" });
    await expect(desktop.getByRole("button")).toHaveCount(7);
    const kit = board(page).locator('[data-widget="kit"]');
    await expect(kit.getByRole("tab", { name: "Send" })).toHaveAttribute("aria-selected", "true");

    await desktop.getByRole("button", { name: "Photos" }).click();
    await expect(kit.getByRole("tab", { name: "Your kit" })).toHaveAttribute("aria-selected", "true");
    const heading = kit.getByRole("heading", { name: "Photos", exact: true });
    await expect(heading).toBeInViewport();
    expect(errorsThrown(page)).toEqual([]);
  });

  /** From a board, an activity or a widget's own button goes back to the decks. */
  test("an activity or a widget's dock button goes back to the decks", async ({ page }) => {
    await shell(page);
    await pick(page, "Social");
    const links = board(page).locator('[data-widget="links"]');
    await links.getByRole("button", { name: /Mix/ }).first().click();
    await expect(board(page)).toHaveCount(0);

    await pick(page, "Experiment");
    await board(page).getByRole("button", { name: "Sampler beside the decks" }).click();
    await expect(board(page)).toHaveCount(0);
    await expect(page.locator('.surface[data-surface="sampler"]')).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /** §117's tile screen is the launcher now, under its own name. */
  test("the tile screen is called the launcher", async ({ page }) => {
    await openShell(page, "/", {}, { interface_settings: { toolbars: false, uses: {} } });
    await page.getByRole("button", { name: /Launcher/ }).click();
    await expect(page.getByRole("dialog", { name: "Launcher" })).toBeVisible();
  });
});
