/**
 * §27's ghost, and the two lines that govern it.
 *
 * > Make the future visible. The DJ should understand: "If I bring this in
 * > here, this is what happens." **before loading or committing.**
 *
 * So the tests that matter are not about the arithmetic — where the mix starts
 * and where the candidate's first phrase lands are Rust's, and are tested in
 * `dj_app::ghost`. They are about the two properties a type-check cannot see:
 * that the overlay is **drawn** rather than described, and that asking for it
 * **loads nothing and arms nothing**, which is the whole of "non-destructive".
 *
 * And one more, which is the reason §27 is not simply five marks: the two
 * things djmanzo cannot see are said out loud. An overlay quietly showing five
 * of the seven reads as a record with no vocal and no drop.
 */
import { expect, test } from "@playwright/test";

import { ANSWERS, errorsThrown, openShell } from "./shell";

const RAIL = '.surface[data-surface="next"]';

async function railOpen(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Next", exact: true }).click();
  await expect(page.locator(RAIL)).toBeVisible();
}

/** The ghost button on the first candidate. */
function ask(page: import("@playwright/test").Page) {
  return page.locator(`${RAIL} li`).first().getByRole("button", { name: /^Preview / });
}

/** What the page has asked Rust for since `watch` was called. */
async function watch(page: import("@playwright/test").Page) {
  await page.evaluate(() => ((window as unknown as Record<string, unknown>).__asked = []));
  return () =>
    page.evaluate(() => (window as unknown as { __asked: string[] }).__asked);
}

