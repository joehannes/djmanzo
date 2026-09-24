/**
 * §115: a background assistant that proposes without interrupting.
 *
 * > a background AI always auto watching the DJ and proposing useful things
 * > without being intrusive
 *
 * What is proposed, and when, is `dj_app::whisper`'s, tested there: the hold
 * before a condition is said, one at a time, the urgent first, the quiet
 * ones waiting for the hands, and declining. What the browser holds is the
 * line: it says the proposal where the top bar already has room — so it moves
 * nothing — taking it runs the proposal's own action through the bus, and
 * each way of declining is sent as what it is.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const clipping = {
  kind: "clipping",
  says: "Deck 2 is hitting the ceiling. Its trim down 3 dB?",
  offer: "Trim 3 dB",
  run: "deck 2 gain -1.0",
  urgent: true,
};

const answered = (page: Page) =>
  page.evaluate(() => (window as unknown as { __whisperAnswered?: unknown[] }).__whisperAnswered ?? []);

/** Deliver a frame carrying `proposal`, as the pump does. */
async function propose(page: Page, proposal: unknown) {
  await page.evaluate((proposal) => {
    const win = window as unknown as {
      __lastState?: Record<string, unknown>;
      __emit?: (next: unknown) => void;
    };
    win.__emit?.({ ...win.__lastState, whisper: proposal });
  }, proposal);
}

const decks = (page: Page) =>
  page.locator('.deck[data-deck="1"]').evaluate((el) => el.getBoundingClientRect().top);

test.describe("§115: the quiet proposer", () => {
  test("a quiet booth says nothing, and takes no room for it", async ({ page }) => {
    await openShell(page, "/");
    await expect(page.locator(".whisper")).toBeAttached();
    await expect(page.locator(".whisper .says")).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The load-bearing one: taking it runs its action, and a proposal
   * arriving moves nothing.** The decks are where they were before it came.
   */
  test("a proposal is said in one line, and taking it runs its action", async ({ page }) => {
    await openShell(page, "/");
    const before = await decks(page);
    // Longer than any top bar has room for, which is the case the rule is
    // for: the sentence is cut short, the bar does not wrap, and the decks
    // stay where they were.
    const long = {
      ...clipping,
      says: `${clipping.says} It has been at the ceiling for most of the last minute, on every kick and every snare, and the room is hearing it.`,
    };
    await propose(page, long);
    const line = page.locator('.whisper[data-whisper="clipping"]');
    await expect(line.locator(".says")).toHaveText(long.says);
    expect(await decks(page)).toBe(before);
    expect(
      await line.locator(".says").evaluate((el) => el.scrollWidth > el.clientWidth),
      "the sentence was not cut short",
    ).toBe(true);
    // Cut short, not squeezed out: in the first place this stood, the room
    // ran out and only the button was left.
    expect(await line.locator(".says").evaluate((el) => el.getBoundingClientRect().width)).toBeGreaterThan(200);
    await expect(line.getByRole("button", { name: "Trim 3 dB" })).toBeInViewport();

    await line.getByRole("button", { name: "Trim 3 dB" }).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []))
      .toContain("deck 2 gain -1.0");
    await expect.poll(() => answered(page)).toEqual([{ kind: "clipping", answer: "taken" }]);
    await expect(page.locator(".whisper .says")).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Not now and not tonight are two different promises, and each is sent as itself. */
  test("declining sends which kind of no it was", async ({ page }) => {
    await openShell(page, "/");
    await propose(page, { ...clipping, kind: "record", run: "record on", offer: "Record", urgent: false });
    const line = page.locator('.whisper[data-whisper="record"]');
    await line.getByRole("button", { name: "Not tonight" }).click();
    await expect.poll(() => answered(page)).toEqual([{ kind: "record", answer: "not-tonight" }]);
    await expect(page.locator(".whisper .says")).toHaveCount(0);
    // Gone at once, though the next frame may still carry it until Rust's
    // watcher has heard the answer; and nothing was run.
    await propose(page, { ...clipping, kind: "record", run: "record on", offer: "Record", urgent: false });
    await expect(page.locator(".whisper .says")).toHaveCount(0);
    expect(
      await page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []),
    ).not.toContain("record on");
  });
});
