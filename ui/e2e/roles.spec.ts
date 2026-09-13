/**
 * §30's semantic roles, where a DJ can actually see them.
 *
 * > Color must communicate meaning. Do NOT produce a neon application where
 * > everything is colorful and nothing is semantically distinct.
 *
 * The roles have existed since §30 and almost nothing asked for them. Every
 * control in the interface painted its "this is on" state with `--accent` or
 * `--accent-2` directly, which renders identically today — `--active` *is* the
 * accent — and throws the meaning away. `Role::must_differ_from` names
 * **selected against active** as a pair that has to stay distinguishable, and
 * in thirty places they were one colour.
 *
 * Which roles exist, which pairs must differ and whether any component still
 * names a hue instead of a role is Rust's and vitest's, and is tested there.
 * This says the one thing only a browser can: that the two roles reach the
 * screen as two different colours, on two controls that sit side by side.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

test.describe("§30's roles on screen", () => {
  /**
   * **The load-bearing one: a thing you chose and a thing that is running do
   * not look the same.**
   *
   * The browser's view switch and the AI lens beside it, which shared one CSS
   * rule until the roles were applied — so turning the lens on looked exactly
   * like picking a third view, and §76 is explicit that the lens is not a
   * view. Computed colours rather than class names, because the claim is about
   * what a DJ sees: a rule that named the right role and resolved to the same
   * hue would pass a class-name test and fail a DJ.
   */
  test("a chosen control and a running one are different colours", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();

    const chosen = page.getByRole("button", { name: "Table", exact: true });
    const lens = page.getByTestId("lens-toggle");
    await expect(chosen).toHaveAttribute("aria-pressed", "true");
    await lens.click();
    await expect(lens).toHaveAttribute("aria-pressed", "true");

    const fill = (target: typeof chosen) =>
      target.evaluate((el) => getComputedStyle(el).backgroundColor);
    const picked = await fill(chosen);
    const running = await fill(lens);

    expect(picked, "the chosen view is not filled at all").not.toBe(
      "rgba(0, 0, 0, 0)",
    );
    expect(running, "the running lens is not filled at all").not.toBe(
      "rgba(0, 0, 0, 0)",
    );
    expect(
      running,
      "a control the DJ picked and a mode that is running are one colour, " +
        "which is the pair §30 names as one that must stay distinguishable",
    ).not.toBe(picked);
    expect(thrown).toEqual([]);
  });

  /**
   * **And the text on a filled state is readable on the fill it is on.**
   *
   * The bug the sweep found: every filled "on" button paired its background
   * with `--on-accent`, including the ones filled with `--accent-2`. It read
   * well on all seven palettes by luck rather than by construction. `--active`
   * and `--selected` have partners now, and this checks that the pairing
   * reaches the page rather than only the stylesheet — a token defined and
   * never resolved is the failure `theme-tokens.test.ts` exists for, one level
   * up.
   */
  test("a filled state has text that resolves against its fill", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();

    const chosen = page.getByRole("button", { name: "Table", exact: true });
    const { background, text } = await chosen.evaluate((el) => {
      const style = getComputedStyle(el);
      return { background: style.backgroundColor, text: style.color };
    });
    expect(background).not.toBe("rgba(0, 0, 0, 0)");
    expect(
      text,
      "the text on a filled control is the same colour as the fill",
    ).not.toBe(background);
    expect(thrown).toEqual([]);
  });
});
