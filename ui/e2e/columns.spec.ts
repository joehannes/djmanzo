/**
 * §20's performance table, and the "instant custom column configuration" it
 * asks for.
 *
 * The section lists twenty recommended columns; the browser drew six and there
 * was no way to change them. What is checkable here is the configuration
 * itself — that a column can be turned on and appears, that the choice
 * survives a restart, and that the headings and the cells cannot disagree,
 * because a table whose header says BPM over a column of keys is worse than
 * one with fewer columns.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LIBRARY = '.surface[data-surface="library"]';

async function browse(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Browse", exact: true }).click();
  await expect(page.locator(`${LIBRARY} table`)).toBeVisible();
}

/** The headings, in order, without the checkbox and Load columns. */
async function headings(page: import("@playwright/test").Page) {
  return page.locator(`${LIBRARY} thead th button.sort`).allInnerTexts();
}

test.describe("the performance table's columns", () => {
  test("opens on the six that fit", async ({ page }) => {
    await browse(page);
    expect((await headings(page)).map((h) => h.replace(/[▲▼]/g, "").trim())).toEqual([
      "Title",
      "Artist",
      "Album",
      "BPM",
      "Camelot",
      "Time",
    ]);
  });

  /**
   * The one that matters: a column turned on shows the record's own value, not
   * a blank or the column beside it. Play count is a good witness — the
   * fixture's record has been played three times, which is a number nothing
   * else on the row carries.
   */
  test("a column turned on draws what the record actually holds", async ({ page }) => {
    await browse(page);
    await page.getByRole("button", { name: "Which columns to show" }).click();
    await page.getByRole("checkbox", { name: "Plays" }).check();

    const headers = (await headings(page)).map((h) => h.replace(/[▲▼]/g, "").trim());
    expect(headers).toContain("Plays");

    const column = headers.indexOf("Plays");
    // +1 for the select checkbox, which has no heading.
    const cell = page.locator(`${LIBRARY} tbody tr td`).nth(column + 1);
    await expect(cell, "the Plays column does not hold the play count").toHaveText("3");
  });

  /**
   * **The headings and the cells come from one list**, so they cannot drift
   * apart. Checked by turning on every column and asserting each heading's
   * cell holds what that column means — the failure this guards is a table
   * whose header says BPM over a column of keys.
   */
  test("every heading sits over its own values", async ({ page }) => {
    await browse(page);
    await page.getByRole("button", { name: "Which columns to show" }).click();
    for (const name of ["Genre", "Year", "Key", "Energy", "Rating", "Plays", "Phrase"]) {
      await page.getByRole("checkbox", { name, exact: true }).check();
    }

    const headers = (await headings(page)).map((h) => h.replace(/[▲▼]/g, "").trim());
    const cells = await page.locator(`${LIBRARY} tbody tr td`).allInnerTexts();
    const at = (heading: string) => cells[headers.indexOf(heading) + 1].trim();

    expect(at("Title")).toContain("Bachata Rosa");
    expect(at("Artist")).toBe("Juan Luis Guerra");
    expect(at("Genre")).toBe("Bachata");
    expect(at("Year")).toBe("1990");
    expect(at("BPM")).toBe("124.0");
    expect(at("Camelot")).toBe("8A");
    expect(at("Key"), "the notation column is holding the Camelot one").toBe("Am");
    expect(at("Energy")).toBe("-9.4 LU");
    expect(at("Rating")).toBe("★★★★");
    expect(at("Plays")).toBe("3");
    expect(at("Phrase")).toBe("32");
  });

  /** A working preference, like the view itself. */
  test("the chosen columns survive a restart", async ({ page }) => {
    await browse(page);
    await page.getByRole("button", { name: "Which columns to show" }).click();
    await page.getByRole("checkbox", { name: "Genre" }).check();

    await page.reload();
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    expect((await headings(page)).map((h) => h.replace(/[▲▼]/g, "").trim())).toContain("Genre");
  });

  /**
   * A table of nothing but checkboxes and load buttons is a state to refuse
   * rather than to store, so the last column stays.
   */
  test("the last column cannot be turned off", async ({ page }) => {
    await browse(page);
    await page.getByRole("button", { name: "Which columns to show" }).click();
    for (const name of ["Artist", "Album", "BPM", "Camelot", "Time"]) {
      await page.getByRole("checkbox", { name, exact: true }).uncheck();
    }
    await page.getByRole("checkbox", { name: "Title", exact: true }).uncheck();

    expect(await headings(page)).toHaveLength(1);
    expect(errorsThrown(page)).toEqual([]);
  });
});
