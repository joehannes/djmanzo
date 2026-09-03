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
 * # A limitation this harness used to have, and what it cost
 *
 * For three runs the deck measured here **had no pad zone** -- no page strip,
 * no eight-pad grid -- and the numbers below were quoted with a note admitting
 * they were floors rather than values. It was found by a human comparing a
 * screenshot with the running application, not by anything here.
 *
 * The cause turned out to be one line in `shell.ts`. The stub answers a
 * command it does not know with `null`, deliberately; `stems_status` was not
 * in its list; `Stems.svelte` reads `status.available` straight off the answer,
 * because the application's own type is not optional and cannot be null there.
 * So the deck's subtree threw, Svelte abandoned the rest of that render pass,
 * and the pad zone never appeared -- while every assertion in this file stayed
 * green, because a shorter deck is not a taller one.
 *
 * Two things came out of it. The stub answers `stems_status`, and `openShell`
 * now collects what the page threw so a test can refuse it: a thrown error is
 * the one signal that separates "this layout is fine" from "this layout did
 * not finish".
 *
 * **The numbers moved a long way when it was fixed.** A deck went from 675 px
 * to 878, the pad zone being 197 of that, which is close to the two hundred
 * that had been guessed from a screenshot. Everything below is measured
 * against the deck djmanzo actually draws.
 *
 * # Why geometry and not the template
 *
 * Asserting that the markup contains a crossfader is a test that passes while
 * the crossfader is 900 px below the window. Only rendered geometry can fail
 * for the right reason, which is why this runs in a real browser doing real
 * layout rather than in jsdom.
 */
import { type Page, expect, test } from "@playwright/test";

import { SLACK, WINDOW, centreOf, errorsThrown, openShell, within } from "./shell";

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
  test("every control this budget names is in the interface", async ({ page }) => {
    await openShell(page, "/");
    const { missing } = await survey(page, [...ON_THE_DECK, ...ON_THE_MASTER]);

    expect(
      missing,
      "a control this test names is not in the interface at all -- either it was " +
        "renamed, in which case rename it here too, or it stopped being rendered",
    ).toEqual([]);
  });

  /**
   * Nothing the interface draws may throw while it is being measured.
   *
   * The general form of the bug that hid the pad zone for three runs: a
   * component threw during its render, Svelte abandoned the pass, a whole zone
   * of the deck was absent, and every geometry assertion here stayed green
   * because a shorter deck is not a taller one. Geometry cannot catch a
   * missing thing. This can.
   */
  test("the interface finishes rendering without throwing", async ({ page }) => {
    await openShell(page, "/");
    await expect(page.locator(".deck").first()).toBeVisible();

    expect(
      errorsThrown(page),
      "the interface threw while it was being measured, so some part of it did " +
        "not finish rendering and every figure below is measuring a screen " +
        "djmanzo does not draw",
    ).toEqual([]);
  });

  /**
   * **Fixed, and this is what holds it fixed.**
   *
   * These two were at y 873 and y 915 on a window djmanzo opens 800 px tall --
   * not the crossfader below the decks, the channel controls *on* the deck,
   * which are reached for continuously rather than at a transition.
   *
   * Three things brought the deck down from 878 px to 685: the pad grid
   * stopped taking its height from the deck's *width*, the SVG controls
   * started answering to `--density`, and djmanzo picks a density that fits
   * the window. That was not enough -- the stage has 559 -- so the deck now
   * does what the master strip does one level up: **its body scrolls and its
   * channel strip is pinned.** The waveform and the pads go below the fold on
   * a short window instead of the volume fader and the filter.
   *
   * Measured against `.stage` rather than the window, because that is the box
   * that clips a deck. An earlier version of this test asked for a page
   * coordinate under 800 and passed while both controls sat behind the pinned
   * master strip; a screenshot showed it and the assertion could not.
   */
  test("every control on a deck is on the first screen", async ({ page }, info) => {
    await openShell(page, "/");

    const offscreen: string[] = [];
    const measured: string[] = [];
    for (const { role, name } of ON_THE_DECK) {
      const centre = await centreOf(page, role, name);
      const inside = await within(page, ".stage", role, name);
      measured.push(`${name}: y ${centre ? Math.round(centre.y) : "?"}, ${inside ? "reachable" : "clipped"}`);
      if (!inside) offscreen.push(name);
    }
    await info.attach("where the deck controls landed", {
      body: measured.join("\n"),
      contentType: "text/plain",
    });

    expect(
      offscreen,
      "these controls are outside the part of the stage a DJ can see, so " +
        "reaching them means scrolling mid-mix",
    ).toEqual([]);
  });

  /**
   * **Fixed, and this is what holds it fixed.**
   *
   * The crossfader has ended up below the fold three times in three different
   * forms, and the first two were found by a human with a screenshot. Most
   * recently it was about 280 px past the bottom -- a figure that read
   * seventy-seven until the harness stopped losing the deck's pad zone.
   *
   * Two changes brought it back. The deck shrank, because density finally
   * reaches it. And the master strip came *out of the scrolling stage*: it sat
   * inside, under decks taller than the room the stage has, so it scrolled
   * away with them. Nothing moved in the reading order -- deck, crossfader,
   * deck -- it simply stopped being part of what scrolls, which is what every
   * DJ application does with its mixer.
   *
   * Measured against the window rather than against a container, because being
   * pinned is exactly the claim: wherever the decks are scrolled to, these are
   * where a DJ left them.
   */
  test("the master strip is on it too", async ({ page }, info) => {
    await openShell(page, "/");

    const offscreen: string[] = [];
    const measured: string[] = [];
    for (const { role, name } of ON_THE_MASTER) {
      const centre = await centreOf(page, role, name);
      const inside = await within(page, "window", role, name);
      measured.push(`${name}: y ${centre ? Math.round(centre.y) : "?"}, ${inside ? "reachable" : "off the window"}`);
      if (!inside) offscreen.push(name);
    }
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
    // To the console as well as the report: on CI the report is an artifact
    // nobody downloads, and this number is the whole point of the run.
    console.log(`deck column height: ${height}px (budget 760)`);

    // 760 against the 685 it measures here -- a ratchet, not a target, and
    // deliberately slack. Two reasons for the 75 px of room: the number that
    // matters is a *regression*, and the two on record were +156 and +117, not
    // +20; and CI runs a different Chromium build with a different font stack,
    // on which this has never been measured. The height is printed above on
    // every run, so the runner's own figure is in the log and this can be
    // tightened on evidence rather than on a guess.
    //
    // This number has been 740, then 940, and is now 760, and only the last
    // move was the deck itself changing. 740 was measured against a deck the
    // harness drew without its pad zone; 940 was that same deck measured
    // honestly; 760 is the deck after the pad grid stopped taking its height
    // from the deck's width and the SVG controls started answering to
    // `--density`. Every performing control is on screen at 1280x800 here.
    expect(
      height,
      "the deck column has grown past where it already was, which is how the " +
        "crossfader ended up below the fold all three times",
    ).toBeLessThanOrEqual(760);
  });
});


