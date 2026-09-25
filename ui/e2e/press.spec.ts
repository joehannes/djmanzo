/**
 * A pad takes the pointer itself, not its drawing.
 *
 * Pressing an unlit pad draws a glow -- a path the theme engine adds for
 * `pressed` -- and letting go takes it away. A press that landed on that path
 * landed on a node that was gone by the time the click was dispatched, and
 * WebKit, the webview djmanzo runs in on Linux and macOS, then fires no click
 * at all: PLAY on a paused deck did nothing from the mouse. Found by driving
 * the application; Chromium forgives it, so a test that only clicked would
 * pass here and fail nowhere a DJ could see. This holds the cause instead:
 * what is under the pointer, before and during a press, is the pad.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

test("a pad takes the pointer itself, pressed or not", async ({ page }) => {
  await openShell(page, "/");
  for (const does of ["cue", "play", "sync", "eject"]) {
    const pad = page.locator(`section.deck[data-deck="1"] .pad-container[data-does="${does}"]`);
    await pad.scrollIntoViewIfNeeded();
    const box = (await pad.boundingBox())!;
    const at = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
    const under = () =>
      page.evaluate(({ x, y }) => {
        const found = document.elementFromPoint(x, y);
        return {
          drawing: found instanceof SVGElement,
          pad: found?.closest(".pad-container")?.getAttribute("data-does") ?? null,
        };
      }, at);

    expect(await under(), `${does}: at rest`).toEqual({ drawing: false, pad: does });
    await page.mouse.move(at.x, at.y);
    await page.mouse.down();
    expect(await under(), `${does}: pressed, the glow must not take the pointer`).toEqual({
      drawing: false,
      pad: does,
    });
    await page.mouse.up();
  }
  // And the press is the pad's: PLAY sends the deck's play.
  await page.locator('section.deck[data-deck="1"] .pad-container[data-does="play"]').click();
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []))
    .toContain("deck 1 play_pause");
  expect(errorsThrown(page)).toEqual([]);
});
