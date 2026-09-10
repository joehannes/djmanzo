/**
 * §14's behavioural signals, read through §13's rule.
 *
 * §13 is the one that matters and it is about *words* as much as counting:
 *
 * > A DJ raises BPM dramatically once because the crowd suddenly explodes. Do
 * > not conclude "User likes enormous BPM jumps". Instead: "Large jumps
 * > occasionally occur in high-energy contexts."
 *
 * The counting is `dj_app::signals` and is tested there — four occurrences in
 * one phase, never across the night, never without one. What a browser can
 * prove is that the sentence a DJ actually reads keeps §13's shape: it names
 * the part of the night it is about, and it never claims a preference.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

async function openConduct(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Assistant", exact: true }).click();
}

test.describe("what you do, and when", () => {
  /**
   * **Every line says which part of the night it is about.**
   *
   * A gesture without its context is a preference waiting to be learned
   * wrongly, which is the whole of §13 — so a line that dropped the phase
   * would be the failure, not a shorter sentence.
   */
  test("each tendency names the night it was seen in", async ({ page }) => {
    await openConduct(page);
    const list = page.locator(".tendencies li");
    await expect(list).toHaveCount(2);

    await expect(list.first()).toContainText("when the night is at its peak");
    await expect(list.nth(1)).toContainText("when the night is building");
    // And how many times, because a count is the difference between a claim
    // and a guess.
    await expect(list.first()).toContainText("Seen 9 times");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **No line claims a preference.**
   *
   * The directive names the sentence it does not want. This is the guard
   * against it coming back through a well-meaning rewrite of the wording.
   */
  test("nothing on screen says the DJ likes anything", async ({ page }) => {
    await openConduct(page);
    const said = (await page.locator(".tendencies li").allTextContents())
      .join(" ")
      .toLowerCase();

    expect(said.length).toBeGreaterThan(0);
    for (const forbidden of ["likes", "prefers", "always", "never"]) {
      expect(said, `a tendency claims a preference: ${said}`).not.toContain(forbidden);
    }
    // And it does say what it did see.
    expect(said).toContain("sometimes");
    expect(said).toContain("often");
  });
});
