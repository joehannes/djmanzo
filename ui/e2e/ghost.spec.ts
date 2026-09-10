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

import { errorsThrown, openShell } from "./shell";

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
   * **The two djmanzo cannot see are named.**
   *
   * §27 asks for seven things and nothing in `dj_analysis` finds a vocal entry
   * or a drop. Saying so is the difference between an honest overlay and one
   * that reads as a record with neither.
   */
  test("says which of the seven it cannot answer", async ({ page }) => {
    await railOpen(page);
    await ask(page).click();

    const unseen = page.locator(`${RAIL} .ghost-line.unseen`);
    await expect(unseen).toBeVisible();
    const said = await unseen.textContent();
    expect(said, "the vocal entry is dropped silently").toContain(
      "where the vocal enters",
    );
    expect(said, "the drop is dropped silently").toContain(
      "where the drop occurs",
    );
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
