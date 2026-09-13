/**
 * §20's performance table, and the *instant custom column configuration* it
 * asks for.
 *
 * > Compact, information-dense. Recommended columns: […] Allow instant custom
 * > column configuration.
 *
 * The browser drew a fixed six — title, artist, album, BPM, key, time — which is
 * a spreadsheet, and §20 opens by asking for a DJ-native library surface rather
 * than one. The gap was never the data: a library row already carried the year,
 * the loudness, the phrase length, the rating, the play count and when it was
 * last played, and every one of them was read off the disk and thrown away.
 *
 * Which columns exist, what each is called and what happens to a nonsense
 * choice is `dj_app::columns` and is tested there. These say the two things
 * only a browser can: that ticking one puts the column on the table with the
 * right reading in it, and that unticking takes it away — and that the one
 * column a table cannot do without survives being unticked.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const TABLE = ".table-scroll table";
const HEAD = (slug: string) => `${TABLE} th[data-column="${slug}"]`;
const CELL = (slug: string) => `${TABLE} td[data-column="${slug}"]`;

async function openColumns(page: Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Browse", exact: true }).click();
  await expect(page.locator(TABLE)).toBeVisible();
  await page.getByTestId("columns-toggle").click();
  await expect(page.locator(".column-picker")).toBeVisible();
}

const tick = (page: Page, slug: string) =>
  page.locator(`.column-picker label[data-column="${slug}"] input`);

test.describe("§20's columns", () => {
  /**
   * **The load-bearing one: ticking a column puts it on the table, with its
   * reading in it.**
   *
   * Both halves, and the second is the one that catches the real defect. A
   * header that appears over a column of blanks is what a table gets when the
   * picker and the cells are two lists — which is exactly how this was built
   * before, and why the header and the cells are now one `{#each}`.
   */
  test("ticking a column adds it to the table with the record's own reading", async ({
    page,
  }) => {
    await openColumns(page);
    await expect(
      page.locator(HEAD("year")),
      "a fresh install already shows every column, so there is nothing for the " +
        "picker to add",
    ).toHaveCount(0);

    await tick(page, "year").check();

    await expect(page.locator(HEAD("year"))).toHaveCount(1);
    await expect(
      page.locator(CELL("year")).first(),
      "the column appeared over a blank -- the picker and the cells are two " +
        "lists again",
    ).toHaveText("1990");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * And unticking takes it away.
   *
   * Worth its own test because the obvious implementation of the picker — a set
   * that is only ever added to — passes the one above and fails this.
   */
  test("unticking a column takes it off the table", async ({ page }) => {
    await openColumns(page);
    await expect(page.locator(HEAD("album"))).toHaveCount(1);

    await tick(page, "album").uncheck();
    await expect(page.locator(HEAD("album"))).toHaveCount(0);
    await expect(
      page.locator(HEAD("title")),
      "unticking one column took the others with it",
    ).toHaveCount(1);
  });

  /**
   * **§20's energy is a different column from its loudness, and reads
   * differently.**
   *
   * The two were one column under the wrong name for the whole life of the
   * library: djmanzo measured integrated LUFS and the picker called it
   * loudness precisely because nothing had measured energy yet. Now both are
   * offered, and the test that matters is that the second is not the first
   * wearing a new header — so the fixture's quietest record is its most
   * energetic one, and a cell reading the wrong field would say so.
   */
  test("energy and loudness are two columns that disagree", async ({ page }) => {
    await openColumns(page);

    // Both, because the claim is about the two of them side by side and a
    // fresh install shows neither.
    await tick(page, "energy").check();
    await tick(page, "loudness").check();
    await expect(page.locator(HEAD("energy"))).toHaveCount(1);
    await expect(page.locator(HEAD("loudness"))).toHaveCount(1);

    const energies = await page.locator(CELL("energy")).allTextContents();
    const louds = await page.locator(CELL("loudness")).allTextContents();
    expect(energies.length).toBeGreaterThan(1);
    expect(energies.length).toBe(louds.length);

    // Out of a hundred, not a fraction: "72" is read faster than "0.72" down a
    // column of a hundred rows.
    for (const value of energies) {
      expect(value, `energy read "${value}"`).toMatch(/^\d{1,3}$/);
    }

    // The fixture's own numbers, which is the assertion that pins the *field*.
    // The ordering check below is weaker than it looks -- a cell rendering any
    // decreasing function of loudness reverses the ranking and passes it, as a
    // mutation showed -- so the values are checked too.
    expect(
      energies.slice().sort(),
      "the energy cells are not `energy` out of a hundred",
    ).toEqual(["31", "88"]);

    // And the orders disagree, which is the claim about the two columns rather
    // than about one cell.
    const order = (values: string[]) =>
      values
        .map((value, index) => [Number(value), index] as const)
        .sort((a, b) => b[0] - a[0])
        .map(([, index]) => index);
    expect(
      order(energies),
      "the energy column ranks the records exactly as loudness does, so it is " +
        "loudness with a new header",
    ).not.toEqual(order(louds));
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * §20's readings, drawn as readings rather than as numbers.
   *
   * A rating is stars because it is read at a glance and "4" looks like a
   * measurement; a last-played date is an age because the question this column
   * is actually asked is *was this in last weekend's set*. Both are decisions
   * about pixels, which is why they are here and not in Rust.
   */
  test("a rating reads as stars and a last-played reads as an age", async ({
    page,
  }) => {
    await openColumns(page);
    await tick(page, "rating").check();
    await tick(page, "last-played").check();

    // The second fixture record: played, rated five.
    const rated = page.locator(`${TABLE} tbody tr`).nth(1);
    await expect(rated.locator(`td[data-column="rating"]`)).toHaveText("\u2605".repeat(5));
    await expect(
      rated.locator(`td[data-column="last-played"]`),
      "a date rather than an age, which is a number to read instead of an answer",
    ).not.toHaveText("");

    // The first has neither, and says so with a blank rather than a zero.
    const never = page.locator(`${TABLE} tbody tr`).first();
    await expect(
      never.locator(`td[data-column="rating"]`),
      "an unrated record was drawn as zero stars, which reads as a judgement " +
        "rather than as an absence",
    ).toHaveText("");
    await expect(never.locator(`td[data-column="last-played"]`)).toHaveText("");
  });

  /**
   * The title cannot be unticked, and the picker says so rather than fighting.
   *
   * The one column that is not a preference: a table without it is a list of
   * BPMs with no records attached, and the only way back is a picker whose rows
   * the DJ can no longer tell apart. Rust puts it back whatever is asked — a
   * preferences file or another client cannot drop it either — and the box is
   * disabled so a DJ is told the rule instead of watching a checkbox re-tick
   * itself and wondering whether the picker works.
   */
  test("the title cannot be taken off the table, and the picker says so", async ({
    page,
  }) => {
    await openColumns(page);
    await expect(
      tick(page, "title"),
      "a checkbox that silently re-ticks itself reads as a broken picker",
    ).toBeDisabled();
    await expect(tick(page, "title")).toBeChecked();

    await expect(page.locator(HEAD("title"))).toHaveCount(1);
    await expect(page.locator(`${TABLE} td.title`).first()).toContainText("Bachata Rosa");
  });

  /**
   * The picker is only over the table.
   *
   * The cards view has no columns, and a control that does nothing where it is
   * drawn is worse than one that is absent.
   */
  test("the cards view offers no column picker", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await page.getByRole("button", { name: "Cards", exact: true }).click();
    await expect(page.getByTestId("columns-toggle")).toHaveCount(0);
  });
});
