/**
 * What djmanzo makes of the night.
 *
 * §11 asks for a context engine: one place that works out what the session is
 * doing, so the interface, the theme, the suggester and the assistant read the
 * same answer instead of each forming a private opinion. The engine and its
 * arithmetic are Rust's and are tested there. What a browser can prove is that
 * the answer reaches a DJ *with its reasoning*, and that the panel says
 * plainly when there is no answer yet.
 *
 * The second half is the one worth a test. `SessionRead` was once defaulted to
 * *Peak at 0.95*, so the interface announced peak time thirty seconds into a
 * warm-up — a claim nothing had made and nothing could check. A panel that
 * quietly drew an empty phase would be the same bug wearing a different face.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const ASSISTANT = '.surface[data-surface="assistant"]';

test.describe("the night", () => {
  test("says what the set is doing, and what it read that off", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Assistant", exact: true }).click();

    const night = page.locator(`${ASSISTANT} .night`);
    await expect(night.locator(".phase")).toHaveText("Peak");
    await expect(
      night.locator(".how-sure"),
      "it does not say how much is behind the reading. A DJ told 'peak' off " +
        "five records should be told it is five",
    ).toContainText("5 records");
    await expect(night.locator(".why li").first()).toContainText("ceiling");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A night with no shape yet says so.**
   *
   * This is the state every set starts in, so it is the message a DJ sees
   * most often — and the one place the interface could most easily start
   * guessing.
   */
  test("says plainly when the night has no shape yet", async ({ page }) => {
    await openShell(page, "/", {}, { session_read: null });
    await page.getByRole("button", { name: "Assistant", exact: true }).click();

    await expect(page.locator(`${ASSISTANT} .night`)).toHaveCount(0);
    await expect(
      page.locator(`${ASSISTANT} .hint`).filter({ hasText: "Not enough of the night" }),
    ).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });
});
