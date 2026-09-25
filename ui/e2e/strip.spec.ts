/**
 * The deck's own mixer: compact, on one line, and no less precise for it.
 *
 * §120: *"the rest of the faders/knobs/controls seem to take a lot of space
 * ... please improve"*. At djmanzo's own window the strip under each deck was
 * 125 px on one row and its foot -- the cue button and the crossfader switch
 * -- another 29 on a row of its own, with the four groups spread about 34 px
 * apart; the faders were 140 px tall beside 46 px knobs, so the knobs sat at
 * the bottom of the strip under 60 px of nothing. And the pad tabs took their
 * height from their width: 38 px here, 57 in WebKitGTK on a deck with five
 * pages. The strip is packed now, the foot is part of it, the faders are
 * shorter, and the tabs are as tall as a label.
 *
 * Shorter faders cost travel, and a fader's drag is its drawn height, so the
 * pitch fader went from a quarter of a percent a pixel to a third. Shift is a
 * quarter of a drag on it now, as on every knob -- and on both it now counts
 * from the moment it goes down, not from the press, which is the only way
 * "overshoot, hold shift, creep back" works.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const deck = (page: Page, n = 1) => page.locator(`section.deck[data-deck="${n}"]`);

const dispatched = (page: Page) =>
  page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []);

/** The values a drag sent for one parameter, in order. */
async function sentFor(page: Page, before: number, prefix: string) {
  return (await dispatched(page))
    .slice(before)
    .filter((action) => action.startsWith(prefix))
    .map((action) => Number(action.slice(prefix.length)));
}

