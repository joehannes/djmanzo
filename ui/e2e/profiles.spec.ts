/**
 * §81, and the one thing it is about.
 *
 * > Do not store one universal DJ profile. Store conditional profiles.
 *
 * A DJ who plays bachata at weddings and techno at clubs, averaged, is a DJ who
 * plays neither — and the average carries twice the evidence of either real
 * answer, so a system offering it would offer it strongly. Everything below
 * checks the interface keeps them apart, says how much each one rests on, and
 * never guesses which kind of night this is.
 *
 * The arithmetic is Rust's and is tested in `dj_app::profile`. What a browser
 * can prove is that the press reaches Rust, the answer comes back, and the
 * panel does not quietly assemble a stronger claim than djmanzo made.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const NIGHT = '.surface[data-surface="night"]';

async function nightOpen(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Night", exact: true }).click();
  await expect(page.locator(NIGHT)).toBeVisible();
}

test.describe("what kind of night this is", () => {
  /**
   * **djmanzo does not guess.**
   *
   * It reads the arc of a night from the music and is right to — energy and
   * tempo are in the signal. Nothing in the signal says *wedding*. So the
   * panel opens with nothing chosen and says what choosing is for, rather than
   * defaulting to one and learning a whole night's habits under the wrong
   * heading.
   */
  test("opens with no kind chosen, and says why to choose one", async ({
    page,
  }) => {
    await nightOpen(page);

    await expect(page.locator(`${NIGHT} .kind button.on`)).toHaveCount(0);
    await expect(page.getByTestId("night-unsaid")).toContainText(
      "averaging your weddings with your club nights",
    );
    expect(errorsThrown(page), "the night panel threw").toEqual([]);
  });

  /** Every setting §81 lists is on offer, and saying one marks it. */
  test("naming the night marks it, and it stays named", async ({ page }) => {
    await nightOpen(page);

    const kinds = page.locator(`${NIGHT} .kind button`);
    await expect(kinds).toHaveCount(6);

    await kinds.filter({ hasText: /^Wedding$/ }).click();
    await expect(page.locator(`${NIGHT} .kind button.on`)).toHaveText("Wedding");
    await expect(page.getByTestId("night-unsaid")).toHaveCount(0);

    // Past the panel's own refresh, which calls back without a setting. That
    // call must not un-say the answer — the rule `Library::note_night`
    // enforces, and the one a DJ would never think to check.
    await page.waitForTimeout(2500);
    await expect(
      page.locator(`${NIGHT} .kind button.on`),
      "a refresh un-said what kind of night it is",
    ).toHaveText("Wedding");
    expect(errorsThrown(page)).toEqual([]);
  });

  /** And a DJ who picked wrong can say so. */
  test("the kind can be corrected", async ({ page }) => {
    await nightOpen(page);
    const kinds = page.locator(`${NIGHT} .kind button`);
    await kinds.filter({ hasText: /^Club$/ }).click();
    await expect(page.locator(`${NIGHT} .kind button.on`)).toHaveText("Club");
    await kinds.filter({ hasText: /^Wedding$/ }).click();
    await expect(page.locator(`${NIGHT} .kind button.on`)).toHaveText("Wedding");
    expect(errorsThrown(page)).toEqual([]);
  });
});

test.describe("how you play, by the kind of night", () => {
  /**
   * **Two settings are two profiles, drawn as two.**
   *
   * The whole of §81 on screen. If these ever merged into one line the
   * interface would be presenting the average — which describes neither
   * evening and reads more confident than either.
   */
  test("draws a profile per setting rather than one for everything", async ({
    page,
  }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Assistant", exact: true }).click();

    const profiles = page.getByTestId("profiles");
    await expect(profiles.locator("li")).toHaveCount(2);
    await expect(profiles).toContainText("Wedding");
    await expect(profiles).toContainText("Club");
    // And they say different things, which is the point.
    await expect(profiles).toContainText("Bachata");
    await expect(profiles).toContainText("Techno");
    expect(errorsThrown(page), "the conduct panel threw").toEqual([]);
  });

  /**
   * **Each one says how much it rests on.**
   *
   * Three nights and thirty are not the same claim, and a profile that hides
   * the difference is asking to be over-trusted. The sentence comes from Rust
   * with the count already in it, so a panel cannot drop it.
   */
  test("every profile names the evidence behind it", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Assistant", exact: true }).click();

    const profiles = page.getByTestId("profiles");
    await expect(profiles).toContainText("3 nights");
    await expect(profiles).toContainText("4 nights");
    expect(errorsThrown(page)).toEqual([]);
  });
});
