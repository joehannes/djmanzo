/**
 * The pair view, and the object it draws.
 *
 * §20's fourth view asks for two records side by side with the seam between
 * them; §68 asks for the transition to be an explicit object rather than an
 * answer that lives for one call. The two arrive together, because an object
 * nothing draws is a promise and a view with nothing behind it is a table.
 *
 * What is measured here is the part a type-check cannot see: that the panel
 * opens, that both records and the seam are actually on screen, that the
 * adjustments are refused until a transition is being held, and that pressing
 * one changes what is drawn. The arithmetic behind the adjustment is Rust's
 * and is tested there -- see `dj_app::transition`. What a browser can prove is
 * that the press reaches it and the answer comes back.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Open the shell with the pair view docked. */
async function pairOpen(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Pair", exact: true }).click();
  await expect(page.locator('.surface[data-surface="pair"]')).toBeVisible();
}

const PAIR = '.surface[data-surface="pair"]';

test.describe("the pair view", () => {
  /**
   * Nothing is held when djmanzo starts, and the panel says so in words that
   * tell a DJ what to do about it. An empty panel that merely looks broken is
   * the failure this asserts against.
   */
  test("opens saying what it needs, rather than empty", async ({ page }) => {
    await pairOpen(page);

    await expect(page.locator(`${PAIR} .empty`)).toContainText("Compare");
    expect(errorsThrown(page), "the pair view threw while rendering").toEqual([]);
  });

  /**
   * **Both records and the seam, which is the whole view.**
   *
   * A pair view that draws the two records and not what happens between them
   * is the browser at two rows. The deltas line is the thing a DJ reads first:
   * what the tempo does, what the keys do, and how well the two go together.
   */
  test("draws both records and the seam between them", async ({ page }) => {
    await pairOpen(page);
    await page.getByRole("button", { name: "Compare", exact: true }).click();

    await expect(page.locator(`${PAIR} .side`)).toHaveCount(2);
    await expect(page.locator(`${PAIR} .side h3`).first()).toHaveText("Bachata Rosa");
    await expect(page.locator(`${PAIR} .side h3`).nth(1)).toContainText("Ojal");

    const deltas = page.locator(`${PAIR} .deltas`);
    await expect(deltas).toContainText("BPM");
    await expect(
      deltas,
      "the seam does not say what the keys do, which is half of what a DJ is " +
        "comparing two records for",
    ).toContainText("8A → 9A");
    await expect(page.locator(`${PAIR} .when`)).toContainText("beats");
    await expect(
      page.locator(`${PAIR} .why li`).first(),
      "the transition does not say why it is where it is",
    ).toContainText("phrase start");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **An opinion is not a held transition, and the panel does not pretend.**
   *
   * Comparing plans without holding anything, so there is nothing to adjust
   * and the controls say so by being unavailable. A panel offering buttons
   * that quietly do nothing is how a DJ learns to distrust the whole surface.
   */
  test("the adjustments wait until a transition is being held", async ({ page }) => {
    await pairOpen(page);
    await page.getByRole("button", { name: "Compare", exact: true }).click();

    const move = page.getByRole("group", { name: "Move the mix point" });
    await expect(move.getByRole("button", { name: "+4" })).toBeDisabled();
    await expect(page.locator(`${PAIR} .hint`)).toContainText("Set it up");

    await page.getByRole("button", { name: "Set up", exact: true }).click();
    await expect(move.getByRole("button", { name: "+4" })).toBeEnabled();
  });

  /**
   * **Setting a transition up does not hand the mix over.**
   *
   * Holding one and performing one are different things, and the panel has to
   * say which. With the automix off nothing will run it — an interface that
   * let "set up" imply "will happen" would be lying at the one moment a DJ is
   * deciding whether to keep their hands free.
   */
  test("it says nothing will run the transition while automix is off", async ({
    page,
  }) => {
    await pairOpen(page);
    await page.getByRole("button", { name: "Set up", exact: true }).click();

    await expect(page.locator(`${PAIR} .hint`)).toContainText("automix is off");
  });

  /** And when the mix *has* been handed over, it says what will happen. */
  test("with automix on it says when it will run", async ({ page }) => {
    await openShell(page, "/", {
      automix: { enabled: true, mixing: false, beats: 16.0, style: "blend" },
    });
    await page.getByRole("button", { name: "Pair", exact: true }).click();
    await page.getByRole("button", { name: "Set up", exact: true }).click();

    const hint = page.locator(`${PAIR} .hint`);
    await expect(hint).toContainText("Automix will run this at");
    await expect(
      hint,
      "it does not say when, which is the whole of what a DJ needs from the line",
    ).toContainText("2:34");
  });

  /**
   * **The mix point is a thing you can grab.**
   *
   * §26 in one sentence: *"The DJ should be able to physically grab the thing
   * they are thinking about. Do not force them to edit a numerical property in
   * a settings panel."* The ±16 buttons are the keyboard path and stay; this is
   * the one a hand takes.
   *
   * What a browser can prove is the gesture: that the mark answers a pointer,
   * that the frame it was dropped on reaches djmanzo, and that the panel then
   * draws the answer rather than the drag. Which beat that frame is belongs to
   * Rust, which has the grid, and is tested there.
   */
  test("the mix point can be dragged along the waveform", async ({ page }) => {
    await pairOpen(page);
    await page.getByRole("button", { name: "Set up", exact: true }).click();
    await expect(page.locator(`${PAIR} .when`)).toContainText("2:34");

    const mark = page.locator(`${PAIR} .mark.grabbable`).first();
    await expect(mark).toBeVisible();
    const box = (await mark.boundingBox())!;
    const y = box.y + box.height / 2;
    await page.mouse.move(box.x + box.width / 2, y);
    await page.mouse.down();
    await page.mouse.move(box.x - 60, y, { steps: 8 });
    await page.mouse.up();

    await expect(
      page.locator(`${PAIR} .when`),
      "the mix point did not move, so the drag reached nothing",
    ).not.toContainText("2:34");
    await expect(
      page.locator(`${PAIR} .edited`),
      "the panel moved the mark itself rather than drawing djmanzo's answer",
    ).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Making a mark grabbable must not move it.**
   *
   * It did. The wider hit target was a `padding-inline` and a matching
   * negative `margin-inline`, and a negative margin shifts the box — so the
   * dashed line was *drawn* six pixels earlier than djmanzo said the mix
   * point was, but only once the transition was armed. Six pixels is about
   * thirty-five milliseconds at this lane's zoom, and it was in the one thing
   * this surface exists to place accurately.
   *
   * The same transition either way — the harness answers `transition_arm`
   * with exactly what `plan_transition` said — so the mark belongs at the same
   * pixel. Measured against the strip's own left edge rather than the
   * viewport's, because the lane scrolls with the record and the two readings
   * are seconds apart.
   */
  test("arming a transition does not move where its mark is drawn", async ({
    page,
  }) => {
    const offset = () =>
      page.evaluate(() => {
        const pair = document.querySelector('.surface[data-surface="pair"]');
        const strip = pair?.querySelector(".strip");
        const mark = pair?.querySelector(".mark");
        if (!strip || !mark) return null;
        return mark.getBoundingClientRect().x - strip.getBoundingClientRect().x;
      });

    await pairOpen(page);
    await page.getByRole("button", { name: "Compare", exact: true }).click();
    await expect(page.locator(`${PAIR} .mark`).first()).toBeVisible();
    const proposed = await offset();

    await page.getByRole("button", { name: "Set up", exact: true }).click();
    await expect(page.locator(`${PAIR} .mark.grabbable`).first()).toBeVisible();
    const armed = await offset();

    expect(proposed).not.toBeNull();
    expect(
      armed!,
      "the mark moved when it became grabbable, so the line is no longer where " +
        "djmanzo put the mix point",
    ).toBeCloseTo(proposed!, 1);
  });

  /**
   * An opinion is not something to grab, for the same reason it is not
   * something to adjust: nothing is holding it.
   */
  test("a transition that is only proposed cannot be dragged", async ({ page }) => {
    await pairOpen(page);
    await page.getByRole("button", { name: "Compare", exact: true }).click();

    await expect(page.locator(`${PAIR} .mark`)).toHaveCount(2);
    await expect(page.locator(`${PAIR} .mark.grabbable`)).toHaveCount(0);
  });

  /**
   * **Pressing a length changes the transition that is drawn.**
   *
   * The round trip: the press goes to djmanzo, the answer comes back, and the
   * panel draws the answer rather than what it hoped for. A view that changed
   * its own state and never asked would look identical until the moment two
   * surfaces disagreed about the same mix.
   */
  test("shortening the mix redraws it from djmanzo's answer", async ({ page }) => {
    await pairOpen(page);
    await page.getByRole("button", { name: "Set up", exact: true }).click();
    await expect(page.locator(`${PAIR} .when`)).toContainText("over 32 beats");

    await page
      .getByRole("group", { name: "How long the mix runs" })
      .getByRole("button", { name: "8", exact: true })
      .click();

    await expect(page.locator(`${PAIR} .when`)).toContainText("over 8 beats");
    await expect(
      page.locator(`${PAIR} .edited`),
      "an adjusted transition does not say it has been adjusted, so a DJ " +
        "cannot tell djmanzo's proposal from their own change to it",
    ).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });
});
