/**
 * §72's user override matrix.
 *
 * "Create an explicit matrix specifying what AI may do at every posture." It
 * was four booleans on `Posture` before this, and `may_act` in particular
 * answered one question for four different powers — nudging an EQ, riding the
 * pitch fader, switching sync on and pulling the crossfader are not the same
 * permission.
 *
 * What a browser can measure is the part that makes it *explicit*: that the
 * table is drawn, that the DJ's current posture is marked in it, and that a
 * capability djmanzo does not have yet says so instead of showing an empty
 * column that looks like a setting. The rules themselves are Rust's and are
 * tested there — including that a quieter posture is never permitted more.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** The Conduct panel lives inside the Assistant surface. */
const CONDUCT = '.surface[data-surface="assistant"]';

async function conductOpen(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Assistant", exact: true }).click();
  await expect(page.locator(CONDUCT)).toBeVisible();
}

test.describe("the override matrix", () => {
  test("is drawn, one row per thing the assistant might do", async ({ page }) => {
    await conductOpen(page);

    const table = page.locator(`${CONDUCT} table.authority`);
    await expect(table).toBeVisible();
    // §72's ten rows.
    await expect(table.locator("tbody tr")).toHaveCount(10);
    await expect(table.locator('thead th[scope="col"]')).toHaveCount(6);
  });

  /**
   * A DJ deciding whether to turn the dial up is asking *what changes*. Marking
   * the current column rather than filtering to it is what lets the table
   * answer that.
   */
  test("marks the posture the DJ is on without hiding the others", async ({
    page,
  }) => {
    await conductOpen(page);

    const marked = page.locator(`${CONDUCT} table.authority thead th.now`);
    await expect(marked).toHaveCount(1);
    await expect(marked).toHaveText("suggest");
    // And the louder postures are still on screen.
    await expect(
      page.locator(`${CONDUCT} table.authority thead th`, { hasText: "autopilot" }),
    ).toBeVisible();
  });

  /**
   * Three of §72's rows describe powers nothing in djmanzo exercises. A row of
   * empty cells that looked like a *choice* would be worse than a row that
   * says so.
   */
  test("a capability djmanzo does not have says so", async ({ page }) => {
    await conductOpen(page);

    const unbuilt = page.locator(`${CONDUCT} table.authority tr.unbuilt`);
    await expect(unbuilt).toHaveCount(3);
    await expect(unbuilt.first().locator(".soon")).toHaveText("not yet");
    // And nothing is ticked on those rows.
    await expect(unbuilt.locator("td.yes")).toHaveCount(0);
    await expect(unbuilt.locator("td.limited")).toHaveCount(0);
  });

  /**
   * A panel that quietly omits the permissions grid looks exactly like a build
   * that never had one, which is how a broken read stays unnoticed. Stale or
   * invented cells would be worse than none — the value of this table is that
   * it is what djmanzo will actually do — so the failure is said instead.
   */
  test("says so when it cannot read what the assistant may do", async ({ page }) => {
    await openShell(page, "/", {}, { authority_matrix: null });
    await page.getByRole("button", { name: "Assistant", exact: true }).click();
    await expect(page.locator(CONDUCT)).toBeVisible();

    await expect(page.locator(`${CONDUCT} table.authority`)).toHaveCount(0);
    await expect(page.locator(CONDUCT)).toContainText("Could not read what the assistant");
  });

  test("drawing it throws nothing", async ({ page }) => {
    await conductOpen(page);
    expect(errorsThrown(page)).toEqual([]);
  });
});
