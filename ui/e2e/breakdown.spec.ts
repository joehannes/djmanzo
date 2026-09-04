/**
 * §25's breakdown and drop layers.
 *
 * The arithmetic is Rust's — `dj_analysis::energy` finds them and `commands`
 * turns beats into frames — and both are tested there. What a browser can
 * prove is the half Rust cannot: that the answer reaches the lane, lands in
 * the right place along it, and does not tint the waveform it is drawn over.
 *
 * That last one is not decoration. §57 forbids overloading a colour with two
 * meanings, and the waveform's colour *is* the spectral balance; a breakdown
 * drawn as a wash would recolour every band underneath it. The band is at the
 * top edge instead, which is a thing a test can check.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Deck 1's scrolling lane. */
const LANE = ".deck .lane";
const OVERVIEW = ".deck .overview";

test.describe("breakdowns and drops", () => {
  test("the lane draws the breakdown where Rust put it", async ({ page }) => {
    await openShell(page, "/");

    const band = page.locator(`${LANE} .breakdown`).first();
    await expect(band).toBeVisible();

    // The fixture's breakdown is 1M frames long inside an 8M-frame record.
    // The lane is in frames-per-pixel, so what is checkable here is that the
    // band starts before it ends and has real width — the exact pixel is the
    // zoom's business and changes with the window.
    const box = await band.boundingBox();
    expect(box, "the breakdown band has no geometry").not.toBeNull();
    expect(box!.width).toBeGreaterThan(1);
  });

  /**
   * The one that matters for §57: a breakdown may not recolour the waveform.
   * A band along the top edge cannot; a wash over the lane would.
   */
  test("the breakdown sits at the edge rather than over the waveform", async ({
    page,
  }) => {
    await openShell(page, "/");

    await expect(page.locator(`${LANE} .breakdown`).first()).toBeVisible();
    const measured = await page.evaluate(() => {
      const lane = document.querySelector(".deck .lane");
      const band = lane?.querySelector(".breakdown");
      if (!lane || !band) return null;
      const l = lane.getBoundingClientRect();
      const b = band.getBoundingClientRect();
      return { laneHeight: l.height, laneTop: l.y, bandHeight: b.height, bandTop: b.y };
    });
    expect(measured).not.toBeNull();

    expect(
      measured!.bandHeight,
      "the breakdown covers the waveform instead of edging it, which recolours " +
        "the spectral bands underneath — see §57",
    ).toBeLessThan(measured!.laneHeight / 4);
    expect(Math.abs(measured!.bandTop - measured!.laneTop)).toBeLessThan(4);
  });

  /**
   * A drop is where the drums come back, so it belongs at the far end of the
   * breakdown it ends. Drawn apart from it, it would be a marker with no
   * subject.
   */
  test("the drop lands at the end of the breakdown", async ({ page }) => {
    await openShell(page, "/");

    await expect(page.locator(`${LANE} .breakdown`).first()).toBeVisible();
    // **Both in one evaluation.** The lane scrolls with the record, so a box
    // measured in one call and compared against a box measured in the next is
    // stale by however far the deck moved in between — nine pixels, when this
    // test was written the other way. The same trap that once looked like a
    // WebKitGTK bug; see `docs/HANDOFF.md`.
    const measured = await page.evaluate(() => {
      const lane = document.querySelector(".deck .lane");
      const band = lane?.querySelector(".breakdown");
      const drop = lane?.querySelector(".drop");
      if (!band || !drop) return null;
      const b = band.getBoundingClientRect();
      const d = drop.getBoundingClientRect();
      return { bandRight: b.x + b.width, dropLeft: d.x };
    });
    expect(measured).not.toBeNull();

    expect(
      Math.abs(measured!.dropLeft - measured!.bandRight),
      "the drop is not at the end of the breakdown",
    ).toBeLessThan(2);
  });

  /**
   * The overview is the view that most wants this — it is where a DJ asks
   * "where is the breakdown in this record", which is what the component's own
   * comment has claimed since it was written.
   */
  test("the overview shows the shape of the record too", async ({ page }) => {
    await openShell(page, "/");

    const overview = page.locator(OVERVIEW).first();
    const band = overview.locator(".breakdown");
    await expect(band).toBeVisible();

    // A quarter of the way in, an eighth of the record wide: the overview is
    // the whole track, so these are checkable proportions rather than pixels.
    const overviewBox = await overview.boundingBox();
    const bandBox = await band.boundingBox();
    expect(overviewBox).not.toBeNull();
    expect(bandBox).not.toBeNull();
    expect(
      (bandBox!.x - overviewBox!.x) / overviewBox!.width,
      "the breakdown is not a quarter of the way into the record",
    ).toBeCloseTo(0.25, 1);
    expect(bandBox!.width / overviewBox!.width).toBeCloseTo(0.125, 1);
  });

  test("a record with no breakdown draws none", async ({ page }) => {
    await openShell(page, "/", {}, {
      waveform_info: {
        deck: 1,
        ready: true,
        total_frames: 8_000_000,
        epoch: 1,
        breakdowns: [],
        drops: [],
      },
    });

    await expect(page.locator(`${LANE} .breakdown`)).toHaveCount(0);
    await expect(page.locator(`${LANE} .drop`)).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});
