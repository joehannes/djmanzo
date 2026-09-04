/**
 * §25's saved-loop layer, and §26's answer to it.
 *
 * A saved loop was invisible in both places it should not have been. The lane
 * did not draw it, so a region kept with the record could not be seen against
 * the audio it was cut from; and the pad page that recalls them named
 * `Lit::Never` for all eight slots, so the only way to find out whether slot 5
 * held anything was to press it in front of a room.
 *
 * The fixture's stopped deck carries two: slot 3 over frames 48 000..60 000 and
 * slot 6 over 64 000..76 000. Two rather than one, because a single region
 * cannot tell a marker that knows its slot apart from one that draws a
 * constant.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LANE = ".deck:not(.playing) .lane";
const EMPTY = ".deck.playing .lane";

test.describe("saved loops on the lane", () => {
  test("every saved loop is drawn, labelled with the slot it is recalled by", async ({
    page,
  }) => {
    await openShell(page, "/");

    await expect(page.locator(`${LANE} .saved-loop`)).toHaveCount(2);
    await expect(
      page.locator(`${LANE} .saved-loop-flag`).first(),
    ).toHaveText("3");
    await expect(page.locator(`${LANE} .saved-loop-flag`).last()).toHaveText("6");
  });

  /**
   * A record nobody has saved a loop against is the ordinary case, and it must
   * draw nothing rather than borrowing the other deck's regions.
   */
  test("a record with none draws none", async ({ page }) => {
    await openShell(page, "/");
    await expect(page.locator(`${EMPTY} .saved-loop`)).toHaveCount(0);
  });

  /**
   * §57. The running loop already learnt this: a region drawn *over* the
   * waveform tints the spectral colouring, and the colour of this waveform is
   * its spectral balance. A saved loop is the same shape of mistake waiting to
   * be made again, so it is measured the same way.
   */
  test("the bracket edges the lane rather than washing over the audio", async ({
    page,
  }) => {
    await openShell(page, "/");

    const measured = await page.evaluate(() => {
      const lane = document.querySelector(".deck:not(.playing) .lane");
      const region = lane?.querySelector(".saved-loop");
      if (!lane || !region) return null;
      const l = lane.getBoundingClientRect();
      const r = region.getBoundingClientRect();
      return {
        laneHeight: l.height,
        height: r.height,
        bottom: r.bottom,
        laneBottom: l.bottom,
      };
    });
    expect(measured).not.toBeNull();

    expect(
      measured!.height,
      "a saved loop covers the waveform instead of edging it, which tints the " +
        "spectral colours underneath — see §57",
    ).toBeLessThan(measured!.laneHeight / 4);
    expect(Math.abs(measured!.bottom - measured!.laneBottom)).toBeLessThan(4);
  });

  /**
   * The layer is only instrumentation if it is drawn where the data says. Both
   * regions are twelve thousand frames long and sixteen thousand apart, so the
   * second sits to the right of the first by more than its own width.
   */
  test("each region spans the frames it was saved over", async ({ page }) => {
    await openShell(page, "/");

    const boxes = await page.evaluate(() =>
      [...document.querySelectorAll(".deck:not(.playing) .lane .saved-loop")].map(
        (element) => {
          const box = element.getBoundingClientRect();
          return { x: box.x, width: box.width };
        },
      ),
    );
    expect(boxes.length).toBe(2);

    // The same length in frames, so the same width on the lane.
    expect(Math.abs(boxes[0].width - boxes[1].width)).toBeLessThan(2);
    expect(boxes[0].width).toBeGreaterThan(8);
    // 64 000 comes after 60 000, and the gap between them is a third of a span.
    const gap = boxes[1].x - (boxes[0].x + boxes[0].width);
    expect(gap).toBeGreaterThan(0);
    expect(Math.abs(gap - boxes[0].width / 3)).toBeLessThan(3);
  });

  test("pressing one asks djmanzo for that slot, not the first", async ({
    page,
  }) => {
    await openShell(page, "/");

    await page.evaluate(() => {
      (window as unknown as Record<string, unknown>).__dispatched = [];
    });

    await page.locator(`${LANE} button.saved-loop-flag`).last().click();

    await expect
      .poll(
        () =>
          page.evaluate(
            () => (window as unknown as { __dispatched: string[] }).__dispatched,
          ),
        { message: "the press did not reach djmanzo" },
      )
      .toContain("deck 2 loop_recall 6");
  });

  /**
   * The other half of the same invisibility. Every pad on this page named
   * `Lit::Never`, so eight identical squares said nothing about which of them
   * held a loop — a page a DJ had to remember rather than read.
   */
  test("the saved pad page lights the slots that hold a loop, and only those", async ({
    page,
  }) => {
    await openShell(page, "/");

    const deck = page.locator(".deck:not(.playing)");
    await deck.getByRole("button", { name: "saved", exact: true }).click();

    const pads = deck.locator(".grid button");
    await expect(pads).toHaveCount(8);
    for (let slot = 1; slot <= 8; slot += 1) {
      const pad = pads.nth(slot - 1);
      if (slot === 3 || slot === 6) {
        await expect(pad, `pad ${slot} holds a loop and should be lit`).toHaveClass(
          /lit/,
        );
      } else {
        await expect(
          pad,
          `pad ${slot} holds nothing and should be dark`,
        ).not.toHaveClass(/lit/);
      }
    }
  });

  test("pressing a saved loop throws nothing", async ({ page }) => {
    await openShell(page, "/");
    await page.locator(`${LANE} button.saved-loop-flag`).first().click();
    expect(errorsThrown(page)).toEqual([]);
  });
});
