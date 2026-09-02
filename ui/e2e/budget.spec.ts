/**
 * The first screen is for mixing, and this is what says so.
 *
 * # Why this test exists
 *
 * Twice now the controls a DJ performs a mix with have drifted below the fold
 * at djmanzo's own default window size, and twice it was found by a human
 * taking a screenshot. The first time the crossfader was about 1,500 px down --
 * two screens below the waveforms it is used against. It was fixed, and then it
 * grew back to 917, because a control was added to a grid and the grid wrapped.
 *
 * `docs/ROADMAP.md` draws the conclusion itself: *"a layout budget with no test
 * drifts back, and nothing here measures it."* This is the measurement.
 *
 * # Why geometry and not the template
 *
 * Asserting that the markup contains a crossfader is a test that passes while
 * the crossfader is 900 px below the window. Only rendered geometry can fail
 * for the right reason, which is why this runs in a real browser doing real
 * layout rather than in jsdom.
 */
import { type Page, expect, test } from "@playwright/test";

import { SLACK, WINDOW, centreOf, openShell } from "./shell";

/**
 * The controls a DJ actually touches to perform a mix.
 *
 * Not "every control" -- the browser, the sampler and the settings panel are
 * allowed to be anywhere, because a DJ opens those deliberately and can scroll.
 * These are the ones reached for mid-transition, in a dark room, without
 * looking. If one of these is off the screen the application opens at, the
 * first screen is not for mixing.
 */
type Control = { role: "slider" | "button"; name: string };

/** The ones on a deck. */
const ON_THE_DECK: Control[] = [
  { role: "slider", name: "Volume" },
  { role: "slider", name: "Filter" },
];

/** The ones on the master strip below the decks. */
const ON_THE_MASTER: Control[] = [
  { role: "slider", name: "Crossfader" },
  { role: "slider", name: "Master gain" },
];

/** Where each of `controls` sits, and which of them are past the window. */
async function survey(page: Page, controls: Control[]) {
  const offscreen: string[] = [];
  const missing: string[] = [];
  const measured: string[] = [];

  for (const { role, name } of controls) {
    const centre = await centreOf(page, role, name);
    if (!centre) {
      missing.push(name);
      continue;
    }
    measured.push(`${name}: y ${Math.round(centre.y)}, x ${Math.round(centre.x)}`);
    if (centre.y > WINDOW.height + SLACK || centre.x > WINDOW.width + SLACK) {
      offscreen.push(`${name} centre at (${Math.round(centre.x)}, ${Math.round(centre.y)})`);
    }
  }
  return { offscreen, missing, measured };
}

