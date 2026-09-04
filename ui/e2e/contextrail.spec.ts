/**
 * §74's contextual control rail.
 *
 * A compact strip of four to eight controls, whichever the moment calls for.
 * The moment is a *mode* — scratching, stems, preparing, mixing — and djmanzo
 * decides which one from what the deck is doing. The interface is handed the
 * table once and the mode name in every snapshot, which is the arrangement the
 * pad zone already uses.
 *
 * What is measured here is what a type-check cannot see: that the rail is on
 * both decks, that the two decks in the fixture are in *different* modes and
 * so show different controls, and that pressing one sends that deck's action
 * rather than the other's.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const PLAYING = ".deck.playing";
const STOPPED = ".deck:not(.playing)";

test.describe("the contextual rail", () => {
  test("every loaded deck has one, and it says which mode it is in", async ({
    page,
  }) => {
    await openShell(page, "/");

    await expect(page.locator(`${PLAYING} .rail`)).toHaveCount(1);
    await expect(page.locator(`${STOPPED} .rail`)).toHaveCount(1);
    // The playing deck is mixing; the one with a record sitting on it is being
    // prepared. Different facts, different rails.
    await expect(page.locator(`${PLAYING} .rail .mode`)).toHaveText("mixing");
    await expect(page.locator(`${STOPPED} .rail .mode`)).toHaveText("preparing");
  });

  /**
   * The whole point of the section. If both decks drew the same six controls
   * the rail would be a second toolbar, not a contextual one.
   */
  test("a deck being prepared is offered different controls from one mixing", async ({
    page,
  }) => {
    await openShell(page, "/");

    const labels = async (deck: string) =>
      page.locator(`${deck} .rail button`).allInnerTexts();

    const mixing = await labels(PLAYING);
    const preparing = await labels(STOPPED);

    expect(mixing.length).toBeGreaterThanOrEqual(4);
    expect(mixing.length).toBeLessThanOrEqual(8);
    expect(preparing.length).toBeGreaterThanOrEqual(4);
    expect(
      preparing,
      "both decks drew the same rail, so nothing about it is contextual",
    ).not.toEqual(mixing);
    // Sync belongs to a mix and the grid controls to readying a record.
    expect(mixing.join(" ")).toContain("Sync");
    expect(preparing.join(" ")).toContain("Tap");
  });

  test("pressing one sends that deck's action, not its neighbour's", async ({
    page,
  }) => {
    await openShell(page, "/");

    await page.evaluate(() => {
      (window as unknown as Record<string, unknown>).__dispatched = [];
    });

    await page.locator(`${STOPPED} .rail button`).first().click();

    await expect
      .poll(
        () =>
          page.evaluate(
            () => (window as unknown as { __dispatched: string[] }).__dispatched,
          ),
        { message: "the rail press did not reach djmanzo" },
      )
      .toContain("deck 2 cue");
  });

  test("pressing a rail control throws nothing", async ({ page }) => {
    await openShell(page, "/");
    await page.locator(`${PLAYING} .rail button`).first().click();
    expect(errorsThrown(page)).toEqual([]);
  });
});
