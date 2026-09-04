/**
 * §26's first example: "Cue marker — drag to move."
 *
 * A hot cue could only ever be set at the playhead, so moving one meant
 * driving the record there and pressing the pad again — which is the
 * numerical-property-in-a-panel §26 is complaining about, wearing a transport
 * control's clothes.
 *
 * What a browser can prove is the gesture: that the marker answers to a
 * pointer, and that letting go reaches djmanzo with the slot and a frame. What
 * happens to that frame — snapped to a beat or not, clamped into the record,
 * ignored for an empty slot — is the engine's, and is tested there.
 *
 * **It drags on the deck that is stopped, and that is not incidental.** A
 * playing deck's lane scrolls at about two hundred pixels a second, so the
 * marker has moved a target's width away between measuring it and pressing on
 * it — the trap `docs/HANDOFF.md` records, which has now cost this project
 * twice. A stopped deck's lane does not move, so the gesture is the only
 * variable. Deck 2's cue is also near its playhead on purpose: the captured
 * cue on deck 1 sits two thousand pixels off the right-hand edge, real and
 * drawn and outside the window, and Playwright calls such an element visible.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** The stopped deck's own lane — see the note above about why. */
const LANE = ".deck:not(.playing) .lane";

test.describe("dragging a cue", () => {
  test("a cue on the deck's own lane can be grabbed", async ({ page }) => {
    await openShell(page, "/");

    const cue = page.locator(`${LANE} .cue-marker.grabbable`).first();
    await expect(cue).toBeVisible();
    await expect(cue).toHaveAttribute("title", "Cue 1 — drag to move it");
  });

  /**
   * The round trip. The frame is the pointer's; which beat that is belongs to
   * djmanzo, which has the grid.
   */
  test("letting go reaches djmanzo with the slot and a frame", async ({ page }) => {
    await openShell(page, "/");

    const cue = page.locator(`${LANE} .cue-marker.grabbable`).first();
    await expect(cue).toBeVisible();
    const box = (await cue.boundingBox())!;
    const y = box.y + box.height / 2;

    await page.evaluate(() => {
      const win = window as unknown as Record<string, unknown>;
      win.__asked = [];
      win.__cueArgs = [];
    });

    await page.mouse.move(box.x + box.width / 2, y);
    await page.mouse.down();
    await page.mouse.move(box.x - 80, y, { steps: 8 });
    await page.mouse.up();

    await expect
      .poll(
        () => page.evaluate(() => (window as unknown as { __asked: string[] }).__asked),
        { message: "the drag did not reach djmanzo" },
      )
      .toContain("move_hot_cue");

    const args = await page.evaluate(
      () => (window as unknown as { __cueArgs: Record<string, number>[] }).__cueArgs,
    );
    expect(args.length, "the command arrived without arguments").toBeGreaterThan(0);
    expect(args[0].slot, "the wrong slot was moved").toBe(1);
    expect(args[0].deck, "the cue moved on the wrong deck").toBe(2);
    expect(args[0].frame, "the frame is not a frame").toBeGreaterThanOrEqual(0);
  });

  /**
   * **A cue belongs to its deck.** The pair view draws the same record and has
   * no business moving its cues from a panel about a transition — so it draws
   * them and leaves them alone, which is what the separate prop is for.
   */
  test("the pair view draws cues without offering to move them", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Pair", exact: true }).click();

    const pair = '.surface[data-surface="pair"]';
    await expect(page.locator(pair)).toBeVisible();
    await expect(page.locator(`${pair} .cue-marker.grabbable`)).toHaveCount(0);
  });

  test("dragging a cue throws nothing", async ({ page }) => {
    await openShell(page, "/");
    const cue = page.locator(`${LANE} .cue-marker.grabbable`).first();
    await expect(cue).toBeVisible();
    const box = (await cue.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + 40, box.y + box.height / 2, { steps: 4 });
    await page.mouse.up();
    expect(errorsThrown(page)).toEqual([]);
  });
});
