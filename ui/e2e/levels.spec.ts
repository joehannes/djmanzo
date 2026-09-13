/**
 * §8's adaptation levels, as the one axis §8 asks for.
 *
 * > The application needs multiple layers of adaptive behavior. Level 0 —
 * > Static … Level 6 — Autopilot.
 *
 * Which levels there are, what each one sets and what counts as having drifted
 * from one are `dj_app::level`, and are tested there against the tables that own
 * the posture and §78's freedoms. These say the two things only a browser can.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

async function openLevels(page: Page) {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.locator(".levels")).toBeVisible();
}

const step = (page: Page, slug: string) =>
  page.locator(`.levels li[data-level="${slug}"]`);

test.describe("§8's adaptation levels", () => {
  /**
   * **The load-bearing one: one press reaches the locks below it.**
   *
   * §8's whole point is that this is one decision with a range, and the proof a
   * browser can give is that pressing the axis moves the six switches a DJ
   * previously had to find one at a time in another block. A level that set the
   * posture and left the locks alone would look like it worked and do half its
   * job — and the half that did not happen is the half a DJ was worried about.
   */
  test("moving the axis moves the six locks under it", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await openLevels(page);

    await expect(page.locator(".levels li")).toHaveCount(7);
    const ticked = page.locator(".locks .lock-list input:checked");
    await expect(ticked, "the shell opened with locks already on").toHaveCount(0);

    await step(page, "prepare").getByRole("button").click();
    await expect(step(page, "prepare").getByRole("button")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    // Every one of §79's six, because Prepare is below the level at which §8
    // says the interface itself may change.
    await expect(ticked).toHaveCount(6);

    // And the top of the axis gives them back, so this is a control rather than
    // a ratchet.
    await step(page, "autopilot").getByRole("button").click();
    await expect(ticked).toHaveCount(0);
    expect(thrown).toEqual([]);
  });

  /**
   * **Never chosen is not Level 0, and the panel says which.**
   *
   * djmanzo's shipped behaviour is not Static — it suggests, it fits the density
   * to the window, it adapts the theme — so a fresh install showing Static
   * selected would be describing itself wrongly, and every departure it then
   * listed would be djmanzo telling a DJ they had drifted from a decision they
   * never made.
   */
  test("a DJ who never chose is told so rather than shown Static", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await openLevels(page);

    await expect(page.getByTestId("level-unset")).toBeVisible();
    await expect(page.locator(".levels button[aria-pressed='true']")).toHaveCount(0);

    // And choosing one ends that state rather than adding to it.
    await step(page, "suggest").getByRole("button").click();
    await expect(page.getByTestId("level-unset")).toHaveCount(0);
    expect(thrown).toEqual([]);
  });
});
