/**
 * §109's activity mode, driven.
 *
 * > create a different more interactive GUI mode ... where the current screen
 * > basically show the necessary (as of current activity) controls/widgets
 * > only ... then you can change activies. there's pre-arranged activities and
 * > the user can create his own
 * >
 * > i 's also like the ability to nicely switch those activies comfortable and
 * > easily and in a way that fits the task of a DJ (live)
 *
 * Which activities exist, their keys, and what the moment suggests are
 * `dj_app::activity`'s and tested there. What is held here is what a DJ meets:
 * that the strip takes the panel row's place without moving the decks, that a
 * key moves between activities and one key goes back, that the keys never fire
 * while typing or outside the mode, that a DJ's own activity is kept and can
 * be forgotten, and that a suggestion is a mark and not a switch.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const STRIP = "[data-activity-strip]";
const tab = (page: Page, slug: string) => page.locator(`${STRIP} [data-activity="${slug}"]`);
const surface = (page: Page, name: string) => page.locator(`.surface[data-surface="${name}"]`);

async function enter(page: Page) {
  await page.getByRole("button", { name: "Activities", exact: true }).click();
  await expect(page.locator(STRIP)).toBeVisible();
}

test.describe("§109: activity mode", () => {
  /**
   * **The load-bearing one: the strip takes the panel row's place, and the
   * decks do not move.** A DJ entering activity mode mid-set has a hand on a
   * fader; the row above changes and nothing under the hand does.
   */
  test("the strip replaces the panel row and the decks stay where they were", async ({ page }) => {
    await openShell(page, "/");
    // Where the top bar ends is where the decks begin. Measured there rather
    // than on a deck, because a deck can also be moved by things that are
    // not the strip's — §48's low-frame-rate banner appears above the decks
    // on a loaded machine, and a test measuring the deck measured that.
    const bar = page.locator(".topbar");
    const bottom = async () => {
      const box = await bar.boundingBox();
      return (box?.y ?? 0) + (box?.height ?? 0);
    };
    const before = await bottom();
    await enter(page);

    await expect(page.getByRole("navigation", { name: "Panels" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Browse", exact: true })).toHaveCount(0);
    // The set's own controls stay: they are read from across the booth.
    await expect(page.getByRole("group", { name: "This set" })).toBeVisible();

    const after = await bottom();
    expect(Math.abs(after - before), "the decks moved when the strip came in").toBeLessThanOrEqual(2);

    // And between activities, which a night does far more often than it
    // enters the mode: none of them moves where the decks begin.
    for (const key of ["F1", "F3", "F4", "F5", "F6", "F7", "F2"]) {
      await page.keyboard.press(key);
      await expect(page.locator(`${STRIP} [aria-pressed="true"]`)).toHaveCount(1);
      expect(Math.abs((await bottom()) - after), `${key} moved the decks`).toBeLessThanOrEqual(2);
    }
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A key each, and one key back.** F1 digs — the collection opens — F2
   * mixes and it closes, and the key under Escape returns to the dig: the
   * round trip a night makes most often, in two presses.
   */
  test("F-keys move between activities and the back key returns", async ({ page }) => {
    await openShell(page, "/");
    await enter(page);

    await page.keyboard.press("F1");
    await expect(tab(page, "dig")).toHaveAttribute("aria-pressed", "true");
    await expect(surface(page, "library")).toBeVisible();

    await page.keyboard.press("F2");
    await expect(tab(page, "mix")).toHaveAttribute("aria-pressed", "true");
    await expect(surface(page, "library")).toHaveCount(0);

    await page.keyboard.press("Backquote");
    await expect(tab(page, "dig")).toHaveAttribute("aria-pressed", "true");
    await expect(surface(page, "library")).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The keys are the activity mode's alone.** In the full cockpit an F-key
   * does what it always did — nothing of djmanzo's — and while the DJ is
   * typing a name it is a keystroke, not a switch.
   */
  test("the keys do nothing outside the mode or while typing", async ({ page }) => {
    await openShell(page, "/");
    await page.keyboard.press("F1");
    await expect(surface(page, "library")).toHaveCount(0);
    await expect(page.locator(STRIP)).toHaveCount(0);

    await enter(page);
    await page.keyboard.press("F2");
    await expect(tab(page, "mix")).toHaveAttribute("aria-pressed", "true");
    await page.getByRole("button", { name: "Keep as activity" }).click();
    await page.getByRole("textbox", { name: "Name for the new activity" }).press("F1");
    await expect(tab(page, "mix")).toHaveAttribute("aria-pressed", "true");
    await expect(surface(page, "library")).toHaveCount(0);
  });

  /**
   * **A DJ's own: arranged in the full cockpit, kept by name, reachable by
   * its key, and forgotten.** The whole life of one, as a DJ would live it.
   */
  test("a DJ's own activity is kept, switched to by its key, and forgotten", async ({ page }) => {
    await openShell(page, "/");
    // Arranged by hand in the full cockpit: the night panel open.
    await page.getByRole("button", { name: "Night", exact: true }).click();
    await expect(surface(page, "night")).toBeVisible();
    await page.getByRole("button", { name: "Keep this arrangement as an activity" }).click();
    await page.getByRole("textbox", { name: "Name for the new activity" }).fill("Warm-up");
    await page.getByRole("button", { name: "Keep", exact: true }).click();

    // Kept, and in it: the strip is up and the new tab is the current one.
    await expect(tab(page, "warm-up")).toHaveAttribute("aria-pressed", "true");
    await expect(tab(page, "warm-up")).toContainText("F8");

    // Away and back by its key: its arrangement comes back with it.
    await page.keyboard.press("F2");
    await expect(surface(page, "night")).toHaveCount(0);
    await page.keyboard.press("F8");
    await expect(surface(page, "night")).toBeVisible();

    await page.getByRole("button", { name: "Forget Warm-up" }).click();
    await expect(tab(page, "warm-up")).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A name djmanzo ships is refused, and said on the strip. */
  test("a shipped name cannot be taken", async ({ page }) => {
    await openShell(page, "/");
    await enter(page);
    await page.getByRole("button", { name: "Keep as activity" }).click();
    await page.getByRole("textbox", { name: "Name for the new activity" }).fill("Mix");
    await page.getByRole("button", { name: "Keep", exact: true }).click();
    await expect(page.locator(`${STRIP} [role="alert"]`)).toContainText("already has an activity");
    await expect(page.locator(`${STRIP} [data-activity]`)).toHaveCount(7);
  });

  /**
   * **§115: the assistant suggests, and the screen does not move.** The
   * suggested tab is marked and carries the reason; the activity on screen
   * stays the DJ's.
   */
  test("a suggestion is a mark with a reason, never a switch", async ({ page }) => {
    const because = "80 seconds left on deck 1 and nothing loaded to follow it.";
    await openShell(page, "/", {}, { activity_suggestion: { activity: "dig", because } });
    await enter(page);
    await page.keyboard.press("F2");
    await expect(tab(page, "mix")).toHaveAttribute("aria-pressed", "true");

    await expect(tab(page, "dig")).toHaveClass(/suggested/);
    await expect(tab(page, "dig")).toHaveAttribute("title", new RegExp(because));
    await expect(tab(page, "dig").getByLabel(`Suggested: ${because}`)).toBeVisible();

    // Nothing switched — and "nothing" has to include the next time the
    // suggestion is asked for, not only the moment it was drawn. The first
    // version of this test asserted straight away, and a strip that followed
    // its own suggestion on the next poll passed it.
    const asked = () =>
      page.evaluate(
        () => ((window as unknown as { __asked: string[] }).__asked ?? []).filter((c) => c === "activity_suggestion").length,
      );
    const before = await asked();
    await page.waitForFunction(
      (n) => ((window as unknown as { __asked: string[] }).__asked ?? []).filter((c) => c === "activity_suggestion").length > n,
      before,
      { timeout: 10_000 },
    );
    await page.waitForTimeout(300);
    await expect(tab(page, "mix")).toHaveAttribute("aria-pressed", "true");
    await expect(surface(page, "library")).toHaveCount(0);
  });

  /**
   * **The Requests activity shows the requests, and finding one searches the
   * collection.** It shipped placing a surface the shell never drew, so it
   * opened as the collection alone; the room's asks are now a surface of
   * their own beside the decks, and "Find it" reaches the search box of a
   * collection in another component.
   */
  test("the Requests activity shows the room's asks and finds one in the collection", async ({ page }) => {
    await openShell(page, "/", {}, {
      audience_status: {
        running: true,
        open: true,
        port: 8765,
        heading: "Ask for a song",
        language: "en",
        show_playing: false,
        ways_in: [],
        announcing: false,
        announce_error: null,
        error: null,
        waiting: 1,
      },
      audience_waiting: [
        { id: 1, text: "Tune Rising", voices: 3, first_asked: 1_700_000_000, last_asked: 1_700_000_100, standing: "waiting" },
      ],
    });
    await enter(page);
    await page.keyboard.press("F5");
    await expect(tab(page, "requests")).toHaveAttribute("aria-pressed", "true");
    await expect(surface(page, "requests")).toBeVisible();
    await expect(surface(page, "library")).toBeVisible();

    await surface(page, "requests").getByRole("button", { name: "Find it" }).click();
    await expect(surface(page, "library").getByPlaceholder(/search/i).first()).toHaveValue("Tune Rising");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Back to the full cockpit, and the arrangement stays.** Leaving the mode
   * is a change of chrome, not of what is on screen.
   */
  test("leaving brings the panel row back and keeps the arrangement", async ({ page }) => {
    await openShell(page, "/");
    await enter(page);
    await page.keyboard.press("F1");
    await expect(surface(page, "library")).toBeVisible();
    await page.getByRole("button", { name: "Full cockpit" }).click();
    await expect(page.locator(STRIP)).toHaveCount(0);
    await expect(page.getByRole("navigation", { name: "Panels" })).toBeVisible();
    await expect(surface(page, "library")).toBeVisible();
  });

  /**
   * **Every tab is on screen at djmanzo's own window size.** A strip that
   * clipped its last tab would be an activity with a key and no button.
   */
  test("every tab fits at 1280 by 800", async ({ page }) => {
    await openShell(page, "/");
    await enter(page);
    const width = page.viewportSize()?.width ?? 1280;
    for (const box of await page.locator(`${STRIP} button`).evaluateAll((buttons) =>
      buttons.map((b) => b.getBoundingClientRect().toJSON()),
    )) {
      expect(box.right, "a strip button past the window's edge").toBeLessThanOrEqual(width);
      expect(box.width).toBeGreaterThan(0);
    }
  });
});