test.describe("§120: the channel strip", () => {
  /**
   * **The load-bearing one.** At the opening window the strip is one line,
   * the cue and the crossfader switch are on it, and the deck has the room
   * the strip gave back.
   */
  test("the strip is one line, with where the deck is heard at its end", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await openShell(page, "/");
    const channel = deck(page).locator(".channel");
    await expect(channel).toBeVisible();

    // The cue and the crossfader switch belong to the strip, not to a row
    // of their own under it.
    await expect(channel.locator(".routing .cue")).toHaveCount(1);
    await expect(channel.locator('.routing [aria-label="crossfader assignment"]')).toHaveCount(1);
    await expect(deck(page).locator(".channel-foot")).toHaveCount(0);

    const box = (await channel.boundingBox())!;
    expect(box.height, "the strip wrapped, or grew back").toBeLessThanOrEqual(105);

    // One line: every group of the strip starts on the strip's first line.
    const tops = await channel.evaluate((el) =>
      [...el.children].map((child) => {
        const r = child.getBoundingClientRect();
        return { name: child.className.split(" ")[0], bottom: Math.round(r.bottom) };
      }),
    );
    const lowest = Math.max(...tops.map((t) => t.bottom));
    for (const { name, bottom } of tops) {
      expect(lowest - bottom, `${name} is on a second line of the strip`).toBeLessThan(40);
    }

    // What the strip gave back went to the deck above it.
    const foot = (await deck(page).locator(".deck-foot").boundingBox())!;
    const body = (await deck(page).locator(".deck-body").boundingBox())!;
    expect(foot.height, "the pinned strip is as tall as it was").toBeLessThanOrEqual(115);
    expect(body.height, "the waveform and the pads did not get the room").toBeGreaterThanOrEqual(320);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Nothing in the strip is packed so tight it overlaps a neighbour. */
  test("no two groups of the strip overlap", async ({ page }) => {
    for (const width of [1280, 1100, 920]) {
      await page.setViewportSize({ width, height: 800 });
      await openShell(page, "/");
      const boxes = await deck(page)
        .locator(".channel")
        .evaluate((el) =>
          [...el.children].map((child) => {
            const r = child.getBoundingClientRect();
            return { name: child.className.split(" ")[0], l: r.left, r: r.right, t: r.top, b: r.bottom };
          }),
        );
      for (let i = 0; i < boxes.length; i++) {
        for (let j = i + 1; j < boxes.length; j++) {
          const a = boxes[i];
          const b = boxes[j];
          const overlap = a.l < b.r - 1 && b.l < a.r - 1 && a.t < b.b - 1 && b.t < a.b - 1;
          expect(overlap, `${a.name} and ${b.name} overlap at ${width} px`).toBe(false);
        }
      }
    }
  });

  /**
   * **A tab is as tall as a label, however wide the deck is.** It used to be
   * as tall as its width made it.
   */
  test("the pad tabs keep their height whatever the deck's width", async ({ page }) => {
    for (const width of [1920, 1280]) {
      await page.setViewportSize({ width, height: 900 });
      await openShell(page, "/");
      const heights = await deck(page)
        .locator(".tabs > *")
        .evaluateAll((tabs) => tabs.map((tab) => Math.round(tab.getBoundingClientRect().height)));
      expect(heights.length).toBeGreaterThan(1);
      for (const height of heights) {
        expect(height, `a pad tab is ${height} px tall at ${width} px`).toBeLessThanOrEqual(26);
      }
      // And they still share the row, rather than each being as wide as its word.
      const widths = await deck(page)
        .locator(".tabs > *")
        .evaluateAll((tabs) => tabs.map((tab) => Math.round(tab.getBoundingClientRect().width)));
      expect(Math.max(...widths) - Math.min(...widths)).toBeLessThanOrEqual(2);
    }
  });
});

test.describe("§120: fine control survives the shorter faders", () => {
  /**
   * **Shift is a quarter of a drag on the pitch fader, from when it goes
   * down.** Twenty pixels plain on a 100 px fader is a fifth of ±16%; twenty
   * more with shift held is a quarter of that again: 0.064 + 0.016. Scaled
   * from the press instead, the last move would have read 40 px at a
   * quarter -- 0.032 -- and thrown the pitch back towards where it began.
   */
  test("shift half way through a pitch drag slows what follows", async ({ page }) => {
    await openShell(page, "/");
    const fader = deck(page).locator('[role="slider"][aria-label="Pitch"]');
    await fader.scrollIntoViewIfNeeded();
    const start = Number(await fader.getAttribute("aria-valuenow"));
    const box = (await fader.boundingBox())!;
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;
    const before = (await dispatched(page)).length;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x, y - 10, { steps: 2 });
    await page.mouse.move(x, y - 20, { steps: 2 });
    await page.keyboard.down("Shift");
    await page.mouse.move(x, y - 30, { steps: 2 });
    await page.mouse.move(x, y - 40, { steps: 2 });
    await page.keyboard.up("Shift");
    await page.mouse.up();

    const sent = await sentFor(page, before, "deck 1 pitch ");
    expect(sent.length).toBeGreaterThan(1);
    expect(sent.at(-1)! - start).toBeCloseTo(0.08, 3);
    // Never back down on the way: shift slowed the drag, it did not undo it.
    for (let i = 1; i < sent.length; i++) expect(sent[i]).toBeGreaterThanOrEqual(sent[i - 1]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The same on a knob: its comment always promised it, and it did not. */
  test("shift half way through a knob drag slows what follows", async ({ page }) => {
    await openShell(page, "/");
    const knob = deck(page).locator('[role="slider"][aria-label="MID"]').first();
    await knob.scrollIntoViewIfNeeded();
    const start = Number(await knob.getAttribute("aria-valuenow"));
    const box = (await knob.boundingBox())!;
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;
    const before = (await dispatched(page)).length;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x, y - 10, { steps: 2 });
    await page.mouse.move(x, y - 20, { steps: 2 });
    await page.keyboard.down("Shift");
    await page.mouse.move(x, y - 30, { steps: 2 });
    await page.mouse.move(x, y - 40, { steps: 2 });
    await page.keyboard.up("Shift");
    await page.mouse.up();

    // 0..4 over a hundred pixels: 20 px is 0.8, and 20 more at a quarter 0.2.
    const sent = await sentFor(page, before, "deck 1 eq_mid ");
    expect(sent.at(-1)! - start).toBeCloseTo(1.0, 2);
    for (let i = 1; i < sent.length; i++) expect(sent[i]).toBeGreaterThanOrEqual(sent[i - 1]);
    expect(errorsThrown(page)).toEqual([]);
  });
});