/**
 * The same budget, in the shapes the window is actually put into.
 *
 * # Why this block exists
 *
 * Everything above measures one configuration: two decks, nothing docked, at
 * the window djmanzo opens at. That is the right default to hold and it is
 * not the whole claim. The deck-count button offers four and six; the dock
 * manager exists so a DJ can keep the browser along the bottom and Prepare
 * beside the decks *while mixing*. A budget that only holds for the opening
 * screenshot is a budget for the screenshot.
 *
 * It was found the way the last three were -- by driving the running
 * application, not by anything in this file. Two faults, one visible:
 *
 * - `.decks.four` and `.decks.six` set `grid-auto-rows: min-content`, meaning
 *   "let the extra rows be as tall as they need and scroll". Nothing scrolls
 *   them: the stage does not scroll, by design, because that is what lets a
 *   deck pin anything. So the free space went to whichever row was *not*
 *   `min-content`. Four decks at 1280x800 gave row one **115 px** and row two
 *   433. With a surface docked, 22 px against 565.
 * - The pinned channel strip is `flex: none`, and a `flex: none` region in a
 *   column with less room than it wants does not scroll -- it overflows. Row
 *   one's 168 px foot ran a hundred pixels down into the deck below it, and
 *   with four decks and a dock the 300 px foot ran 328 px past the card and
 *   straight across the master strip.
 *
 * Both are the §99/§103 failure in a third shape. Pinning a region guarantees
 * that region is on screen; on its own it guarantees nothing about what the
 * guarantee costs the regions around it. What follows measures the cost.
 */
type Shape = { decks: 2 | 4 | 6; docked: boolean; height: number };

/** The configurations a DJ can reach from the top bar in one or two presses. */
const SHAPES: Shape[] = [
  { decks: 2, docked: false, height: 800 },
  { decks: 2, docked: true, height: 800 },
  { decks: 2, docked: true, height: 680 },
  { decks: 4, docked: false, height: 800 },
  { decks: 4, docked: true, height: 800 },
  { decks: 6, docked: true, height: 800 },
];

const describes = ({ decks, docked, height }: Shape) =>
  `${decks} decks, ${docked ? "a surface docked" : "nothing docked"}, ${WINDOW.width}x${height}`;

/** Put the shell into `shape` and let the layout settle. */
async function shaped(page: Page, { decks, docked, height }: Shape) {
  await openShell(page, "/");
  await page.setViewportSize({ width: WINDOW.width, height });
  if (docked) await page.getByRole("button", { name: "Prepare", exact: true }).click();
  // The button cycles 2 -> 4 -> 6 and names the count it is showing.
  for (let n = 2; n < decks; n += 2) {
    await page.getByRole("button", { name: /decks$/ }).click();
  }
  // Two frames: one for the change to mount, one for everything else to
  // reflow into what is left. Measuring after the first measures a layout
  // half way through moving.
  await page.evaluate(
    () => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))),
  );
}

