/**
 * §17: the cockpit follows the night's phase — except when it must not.
 *
 * > The system may infer phase, but the DJ must always be able to override it.
 *
 * And §18, which is the harder half: *when actively mixing — no major layout
 * reflow*. `Attention::reflow` is false during every mix, and a phase turning
 * over in the middle of one must change nothing. Moving a panel while somebody
 * is reaching for it is the failure that makes adaptive interfaces feel
 * hostile, and it is the failure this file exists to prevent.
 *
 * Which surfaces each phase asks for is `dj_app::cockpit::priorities` and is
 * tested there against the shell's own list of what it draws.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Deliver a new snapshot with the phase and the reflow budget changed. */
async function nightReads(
  page: import("@playwright/test").Page,
  phase: string | null,
  reflow: boolean,
) {
  await page.evaluate(
    ([phase, reflow]) => {
      const win = window as unknown as {
        __lastState?: {
          context: { session: Record<string, unknown> | null };
          attention: { reflow: boolean };
        };
        __emit?: (next: unknown) => void;
      };
      const state = win.__lastState;
      if (!state) throw new Error("the harness delivered no state to start from");
      win.__emit?.({
        ...state,
        attention: { ...state.attention, reflow },
        context: {
          ...state.context,
          session:
            phase === null
              ? null
              : {
                  phase,
                  energy: 0.8,
                  environment: "club",
                  certainty: "sure",
                  basis: "measured",
                  drift: null,
                },
        },
      });
    },
    [phase, reflow] as const,
  );
}

const ROOM = '.surface[data-surface="room"]';

test.describe("§17's phase priorities", () => {
  /**
   * **The load-bearing one: nothing moves during a mix.**
   *
   * §18 states it as a rule with no exceptions, and this is the case that
   * would break it — a phase turning over at exactly the wrong moment. The
   * phase is skipped rather than queued: by the time the mix ends it is either
   * still this one, and the next snapshot applies it, or it has moved on and
   * the stale one was never worth applying.
   */
  test("a phase that turns over mid-mix moves nothing", async ({ page }) => {
    await openShell(page, "/");
    await expect(page.locator(ROOM)).toHaveCount(0);

    await nightReads(page, "peak", false);

    // Given a moment to do the wrong thing, and asserted afterwards rather
    // than immediately: a `toHaveCount(0)` that passes because nothing has
    // happened *yet* is the vacuous version of this test.
    await page.waitForTimeout(400);
    await expect(
      page.locator(ROOM),
      "the cockpit rearranged itself during a mix -- §18 says no major layout " +
        "reflow while performing, and moving a panel under a reaching hand is " +
        "the failure adaptive interfaces are hated for",
    ).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** With room to think, the phase opens what it is for. */
  test("a phase that turns over between records opens what it asks for", async ({
    page,
  }) => {
    await openShell(page, "/");
    await expect(page.locator(ROOM)).toHaveCount(0);

    await nightReads(page, "peak", true);

    await expect(
      page.locator(ROOM),
      "the night reached peak with room to think and the cockpit did nothing",
    ).toBeVisible();
  });

  /**
   * §17's closing instruction: the DJ must always be able to override it.
   *
   * Which, without a dialog, means two things — a phase only ever *adds*, so
   * nothing the DJ opened is taken away, and closing one of its panels sticks
   * rather than being reopened on the next snapshot.
   */
  test("a phase adds to what the DJ has, and does not undo their closing it", async ({
    page,
  }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();

    await nightReads(page, "peak", true);
    await expect(page.locator(ROOM)).toBeVisible();
    await expect(
      page.locator('.surface[data-surface="library"]'),
      "the phase closed something the DJ had opened",
    ).toBeVisible();

    // The override: close it, and stay closed through another frame of the
    // same phase.
    await page.locator(`${ROOM} .surface-head button`).first().click();
    await expect(page.locator(ROOM)).toHaveCount(0);
    await nightReads(page, "peak", true);
    await page.waitForTimeout(400);
    await expect(
      page.locator(ROOM),
      "the phase reopened a panel the DJ had just closed, so the override does " +
        "not hold",
    ).toHaveCount(0);
  });
});