test.describe("the first screen", () => {
  test("every control on a deck is on it", async ({ page }, info) => {
    await openShell(page, "/");
    const { offscreen, missing, measured } = await survey(page, ON_THE_DECK);

    // Attached whether or not it failed, so a run that passes still records
    // how much room was left -- which is what tells you a change ate the
    // margin before the next one goes over it.
    await info.attach("where the deck controls landed", {
      body: measured.join("\n"),
      contentType: "text/plain",
    });

    expect(
      missing,
      "a control this test names is not in the interface at all -- either it was " +
        "renamed, in which case rename it here too, or it stopped being rendered",
    ).toEqual([]);
    expect(
      offscreen,
      `the window djmanzo opens at is ${WINDOW.width}x${WINDOW.height}, and these ` +
        "controls are past it",
    ).toEqual([]);
  });

  /**
   * **A known, measured failure, recorded rather than hidden.**
   *
   * With two records loaded -- which is what the fixture captures, and the only
   * state worth measuring -- a deck column comes to 675 px. The top bar takes
   * 138 and the master strip 110, so the strip starts at about 839 and the
   * crossfader's centre lands at y 877: seventy-seven pixels below the window
   * djmanzo opens itself at. Master gain is beside it and equally gone.
   *
   * This is the third time a performing control has ended up below the fold,
   * and the first two were found by a human with a screenshot. It is here as a
   * running test rather than a line in a document because a document does not
   * notice when it stops being true.
   *
   * `test.fail()` rather than a skip: this asserts the failure is *still*
   * there, so whoever fixes the deck's height gets a red test telling them to
   * delete this line -- instead of a green suite that quietly forgot.
   *
   * Fixing it means finding about 150 px in the deck column, and that is a
   * design decision with several defensible answers -- merging the beat-jump
   * and loop rows, folding the overview the way the stems module folds,
   * shortening the channel fader, tightening the row gaps. The measurement is
   * here; the choice is the owner's.
   */
  test("the master strip is on it too", async ({ page }, info) => {
    test.fail();
    await openShell(page, "/");
    const { offscreen, measured } = await survey(page, ON_THE_MASTER);

    await info.attach("where the master controls landed", {
      body: measured.join("\n"),
      contentType: "text/plain",
    });
    expect(
      offscreen,
      `the window djmanzo opens at is ${WINDOW.width}x${WINDOW.height}, and these ` +
        "controls are past it. A DJ cannot reach a crossfader they have to scroll to.",
    ).toEqual([]);
  });

  /**
   * The page must not scroll sideways at the size it opens at.
   *
   * A separate assertion because horizontal overflow has a different cause than
   * vertical -- something too wide rather than too much stacked -- and lumping
   * them together would report the wrong one.
   */
  test("does not scroll sideways", async ({ page }) => {
    await openShell(page, "/");
    const width = await page.evaluate(() => document.documentElement.scrollWidth);
    expect(
      width,
      "something is wider than the window, so the interface scrolls sideways",
    ).toBeLessThanOrEqual(WINDOW.width + SLACK);
  });

  /**
   * Nothing in the master row sits on top of anything else.
   *
   * This is the bug the one-row rewrite fixed on the way past, and it had been
   * shipped: with a four-channel cue the SPLIT button was drawn at x 666..760
   * and the output meters at x 740..860, over the same rows -- so the meter
   * bars ran behind the button. Nobody saw it because the machine this is
   * built on has no cue device, so the branch that draws SPLIT never ran.
   *
   * Asserted for that pair specifically rather than for every pair in the SVG:
   * a general no-overlap rule would flag the deliberate ones -- a thumb sits on
   * its own track by design -- and a test that has to be taught its exceptions
   * stops being read.
   */
  test("the split-cue button does not sit on the output meters", async ({ page }) => {
    await openShell(page, "/", { cue_available: true, cue_mix: 0.4 });

    const split = await page
      .getByRole("button", { name: "Toggle split cue" })
      .boundingBox();
    const meters = await page.locator(".master-mixer .meters").boundingBox();
    expect(split, "the split-cue button was not drawn").not.toBeNull();
    expect(meters, "the output meters were not drawn").not.toBeNull();

    const apart =
      split!.x + split!.width <= meters!.x ||
      meters!.x + meters!.width <= split!.x ||
      split!.y + split!.height <= meters!.y ||
      meters!.y + meters!.height <= split!.y;

    expect(
      apart,
      `SPLIT (${Math.round(split!.x)}..${Math.round(split!.x + split!.width)}) ` +
        `overlaps the meters (${Math.round(meters!.x)}..` +
        `${Math.round(meters!.x + meters!.width)}) -- the bars are drawn behind the button`,
    ).toBe(true);
  });

  /**
   * The deck column has a budget of its own.
   *
   * This is the number that actually regressed: the crossfader went below the
   * fold *because* the deck above it grew from 539 px to 695. Measuring the
   * cause as well as the symptom means the next failure names the thing that
   * changed rather than the thing that moved.
   */
  test("a deck stays inside its height budget", async ({ page }, info) => {
    await openShell(page, "/");
    const deck = page.locator(".deck").first();
    await deck.waitFor();
    const box = await deck.boundingBox();
    expect(box, "no deck was rendered").not.toBeNull();

    const height = Math.round(box!.height);
    await info.attach("deck height", { body: `${height}px`, contentType: "text/plain" });

    // 690 against the 675 it measures today -- a ratchet, not a target. The
    // number that would put the crossfader back on the screen is about 527,
    // and the test above is what records that gap. This one exists so the
    // column cannot quietly grow *further* while that is being decided.
    expect(
      height,
      "the deck column has grown past where it already was, which is how the " +
        "crossfader ended up below the fold all three times",
    ).toBeLessThanOrEqual(690);
  });
});
