/**
 * §75: the phrase boundary as a control, not only as a drawing.
 *
 * > Where meaningful, expose audio properties visually. […] Allow
 * > clicking/dragging where this maps to a genuine action. Do not confuse
 * > visualization with control. Every interactive visual needs clear semantics.
 *
 * §25 draws the phrase lines and §26 asked for them to be movable. This is the
 * one audio property on the waveform that a drag means something for: the
 * analyser reads where the music starts again, and it is wrong often enough to
 * be worth a handle — a record with a four-beat pickup and one without look
 * identical to a structure detector and put every marker a bar out.
 *
 * Where a boundary *is*, and which beat a drag lands on, is `dj_app::grid` and
 * is tested there. These say what only a browser can.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const HANDLES = '.lane .strip [data-layer="phrases"]';

/**
 * Stop deck 1, the way `cues.spec.ts` does before grabbing a marker.
 *
 * A playing deck's strip is translated every frame, and Playwright refuses to
 * click something that is still moving — which is the right refusal: a handle a
 * DJ cannot reliably hit while the record runs is a handle that does not work.
 * §26's marks are all edited on a stopped or cued deck for the same reason.
 */
async function stopped(page: Page) {
  await page.evaluate(() => {
    const win = window as unknown as {
      __lastState?: { decks: { playing: boolean }[] };
      __emit?: (next: unknown) => void;
    };
    const state = win.__lastState;
    if (!state) throw new Error("the harness delivered no state to start from");
    win.__emit?.({
      ...state,
      decks: state.decks.map((deck, index) =>
        index === 0 ? { ...deck, playing: false } : deck,
      ),
    });
  });
}

test.describe("§75's phrase handle", () => {
  /**
   * **The load-bearing one: dragging a boundary sends a phrase edit through the
   * bus, and only the visible boundaries are in the DOM.**
   *
   * The action, because §75's rule is that a drag must map to a genuine one —
   * and `grid_phrase` is a verb the parser accepts, so the drag is the same
   * event as typing it or mapping a controller to it. The count, because a
   * ten-minute record at sixteen beats a phrase has about seventy boundaries
   * and putting seventy sliders in the DOM to show four would be paying for the
   * whole record on every frame of a scroll.
   */
  test("dragging a phrase boundary sends a grid_phrase for where it landed", async ({
    page,
  }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await stopped(page);

    const handles = page.locator(HANDLES);
    await expect(handles.first()).toBeVisible();
    const shown = await handles.count();
    expect(shown, "every boundary in the record is in the DOM").toBeLessThan(20);

    // Dispatched on the element rather than driven through the mouse, and the
    // reason is worth writing down: the strip is translated under a lane that
    // clips it, so a handle can have a layout box, be off-screen, and still
    // report one — and `page.mouse` aims at coordinates that a scrolling
    // waveform has already moved. What is under test is the handler chain
    // (`pointerdown` on the handle, then `pointermove` and `pointerup` on the
    // window, which is how `grab` listens on purpose), and dispatching that
    // chain tests it without a race. The pointer-reachability of the handle is
    // a CSS question and `theme-tokens.test.ts` holds the rule that answers it.
    const landed = await handles.first().evaluate((el) => {
      const box = el.getBoundingClientRect();
      const at = (x: number, type: string) =>
        new PointerEvent(type, {
          clientX: x,
          clientY: box.top + box.height / 2,
          bubbles: true,
          cancelable: true,
        });
      el.dispatchEvent(at(box.left + box.width / 2, "pointerdown"));
      window.dispatchEvent(at(box.left + 120, "pointermove"));
      window.dispatchEvent(at(box.left + 120, "pointerup"));
      return box.left + 120;
    });
    expect(landed).toBeGreaterThan(0);

    const sent = await page.evaluate(
      () => (window as unknown as { __dispatched?: string[] }).__dispatched ?? [],
    );
    const phrase = sent.filter((action) => action.includes("grid_phrase"));
    expect(phrase, `nothing reached the bus: ${JSON.stringify(sent)}`).not.toEqual([]);
    expect(phrase[0]).toMatch(/^deck \d+ grid_phrase \d+$/);
    expect(thrown).toEqual([]);
  });

  /**
   * **The handle says what moving it does.**
   *
   * §75 closes with *every interactive visual needs clear semantics*. A vitest
   * rule holds the markup to that; what this adds is that the semantics reach
   * the accessibility tree, which is where a DJ on a keyboard meets them —
   * `role="slider"` is also what makes the arrow keys work, and a cue set a
   * hair out is exactly what a keyboard nudge is for.
   */
  test("the handle is a labelled slider, not a decorated line", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");

    const handle = page.locator(HANDLES).first();
    await expect(handle).toHaveAttribute("role", "slider");
    await expect(handle).toHaveAttribute("aria-label", /phrase boundary/i);
    await expect(handle).toHaveAttribute("aria-valuemin", "0");

    // And the keyboard reaches it, which is the half a pointer test misses.
    await handle.focus();
    await page.keyboard.press("ArrowRight");
    const sent = await page.evaluate(
      () => (window as unknown as { __dispatched?: string[] }).__dispatched ?? [],
    );
    expect(sent.filter((a) => a.includes("grid_phrase"))).not.toEqual([]);
    expect(thrown).toEqual([]);
  });

  /**
   * **A record with no phrase structure gets no handles.**
   *
   * `null` is a real answer — plenty of records have no phrase the analyser can
   * find — and a handle over a boundary that does not exist would be §75's own
   * warning, a control invented for a visualisation that is not there.
   */
  test("no phrase structure means no handles", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, { phrase_grid: null });
    await expect(page.locator(HANDLES)).toHaveCount(0);
    expect(thrown).toEqual([]);
  });
});
