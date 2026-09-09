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
    await expect(page.locator(`${PAIR} .hint`)).toHaveCount(0);
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

  /**
   * **§26: the mix point can be grabbed.**
   *
   * The directive is blunt about this — "the DJ should be able to physically
   * grab the thing they are thinking about. Do not force them to edit a
   * numerical property in a settings panel." The panel's shorten and move
   * buttons are the settings panel; this is the waveform.
   *
   * What is asserted is the round trip, which is the half a type-check cannot
   * see: a drag on the lane reaches Rust and what comes back is what is drawn.
   * The arithmetic of where a mix may go is Rust's and is tested there.
   */
  test("the mix point can be dragged on the waveform", async ({ page }) => {
    await pairOpen(page);
    await page.getByRole("button", { name: "Compare", exact: true }).click();

    // Not until djmanzo is holding it: a proposal is an opinion, and offering
    // a handle for one would be offering a control that does nothing.
    await expect(page.getByRole("slider", { name: /mix in/ })).toHaveCount(0);
    await page.getByRole("button", { name: "Set up", exact: true }).click();

    const handle = page.getByRole("slider", { name: /mix in/ });
    await expect(handle).toBeVisible();
    const before = await handle.getAttribute("aria-valuenow");

    const box = await handle.boundingBox();
    expect(box, "the mix point has no handle to grab").not.toBeNull();
    await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
    await page.mouse.down();
    await page.mouse.move(box!.x + 140, box!.y + box!.height / 2, { steps: 8 });
    await page.mouse.up();

    // Rust answered, and the handle is where Rust put it — which is on a beat,
    // not where the pointer happened to stop.
    await expect
      .poll(async () => handle.getAttribute("aria-valuenow"))
      .not.toBe(before);
    await expect(page.locator(`${PAIR} .edited`)).toBeVisible();
    expect(errorsThrown(page), "the pair view threw while dragging").toEqual([]);
  });

  /** And the same handle answers the keyboard, because a mouse is not the only hand. */
  test("the mix point moves with the arrow keys", async ({ page }) => {
    await pairOpen(page);
    await page.getByRole("button", { name: "Compare", exact: true }).click();
    await page.getByRole("button", { name: "Set up", exact: true }).click();

    const handle = page.getByRole("slider", { name: /mix in/ });
    const before = await handle.getAttribute("aria-valuenow");
    await handle.focus();
    await page.keyboard.press("ArrowRight");

    await expect
      .poll(async () => handle.getAttribute("aria-valuenow"))
      .not.toBe(before);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **§68: the automix performs the mix you set up.**
   *
   * The transition object exists so that everything performs the *same* mix.
   * Before this the automix decided its own moment, so a DJ who spent a minute
   * adjusting a mix point and then switched automix on watched it be ignored.
   * The arithmetic is Rust's and is tested in `dj_app::automix`; what a browser
   * can prove is that the panel stops claiming to be in charge of a handover
   * it is not deciding.
   */
  test("the automix says when it is performing the held mix", async ({
    page,
  }) => {
    await openShell(page, "/", {
      automix: {
        enabled: true,
        mixing: false,
        beats: 16,
        style: "blend",
        holding: true,
      },
    });
    await page.getByRole("button", { name: "Booth", exact: true }).click();

    const panel = page.locator("section.automix");
    await expect(panel.locator(".holding")).toContainText(
      "Performing the mix you set up",
    );
    // And the controls that no longer decide this handover say so.
    await expect(panel.locator(".styles.deferred")).toHaveCount(1);
    expect(errorsThrown(page)).toEqual([]);
  });
});
