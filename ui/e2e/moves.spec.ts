/**
 * §26's *beat jump: contextual action* — the one item on its list that is not
 * a drag.
 *
 * > The waveform should become an interactive control surface. […] **Beat
 * > jump: contextual action.** […] The DJ should be able to physically grab
 * > the thing they are thinking about.
 *
 * Which moves are offered, and whether one fits, is Rust's and is tested in
 * `dj_app::jumps`. These say the three things only a browser can: that a
 * right-click on the lane **opens** something, that choosing an entry
 * dispatches the **action Rust named** rather than one the component invented,
 * and that the browser's own menu does not come up over it.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LANE = ".lane";

/** Everything the page has dispatched, oldest first. */
const dispatched = (page: Page) =>
  page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []);

test.describe("§26's contextual beat jump", () => {
  /**
   * **The load-bearing one: it opens, and it offers Rust's list.**
   *
   * Not "a menu appears": the labels are Rust's words, and a component that
   * built its own would look identical until the day the sizes changed on one
   * side only.
   */
  test("a right-click on the waveform offers the moves that fit", async ({ page }) => {
    await openShell(page, "/");
    await page.locator(LANE).first().click({ button: "right" });

    const menu = page.locator("[data-moves]").first();
    await expect(menu).toBeVisible();
    await expect(menu).toContainText("Back 4 beats");
    await expect(menu).toContainText("Forward a phrase");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Nothing clips the menu.**
   *
   * The defect this exists to catch is the one driving the application found:
   * the menu was drawn *inside* the lane, which is `overflow: hidden` and
   * `contain: strict` — it has to be, for the transform that scrolls the strip
   * — so it was cut off at the waveform's edge. Five moves were offered and
   * two were visible, looking exactly like a shorter menu.
   *
   * **Two obvious ways to assert this do not work, and both were tried.**
   * `boundingBox()` reports an element's layout box whether or not an ancestor
   * clips it away, and `toBeVisible()` reads `display`, `visibility` and
   * `opacity` and knows nothing about clipping. A trial click is worse than
   * useless here: Playwright scrolls an element into view before its
   * actionability check, and an `overflow: hidden` box *is* programmatically
   * scrollable — so the check reveals the entry it was supposed to fail on.
   * All three passed against the bug.
   *
   * So this asserts the invariant the fix actually establishes: between the
   * menu and the page there is no ancestor that cuts it off. That is the rule
   * a reader can check by eye, and it is the rule the bug broke.
   */
  test("nothing between the menu and the page clips it", async ({ page }) => {
    await openShell(page, "/");
    await page.locator(LANE).first().click({ button: "right" });
    await expect(page.locator("[data-moves]").first()).toBeVisible();

    const cut = await page.evaluate(() => {
      const menu = document.querySelector("[data-moves]");
      if (!menu) return "there is no menu";
      const box = menu.getBoundingClientRect();
      for (
        let node = menu.parentElement;
        node && node !== document.body;
        node = node.parentElement
      ) {
        const style = getComputedStyle(node);
        const clips =
          style.overflow !== "visible" ||
          style.overflowX !== "visible" ||
          style.overflowY !== "visible" ||
          style.contain.includes("strict") ||
          style.contain.includes("paint");
        if (!clips) continue;
        const edge = node.getBoundingClientRect();
        // A little slack: a border or a sub-pixel rounding difference is not
        // a menu a DJ cannot read.
        if (box.bottom > edge.bottom + 1 || box.right > edge.right + 1) {
          return `${node.className || node.tagName} cuts the menu off: menu ends at ${Math.round(
            box.bottom,
          )},${Math.round(box.right)} and it ends at ${Math.round(edge.bottom)},${Math.round(
            edge.right,
          )}`;
        }
      }
      return "";
    });

    expect(cut, cut).toBe("");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Choosing one dispatches exactly what Rust said.**
   *
   * The failure this catches is the component mapping a label to an action of
   * its own — `handle::Option_`'s "a menu whose entries are not actions is a
   * second vocabulary", one section over. A menu that sent `deck 1 beatjump 4`
   * for *Back 4 beats* would look perfect and move the record the wrong way.
   */
  test("choosing a move dispatches the action rather than a label", async ({ page }) => {
    await openShell(page, "/");
    await page.locator(LANE).first().click({ button: "right" });
    await page.getByRole("button", { name: "Back 4 beats" }).click();

    expect(await dispatched(page)).toContain("deck 1 beatjump -4");
    // And it closes, rather than sitting over the record it just moved.
    await expect(page.locator("[data-moves]")).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A press away from it closes it without dispatching.**
   *
   * A DJ who opened the menu by accident mid-mix has to be able to get rid of
   * it, and a menu that only closes when something in it is chosen makes the
   * accident into a move.
   */
  test("pressing away closes the menu and moves nothing", async ({ page }) => {
    await openShell(page, "/");
    await page.locator(LANE).first().click({ button: "right" });
    await expect(page.locator("[data-moves]").first()).toBeVisible();

    const before = await dispatched(page);
    await page.locator(".moves-away").first().click();
    await expect(page.locator("[data-moves]")).toHaveCount(0);
    expect(await dispatched(page)).toEqual(before);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A deck with nothing to offer gets no menu.**
   *
   * Rust answers empty for a record it cannot count beats in, and an empty
   * menu is worse than none: a DJ who opens it twice and finds nothing stops
   * opening it. Asserted on the empty answer rather than on an unloaded deck,
   * because the claim is about what the component does with *empty*.
   */
  test("nothing to offer means no menu at all", async ({ page }) => {
    await openShell(page, "/", {}, { waveform_moves: [] });
    await page.locator(LANE).first().click({ button: "right" });
    await expect(page.locator("[data-moves]")).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});
