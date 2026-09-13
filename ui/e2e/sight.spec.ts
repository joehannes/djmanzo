/**
 * §40: the assistant is told what is happening — and says what it is not.
 *
 * > The AI context should include: current tracks, play positions, BPM, beat
 * > grids, key, energy, phrase structure, stems, current transitions, cue
 * > points, loop state, FX, mixer state, library, prepared tracks, next
 * > candidates, history, session plan, user preferences, current
 * > venue/occasion, audience context, hardware, assistant posture, session
 * > phase, recent actions, current GUI focus.
 *
 * Which of the twenty-six djmanzo gathers, and the lines the model is handed,
 * are `dj_app::sight` and are tested there against a real capture. What is here
 * is the half only a browser can answer: that both lists reach the screen.
 *
 * The unseen half is the point. An answer that ignored your history reads very
 * differently once you know the thing answering could not see it, and a panel
 * showing only what the assistant knows is a reassurance rather than a fact.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { compositions, errorsThrown, openShell, sightRows } from "./shell";

async function openAssistant(page: Page) {
  await page.getByRole("button", { name: "Assistant", exact: true }).click();
  await expect(page.locator('.surface[data-surface="assistant"]')).toBeVisible();
}

test.describe("§40's context, and its edges", () => {
  /**
   * **The load-bearing one: the panel draws both halves, from Rust's list.**
   *
   * Both, and neither invented here: the names are read out of the same golden
   * file `dj_app::sight::ALL` generates, so "the assistant cannot see your
   * history" is a claim about the application rather than about this file.
   */
  test("the assistant says what it can see and what it cannot", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await openAssistant(page);

    const fold = page.locator(".sight");
    await expect(fold).toBeVisible();
    await fold.locator("summary").click();

    const told = sightRows.filter((row) => row.told);
    const blind = sightRows.filter((row) => !row.told);
    expect(
      told.length > 0 && blind.length > 0,
      "the fixture has only one half of §40's list, so this test cannot tell the " +
        "two apart",
    ).toBe(true);

    await expect(fold.locator(".seen li")).toHaveCount(told.length);
    await expect(fold.locator(".blind li")).toHaveCount(blind.length);

    // Every name, in both halves — not a count, which a panel drawing the same
    // row twenty-six times would also satisfy.
    for (const row of sightRows) {
      const half = row.told ? ".seen" : ".blind";
      await expect(
        fold.locator(`${half} .what`).filter({ hasText: row.name }).first(),
        `§40's \`${row.name}\` is missing from the ${row.told ? "seen" : "unseen"} half`,
      ).toBeVisible();
    }
    expect(thrown).toEqual([]);
  });

  /**
   * **And an absence carries its reason.**
   *
   * A list of eleven things the assistant cannot see, with no reason beside
   * any of them, is a list of complaints. The reason is what makes it
   * actionable — some of them are a camera away and some are a design choice,
   * and those are different facts about an answer.
   */
  test("everything the assistant cannot see says why not", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await openAssistant(page);
    await page.locator(".sight summary").click();

    const reasons = await page.locator(".sight .blind .why").allTextContents();
    expect(reasons.length).toBeGreaterThan(0);
    for (const reason of reasons) {
      expect(
        reason.trim().length,
        `an absence is on screen with "${reason}" beside it, which is a label`,
      ).toBeGreaterThan(30);
    }
    expect(thrown).toEqual([]);
  });

  /**
   * **The fold is closed until it is opened.**
   *
   * §18: the panel a DJ opens to ask a question should not first hand them
   * twenty-six rows of inventory. This is the thing you open when an answer
   * surprised you.
   */
  test("the list is folded away until it is wanted", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, { layout_tree: compositions.Starter });
    await openAssistant(page);

    await expect(page.locator(".sight")).toBeVisible();
    await expect(page.locator(".sight .seen li").first()).toBeHidden();
    expect(thrown).toEqual([]);
  });
});
