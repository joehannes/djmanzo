/**
 * §74's contextual control rail: the four to eight controls that matter now.
 *
 * Called "At hand" rather than "the rail" because djmanzo already has a rail —
 * §22's **Next** rail, in `rail.spec.ts`, which is about which record comes
 * next rather than which controls are under your hands. Two things with one
 * name is a session lost to reading the wrong one.
 *
 * The judgement — which deck, which state, which controls — is
 * `dj_app::at_hand` and is tested there against the snapshot it reads. What a
 * browser can prove is the part Rust cannot: that the controls reach the
 * action bus unchanged, that a latched one shows its state, and that the panel
 * says which deck it is about and why.
 *
 * That last one is not decoration. A row of controls that silently became a
 * different row is a row a DJ stops trusting, and the whole of §74 rests on
 * being trusted enough to reach for without looking.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

async function openAtHand(page: import("@playwright/test").Page) {
  await openShell(page, "/", {}, {
    palette: [
      {
        label: "Show At hand",
        about: "The four to eight controls that matter on the focused deck right now.",
        kind: "surface",
        run: "athand",
      },
    ],
  });
  await page.keyboard.press("Control+k");
  await page.getByRole("button", { name: /Show At hand/ }).first().click();
}

test.describe("what is at hand", () => {
  /** The controls are on screen, and the panel says what they are for. */
  test("it names its deck and why these controls", async ({ page }) => {
    await openAtHand(page);
    const panel = page.locator('[data-surface="athand"]');
    await expect(panel).toBeVisible();

    await expect(panel).toContainText("Deck 2");
    await expect(panel).toContainText("this record is cued and waiting");

    // §74 asks for four to eight. Five is what this state offers.
    await expect(panel.locator(".controls button")).toHaveCount(5);
    await expect(panel.getByRole("button", { name: "cue" })).toBeVisible();
    await expect(panel.getByRole("button", { name: "loop 4" })).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A latched control shows that it is on.**
   *
   * Sync is engaged in the fixture and the button says so. Without it the row
   * is five identical squares and a DJ has to press one to find out what it
   * was already doing.
   */
  test("a control that is on looks like it", async ({ page }) => {
    await openAtHand(page);
    const panel = page.locator('[data-surface="athand"]');

    await expect(panel.getByRole("button", { name: "sync" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await expect(panel.getByRole("button", { name: "cue" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  /**
   * **Pressing one dispatches that exact action.**
   *
   * The whole design: it proposes nothing new, so a press is the same event as
   * typing it or mapping a controller to it — one execution path (ADR-0003).
   * What would break it is the interface deciding what a control means, so the
   * assertion is on the text that reached Rust.
   */
  test("a press reaches the action bus unchanged", async ({ page }) => {
    await openAtHand(page);
    await page
      .locator('[data-surface="athand"]')
      .getByRole("button", { name: "loop 4" })
      .click();

    const sent = await page.evaluate(
      () => (window as unknown as { __dispatched?: string[] }).__dispatched ?? [],
    );
    expect(sent).toContain("deck 2 loop 4");
  });
});
