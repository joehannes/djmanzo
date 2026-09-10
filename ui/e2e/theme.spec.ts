/**
 * §31's theme adaptation, and the sentence that governs it.
 *
 * > But use slow adaptation. Never allow the interface to flicker from color
 * > to color every time the track changes.
 *
 * The brakes themselves are Rust's and are tested in `dj_app::mood` — a
 * four-minute minimum, a forty-second settling time, a lock that beats both.
 * What a browser can prove is the other half: that a DJ choosing a theme tells
 * djmanzo to stop deciding, that the lock is read from djmanzo rather than
 * from whatever this component last saw, and that the fade is applied rather
 * than the colours cutting.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Which commands djmanzo was asked for, in order. */
async function called(page: import("@playwright/test").Page) {
  return page.evaluate(
    () => (window as unknown as { __asked?: string[] }).__asked ?? [],
  );
}

async function openThemes(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.locator(".switcher button.icon").click();
  await expect(page.locator(".switcher .menu")).toBeVisible();
}

test.describe("theme adaptation", () => {
  /**
   * **A DJ choosing a theme tells djmanzo to stop deciding.**
   *
   * Otherwise the deliberate choice is quietly replaced four minutes later,
   * which is worse than never adapting: the DJ picked, watched it take, and
   * then watched it be undone by something that gave no reason.
   */
  test("choosing a theme tells djmanzo the DJ decided", async ({ page }) => {
    await openThemes(page);

    await page.locator(".switcher .theme").first().click();
    await expect
      .poll(() => called(page))
      .toContain("theme_chosen");
    expect(errorsThrown(page), "the theme switcher threw").toEqual([]);
  });

  /**
   * **The lock is djmanzo's, and is read back rather than assumed.**
   *
   * It has to survive the menu closing — a DJ who locked the theme an hour ago
   * should still find it locked — so the button shows what djmanzo says, not
   * what this component last did.
   */
  test("the lock is read from djmanzo when the menu opens", async ({ page }) => {
    await openShell(page, "/", {}, {
      theme_now: { theme: "pkg-organic", over_ms: 0, locked: true },
    });
    await page.locator(".switcher button.icon").click();

    await expect(
      page.locator('.switcher [title^="Locked"]'),
      "the menu opened without asking djmanzo whether it was locked",
    ).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /** And pressing it tells djmanzo, rather than only changing a local flag. */
  test("locking reaches djmanzo", async ({ page }) => {
    await openThemes(page);
    await page.locator('.switcher [title="Lock this theme"]').click();

    await expect.poll(() => called(page)).toContain("theme_lock");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The change is a fade, not a cut.**
   *
   * §31 lists smoothing among its brakes. The duration comes from Rust with
   * the theme — so the interface cannot fade for a different length than
   * djmanzo intended — and is nought when a DJ chose from the menu, because
   * they have already decided and should not have to watch.
   *
   * What a browser can check is the wiring: that the ground actually honours
   * the property rather than the property being set and read by nothing.
   */
  test("the ground fades over the duration djmanzo sets", async ({ page }) => {
    await openShell(page, "/");

    // Nothing changing is the usual state, and it must not fade.
    const resting = await page.evaluate(
      () => getComputedStyle(document.body).transitionDuration,
    );
    expect(resting, "the interface fades when nothing is changing").toContain("0s");

    await page.evaluate(() =>
      document.documentElement.style.setProperty("--theme-fade", "3000ms"),
    );
    const fading = await page.evaluate(
      () => getComputedStyle(document.body).transitionDuration,
    );
    expect(fading, "the fade duration reaches nothing").toContain("3s");
    expect(errorsThrown(page)).toEqual([]);
  });
});
