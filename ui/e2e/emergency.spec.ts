/**
 * §47's emergency control.
 *
 * > When something goes wrong, the user must not search through menus.
 *
 * The gesture already existed — `assistant_take_over` — inside the Conduct
 * panel, which is a panel you have to open. What this measures is the part the
 * section is actually about: that it is in the top bar beside REC and Mark,
 * that one press reaches djmanzo, and that what it stopped is *said* rather
 * than flashed and forgotten.
 *
 * What it deliberately does not do — touch the audio — is a Rust test, because
 * that is where the faders are.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

test.describe("the emergency control", () => {
  test("is in the top bar, not inside a panel", async ({ page }) => {
    await openShell(page, "/");

    const control = page.getByRole("button", { name: "Take over", exact: true });
    await expect(control).toBeVisible();
    // In the header, which is the whole claim: no panel was opened to get here.
    const inHeader = await page.evaluate(
      () =>
        !!document
          .querySelector("header")
          ?.querySelector("button.takeover"),
    );
    expect(inHeader, "the emergency control is not in the top bar").toBe(true);
  });

  test("one press reaches djmanzo and says what it stopped", async ({ page }) => {
    await openShell(page, "/");

    await page.evaluate(() => {
      (window as unknown as Record<string, unknown>).__asked = [];
    });

    await page.getByRole("button", { name: "Take over", exact: true }).click();

    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __asked: string[] }).__asked))
      .toContain("take_over_everything");

    // The words djmanzo answered with, not a fixed sentence written here.
    const notice = page.locator(".notice");
    await expect(notice).toContainText("the automix");
    await expect(notice).toContainText("autopilot");
    // And it says the audio was left alone, because that is the design
    // decision a DJ needs to trust before pressing it a second time.
    await expect(notice).toContainText("audio");
  });

  /**
   * Pressed twice in quick succession, the second message has to survive.
   *
   * The first press's dismissal timer used to clear the *second* message when
   * it fired, so the control a DJ presses when they are already in trouble
   * would appear to have done nothing the one time they pressed it twice.
   * Found by pressing it twice while driving the application.
   */
  test("a second press does not have its answer wiped by the first", async ({
    page,
  }) => {
    // Installed *before* the first press, so both dismissal timers are fake
    // ones this test controls. Installed after it, the first press's timer is a
    // real one that `runFor` cannot advance — which is how the first version of
    // this test passed against the bug it was written for.
    await page.clock.install();
    await openShell(page, "/");
    const control = page.getByRole("button", { name: "Take over", exact: true });

    await control.click();
    await expect(page.locator(".notice")).toBeVisible();

    // Four seconds later, inside the first notice's six.
    await page.clock.runFor(4_000);
    await control.click();
    await expect(page.locator(".notice")).toBeVisible();

    // Now past six seconds since the *first* press, and three since the second.
    await page.clock.runFor(3_000);
    await expect(
      page.locator(".notice"),
      "the first press's timer cleared the second press's answer",
    ).toBeVisible();
  });

  test("pressing it throws nothing", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Take over", exact: true }).click();
    expect(errorsThrown(page)).toEqual([]);
  });
});
