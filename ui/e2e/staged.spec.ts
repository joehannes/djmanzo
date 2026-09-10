/**
 * The staged transaction, and the emergency.
 *
 * §44 asks for non-trivial AI action to be a transaction with Accept, Modify
 * and Reject; §47 asks for an emergency control a DJ finds without searching.
 * Both are as much about *where they are on the screen* as about what they do,
 * which is exactly the half a type-check cannot see.
 *
 * The arithmetic — which moves belong in a plan, which the posture allows,
 * what Accept actually carries out — is Rust's and is tested there, in
 * `dj_app::staged` and `dj_assistant::authority`. What a browser can prove is
 * that the plan reaches the screen, that a refused move is visible rather than
 * dropped, that turning one off changes what Accept will do, and that SAFE is
 * on screen without opening anything.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const STRIP = "section.staged";

/** A plan at Prepare: the silent half permitted, the audible half refused. */
const PREPARED = {
  headline: "Prepared next transition",
  because: "44s left on deck 1",
  live_deck: 1,
  moves: [
    {
      about: "Load the next record onto deck 2",
      capability: "load_next_deck",
      allowance: "yes",
      chosen: true,
    },
    {
      about: "Cue deck 2 to the start of its first phrase",
      capability: "set_cue",
      allowance: "yes",
      chosen: true,
    },
    {
      about: "Trim deck 2 by -2.5 dB to match deck 1",
      capability: "gain_match",
      allowance: "yes",
      chosen: true,
    },
    {
      about: "Engage sync on deck 2",
      capability: "sync",
      allowance: "no",
      chosen: false,
    },
    {
      about: "Mix deck 1 into deck 2 over 32 beats, blend",
      capability: "crossfader",
      allowance: "no",
      chosen: false,
    },
  ],
};

async function withPlan(page: import("@playwright/test").Page) {
  await openShell(page, "/", {}, { staged_prepare: PREPARED });
  // Asked for the way the interface asks for it, through the assistant.
  await page.getByRole("button", { name: "Assistant", exact: true }).click();
  await page
    .getByRole("button", { name: "Prepare the next transition" })
    .click();
  await expect(page.locator(STRIP)).toBeVisible();
}

test.describe("the staged transaction", () => {
  /**
   * Nothing staged costs the decks nothing. The strip is not a panel that sits
   * there empty — it is absent.
   */
  test("is not on screen until something is staged", async ({ page }) => {
    await openShell(page, "/");
    await expect(page.locator(STRIP)).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The whole transition, as one thing, with its refused half visible.**
   *
   * A plan that silently stopped after the trim would leave a DJ wondering
   * what djmanzo thinks a cued deck is for. The greyed rows are the answer,
   * and they name the row of the matrix that refused them.
   */
  test("shows every move, including the ones the posture refuses", async ({
    page,
  }) => {
    await withPlan(page);

    await expect(page.locator(`${STRIP} .head`)).toContainText(
      "Prepared next transition",
    );
    await expect(page.locator(`${STRIP} .moves li`)).toHaveCount(5);
    await expect(page.locator(`${STRIP} .moves li.refused`)).toHaveCount(2);
    await expect(page.locator(`${STRIP} .moves li.refused`).last()).toContainText(
      "not at this level",
    );
    // And the refused ones cannot be ticked.
    await expect(
      page.locator(`${STRIP} .moves li.refused input`).first(),
    ).toBeDisabled();
    // Accept says how much it will actually do, which is the permitted half.
    await expect(page.getByRole("button", { name: /^Accept/ })).toContainText("3");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Modify is turning one off**, and Accept then does less.
   *
   * "Load it and cue it, but I will bring it in myself" is the realistic
   * modification, and it must not mean rejecting the plan and doing it by hand.
   */
  test("turning a move off changes what Accept will do", async ({ page }) => {
    await withPlan(page);

    const accept = page.getByRole("button", { name: /^Accept/ });
    await expect(accept).toContainText("3");
    await page.locator(`${STRIP} .moves li:not(.refused) input`).last().uncheck();
    await expect(accept).toContainText("2");

    await accept.click();
    // Accepting reports what was carried out, and the plan is gone.
    await expect(page.locator(`${STRIP}.done`)).toContainText("Done");
    await expect(page.locator(`${STRIP}.done .because`)).toContainText("Load");
    await expect(page.locator(`${STRIP}.done .because`)).not.toContainText("Trim");
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Reject takes it away and does nothing. */
  test("rejecting removes the plan", async ({ page }) => {
    await withPlan(page);
    await page.getByRole("button", { name: "Reject", exact: true }).click();
    await expect(page.locator(STRIP)).toHaveCount(0);
  });
});

test.describe("the emergency control", () => {
  /**
   * §47: "the user must not search through menus". So it is in the top bar,
   * beside REC and Mark, with nothing opened and nothing scrolled.
   */
  test("SAFE is on screen with nothing opened", async ({ page }) => {
    await openShell(page, "/");
    const safe = page.getByRole("button", { name: "SAFE", exact: true });
    await expect(safe).toBeVisible();

    // Inside the window, not below the fold — the whole point of it.
    const box = await safe.boundingBox();
    const viewport = page.viewportSize();
    expect(box, "SAFE has no box").not.toBeNull();
    expect(viewport, "no viewport").not.toBeNull();
    expect(box!.y + box!.height).toBeLessThanOrEqual(viewport!.height);
    expect(box!.x + box!.width).toBeLessThanOrEqual(viewport!.width);
  });

  /** And it says `safe` on the action bus, like every other control. */
  test("SAFE goes through the action bus", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "SAFE", exact: true }).click();
    const asked = await page.evaluate(
      () => (window as unknown as { __asked: string[] }).__asked,
    );
    expect(asked).toContain("dispatch");
    expect(errorsThrown(page)).toEqual([]);
  });
});
