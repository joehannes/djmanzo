/**
 * The interface's own frame rate, said without moving anything.
 *
 * When the webview cannot keep up — no hardware acceleration, a struggling
 * laptop — djmanzo says so. It used to say it in the band above the decks,
 * which pushed them down 36 px when the frame rate dropped and back up when it
 * recovered: a warning bouncing the decks, on exactly the machine that was
 * already struggling, in the middle of whatever mix was slowing it down.
 * Found by accident, by a test measuring the decks under load.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Make every animation frame arrive a tenth of a second late: 10 fps. */
const SLOW = () => {
  window.requestAnimationFrame = (callback: FrameRequestCallback) =>
    window.setTimeout(() => callback(performance.now()), 100) as unknown as number;
};

test("a slow interface is said beside the readings, and the decks stay put", async ({ page, browser }) => {
  // The same shell, once at full speed, to measure where the decks belong.
  await openShell(page, "/");
  const where = async (p: typeof page) => (await p.locator('.deck[data-deck="1"]').boundingBox())?.y ?? -1;
  await expect(page.locator("[data-slow-frames]")).toHaveCount(0);
  const normal = await where(page);

  const slowContext = await browser.newContext({ baseURL: test.info().project.use.baseURL });
  const slow = await slowContext.newPage();
  await slow.addInitScript(SLOW);
  await openShell(slow, "/");
  const chip = slow.locator("[data-slow-frames]");
  await expect(chip).toBeVisible({ timeout: 20_000 });
  await expect(chip).toContainText(/UI \d+ fps/);
  await expect(chip).toHaveAttribute("title", /audio engine is unaffected/);

  expect(Math.abs((await where(slow)) - normal), "the slow-frames notice moved the decks").toBeLessThanOrEqual(2);
  expect(errorsThrown(slow)).toEqual([]);
  await slowContext.close();
});

/**
 * **§18 for every notice, not only this one.** A headphone device failing is
 * exactly the kind of thing that happens mid-mix, and its notice used to push
 * the decks down like any other. While two records are audible the band floats
 * over the stage's top edge instead; between records it takes its room in the
 * flow, as it always did.
 */
test("a notice that arrives mid-mix does not move the decks", async ({ page }) => {
  await openShell(page, "/");
  const deck = page.locator('.deck[data-deck="1"]');
  const where = async () => (await deck.boundingBox())?.y ?? -1;
  const emit = (reflow: boolean, failing: boolean) =>
    page.evaluate(
      ({ reflow, failing }) => {
        const win = window as unknown as { __lastState: Record<string, any>; __emit: (s: unknown) => void };
        const next = structuredClone(win.__lastState);
        next.attention = { ...next.attention, reflow };
        next.master = {
          ...next.master,
          split_output: failing
            ? { drift_ppm: 3, queue_ms: 0, target_ms: 20, starved_frames: 4800, dropped_samples: 0, healthy: false }
            : null,
        };
        win.__emit(next);
      },
      { reflow, failing },
    );

  // Mid-mix: the layout must hold still.
  await emit(false, false);
  const before = await where();
  await emit(false, true);
  const notice = page.locator("[data-notices]").getByText("The headphone device has lost audio");
  await expect(notice).toBeVisible();
  expect(Math.abs((await where()) - before), "a notice moved the decks mid-mix").toBeLessThanOrEqual(2);

  // Between records the band may take its room: the same notice in the flow.
  await emit(true, true);
  await expect(page.locator("[data-notices]")).not.toHaveClass(/floating/);
  await expect.poll(where).toBeGreaterThan(before + 10);
  expect(errorsThrown(page)).toEqual([]);
});
