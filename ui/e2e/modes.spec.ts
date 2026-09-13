/**
 * §77's two mental modes, and the one-way rule that connects them.
 *
 * > The UI should recognize two fundamentally different mental modes […] The
 * > system should transition between these modes gracefully.
 *
 * `cockpit::Focus` has modelled them since the cockpit was written, stored on
 * the workspace — and, like §78's `frozen` before it, **nothing read it**. What
 * reads it now is §18's attention budget: the machine's reading of the night is
 * the ceiling, and the focus a DJ chose can only lower it.
 *
 * Why it is one-way is `cockpit::Attention::quieter_of`'s own note, and the
 * rule is tested there against budgets this container cannot produce. What is
 * here is the half only a browser can show: that the quieter budget reaches the
 * document, so the interface really does stop moving.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Deliver a snapshot carrying a particular attention budget. */
async function budget(page: Page, motion: string, suggestions: number) {
  await page.evaluate(
    ([motion, suggestions]) => {
      const win = window as unknown as {
        __lastState?: { attention: Record<string, unknown> };
        __emit?: (next: unknown) => void;
      };
      const state = win.__lastState;
      if (!state) throw new Error("the harness delivered no state to start from");
      win.__emit?.({
        ...state,
        attention: { ...state.attention, motion, suggestions },
      });
    },
    [motion, suggestions] as const,
  );
}

const motionNow = (page: Page) =>
  page.evaluate(() => document.documentElement.dataset.motion);

test.describe("§77's modes", () => {
  /**
   * **The load-bearing one: a quieter budget reaches the document and the
   * interface stops moving.**
   *
   * Rust decides how quiet — from the night, then lowered by the focus the DJ
   * chose — and this is the wire. A budget that arrived and changed nothing
   * would be §77 modelled and not honoured, which is the state it was in.
   */
  test("a quieter budget stops the interface moving", async ({ page }) => {
    await openShell(page, "/");
    await expect.poll(() => motionNow(page)).toBe("normal");

    await budget(page, "none", 0);
    await expect
      .poll(() => motionNow(page), {
        message:
          "the budget said nothing may move and the document never heard about " +
          "it, so §77 is modelled and not honoured",
      })
      .toBe("none");

    // And the stylesheet acts on it: §18's tier 0 is a requirement rather than
    // a courtesy, so at `none` every animation is stopped rather than merely
    // discouraged.
    const stopped = await page.evaluate(() => {
      const probe = document.createElement("div");
      probe.style.animation = "spin 2s linear infinite";
      document.body.append(probe);
      const duration = getComputedStyle(probe).animationDuration;
      probe.remove();
      return duration;
    });
    expect(
      stopped,
      "at motion `none` an animation still runs, so a DJ sorting out a failed " +
        "recording is reading a moving screen",
    ).not.toBe("2s");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * And it goes back, because §77 asks for the transition to be graceful in
   * both directions.
   *
   * A mode that could only ever be entered would be a trap: a DJ whose set
   * quietened at peak and stayed quiet through the cool-down has an interface
   * that gave up rather than adapted.
   */
  test("the interface comes back when there is room again", async ({ page }) => {
    await openShell(page, "/");
    await budget(page, "none", 0);
    await expect.poll(() => motionNow(page)).toBe("none");

    await budget(page, "normal", 5);
    await expect
      .poll(() => motionNow(page), {
        message: "the quiet mode is a trap: nothing brings the interface back",
      })
      .toBe("normal");
  });
});
