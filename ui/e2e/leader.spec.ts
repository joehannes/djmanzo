/**
 * §117: Space, and the guide to everything behind it.
 *
 * > i'd love something like neovim mnemonic shortcuts ... with a visual
 * > feedback guide/legend where we are after each key in the shortcut chain
 * > with all the possible breadcrumbs/outcomes
 *
 * The tree is Rust's (`dj_app::leader::tree`, every leaf held there to
 * something djmanzo can do), delivered here as `leader.json`. What the
 * browser holds is the walk: the guide shows where the DJ is and every key
 * from there after each press, a leaf runs through the path its own button
 * takes, a key that leads nowhere is said and never reaches a deck, and the
 * performance keys and the digits keep working around it.
 */
import { expect, test, type Page } from "@playwright/test";

import tree from "./leader.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const guide = (page: Page) => page.locator("[data-guide]");

/**
 * The shipped keyboard's chords on the letters these tests type, so a key the
 * guide should take would reach a deck if it did not: `d` syncs deck 1, `p`
 * turns deck 2's loop off, `j` plays deck 2.
 */
const binding = (chord: string, press: string) => ({
  chord,
  label: press,
  group: "Deck",
  held: false,
  press,
  release: null,
});
const KEYS = {
  keyboard_keys: [
    binding("keyd", "deck 1 sync_toggle"),
    binding("keyp", "deck 2 loop_off"),
    binding("keyj", "deck 2 play_pause"),
    binding("shift+digit1", "deck 1 hotcue 1"),
  ],
};
const dispatched = (page: Page) =>
  page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []);

test.describe("§117: the leader key", () => {
  /**
   * **The load-bearing one: `Space d 1 p` plays deck 1, and the guide says
   * where it is at every step.** The top of the tree shows every key Rust
   * put there; each group adds its word to the breadcrumb; the leaf runs and
   * the guide is gone.
   */
  test("Space d 1 p plays deck 1, with the way there shown at each step", async ({ page }) => {
    await openShell(page, "/", {}, KEYS);
    await page.keyboard.press("Space");
    await expect(guide(page)).toBeVisible();
    const trail = guide(page).getByRole("list", { name: "Where you are" });
    await expect(trail).toHaveText("Space");
    // Every key the top of the tree has, and nothing else.
    await expect(guide(page).locator("[data-key]")).toHaveCount(tree.children.length);
    for (const node of tree.children) {
      await expect(guide(page).locator(`[data-key="${node.key}"]`)).toContainText(node.label);
    }

    await page.keyboard.press("d");
    await expect(trail).toContainText("Deck");
    await page.keyboard.press("1");
    await expect(trail).toContainText("Deck 1");
    // The letter is underlined in its word.
    await expect(guide(page).locator('[data-key="p"] u')).toHaveText("P");

    await page.keyboard.press("p");
    await expect(guide(page)).toHaveCount(0);
    await expect.poll(() => dispatched(page)).toContain("deck 1 play_pause");
    // Only that: `d` and `p` are deck keys too, and the guide kept them.
    expect(await dispatched(page)).toEqual(["deck 1 play_pause"]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **One letter off is said, not played.** `Space d j` — `j` is deck 2's
   * play on the keyboard — does nothing but say so; Backspace goes back a
   * step, Escape closes, and nothing reached a deck.
   */
  test("a key that leads nowhere is named, and nothing is sent", async ({ page }) => {
    await openShell(page, "/", {}, KEYS);
    await page.keyboard.press("Space");
    await page.keyboard.press("d");
    await page.keyboard.press("j");
    await expect(guide(page).getByRole("status")).toContainText("Nothing on j here");
    await page.keyboard.press("Backspace");
    await expect(guide(page).getByRole("list", { name: "Where you are" })).toHaveText("Space");
    await page.keyboard.press("Escape");
    await expect(guide(page)).toHaveCount(0);
    expect(await dispatched(page)).toEqual([]);
  });

  /**
   * Space twice is the palette; `Space o l` opens the library, through the
   * path its own button takes; `Space 2` is the second activity, as `2` is.
   */
  test("leaves open the palette, a panel and an activity", async ({ page }) => {
    await openShell(page, "/");
    await page.keyboard.press("Space");
    await page.keyboard.press("Space");
    await expect(page.getByRole("dialog", { name: "Command palette" })).toBeVisible();
    await page.keyboard.press("Escape");

    await page.keyboard.press("Space");
    await page.keyboard.press("o");
    await page.keyboard.press("l");
    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();

    await page.keyboard.press("Space");
    await page.keyboard.press("2");
    await expect(page.locator('[data-activity-strip] [data-activity="mix"]')).toHaveAttribute("aria-pressed", "true");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The performance keys and the digits keep working around it.** With the
   * shipped chord for a hot cue, `Shift+1` fires it and a bare `1` is the
   * first activity; and a space typed into the library's search is a space.
   */
  test("hot cues are Shift and a digit, and Space while typing is a space", async ({ page }) => {
    await openShell(page, "/", {}, KEYS);
    await page.keyboard.press("Shift+Digit1");
    await expect.poll(() => dispatched(page)).toContain("deck 1 hotcue 1");
    await expect(page.locator("[data-activity-strip]")).toHaveCount(0);

    await page.keyboard.press("1");
    await expect(page.locator('[data-activity-strip] [data-activity="dig"]')).toHaveAttribute("aria-pressed", "true");

    const search = page.getByRole("searchbox", { name: "Search the library" });
    await search.click();
    await search.pressSequentially("space jam");
    await expect(search).toHaveValue("space jam");
    await expect(guide(page)).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});
