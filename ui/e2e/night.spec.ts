/**
 * The night, as the interface shows it.
 *
 * §11 asks for a context engine underneath the interface rather than a phase
 * each panel works out for itself. The engine and its arithmetic are Rust's
 * and are tested there — `dj_core::context` for the reading, `dj_app::night`
 * for what it is fed. What a browser can prove, and a type-check cannot, is
 * that the surface opens, that it says something rather than nothing when the
 * night has not been read, and that a disagreement between what the DJ
 * declared and what the music has done actually reaches the screen.
 *
 * The last one is the load-bearing case. A panel that renders the phase and
 * quietly drops the disagreement would look completely correct and would be
 * hiding the one thing this feature exists to say.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const NIGHT = '.surface[data-surface="night"]';

/** Open the shell with the night docked, optionally with a given reading. */
async function nightOpen(
  page: import("@playwright/test").Page,
  read?: Record<string, unknown>,
) {
  await openShell(page, "/", {}, read ? { night_read: read } : {});
  await page.getByRole("button", { name: "Night", exact: true }).click();
  await expect(page.locator(NIGHT)).toBeVisible();
}

/** A night the DJ set to peak while the music has been getting softer. */
const DISPUTED = {
  phase: "peak",
  words: "at its peak",
  energy: 0.32,
  certainty: "unsure",
  certainty_about: "One source, and something disagrees with it.",
  basis: "disputed",
  drift: "cooler",
  time_of_day: "small_hours",
  declared: "peak",
  measured: "cooldown",
  readings: 140,
  still_needed: 0,
  notes: [
    "You have said tonight is at its peak, and the music has been softer than that.",
    "Nothing will be mixed unasked while the two disagree.",
  ],
  warrant: "stage",
};

test.describe("the night", () => {
  /**
   * Before anything has read the night there is no phase, and the honest
   * answer is a sentence saying so plus the count of what has been heard.
   * An empty panel reads as broken, which is the failure this asserts against.
   */
  test("opens saying nothing has read the night, rather than empty", async ({
    page,
  }) => {
    await nightOpen(page);

    await expect(page.locator(`${NIGHT} .says`)).toContainText(
      "Nothing has read the night yet",
    );
    await expect(page.locator(`${NIGHT} .waiting`)).toContainText("readings");
    // No phase has been read, so nothing in the arc may claim to be current.
    await expect(page.locator(`${NIGHT} .arc li.here`)).toHaveCount(0);
    expect(errorsThrown(page), "the night threw while rendering").toEqual([]);
  });

  /**
   * The whole arc is on screen, in the order a night goes through it, so the
   * phase is read as a position rather than as a word with no scale.
   */
  /**
   * The attention budget reaches the stylesheet.
   *
   * §18's rule — that the interface may not move while somebody is reaching
   * for it — is only worth anything if the level djmanzo derives actually
   * governs something. `cockpit::Attention` decides it in Rust and is tested
   * there; what a browser can prove is that the answer arrives and is written
   * where the stylesheet can read it.
   */
  test("the attention budget reaches the stylesheet", async ({ page }) => {
    await nightOpen(page);

    // The fixture is one record playing with nothing wrong: room to think.
    await expect(page.locator("html")).toHaveAttribute("data-motion", "normal");
  });

  test("draws the whole arc, in order", async ({ page }) => {
    await nightOpen(page);

    const steps = page.locator(`${NIGHT} .arc li`);
    await expect(steps).toHaveCount(5);
    await expect(steps.first()).toHaveText("Warm-up");
    await expect(steps.nth(2)).toHaveText("Peak");
    await expect(steps.last()).toHaveText("Winding down");
  });

  /**
   * **The disagreement reaches the screen.**
   *
   * Where the DJ's occasion and the night's evidence disagree, djmanzo keeps
   * the declaration and says which way its evidence points — and steps down
   * from mixing unasked. All three have to be visible, because the value of
   * this feature is entirely in the DJ being told.
   */
  test("says when the music disagrees with the night you set up", async ({
    page,
  }) => {
    await nightOpen(page, DISPUTED);

    await expect(page.locator(`${NIGHT} .says`)).toContainText(
      "softer than that",
    );
    // The declaration is what the arc marks: the DJ has not been overruled.
    await expect(page.locator(`${NIGHT} .arc li.here`)).toHaveText("Peak");
    // And djmanzo's own reading is marked beside it rather than swallowed.
    await expect(page.locator(`${NIGHT} .arc li.reads`)).toHaveText(
      "Coming down",
    );
    await expect(page.locator(`${NIGHT} .facts`)).toContainText(
      "your occasion, against the music",
    );
    await expect(page.locator(`${NIGHT} .certainty`)).toContainText("unsure");
    // And what that costs the assistant, which is §9's rule as a consequence.
    await expect(page.locator(`${NIGHT} .warrant`)).toContainText(
      "not touching anything the room hears",
    );
    expect(errorsThrown(page), "the night threw while rendering").toEqual([]);
  });
});
