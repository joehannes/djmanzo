/**
 * §29's intelligent control handles.
 *
 * > Do not turn every knob into a huge widget.
 *
 * So the tests are about the gestures, not about the drawing: that a knob
 * carries level two and level three without growing, that the contextual menu
 * sends actions rather than numbers, and that it sends them about the deck it
 * was opened on.
 *
 * That last one is the whole reason the table lives in Rust. Every `SvgKnob`
 * used to name its own reset value at its own call site, and a menu that acted
 * on "whichever deck" is the one accident a DJ cannot risk in front of a room.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** What djmanzo was asked to do, in order. */
async function sent(page: import("@playwright/test").Page) {
  return page.evaluate(
    () => (window as unknown as { __dispatched?: string[] }).__dispatched ?? [],
  );
}

/** The LOW band knob on a given deck. */
function lowKnob(page: import("@playwright/test").Page, deck: number) {
  return page
    .locator(`.deck[data-deck="${deck}"] [role="slider"][aria-label="LOW"]`)
    .first();
}

test.describe("a control's gestures", () => {
  /**
   * **Level three opens on a press and hold, and closes on Escape.**
   *
   * Hold as well as right-click, because a booth has trackpads and
   * touchscreens: a control reachable only by right-click is one half the room
   * cannot reach.
   */
  test("a long hold opens the contextual options", async ({ page }) => {
    await openShell(page, "/");
    const knob = lowKnob(page, 1);
    await expect(knob).toBeVisible();
    await expect(page.getByTestId("handle-options")).toHaveCount(0);

    const box = await knob.boundingBox();
    expect(box, "the knob has nothing to press").not.toBeNull();
    await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
    await page.mouse.down();
    await page.waitForTimeout(700);
    await page.mouse.up();

    await expect(page.getByTestId("handle-options")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("handle-options")).toHaveCount(0);
    expect(errorsThrown(page), "the knob threw").toEqual([]);
  });

  /** Right-click opens the same menu, and does not open the browser's. */
  test("a right click opens the same options", async ({ page }) => {
    await openShell(page, "/");
    await lowKnob(page, 1).click({ button: "right" });

    const options = page.getByTestId("handle-options");
    await expect(options).toBeVisible();
    await expect(options.getByRole("menuitem")).toHaveCount(3);
    await expect(options.getByRole("menuitem").first()).toHaveText("Kill");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **An option sends an action, not a number.**
   *
   * §29's last bullet is "MIDI = same underlying parameter": a menu entry, a
   * drag and a MIDI CC have to end up as the same action or they are paths
   * that will eventually disagree. What the menu emits is exactly what a
   * mapping file would.
   */
  test("choosing an option sends the action djmanzo would take", async ({
    page,
  }) => {
    await openShell(page, "/");
    await lowKnob(page, 1).click({ button: "right" });
    await page.getByTestId("handle-options").getByRole("menuitem", { name: "Kill" }).click();

    await expect.poll(() => sent(page)).toContain("deck 1 eq_low 0");
    await expect(page.getByTestId("handle-options")).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The menu acts on the deck it was opened on.**
   *
   * The reason the table is Rust's rather than each call site's. A menu that
   * killed the bass on the wrong deck would do it in front of a room.
   */
  test("the options are about the deck they were opened on", async ({
    page,
  }) => {
    await openShell(page, "/");
    await lowKnob(page, 2).click({ button: "right" });
    await page.getByTestId("handle-options").getByRole("menuitem", { name: "Kill" }).click();

    const all = await sent(page);
    expect(all, "a menu on deck 2 acted on another deck").toContain(
      "deck 2 eq_low 0",
    );
    expect(all).not.toContain("deck 1 eq_low 0");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A double-click resets to the parameter's own unity, from Rust.**
   *
   * The filter is bipolar and resets to centre; a band is gain-like and resets
   * to unity. Which is which is a fact about the parameter, and the call sites
   * used to each carry their own copy of it.
   */
  test("a double click resets to where the parameter's unity is", async ({
    page,
  }) => {
    await openShell(page, "/");
    await lowKnob(page, 1).dblclick();
    await expect.poll(() => sent(page)).toContain("deck 1 eq_low 1");

    const filter = page
      .locator('.deck[data-deck="1"] [role="slider"][aria-label="Filter"]')
      .first();
    await filter.dblclick();
    await expect.poll(() => sent(page)).toContain("deck 1 filter 0");
    expect(errorsThrown(page)).toEqual([]);
  });
});

/**
 * §29's last gesture: **AI hover = suggestion**.
 *
 * The one item on §29's list that had nothing behind it, on the reasoning that
 * the assistant stages whole moves rather than single parameter values. §68's
 * transition object and `dj_app::shape` between them make that false: a style
 * says what it does beyond the two faders, in the values the automix sends, so
 * the answer exists per control.
 *
 * The table and its silences are Rust's (`dj_app::handle::suggested`). What a
 * browser can prove is that the mark is only drawn where there is something to
 * say, that the sentence is djmanzo's own, and that taking it sends the action
 * about the deck it was drawn on.
 */
test.describe("what the assistant would do to a control", () => {
  const PLANNED = {
    control_suggestions: [
      {
        control: "eq_low",
        to: 0,
        action: "deck 1 eq_low 0",
        because:
          "The low end is handed out of this deck by 50% through, then put back.",
      },
    ],
  };

  /**
   * **The mark is only there when there is something to say.**
   *
   * Four of the six controls get nothing from every style djmanzo performs,
   * and no deck gets anything at all until a mix is armed. A dot on every knob
   * of every deck would be how a DJ learns to stop looking at the dots.
   */
  test("a knob the plan says nothing about carries no mark", async ({
    page,
  }) => {
    await openShell(page, "/");
    await expect(lowKnob(page, 1).getByTestId("knob-suggested")).toHaveCount(0);

    await openShell(page, "/", {}, PLANNED);
    await expect(lowKnob(page, 1).getByTestId("knob-suggested")).toHaveCount(1);
    // The other bands still say nothing, because no style touches them.
    await expect(
      page
        .locator('.deck[data-deck="1"] [role="slider"][aria-label="MID"]')
        .first()
        .getByTestId("knob-suggested"),
    ).toHaveCount(0);
    expect(errorsThrown(page), "a deck threw").toEqual([]);
  });

  /**
   * **Hovering says what djmanzo would do, in djmanzo's words.**
   *
   * The sentence comes from Rust with the percentage already in it, so a
   * tooltip cannot describe a bass swap the automix does not perform — the
   * same one-table rule `shape` exists for.
   */
  test("hovering a marked knob says what the plan does to it", async ({
    page,
  }) => {
    await openShell(page, "/", {}, PLANNED);

    const knob = lowKnob(page, 1);
    await expect(page.getByTestId("knob-suggestion")).toHaveCount(0);
    await knob.hover();
    await expect(knob.getByTestId("knob-suggestion")).toContainText(
      "handed out of this deck by 50% through",
    );
    expect(errorsThrown(page), "a deck threw").toEqual([]);
  });

  /**
   * **Taking it sends an action, about the deck it was drawn on.**
   *
   * §29's last bullet is that a drag, a menu entry and a MIDI CC end up as one
   * parameter; a hover a DJ can act on has to join them rather than becoming a
   * seventh path. And the deck number is the accident worth guarding: a
   * suggestion drawn on deck 2 that moved deck 1 is the one mistake nobody can
   * risk in front of a room.
   */
  test("taking a suggestion sends djmanzo's own action for that deck", async ({
    page,
  }) => {
    await openShell(page, "/", {}, PLANNED);

    const knob = lowKnob(page, 2);
    await knob.hover();
    await knob.getByTestId("knob-suggestion").getByRole("button").click();

    expect(await sent(page)).toContain("deck 2 eq_low 0");
    expect(errorsThrown(page), "a deck threw").toEqual([]);
  });

  /**
   * **The fader is a control too.**
   *
   * The channel faders *are* the transition, so the suggestion that matters
   * most is the one on them. It is the same prop and the same sentence, drawn
   * on a fader rather than a knob.
   */
  test("the volume fader carries the plan's fade", async ({ page }) => {
    await openShell(
      page,
      "/",
      {},
      {
        control_suggestions: [
          {
            control: "volume",
            to: 0,
            action: "deck 1 volume 0",
            because: "The assistant brings this fader down over 32 beats.",
          },
        ],
      },
    );

    const fader = page
      .locator('.deck[data-deck="1"] [role="slider"][aria-label="Volume"]')
      .first();
    await expect(fader.getByTestId("fader-suggested")).toHaveCount(1);
    await fader.hover();
    await expect(fader.getByTestId("fader-suggestion")).toContainText(
      "brings this fader down over 32 beats",
    );
    expect(errorsThrown(page), "a deck threw").toEqual([]);
  });
});
