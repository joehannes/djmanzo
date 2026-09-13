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

/**
 * §32's theme packs: the list, and the one whose identity is the metaphor.
 *
 * Which themes there are supposed to be, which ship and which do not is
 * `dj_app::theme`, and is checked there against `packages.ts` in both
 * directions. These say the two things only a browser can.
 */
test.describe("§32's theme packs", () => {
  /**
   * **The load-bearing one: the picker says what djmanzo has not built.**
   *
   * §32's palettes all ship now, and the two rows that remain are the two that
   * are deliberately *not* palettes: High Contrast is an override the
   * stylesheet already applies over every theme, and Minimal is a density §5
   * already fits to the window. Building either as a seventeenth palette would
   * be a second control for something that already has one.
   *
   * The list stays, and the count is asserted rather than the emptiness: a
   * picker that showed only what ships reads as the whole of §32 — a theme that
   * is absent looks exactly like a theme nobody asked for — and the next time
   * §32 grows a name djmanzo has not built, this is where it will say so.
   */
  test("the themes it does not have are on the list, saying why", async ({ page }) => {
    await openThemes(page);

    const absent = page.locator(".switcher .not-yet li");
    await expect(absent).toHaveCount(2);
    // Every row carries its reason. A row that said only "Festival" would be a
    // gap announced and not explained, which reads as an oversight rather than
    // as a decision.
    for (const row of await absent.all()) {
      await expect(row.locator(".why")).not.toBeEmpty();
    }
    await expect(absent.filter({ hasText: "High Contrast" })).toContainText(
      "raises contrast on every theme",
    );
    // And the ones that do ship are not in this list: it is the gap, not the
    // catalogue. By the row's own name rather than by its text, because a
    // reason may mention another theme and matching anywhere in the row would
    // call that a hit.
    for (const shipped of ["Daylight", "Festival", "Stem Lab", "Wedding"]) {
      await expect(
        absent.locator(".name", { hasText: new RegExp(`^${shipped}$`) }),
        `${shipped} ships and is still listed as missing`,
      ).toHaveCount(0);
    }
    await expect(absent.locator(".name", { hasText: /^Minimal$/ })).toHaveCount(1);
    expect(errorsThrown(page), "the theme switcher threw").toEqual([]);
  });

  /**
   * **Choosing Watershed Living opens the watershed, and nothing else does.**
   *
   * §32's second paragraph, both halves of it. The metaphor was a switch beside
   * the themes rather than one of them, which made it a mode — the thing §32
   * says it should stop being. And it must not constrain a DJ who does not want
   * it, so no other theme touches the switch: a DJ who chose Booth keeps their
   * watershed open if it was, and closed if it was not.
   */
  test("the watershed is a theme, and only its own theme opens it", async ({ page }) => {
    await openThemes(page);

    const watershed = page.getByRole("button", { name: "Watershed", exact: true });
    await expect(
      watershed,
      "the watershed was already open before anything was chosen",
    ).toHaveAttribute("aria-pressed", "false");

    // Another theme first: it must leave the switch alone. This is the half of
    // §32 that is a prohibition rather than a feature — the metaphor must not
    // constrain a DJ who did not ask for it.
    await page.locator(".switcher .theme").filter({ hasText: "Booth" }).click();
    await expect(watershed, "choosing Booth opened the watershed").toHaveAttribute(
      "aria-pressed",
      "false",
    );

    // The menu stays open across a choice on purpose — picking a theme and
    // deciding it was wrong is one gesture — so there is nothing to reopen.
    await expect(page.locator(".switcher .menu")).toBeVisible();
    await page
      .locator(".switcher .theme")
      .filter({ hasText: "Watershed Living" })
      .click();
    await expect(
      watershed,
      "choosing Watershed Living did not open the watershed",
    ).toHaveAttribute("aria-pressed", "true");
    expect(errorsThrown(page), "the theme switcher threw").toEqual([]);
  });
});
