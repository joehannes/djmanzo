/**
 * §76's AI lens, and the line that governs it.
 *
 * > This must never replace the standard library view.
 *
 * So the tests that matter are not about the columns being right — the
 * arithmetic is Rust's and is tested in `dj_app::lens`. They are about what
 * the lens does to the view it is added to: that the standard columns are
 * still there and still in their places with it on, that turning it off leaves
 * exactly what was there before, and that a cell djmanzo has no opinion about
 * is blank rather than zero.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LIBRARY = '.surface[data-surface="library"]';

async function libraryOpen(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Browse", exact: true }).click();
  await expect(page.locator(`${LIBRARY} table tbody tr`).first()).toBeVisible();
}

/** The headings djmanzo has always shown, in the order it shows them. */
const STANDARD = ["Title", "Artist", "Album", "BPM", "Key", "Time"];

async function standardHeadings(page: import("@playwright/test").Page) {
  return page
    .locator(`${LIBRARY} thead th button.sort`)
    .allTextContents()
    .then((all) => all.map((t) => t.replace(/[▲▼]/g, "").trim()));
}

test.describe("the AI lens", () => {
  /** Off to begin with, and off is the ordinary collection. */
  test("is off until asked for", async ({ page }) => {
    await libraryOpen(page);

    await expect(page.getByTestId("lens-toggle")).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    await expect(page.locator(`${LIBRARY} th.lens`)).toHaveCount(0);
    expect(await standardHeadings(page)).toEqual(STANDARD);
    expect(errorsThrown(page), "the library threw").toEqual([]);
  });

  /**
   * **§76's closing line, as a test.**
   *
   * With the lens on, every standard column is still there and still in the
   * same place. A lens that reordered or replaced them would be a different
   * view wearing the same name — and a DJ who reaches for Title where Title
   * has always been would find something else.
   */
  test("adds columns without disturbing the standard ones", async ({ page }) => {
    await libraryOpen(page);
    const before = await standardHeadings(page);

    await page.getByTestId("lens-toggle").click();
    await expect(page.locator(`${LIBRARY} th.lens`).first()).toBeVisible();

    expect(
      await standardHeadings(page),
      "the lens moved or replaced a standard column",
    ).toEqual(before);
    await expect(page.locator(`${LIBRARY} th.lens`)).toHaveCount(6);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** And turning it off leaves precisely what was there before. */
  test("turning it off leaves the standard view exactly as it was", async ({
    page,
  }) => {
    await libraryOpen(page);
    const before = await standardHeadings(page);
    const rows = await page.locator(`${LIBRARY} table tbody tr`).count();

    await page.getByTestId("lens-toggle").click();
    await expect(page.locator(`${LIBRARY} th.lens`).first()).toBeVisible();
    await page.getByTestId("lens-toggle").click();

    await expect(page.locator(`${LIBRARY} th.lens`)).toHaveCount(0);
    expect(await standardHeadings(page)).toEqual(before);
    await expect(page.locator(`${LIBRARY} table tbody tr`)).toHaveCount(rows);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A cell with no opinion is blank, not zero.**
   *
   * An empty cell reads as "djmanzo has nothing to say"; a `0.00` reads as
   * "bad". They are different answers, and a lens that could not tell them
   * apart would rank an unanalysed record below a merely unsuitable one while
   * looking like it had considered both.
   */
  test("says nothing where it has nothing to say", async ({ page }) => {
    await libraryOpen(page);
    await page.getByTestId("lens-toggle").click();

    const rows = page.locator(`${LIBRARY} table tbody tr`);
    // The first row has an opinion.
    await expect(rows.first().locator("td.lens").first()).toHaveText("0.82");
    // The second has none, and says so with a blank rather than a zero.
    const second = rows.nth(1).locator("td.lens").first();
    await expect(second).toHaveText("");
    await expect(second, "no opinion was drawn as a bad score").not.toHaveText(
      "0.00",
    );
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The risks are named, because a risk with no reason is one a DJ ignores. */
  test("names what could go wrong rather than scoring it", async ({ page }) => {
    await libraryOpen(page);
    await page.getByTestId("lens-toggle").click();

    const risk = page.locator(`${LIBRARY} tbody tr`).first().locator(".risk");
    await expect(risk).toHaveText("keys");
    await expect(risk).toHaveAttribute("title", "keys clash");
    expect(errorsThrown(page)).toEqual([]);
  });
});
