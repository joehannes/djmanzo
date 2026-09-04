/**
 * §26's last named example: "Phrase marker — drag to adjust."
 *
 * The analyser finds how long a phrase is and which beat starts one, and it
 * can be right about the first and wrong about the second — a record opening
 * on a four-beat pickup is the usual cause. Nothing could correct that, and a
 * wrong anchor is not cosmetic: the planner, the automix, the autopilot and
 * Set Flow all place mixes on it.
 *
 * **The lines are not drawn here.** They are rasterised into the waveform
 * tiles, where they can share a pixel with the audio; the browser adds a
 * transparent grab target over each one. So what a browser test can prove is
 * that the target is there, that it is where Rust said the boundary is, and
 * that letting go reaches djmanzo with a frame.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LANE = ".deck:not(.playing) .lane";

test.describe("moving a phrase boundary", () => {
  test("every boundary Rust reports has somewhere to grab it", async ({ page }) => {
    await openShell(page, "/");

    // Four in the fixture.
    await expect(page.locator(`${LANE} .phrase-grab`)).toHaveCount(4);
    await expect(
      page.locator(`${LANE} .phrase-grab`).first(),
    ).toHaveAttribute("title", "Phrase boundary — drag to say where a phrase starts");
  });

  /**
   * **The target draws nothing.** The line under it belongs to the renderer,
   * which is the only thing that can put it on the same pixel as the audio; a
   * second line here would be a second opinion about where that beat is.
   */
  test("the target is invisible until it is held", async ({ page }) => {
    await openShell(page, "/");

    const painted = await page.evaluate(() => {
      const el = document.querySelector(".deck:not(.playing) .lane .phrase-grab");
      return el ? getComputedStyle(el).backgroundColor : null;
    });
    expect(painted, "the grab target paints a line of its own").toMatch(
      /rgba\(0, 0, 0, 0\)|transparent/,
    );
  });

  test("dragging one reaches djmanzo with a frame", async ({ page }) => {
    await openShell(page, "/");

    const grab = page.locator(`${LANE} .phrase-grab`).first();
    await expect(grab).toBeVisible();
    const box = (await grab.boundingBox())!;
    const y = box.y + box.height / 2;

    await page.evaluate(() => {
      const win = window as unknown as Record<string, unknown>;
      win.__asked = [];
      win.__phraseArgs = [];
    });

    await page.mouse.move(box.x + box.width / 2, y);
    await page.mouse.down();
    await page.mouse.move(box.x + 60, y, { steps: 8 });
    await page.mouse.up();

    await expect
      .poll(
        () => page.evaluate(() => (window as unknown as { __asked: string[] }).__asked),
        { message: "the drag did not reach djmanzo" },
      )
      .toContain("move_phrase");

    const args = await page.evaluate(
      () => (window as unknown as { __phraseArgs: Record<string, unknown>[] }).__phraseArgs,
    );
    expect(args.length).toBeGreaterThan(0);
    expect(args[0].deck).toBe(2);
    expect(Number(args[0].frame)).toBeGreaterThan(0);
  });

  test("dragging a phrase boundary throws nothing", async ({ page }) => {
    await openShell(page, "/");
    const grab = page.locator(`${LANE} .phrase-grab`).first();
    await expect(grab).toBeVisible();
    const box = (await grab.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x - 30, box.y + box.height / 2, { steps: 4 });
    await page.mouse.up();
    expect(errorsThrown(page)).toEqual([]);
  });
});
