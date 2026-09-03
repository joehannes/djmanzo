/**
 * The command palette, and the one thing that makes it more than a menu.
 *
 * §51 asks for a command surface on `Ctrl/Cmd + K` and closes by saying it
 * "can also become the semantic interface exposed to voice/AI". That is why
 * every entry is generated in Rust from `dj_core::vocabulary` rather than
 * typed into a list here, and why **what you type is itself an entry**:
 * `deck 2 loop 8` parses, so the top row runs it. The verbs that take an
 * argument can be reached no other way — a list of buttons would have to
 * invent the number.
 *
 * What is measured here is the part Rust cannot see: that the keys open and
 * close it, that Enter runs the row that is chosen, and that the two kinds of
 * entry take two different paths — an action through the bus, a surface
 * through the dock manager.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** What the page has asked Rust for since `watch` was called. */
async function watch(page: import("@playwright/test").Page) {
  await page.evaluate(() => ((window as unknown as Record<string, unknown>).__asked = []));
  return () =>
    page.evaluate(() => (window as unknown as { __asked: string[] }).__asked);
}

const palette = (page: import("@playwright/test").Page) =>
  page.getByRole("dialog", { name: "Command palette" });

test.describe("the command palette", () => {
  test("opens on Ctrl+K and closes on Escape", async ({ page }) => {
    await openShell(page, "/");
    await expect(palette(page)).toHaveCount(0);

    await page.keyboard.press("Control+k");
    await expect(palette(page)).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(
      palette(page),
      "Escape did not close the palette, which is the only way out for a DJ " +
        "whose hands are on the keyboard",
    ).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Cmd+K too, because half of DJ laptops are Macs. */
  test("opens on Cmd+K as well", async ({ page }) => {
    await openShell(page, "/");
    await page.keyboard.press("Meta+k");
    await expect(palette(page)).toBeVisible();
  });

  /**
   * It opens with something in it.
   *
   * A palette that showed nothing until you typed would waste the first press,
   * and the first press is the one made in a hurry.
   */
  test("opens with commands already offered", async ({ page }) => {
    await openShell(page, "/");
    await page.keyboard.press("Control+k");

    await expect(palette(page).getByRole("button")).not.toHaveCount(0);
    await expect(palette(page).getByRole("button").first()).toContainText("deck 2 loop 8");
  });

  /**
   * **Enter runs the chosen row, and an action goes through the bus.**
   *
   * The same path a button on a deck takes — `perform` in Rust, which records
   * the human touch and then dispatches. A palette that had its own way of
   * reaching the engine would be a second door with different locks.
   */
  test("Enter runs the chosen command through the action bus", async ({ page }) => {
    await openShell(page, "/");
    await page.keyboard.press("Control+k");
    const asked = await watch(page);

    await page.keyboard.press("Enter");

    await expect.poll(asked, {
      message: "Enter did not send the chosen action",
    }).toContain("dispatch");
    await expect(palette(page), "the palette stayed open after running").toHaveCount(0);
  });

  /** The arrows move the choice, so Enter runs a different thing. */
  test("the arrows change what Enter runs", async ({ page }) => {
    await openShell(page, "/");
    await page.keyboard.press("Control+k");

    const rows = palette(page).getByRole("button");
    await expect(rows.first()).toHaveClass(/chosen/);
    await page.keyboard.press("ArrowDown");
    await expect(
      rows.nth(1),
      "the down arrow did not move the choice",
    ).toHaveClass(/chosen/);
    await expect(rows.first()).not.toHaveClass(/chosen/);
  });

  /**
   * **A surface entry opens the surface rather than sending an action.**
   *
   * The two kinds are the reason `kind` is on the entry at all: sending
   * `prepare` to the action parser would be an error, and opening a surface
   * called `deck 1 play` is nonsense.
   */
  test("a surface entry opens the surface", async ({ page }) => {
    await openShell(page, "/");
    await page.keyboard.press("Control+k");

    await palette(page).getByRole("button", { name: /Show Prepare/ }).click();

    await expect(
      page.locator('.surface[data-surface="prepare"]'),
      "choosing Show Prepare did not open the Prepare surface",
    ).toBeVisible();
    await expect(palette(page)).toHaveCount(0);
  });

  /**
   * Ctrl+K is the only key djmanzo takes globally, and it does not steal a K
   * from a DJ typing into the browser's search box.
   */
  test("typing a k somewhere else is still a k", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();

    // `type="search"` is a searchbox, not a textbox.
    const search = page.getByRole("searchbox", { name: "Search the library" });
    await search.click();
    await search.fill("k");

    await expect(palette(page), "a plain k opened the palette").toHaveCount(0);
    await expect(search).toHaveValue("k");
  });
});