test.describe("§27's ghost", () => {
  /**
   * **It is drawn, not described.**
   *
   * The failure this exists to catch is the panel shipping as a paragraph of
   * numbers. §27 asks for an *overlay*: a band on a record, in the colour
   * reserved for what djmanzo proposes. A ghost that is only prose is a ghost
   * a DJ has to read instead of glance at, which is the wrong thirty seconds.
   */
  test("puts a band on the record rather than a paragraph beside it", async ({
    page,
  }) => {
    await railOpen(page);
    await ask(page).click();

    const band = page.locator(`${RAIL} .ghost-band[data-layer="suggestion"]`);
    await expect(band).toBeVisible();
    const over = await page.locator(`${RAIL} .overview`).boundingBox();
    const box = await band.boundingBox();
    expect(box, "the ghost band has no box").not.toBeNull();

    /*
      Measured as a fraction of the record rather than in pixels, and this is
      the assertion the first draft got wrong: it asked for a width greater
      than one pixel, which a band of *zero* width satisfies out of its own
      two-pixel borders. Setting the width to nothing left the test green.

      So the numbers are the stub's own arithmetic instead. The record is
      12 000 000 frames; the mix runs 10 800 000 to 11 568 000 and the
      candidate's first full phrase lands at 10 992 000 -- 90.0%, 96.4% and
      91.6% of the way through. A band drawn anywhere else, or at any other
      scale, is not this mix.
    */
    const left = (box!.x - over!.x) / over!.width;
    const width = box!.width / over!.width;
    expect(left, "the ghost is not where djmanzo said the mix starts").toBeCloseTo(
      0.9,
      2,
    );
    expect(
      width,
      "the ghost covers the wrong amount of record, or none at all",
    ).toBeCloseTo(0.064, 2);

    // And the landing: where the candidate's first full phrase arrives. A
    // position, drawn at a position, rather than "8 beats" in a sentence.
    const landing = page.locator(`${RAIL} .ghost-landing[data-layer="suggestion"]`);
    await expect(landing).toBeVisible();
    const at = await landing.boundingBox();
    expect(
      (at!.x - over!.x) / over!.width,
      "the first phrase does not land where djmanzo said it would",
    ).toBeCloseTo(0.916, 2);
    expect(
      at!.x,
      "the first phrase lands before the mix begins, which is not a lead-in",
    ).toBeGreaterThan(box!.x);
    expect(
      at!.x,
      "the lead-in mark is past the end of the mix it is inside",
    ).toBeLessThan(box!.x + box!.width);
    expect(errorsThrown(page), "the ghost threw while rendering").toEqual([]);
  });

  /**
   * **Non-destructive, which is §27's own word.**
   *
   * Asking what would happen must not load a record, arm a transition or write
   * anything down. This is the test that stops the ghost quietly becoming a
   * load button with a preview attached — the failure that would only show up
   * in a booth, once, badly.
   */
  test("loads nothing and arms nothing", async ({ page }) => {
    await railOpen(page);
    const asked = await watch(page);
    await ask(page).click();
    await expect(page.locator(`${RAIL} .ghost-band`)).toBeVisible();

    const commands = await asked();
    expect(commands, "the ghost never asked Rust anything").toContain(
      "ghost_preview",
    );
    for (const forbidden of [
      "load_track",
      "transition_arm",
      "transition_adjust",
      "sidelist_add",
      "deck_play",
    ]) {
      expect(
        commands,
        `asking what would happen called ${forbidden}, which is not "non-destructive"`,
      ).not.toContain(forbidden);
    }
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **All seven, so the line is gone.**
   *
   * This test was *says which of the seven it cannot answer* for as long as
   * there were any: two at first, one once §75's trajectory answered the drop,
   * and none now that `dj_analysis::voice` answers the vocal entry. A panel
   * that went on naming an answered question as missing would be exactly as
   * wrong as one that dropped an unanswered one, so the absence is asserted
   * rather than assumed.
   */
  test("nothing in the seven is still listed as unanswerable", async ({
    page,
  }) => {
    await railOpen(page);
    await ask(page).click();
    await expect(page.locator(`${RAIL} .ghost-what`)).toBeVisible();
    await expect(page.locator(`${RAIL} .ghost-line.unseen`)).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **And the machinery for saying so still works.**
   *
   * The line above is empty because the answers are all in, not because
   * nothing renders it — and those are indistinguishable from the outside.
   * `Asked::answered` is derived from §25's layer table, so a layer that
   * stopped being drawn would put a question back on this list; this drives
   * that state deliberately, because the day it happens is not the day to
   * discover the panel had quietly lost the ability to say so.
   */
  test("a question djmanzo cannot answer is still named out loud", async ({
    page,
  }) => {
    const ghost = ANSWERS.ghost_preview as { asked: { slug: string }[] };
    await openShell(page, "/", {}, {
      ghost_preview: {
        ...ghost,
        asked: ghost.asked.map((one) =>
          one.slug === "vocal-entry" ? { ...one, answered: false } : one,
        ),
      },
    });
    await page.getByRole("button", { name: "Next", exact: true }).click();
    await expect(page.locator(RAIL)).toBeVisible();
    await ask(page).click();

    const unseen = page.locator(`${RAIL} .ghost-line.unseen`);
    await expect(unseen).toBeVisible();
    expect(await unseen.textContent()).toContain("where the vocal enters");
  });

  /**
   * **And §27's seventh is a mark on the overlay, not a sentence under it.**
   *
   * The same rule the drop is held to below, and the same reason: §27 asks for
   * these as part of the *overlay*, and a frame number in a caption is a
   * number about a place the eye then has to go and find. The fixture puts the
   * entry at a different frame from the drop and from the landing, so a view
   * that drew any one of the three twice fails here.
   */
  test("the candidate's vocal entry is marked on the ghost", async ({
    page,
  }) => {
    await railOpen(page);
    await ask(page).click();

    const overview = page.locator(`${RAIL} .overview`).first();
    await expect(overview).toBeVisible();
    const marks = overview.locator('.ghost-voice[data-layer="suggestion"]');
    await expect(marks).toHaveCount(1);

    const box = await overview.boundingBox();
    const mark = await marks.boundingBox();
    expect(box).not.toBeNull();
    expect(mark).not.toBeNull();
    // 11_184_000 of 12_000_000 frames: halfway through a blend that runs from
    // 10.8M to 11.568M.
    expect((mark!.x - box!.x) / box!.width).toBeCloseTo(0.932, 2);
    // A tick rather than a full-height line, so it is not read as a drop.
    expect(mark!.height).toBeLessThan(box!.height);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **And the drop is drawn on the ghost rather than described beside it.**
   *
   * §27 asks for the drop as part of the *overlay*. A number in a line of text
   * under the rail is a number about a place the eye then has to go and find,
   * which is the difference between an overlay and a caption.
   */
  test("the candidate's drop is marked on the ghost", async ({ page }) => {
    await railOpen(page);
    await ask(page).click();

    const overview = page.locator(`${RAIL} .ghost .overview`);
    await expect(overview).toBeVisible();
    const mark = overview.locator(".ghost-drop");
    await expect(mark, "§27's drop is answered and nothing draws it").toHaveCount(1);

    // On the record, where the fixture puts it: 10,992,000 of 12,000,000.
    const box = await overview.boundingBox();
    const at = await mark.boundingBox();
    expect(box).not.toBeNull();
    expect(at).not.toBeNull();
    expect((at!.x - box!.x) / box!.width).toBeCloseTo(10_992_000 / 12_000_000, 1);
  });

  /**
   * The movement §27 asks for in words: what the tempo does and what the keys
   * do. Both, and the pitch as a percentage — "+3 BPM" is what the record is,
   * "-2.3%" is what the hand does, and a DJ wants the second one.
   */
  test("says what the tempo and the keys would do", async ({ page }) => {
    await railOpen(page);
    await ask(page).click();

    const move = page.locator(`${RAIL} .ghost-move`);
    await expect(move).toBeVisible();
    const said = (await move.textContent()) ?? "";
    expect(said, "no BPM movement").toMatch(/[+-]?\d+(\.\d+)?\s*BPM/);
    expect(said, "no pitch movement, so nothing says what the fader does").toMatch(
      /%\s*pitch/,
    );
    expect(said, "no key relationship").toContain("neighbour");

    // And the lead-in, which is the alignment said in words beside the mark.
    await expect(
      page.locator(`${RAIL} .ghost`).getByText(/beats of lead-in/),
    ).toBeVisible();
  });

  /**
   * A second press closes it. One ghost at a time: §27 is a question about a
   * *pair*, and two open at once would be two answers to "what happens next"
   * with nothing saying which was which.
   */
  test("closes on a second press, and only one is open at a time", async ({
    page,
  }) => {
    await railOpen(page);
    await ask(page).click();
    await expect(page.locator(`${RAIL} .ghost-band`)).toBeVisible();

    await ask(page).click();
    await expect(page.locator(`${RAIL} .ghost`)).toHaveCount(0);

    await ask(page).click();
    await page
      .locator(`${RAIL} li`)
      .nth(1)
      .getByRole("button", { name: /^Preview / })
      .click();
    await expect(page.locator(`${RAIL} .ghost`)).toHaveCount(1);
    expect(errorsThrown(page)).toEqual([]);
  });
});

/**
 * §22's *audition* — the sixth of the six things the rail offers, and the only
 * one that makes a sound.
 *
 * The arithmetic is Rust's and is tested in `dj_app::audition` and
 * `dj_engine::preview`: which landmark a record opens at, and that the audio
 * reaches the cue pair and has no route to the room. What a browser can prove
 * is the part a type-check cannot see — that the button is a **toggle**, that
 * it **says where it started**, and that auditioning is not quietly a load.
 */
test.describe("§22's audition", () => {
  /** The audition button on the first candidate. */
  function listen(page: import("@playwright/test").Page) {
    return page
      .locator(`${RAIL} li`)
      .first()
      .getByRole("button", { name: /^Audition |^Stop auditioning / });
  }

  /** Every track id `audition` has been called with, oldest first. */
  const calls = (page: import("@playwright/test").Page) =>
    page.evaluate(() => (window as unknown as { __auditioned?: string[] }).__auditioned ?? []);

  /**
   * **The load-bearing one: it says where it started.**
   *
   * A preview that opens an unfamiliar record ninety seconds in without
   * saying so is indistinguishable, to the DJ hearing it, from a preview that
   * opened the wrong record. The wording is Rust's -- `says` -- because the
   * decision is made there and two spellings of "from the drop" would be two
   * answers.
   */
  test("says where the audition started and why", async ({ page }) => {
    await railOpen(page);
    await listen(page).click();

    const line = page.locator(`${RAIL} [data-audition]`).first();
    await expect(line).toBeVisible();
    await expect(line).toContainText("from the drop");
    // 88.2 seconds, as `m:ss`. The fixture's frame is deliberately not zero:
    // a rail that dropped the position would still pass on the words alone.
    await expect(line).toContainText("1:28");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A second press stops it.**
   *
   * The failure this catches leaves a record playing in a DJ's headphones with
   * no way to stop it but auditioning something else. Asserted on what reached
   * Rust rather than on the button's own class, because a toggle that looked
   * off and never sent the stop is exactly the bug.
   */
  test("a second press stops the audition", async ({ page }) => {
    await railOpen(page);
    await listen(page).click();
    await expect(page.locator(`${RAIL} [data-audition]`).first()).toBeVisible();

    await listen(page).click();
    await expect(page.locator(`${RAIL} [data-audition]`)).toHaveCount(0);

    const sent = await calls(page);
    expect(sent.length, "the toggle sent one call for two presses").toBe(2);
    expect(sent[0], "the first press did not name the candidate").not.toBe("");
    expect(sent[1], "the second press did not stop anything").toBe("");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Auditioning is not loading.**
   *
   * §22 lists audition and load as two of six separate things a DJ may do to a
   * candidate. A preview that quietly counted as a play would put every record
   * a DJ listened to into their history and into §12's taste -- the same
   * failure the ghost's own test above exists to prevent, one sense over.
   */
  test("hearing a candidate does not load or stage it", async ({ page }) => {
    await railOpen(page);
    const asked = await watch(page);
    await listen(page).click();
    await expect(page.locator(`${RAIL} [data-audition]`).first()).toBeVisible();

    const commands = await asked();
    expect(commands, "the audition never asked Rust anything").toContain("audition");
    for (const forbidden of ["load_track", "sidelist_add", "deck_play", "transition_arm"]) {
      expect(
        commands,
        `auditioning called ${forbidden}, which is a different one of §22's six`,
      ).not.toContain(forbidden);
    }
    expect(errorsThrown(page)).toEqual([]);
  });
});
