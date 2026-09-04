/**
 * The browser's second representation: §20's compact cards.
 *
 * The section asks for *several representations of the same collection*, and
 * the failure to avoid is a card view that is a worse browser — fewer records
 * in it, or fewer things to do with one. So what these measure is sameness:
 * the card holds the record the table holds, and every gesture the row offers
 * is on it and reaches Rust.
 *
 * The sleeve itself cannot be measured here. It is served over a custom URI
 * scheme by the Rust host, which a browser test has no host for, so every card
 * in this harness falls back to its lettering — which is exactly the state most
 * of a part-tagged collection is in, and the one worth being sure looks
 * deliberate rather than broken. `dj_app::cover` tests the serving.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LIBRARY = '.surface[data-surface="library"]';

/** Open the browser and switch it to cards. */
async function cards(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Browse", exact: true }).click();
  await page.getByRole("button", { name: "Show as cards" }).click();
  return page.locator(`${LIBRARY} .card`);
}

test.describe("the card view", () => {
  test("shows the same record the table does, with what a DJ reads off it", async ({
    page,
  }) => {
    const card = (await cards(page)).first();

    await expect(card.locator(".title")).toHaveText("Bachata Rosa");
    await expect(card.locator(".who")).toHaveText("Juan Luis Guerra");
    // BPM, key and length, which is what tells you whether to reach for it.
    await expect(card.locator(".facts")).toContainText("124.0");
    await expect(card.locator(".facts")).toContainText("8A");
    await expect(card.locator(".facts")).toContainText("4:04");
    // No sleeve here, so the record is still identifiable by its lettering
    // rather than being an empty square.
    await expect(card.locator(".lettering")).toHaveText("JB");
  });

  /**
   * The one that matters. A card is only a representation of this browser if
   * it can do what the row can, and the two share one snippet so they cannot
   * drift — this asserts they have not.
   */
  test("every gesture the table row offers is on the card", async ({ page }) => {
    const card = (await cards(page)).first();

    for (const gesture of [
      "Set aside Bachata Rosa",
      "Favourite Bachata Rosa",
      "Find records like Bachata Rosa",
      "Stage Bachata Rosa",
      "Load Bachata Rosa onto deck 1",
      "Load Bachata Rosa onto deck 2",
    ]) {
      await expect(
        card.getByRole("button", { name: gesture }),
        `the card is missing "${gesture}", which the table row has`,
      ).toBeVisible();
    }
  });

  /**
   * §20's "stage": somewhere to put a record without choosing a deck.
   *
   * Deck 1 is playing in the fixture and deck 2 is not, so the honest answer
   * is deck 2 — and the tooltip says so before the press rather than after it.
   */
  test("staging loads the deck that is not playing", async ({ page }) => {
    const card = (await cards(page)).first();
    const stage = card.getByRole("button", { name: "Stage Bachata Rosa" });
    await expect(stage).toHaveAttribute("title", "Stage on deck 2, ready to mix into");

    await page.evaluate(() => ((window as unknown as Record<string, unknown>).__asked = []));
    await stage.click();

    await expect
      .poll(
        () =>
          page.evaluate(() => (window as unknown as { __asked: string[] }).__asked),
        { message: "staging from a card did not reach the deck" },
      )
      .toContain("load_track");
  });

  /**
   * A favourite is five stars — see `favourite` in `Library.svelte` for why
   * this library does not carry a second flag that means the same thing. What
   * a browser can prove is that the press writes the rating.
   */
  test("the star writes the rating rather than a flag of its own", async ({ page }) => {
    const card = (await cards(page)).first();

    await page.evaluate(() => ((window as unknown as Record<string, unknown>).__asked = []));
    await card.getByRole("button", { name: "Favourite Bachata Rosa" }).click();

    await expect
      .poll(
        () =>
          page.evaluate(() => (window as unknown as { __asked: string[] }).__asked),
        { message: "the star did not reach the library" },
      )
      .toContain("edit_tracks");
  });

  /**
   * The table sorts from its column headings; cards have none, so without a
   * control of their own the second view would be the one you cannot order.
   * Three records, because a sort of one is not a sort.
   */
  test("cards can be sorted, and reversed", async ({ page }) => {
    const record = (title: string, artist: string, bpm: number, i: number) => ({
      id: String(i).repeat(64).slice(0, 64),
      path: `/music/${i}.flac`,
      title,
      artist,
      album: null,
      genre: null,
      year: null,
      duration_seconds: 200,
      bpm,
      key: "8A",
      loudness_lufs: null,
      analysed: true,
      play_count: 0,
      rating: null,
      colour: null,
    });
    await openShell(page, "/", {}, {
      library_search: [
        record("Slowest", "Aventura", 96, 1),
        record("Fastest", "Elvis Crespo", 132, 2),
        record("Middling", "Marc Anthony", 118, 3),
      ],
    });
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await page.getByRole("button", { name: "Show as cards" }).click();

    await page.getByLabel("Sort the collection by").selectOption("bpm");
    await expect(page.locator(`${LIBRARY} .card .title`).first()).toHaveText("Slowest");

    await page.getByRole("button", { name: "Reverse the order" }).click();
    await expect(page.locator(`${LIBRARY} .card .title`).first()).toHaveText("Fastest");
  });

  /**
   * A working preference, not a mode. Somebody who chose sleeves gets sleeves
   * back, or they stop choosing them.
   */
  test("the chosen view survives a restart", async ({ page }) => {
    await cards(page);
    await page.reload();
    await page.getByRole("button", { name: "Browse", exact: true }).click();

    await expect(page.locator(`${LIBRARY} .card`).first()).toBeVisible();
    await expect(page.locator(`${LIBRARY} table`)).toHaveCount(0);
  });

  test("switching views throws nothing", async ({ page }) => {
    await cards(page);
    await page.getByRole("button", { name: "Show as a table" }).click();
    await expect(page.locator(`${LIBRARY} table`)).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });
});
