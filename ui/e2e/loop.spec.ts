/**
 * §26's "Loop — resize".
 *
 * A loop's length was a pair of buttons that halve and double it, which covers
 * every length that is a power of two and no other. The edges are drawn and
 * grabbable now, on the same terms as the cue markers: the frame under the
 * hand goes to djmanzo, which decides whether it snaps, whether the loop would
 * be too short, and whether there is a loop to resize at all.
 *
 * It drags on the **stopped** deck for the reason `cues.spec.ts` explains — a
 * playing lane scrolls a target's width between measuring and pressing.
 *
 * The other half of this is what the band stopped doing. It was a sixteen
 * per-cent wash of `--accent-2` across the whole lane height, which tints the
 * spectral colouring underneath it: the audio inside a loop read as slightly
 * more mid-range than it is, and in one theme the wash was *exactly* the mid
 * band. §57 again. It is a bar along the bottom edge now, over nothing.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LANE = ".deck:not(.playing) .lane";

test.describe("resizing a loop", () => {
  test("both edges are drawn and can be grabbed", async ({ page }) => {
    await openShell(page, "/");

    await expect(page.locator(`${LANE} .loop-edge.grabbable`)).toHaveCount(2);
    await expect(
      page.locator(`${LANE} .loop-edge`).first(),
    ).toHaveAttribute("title", "Loop start — drag to resize");
  });

  /**
   * The band marks the region without colouring the audio in it — the whole
   * reason it moved to the edge.
   */
  test("the band marks the region without washing over the waveform", async ({
    page,
  }) => {
    await openShell(page, "/");

    const measured = await page.evaluate(() => {
      const lane = document.querySelector(".deck:not(.playing) .lane");
      const band = lane?.querySelector(".loop-band");
      if (!lane || !band) return null;
      const l = lane.getBoundingClientRect();
      const b = band.getBoundingClientRect();
      return { laneHeight: l.height, bandHeight: b.height, bandBottom: b.bottom, laneBottom: l.bottom };
    });
    expect(measured).not.toBeNull();

    expect(
      measured!.bandHeight,
      "the loop band covers the waveform instead of edging it, which tints the " +
        "spectral colours underneath — see §57",
    ).toBeLessThan(measured!.laneHeight / 4);
    expect(Math.abs(measured!.bandBottom - measured!.laneBottom)).toBeLessThan(4);
  });

  test("dragging an edge reaches djmanzo with the edge and a frame", async ({
    page,
  }) => {
    await openShell(page, "/");

    const edge = page.locator(`${LANE} .loop-edge.grabbable`).last();
    await expect(edge).toBeVisible();
    const box = (await edge.boundingBox())!;
    const y = box.y + box.height / 2;

    await page.evaluate(() => {
      const win = window as unknown as Record<string, unknown>;
      win.__asked = [];
      win.__loopArgs = [];
    });

    await page.mouse.move(box.x + box.width / 2, y);
    await page.mouse.down();
    await page.mouse.move(box.x + 70, y, { steps: 8 });
    await page.mouse.up();

    await expect
      .poll(
        () => page.evaluate(() => (window as unknown as { __asked: string[] }).__asked),
        { message: "the drag did not reach djmanzo" },
      )
      .toContain("move_loop_edge");

    const args = await page.evaluate(
      () => (window as unknown as { __loopArgs: Record<string, unknown>[] }).__loopArgs,
    );
    expect(args.length).toBeGreaterThan(0);
    expect(args[0].edge, "the wrong edge was moved").toBe("end");
    expect(args[0].deck).toBe(2);
    expect(Number(args[0].frame)).toBeGreaterThan(0);
  });

  test("dragging a loop edge throws nothing", async ({ page }) => {
    await openShell(page, "/");
    const edge = page.locator(`${LANE} .loop-edge.grabbable`).first();
    await expect(edge).toBeVisible();
    const box = (await edge.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x - 20, box.y + box.height / 2, { steps: 4 });
    await page.mouse.up();
    expect(errorsThrown(page)).toEqual([]);
  });
});
