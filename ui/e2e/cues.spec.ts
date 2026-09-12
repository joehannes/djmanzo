/**
 * §26's first named example: a cue marker you can grab.
 *
 * > The DJ should be able to physically grab the thing they are thinking
 * > about. Do not force them to edit a numerical property in a settings panel.
 *
 * Before this, a cue set a beat early could only be fixed by playing back to
 * the right spot and setting it again — which is editing a numerical property
 * by the slowest means available.
 *
 * What lands where is Rust's: `Deck::move_hot_cue` clamps into the record,
 * snaps to the grid when quantize is on, and refuses a slot that holds
 * nothing, all tested in `dj-engine`. These say the handle is there, that it
 * sends the right action about the right deck, and that the keyboard reaches
 * it — §26 does not say "with a mouse".
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** What djmanzo was asked to do, in order. */
async function sent(page: import("@playwright/test").Page) {
  return page.evaluate(
    () => (window as unknown as { __dispatched?: string[] }).__dispatched ?? [],
  );
}

/** The draggable cue marker on a deck's scrolling lane. */
function cue(page: import("@playwright/test").Page, deck: number, slot: number) {
  return page
    .locator(`.deck[data-deck="${deck}"] [role="slider"][aria-label="Cue ${slot}, drag to move"]`)
    .first();
}

/**
 * A deck stopped, with its first cue in view.
 *
 * Two changes to the state djmanzo last sent, and both are about being able to
 * point at the thing:
 *
 * - The captured fixture's cue is eleven seconds in with the playhead near the
 *   start, so the marker sits two thousand pixels off the right of a lane six
 *   hundred wide. Present, correct, and off screen.
 *
 * - **The deck is stopped.** The lane interpolates between snapshots and
 *   scrolls at sixty frames a second, so a marker on a *playing* deck moves
 *   about seven pixels between measuring it and pressing — and the press lands
 *   on the tile behind it. That is not a defect: a scrolling lane is a moving
 *   surface in any DJ application, a hand tracks it and a measure-then-click
 *   test cannot. It is also the wrong case to design for. Cues get edited
 *   while a record is *prepared*, and the arrow-key nudge below is what covers
 *   the deck that is playing out to a room.
 *
 * Changed through `__emit` rather than by writing a snapshot here, so the
 * shape is always the one djmanzo actually sends.
 */
async function stoppedWithACueInView(page: import("@playwright/test").Page) {
  await page.evaluate(() => {
    const win = window as unknown as {
      __lastState?: {
        decks: { position_frames: number; playing: boolean; hot_cues: (number | null)[] }[];
      };
      __emit?: (next: unknown) => void;
    };
    const state = win.__lastState;
    if (!state) throw new Error("the harness delivered no state to start from");
    const decks = state.decks.map((deck, index) =>
      index === 0
        ? {
            ...deck,
            playing: false,
            hot_cues: [deck.position_frames + 8_000, ...deck.hot_cues.slice(1)],
          }
        : deck,
    );
    win.__emit?.({ ...state, decks });
  });
}

test.describe("§26's cue handles", () => {
  /**
   * **The load-bearing one: a drag moves the cue it was started on, on the
   * deck it belongs to.**
   *
   * Both halves matter. A handle that reported the wrong slot would silently
   * move a different mark; one that reported the wrong deck is the accident a
   * DJ cannot risk in front of a room, and is exactly why §29's control table
   * lives in Rust rather than at each call site.
   */
  test("dragging a cue asks djmanzo to move that cue on that deck", async ({
    page,
  }) => {
    await openShell(page, "/");
    await stoppedWithACueInView(page);

    const handle = cue(page, 1, 1);
    await expect(
      handle,
      "the cue on deck 1 has no handle -- §26's first example is a marker you " +
        "can grab",
    ).toBeVisible();

    const box = await handle.boundingBox();
    expect(box, "the handle has nothing to grab").not.toBeNull();
    await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
    await page.mouse.down();
    await page.mouse.move(box!.x + box!.width / 2 + 120, box!.y + box!.height / 2);
    await page.mouse.up();

    const moves = (await sent(page)).filter((a) => a.includes("hotcue_move"));
    expect(
      moves,
      "the drag finished and djmanzo was never told -- the mark would spring " +
        "back on the next frame",
    ).toHaveLength(1);
    expect(moves[0]).toMatch(/^deck 1 hotcue_move 1 \d+$/);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * The arrow keys nudge it, which is the gesture this is really for.
   *
   * A cue is usually wrong by a hair rather than by a phrase, and a hair is
   * smaller than a mouse reliably hits. It is also the half of §26 a DJ who
   * cannot use a mouse depends on entirely.
   */
  test("the arrow keys nudge a cue, and shift nudges it further", async ({
    page,
  }) => {
    await openShell(page, "/");

    const handle = cue(page, 1, 1);
    await handle.focus();
    await handle.press("ArrowRight");
    await handle.press("ArrowLeft");

    const moves = (await sent(page)).filter((a) => a.includes("hotcue_move"));
    expect(
      moves,
      "the keyboard cannot reach the handle, so half of §26 is mouse-only",
    ).toHaveLength(2);

    const at = (action: string) => Number(action.split(" ").at(-1));
    expect(at(moves[0]), "right did not move it later").toBeGreaterThan(at(moves[1]));

    // Shift is the coarse nudge: further than a plain press, in the same
    // direction. Measured against the plain press rather than against a pixel
    // count, because the step is in pixels of lane and the zoom is the
    // component's own business.
    await handle.press("Shift+ArrowRight");
    const after = (await sent(page)).filter((a) => a.includes("hotcue_move"));
    expect(
      at(after.at(-1)!) - at(after[1]),
      "shift nudged no further than a plain press",
    ).toBeGreaterThan(at(moves[0]) - at(moves[1]));
  });

  /**
   * A lane whose cues nobody owns has no handles.
   *
   * The pair view draws two records side by side and neither of them is "the"
   * deck whose cues are being edited, so it passes no `onMoveCue` — and a
   * grabbable marker there would be a control that does nothing, which is the
   * same rule the mix point's handle already follows.
   */
  test("a lane that owns no cues draws them as marks rather than handles", async ({
    page,
  }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Pair", exact: true }).click();
    await expect(page.locator('.surface[data-surface="pair"]')).toBeVisible();

    await expect(
      page.locator('.surface[data-surface="pair"] [aria-label="Cue 1, drag to move"]'),
      "the pair view offered a cue handle for a deck it does not own the " +
        "cues of",
    ).toHaveCount(0);
  });
});