/** Every deck's box, every foot's box, and the master strip's, in one round trip. */
async function frames(page: Page) {
  return page.evaluate(() => {
    const box = (el: Element) => {
      const r = el.getBoundingClientRect();
      return { top: Math.round(r.y), height: Math.round(r.height), bottom: Math.round(r.bottom) };
    };
    const bridge = document.querySelector(".bridge");
    return {
      decks: [...document.querySelectorAll(".deck")].map((deck) => ({
        deck: box(deck),
        body: box(deck.querySelector(".deck-body")!),
        foot: box(deck.querySelector(".deck-foot")!),
        pinned: deck.querySelector(".deck-foot")!.children.length > 0,
      })),
      bridge: bridge ? box(bridge) : null,
    };
  });
}

test.describe("the layout budget, in every shape the top bar offers", () => {
  for (const shape of SHAPES) {
    test(`nothing on a deck is drawn outside it: ${describes(shape)}`, async ({ page }) => {
      await shaped(page, shape);
      const { decks } = await frames(page);
      expect(decks.length, "the decks did not render").toBe(shape.decks);

      for (const [index, { deck, foot }] of decks.entries()) {
        expect(
          foot.bottom,
          `deck ${index + 1}'s pinned strip ends ${foot.bottom - deck.bottom} px below ` +
            "the bottom of the deck it is pinned to. A region that cannot shrink, " +
            "in a column with less room than it wants, does not scroll -- it " +
            "overflows, and paints over whatever is drawn below it",
        ).toBeLessThanOrEqual(deck.bottom + SLACK);
      }
    });

    test(`no deck paints over the master strip: ${describes(shape)}`, async ({ page }) => {
      await shaped(page, shape);
      const { decks, bridge } = await frames(page);
      expect(bridge, "the master strip is not on the page").not.toBeNull();

      for (const [index, { deck, foot }] of decks.entries()) {
        expect(
          Math.max(deck.bottom, foot.bottom),
          `deck ${index + 1} reaches into the master strip. This is the failure ` +
            "that put the crossfader below the fold twice, arriving from the " +
            "other direction: not off the screen, but on top of something else",
        ).toBeLessThanOrEqual(bridge!.top + SLACK);
      }
    });

    /**
     * Every deck gets the same room, whatever the row it landed in.
     *
     * The measurement that caught `grid-auto-rows: min-content`: two rows of
     * the same thing came out 115 px and 433 px, which is not a layout anyone
     * chose. `1fr` rows cannot do that, so this fails the moment one comes
     * back.
     */
    test(`the deck rows share the stage evenly: ${describes(shape)}`, async ({ page }) => {
      await shaped(page, shape);
      const { decks } = await frames(page);
      const heights = decks.map(({ deck }) => deck.height);

      expect(
        Math.max(...heights) - Math.min(...heights),
        `the decks came out ${heights.join(", ")} px tall. Rows of the same ` +
          "thing should be the same size; a row that is starved is one whose " +
          "height came from its content rather than from the stage",
      ).toBeLessThanOrEqual(SLACK);
    });

    /**
     * A deck either pins its channel strip and keeps a usable waveform, or it
     * pins nothing and scrolls as one column. There is no third state where
     * the strip has three quarters of the deck.
     */
    test(`what scrolls is worth scrolling: ${describes(shape)}`, async ({ page }) => {
      await shaped(page, shape);
      const { decks } = await frames(page);

      for (const [index, { deck, body, pinned }] of decks.entries()) {
        if (!pinned) continue;
        expect(
          body.height,
          `deck ${index + 1} pinned its channel strip and left ${body.height} px ` +
            `of ${deck.height} for everything else. The waveform, the overview, ` +
            "the pads, the loops, the FX rack and the transport do not fit in " +
            "that, and the waveform is the one that pays",
        ).toBeGreaterThanOrEqual(140 - SLACK);
      }
    });
  }

  /**
   * The default is still the default.
   *
   * The rule above turns pinning off when a deck is too short for it, and a
   * rule that turned it off everywhere would pass every assertion in this
   * block while quietly undoing §103. This is the one shape where the strip
   * must actually be pinned.
   */
  test("two decks at the opening window still pin the channel strip", async ({ page }) => {
    await shaped(page, { decks: 2, docked: false, height: WINDOW.height });
    const { decks } = await frames(page);

    for (const [index, { pinned }] of decks.entries()) {
      expect(
        pinned,
        `deck ${index + 1} is not pinning its channel strip at djmanzo's own ` +
          "window with nothing docked. The volume fader and the filter are " +
          "back below the fold, which is what §103 was",
      ).toBe(true);
    }
  });
});
