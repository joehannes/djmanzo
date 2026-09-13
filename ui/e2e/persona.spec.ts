/**
 * §80's persona learning, and the half of it that makes the rest safe.
 *
 * > But express learned behavior as editable preferences. The system should
 * > show: Learned preference — and let the user reject/modify it.
 *
 * What djmanzo can claim, from how much evidence, and the rule that a rejected
 * claim is neither acted on nor raised again are `dj_app::persona`, and are
 * tested there against §81's profiles. These say the two things only a browser
 * can.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

async function openPersona(page: Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Assistant", exact: true }).click();
  await expect(page.getByTestId("persona")).toBeVisible();
}

const claim = (page: Page, slug: string) =>
  page.locator(`[data-persona="${slug}"]`);

test.describe("§80's learned preferences", () => {
  /**
   * **The load-bearing one: saying no takes the claim away and keeps it away.**
   *
   * §80 spends half its words on this, and either half alone is a rejection
   * that did not take. A DJ asked the same thing every week has not been heard;
   * a DJ whose "no" is recorded and then ignored has been lied to. The second
   * is the worse one and the invisible one, because the interface would look
   * exactly right.
   */
  test("a claim the DJ refuses is not repeated at them", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openPersona(page);

    const density = claim(page, "density");
    await expect(density).toContainText("You keep the layout at dense.");
    await expect(density).toContainText("Learned preference");

    await density.getByRole("button", { name: "No" }).click();

    await expect(density).toHaveClass(/refused/);
    await expect(density, "the refused claim is still being put to them").not.toContainText(
      "You keep the layout at dense.",
    );
    await expect(density).toContainText("will not raise it again");
    // And the answer is changeable — a rejection is a decision, not a
    // deletion, so the row stays with a way back.
    await expect(density.getByRole("button", { name: "Ask me again" })).toBeVisible();
    expect(thrown).toEqual([]);
  });

  /**
   * **All four rows, in the states they are actually in, each saying which.**
   *
   * §80 names four traits. A list of the ones djmanzo happens to believe today
   * would read as the whole of §80, and a trait that is merely quiet looks
   * exactly like one that does not exist — so a row with nothing behind it says
   * "not enough nights yet" rather than going blank.
   */
  test("a trait with nothing behind it says so, and one agreed to is marked", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openPersona(page);

    await expect(page.getByTestId("persona").locator("li")).toHaveCount(4);

    // §80's fourth, which reads the actions rather than §14's gestures: the
    // gesture vocabulary collapses all four stem verbs into one on purpose, and
    // `DeckAction::Stem` has carried the stem all along.
    await expect(claim(page, "stems-for-vocals")).toContainText("mostly for the vocal");
    await expect(claim(page, "stems-for-vocals")).toContainText("Learned preference");

    // The one there is not enough evidence for yet. "Not yet" is a reason to go
    // and play more nights, which is a different thing from a claim that has
    // been made.
    await expect(claim(page, "blend-length")).toContainText("Not enough nights yet.");

    // And one already agreed to, marked as such rather than looking unanswered.
    await expect(
      claim(page, "automix-at-peak").getByRole("button", { name: "That is right" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(thrown).toEqual([]);
  });
});
