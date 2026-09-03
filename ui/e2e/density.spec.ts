/**
 * The interface adapts to the window it is given, and the adaptation works.
 *
 * This is the redesign's own sentence made testable: *the system adapts the
 * presentation to the DJ, rather than forcing the DJ to adapt to the
 * application.* djmanzo picks a density band from the window height
 * (`dj_app::cockpit::BANDS`) and everything on a deck scales with it.
 *
 * # Why this file exists rather than a note in the roadmap
 *
 * The bands were guessed round numbers first, and the guesses were wrong in
 * both directions -- a 1,200 px window was handed Relaxed, whose deck is
 * 1,088 px against about 990 of room, and a 900 px window was handed Pro
 * Dense, which needs 980. Both clipped the deck's channel strip, which is the
 * exact failure the bands exist to prevent. Rust's own test asserts which band
 * a height gets; only a browser can say whether the deck that band produces
 * actually fits, and that is the claim that was wrong.
 */
import { expect, test } from "@playwright/test";

import { openShell } from "./shell";

/**
 * Below this the interface does not fit at any density djmanzo will use.
 *
 * The floor is arithmetic rather than policy: at the densest band a deck is
 * 685 px and the pinned chrome around it is about 210, so a window shorter
 * than roughly 900 cannot show a whole deck however far it is scaled. djmanzo
 * opens at 800, so this is not a hypothetical -- it is the open half of Phase
 * 3, recorded in `budget.spec.ts` as a failing test.
 */
const FITS_FROM = 1000;

test.describe("the interface adapts to the window", () => {
  test("a deck fits the stage at every window tall enough for one", async ({
    page,
  }, info) => {
    await openShell(page, "/");
    const measured: string[] = [];
    const tooTall: string[] = [];

    for (const height of [1000, 1100, 1200, 1400, 1600]) {
      await page.setViewportSize({ width: 1280, height });
      // Two frames, so the resize listener has run and layout has settled.
      await page.evaluate(
        () =>
          new Promise((done) =>
            requestAnimationFrame(() => requestAnimationFrame(done)),
          ),
      );
      const seen = await page.evaluate(() => ({
        density: getComputedStyle(document.documentElement)
          .getPropertyValue("--density")
          .trim(),
        deck: document.querySelector(".deck")!.getBoundingClientRect().height,
        stage: document.querySelector(".stage")!.getBoundingClientRect().height,
      }));
      measured.push(
        `${height}: density ${seen.density}, deck ${Math.round(seen.deck)}, ` +
          `stage ${Math.round(seen.stage)}`,
      );
      // A pixel of slack for a fractional layout, not for a band being wrong.
      if (seen.deck > seen.stage + 2) {
        tooTall.push(`${height}px window: deck ${Math.round(seen.deck)} in a stage of ${Math.round(seen.stage)}`);
      }
    }

    await info.attach("density by window height", {
      body: measured.join("\n"),
      contentType: "text/plain",
    });
    expect(
      tooTall,
      `at these heights djmanzo chose a density whose deck does not fit, so the ` +
        `channel strip is clipped -- which is the failure the bands exist to ` +
        `prevent. Windows below ${FITS_FROM}px are a separate, recorded problem.`,
    ).toEqual([]);
  });

  /** Denser as the window shrinks, and never the other way. */
  test("a shorter window never gets a looser interface", async ({ page }) => {
    await openShell(page, "/");
    let previous = 0;
    for (const height of [800, 1000, 1100, 1400, 1600]) {
      await page.setViewportSize({ width: 1280, height });
      await page.evaluate(
        () =>
          new Promise((done) =>
            requestAnimationFrame(() => requestAnimationFrame(done)),
          ),
      );
      const density = Number(
        await page.evaluate(() =>
          getComputedStyle(document.documentElement)
            .getPropertyValue("--density")
            .trim(),
        ),
      );
      expect(
        density,
        `a ${height}px window got a denser interface than a shorter one, which ` +
          "means the band table is not in order",
      ).toBeGreaterThanOrEqual(previous);
      previous = density;
    }
  });
});
