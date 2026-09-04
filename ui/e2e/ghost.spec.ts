/**
 * §27's ghost track.
 *
 * > "If I bring this in here, this is what happens" — before loading or
 * > committing.
 *
 * The pair view already draws two records and the seam between them. What it
 * could not do is show them *against each other*: two lanes side by side
 * answer "what are these records", and the question §27 asks is what they do
 * together. So the incoming record is drawn over the outgoing lane, from the
 * point the mix begins, at a zoom that makes one of its beats one of the
 * outgoing's — a record drawn at its own frame rate would visibly drift
 * against a lane it is supposed to be beatmatched to.
 *
 * The fixture mixes 124 into 127 BPM, and the incoming record comes in five
 * seconds into itself.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const PAIR = '.surface[data-surface="pair"]';
/** The outgoing lane, which is the first of the two the panel draws. */
const OUT = `${PAIR} .side:first-of-type .lane`;

async function compared(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Pair", exact: true }).click();
  await expect(page.locator(PAIR)).toBeVisible();
  await page.getByRole("button", { name: "Compare", exact: true }).click();
  await expect(page.locator(`${PAIR} .seam`)).toBeVisible();
}

test.describe("the ghost track", () => {
  test("the incoming record is drawn over the outgoing lane", async ({ page }) => {
    await compared(page);

    await expect(page.locator(`${OUT} .ghost`)).toHaveCount(1);
    // And not over the incoming lane, which is already that record: a ghost of
    // a track over itself says nothing.
    await expect(page.locator(`${PAIR} .ghost`)).toHaveCount(1);
  });

  /**
   * Nothing is held or proposed when the panel opens, so there is no mix point
   * and nothing honest to draw a ghost at.
   */
  test("nothing is drawn before there is a mix to draw it at", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Pair", exact: true }).click();
    await expect(page.locator(PAIR)).toBeVisible();

    await expect(page.locator(`${PAIR} .ghost`)).toHaveCount(0);
  });

  /**
   * The whole claim of the layer: it starts *where the mix starts*. A ghost a
   * few pixels off is a preview of a different transition.
   */
  test("it begins at the mix point, not at the edge of the lane", async ({
    page,
  }) => {
    await compared(page);

    const measured = await page.evaluate(() => {
      const lane = document.querySelector(
        '.surface[data-surface="pair"] .side:first-of-type .lane',
      );
      const ghost = lane?.querySelector(".ghost");
      const mark = lane?.querySelector(".mark");
      if (!ghost || !mark) return null;
      return { ghost: ghost.getBoundingClientRect().x, mark: mark.getBoundingClientRect().x };
    });
    expect(measured, "the ghost or the mix point was not drawn").not.toBeNull();
    expect(Math.abs(measured!.ghost - measured!.mark)).toBeLessThan(4);
  });

  /**
   * See-through, because a record that has not come in yet is not a fact about
   * the audio that is playing. And not recoloured: the ghost's colour is its
   * own spectral balance exactly as the lane's is, so telling the two apart is
   * a matter of solid against not — §57 again.
   */
  test("it is drawn through rather than over", async ({ page }) => {
    await compared(page);

    const opacity = await page.evaluate(() => {
      const ghost = document.querySelector(
        '.surface[data-surface="pair"] .side:first-of-type .lane .ghost',
      );
      return ghost ? Number(getComputedStyle(ghost).opacity) : null;
    });
    expect(opacity).not.toBeNull();
    expect(opacity!).toBeGreaterThan(0.15);
    expect(opacity!, "an opaque ghost hides the record it is a preview against").toBeLessThan(
      0.75,
    );
  });

  /**
   * The tiles are the incoming deck's, at a zoom scaled by the tempo ratio.
   * Same zoom as the lane would be a record drifting against beats it is meant
   * to be matched to, and it is the one thing about this layer that a picture
   * of it cannot show.
   */
  test("its tiles are the incoming record's, beat-matched to this lane", async ({
    page,
  }) => {
    await compared(page);

    const zooms = await page.evaluate(() => {
      const lane = document.querySelector(
        '.surface[data-surface="pair"] .side:first-of-type .lane',
      );
      // `tile/<deck>/<width>/<height>/<start>/<zoomMilli>/<theme>/<epoch>`
      const read = (img: Element) => {
        const parts = (img as HTMLImageElement).src.split("/tile/")[1]?.split("/") ?? [];
        return { deck: Number(parts[0]), zoom: Number(parts[4]) };
      };
      const own = [...(lane?.querySelectorAll(".strip > .tile") ?? [])].map(read);
      const ghost = [...(lane?.querySelectorAll(".ghost .tile") ?? [])].map(read);
      return { own, ghost };
    });

    expect(zooms.own.length).toBeGreaterThan(0);
    expect(zooms.ghost.length).toBeGreaterThan(0);
    expect(zooms.ghost.every((t) => t.deck === 2), "the ghost drew the wrong deck").toBe(
      true,
    );
    expect(zooms.own.every((t) => t.deck === 1)).toBe(true);

    // 124 into 127: the incoming record's beats are shorter, so fewer of its
    // frames fill one pixel of this lane.
    const ratio = zooms.ghost[0].zoom / zooms.own[0].zoom;
    expect(Math.abs(ratio - 124 / 127), `zoom ratio was ${ratio}`).toBeLessThan(0.002);
  });

  /**
   * §27's "where the drop occurs" — about the record coming in, which is the
   * one whose structure a DJ cannot see from the lane that is playing. The
   * marks sit inside the ghost's box, so they fade with it: what the new
   * record does is part of the preview, not a fact about the audio now.
   */
  test("it carries the incoming record's own breakdown and drop", async ({
    page,
  }) => {
    await compared(page);

    const ghost = `${OUT} .ghost`;
    await expect(page.locator(`${ghost} .breakdown`)).toHaveCount(1);
    await expect(page.locator(`${ghost} .drop`)).toHaveCount(1);

    // Inside the ghost, not loose on the lane beside the outgoing record's own
    // marks: the fixture gives both records the same structure, so a mark that
    // escaped its box would be indistinguishable from the lane's own.
    const inside = await page.evaluate(() => {
      const box = document.querySelector(
        '.surface[data-surface="pair"] .side:first-of-type .lane .ghost',
      );
      const drop = box?.querySelector(".drop");
      if (!box || !drop) return null;
      const b = box.getBoundingClientRect();
      const d = drop.getBoundingClientRect();
      return d.x >= b.x - 1 && d.x <= b.x + b.width + 1;
    });
    expect(inside, "the ghost's drop was drawn outside the ghost").toBe(true);
  });

  test("drawing a ghost throws nothing", async ({ page }) => {
    await compared(page);
    expect(errorsThrown(page)).toEqual([]);
  });
});
