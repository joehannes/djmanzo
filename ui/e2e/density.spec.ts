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
import { type Page, expect, test } from "@playwright/test";

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

/**
 * Wait until the layout has stopped moving.
 *
 * Two animation frames is not enough, and the failure is instructive: changing
 * the density band rewrites the root font size, every `rem` in the interface
 * reflows, and a measurement taken in the middle of that reads a deck body
 * that needs 591 px inside a stage that has 534 -- neither the old layout nor
 * the new one. The test was green at first and went red on an unrelated
 * change, which is what a race looks like.
 *
 * So this waits for the number to stop changing rather than for a fixed count
 * of frames. Bounded, so a genuinely oscillating layout fails the test rather
 * than hanging it.
 */
async function settle(page: Page) {
  await page.evaluate(async () => {
    const frame = () => new Promise((done) => requestAnimationFrame(done));
    const height = () =>
      (document.querySelector(".deck .deck-body") as HTMLElement).clientHeight;
    let previous = -1;
    for (let tries = 0; tries < 30; tries += 1) {
      await frame();
      const now = height();
      if (now === previous) return;
      previous = now;
    }
  });
}

test.describe("the interface adapts to the window", () => {
  test("a deck shows all of itself at every window tall enough for one", async ({
    page,
  }, info) => {
    await openShell(page, "/");
    const measured: string[] = [];
    const scrolling: string[] = [];

    for (const height of [1000, 1020, 1100, 1200, 1300, 1500, 1600, 1700]) {
      await page.setViewportSize({ width: 1280, height });
      await settle(page);
      // The deck's *content* against the room it has, not the deck against the
      // stage -- the deck is bounded by the stage now, so that comparison is
      // true by construction and would assert nothing. What can still be
      // wrong is a band whose deck does not fit inside itself.
      const seen = await page.evaluate(() => {
        const body = document.querySelector(".deck .deck-body") as HTMLElement;
        return {
          density: getComputedStyle(document.documentElement)
            .getPropertyValue("--density")
            .trim(),
          content: body.scrollHeight,
          room: body.clientHeight,
        };
      });
      measured.push(
        `${height}: density ${seen.density}, deck body needs ${seen.content}, ` +
          `has ${seen.room}`,
      );
      // A pixel of slack for a fractional layout, not for a band being wrong.
      if (seen.content > seen.room + 2) {
        scrolling.push(
          `${height}px window: the deck needs ${seen.content} and has ${seen.room}`,
        );
      }
    }

    await info.attach("density by window height", {
      body: measured.join("\n"),
      contentType: "text/plain",
    });
    expect(
      scrolling,
      `at these heights djmanzo chose a density whose deck still does not fit, ` +
        `so the waveform or the pads are below the fold -- which is the failure ` +
        `the bands exist to prevent. Windows below ${FITS_FROM}px are a ` +
        `separate, recorded problem.`,
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
