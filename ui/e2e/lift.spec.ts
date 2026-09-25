/**
 * §120's temporary windows, which are §3's *temporarily surfaced*.
 *
 * > temporary windows can take a lot of space for the focused moment and
 * > then get out of the way
 *
 * Any open panel lifts over the decks from its header and goes back, where
 * it was and at the size it had, on Escape, on its own button, or when a
 * press elsewhere is released. A dock with nothing left in it gives its room
 * up while the panel is lifted, so the panel has that room too.
 *
 * Rust has long said this verb is done by `Dock::Overlay` -- "dismissed by
 * the next thing the DJ does" -- and a workspace could put the room or the
 * night there, and the interface drew neither. An overlay placement is drawn
 * lifted now, and closes when dismissed.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LIBRARY = '.surface[data-surface="library"]';

const dispatched = (page: Page) =>
  page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []);

const saved = (page: Page) =>
  page.evaluate(
    () =>
      ((window as unknown as { __saved?: unknown[] }).__saved ?? []) as {
        surfaces: { surface: string; size: number | null }[];
      }[],
  );

async function openLibrary(page: Page) {
  await page.setViewportSize({ width: 1280, height: 800 });
  await openShell(page, "/");
  await page.getByRole("button", { name: "Browse", exact: true }).click();
  await expect(page.locator(LIBRARY)).toBeVisible();
}

test.describe("§120: a panel lifted over the decks for the moment", () => {
  /**
   * **The load-bearing one.** Lifted, the library covers the stage -- and
   * the stage has grown into the bottom dock the library left, so the panel
   * is far taller than it was docked. Escape puts it back as it was.
   */
  test("the library lifts over the decks, larger, and Escape puts it back", async ({ page }) => {
    await openLibrary(page);
    const library = page.locator(LIBRARY);
    const docked = (await library.boundingBox())!;
    const stageBefore = (await page.locator(".stage").boundingBox())!;

    await page.getByRole("button", { name: "Lift Library" }).click();
    await expect(library).toHaveAttribute("data-lifted", "true");
    await page.waitForTimeout(250);
    const lifted = (await library.boundingBox())!;
    const stage = (await page.locator(".stage").boundingBox())!;

    // It covers the stage…
    expect(Math.abs(lifted.y - stage.y)).toBeLessThanOrEqual(10);
    expect(Math.abs(lifted.height - stage.height)).toBeLessThanOrEqual(16);
    expect(Math.abs(lifted.width - stage.width)).toBeLessThanOrEqual(16);
    // …the stage took the dock's room…
    expect(stage.height, "the empty dock kept its room").toBeGreaterThan(stageBefore.height + 150);
    // …so the panel has far more than it had docked.
    expect(lifted.height).toBeGreaterThan(docked.height * 1.6);
    // And the master strip is still on screen, below it.
    const bridge = (await page.locator(".bridge").boundingBox())!;
    expect(bridge.y).toBeGreaterThanOrEqual(lifted.y + lifted.height - 10);
    expect(bridge.y + bridge.height).toBeLessThanOrEqual(800);

    await page.mouse.move(lifted.x + 40, lifted.y + 200);
    await page.keyboard.press("Escape");
    await expect(library).toHaveAttribute("data-lifted", "false");
    const back = (await library.boundingBox())!;
    expect(Math.abs(back.y - docked.y)).toBeLessThanOrEqual(4);
    expect(Math.abs(back.height - docked.height)).toBeLessThanOrEqual(4);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A press elsewhere puts it back when it is released, and still does
   * its own work.** Putting the panel back returns its dock and moves the
   * master strip; on the press, the crossfader a DJ had just taken hold of
   * jumped up under the hand. It has to stay put until the hand lets go.
   */
  test("a drag on the crossfader moves it, and the panel goes back on release", async ({ page }) => {
    await openLibrary(page);
    const library = page.locator(LIBRARY);
    await page.getByRole("button", { name: "Lift Library" }).click();
    await expect(library).toHaveAttribute("data-lifted", "true");
    await page.waitForTimeout(250);

    const crossfader = page.getByRole("slider", { name: "Crossfader" });
    const held = (await crossfader.boundingBox())!;
    const before = (await dispatched(page)).length;
    await page.mouse.move(held.x + held.width / 2, held.y + held.height / 2);
    await page.mouse.down();
    await page.mouse.move(held.x + held.width / 2 + 30, held.y + held.height / 2, { steps: 3 });

    // Mid-gesture: nothing has moved under the hand.
    await expect(library).toHaveAttribute("data-lifted", "true");
    const during = (await crossfader.boundingBox())!;
    expect(Math.abs(during.y - held.y)).toBeLessThanOrEqual(1);

    await page.mouse.up();
    await expect(library).toHaveAttribute("data-lifted", "false");
    const sent = (await dispatched(page)).slice(before);
    expect(sent.some((action) => action.startsWith("crossfader ")), "the crossfader did not move").toBe(true);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A press inside the lifted panel is work in it, not a dismissal. */
  test("working in the lifted panel keeps it lifted", async ({ page }) => {
    await openLibrary(page);
    const library = page.locator(LIBRARY);
    await page.getByRole("button", { name: "Lift Library" }).click();
    await expect(library).toHaveAttribute("data-lifted", "true");
    const search = library.getByRole("searchbox", { name: "Search the library" });
    await search.click();
    await search.fill("rosa");
    await expect(library).toHaveAttribute("data-lifted", "true");
    // Its own button puts it back.
    await page.getByRole("button", { name: "Put Library back" }).click();
    await expect(library).toHaveAttribute("data-lifted", "false");
    // A lift is a moment, not an arrangement: nothing about it was saved.
    for (const workspace of await saved(page)) {
      expect(JSON.stringify(workspace)).not.toContain("overlay");
    }
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **An overlay placement is drawn.** A workspace putting the room over the
   * decks used to open nothing at all. It floats, and the next thing the DJ
   * does closes it.
   */
  test("a panel a workspace put over the decks is drawn there, and dismissed", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await openShell(page, "/", {}, {
      cockpit_workspace: {
        workspace: {
          name: "Perform",
          about: "",
          surfaces: [
            { surface: "room", dock: "overlay", order: 0, size: null, collapsed: false, pinned: false },
          ],
          density: "standard",
          focus: "performing",
          theme: "",
          decks: 2,
          locked: [],
        },
        notes: [],
        permits: { rearrange: true, resize: true, retheme: true, restyle: true },
      },
    });
    const room = page.locator('.surface[data-surface="room"]');
    await expect(room).toBeVisible();
    await expect(room).toHaveAttribute("data-lifted", "true");
    const box = (await room.boundingBox())!;
    const stage = (await page.locator(".stage").boundingBox())!;
    expect(Math.abs(box.y - stage.y)).toBeLessThanOrEqual(10);
    // Nothing to put back to, so no put-back button: it is dismissed.
    await expect(room.getByRole("button", { name: /^Put / })).toHaveCount(0);

    await page.mouse.move(box.x + 40, box.y + 100);
    await page.keyboard.press("Escape");
    await expect(room).toHaveCount(0);
    await expect
      .poll(async () => (await saved(page)).at(-1)?.surfaces.some((p) => p.surface === "room"))
      .toBe(false);
    expect(errorsThrown(page)).toEqual([]);
  });
});

/**
 * **A double-click on a panel's edge gives it back its own size.** The
 * quick way out of a size that seemed right at the time.
 */
test("a double-click on the resize edge returns a panel to its own size", async ({ page }) => {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Assistant", exact: true }).click();
  const surface = page.locator('.surface[data-surface="assistant"]');
  await expect(surface).toBeVisible();
  const natural = (await surface.boundingBox())!;
  const grip = page.getByRole("separator", { name: "Resize Assistant" });
  const handle = (await grip.boundingBox())!;
  await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
  await page.mouse.down();
  await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2 + 80, { steps: 4 });
  await page.mouse.up();
  await expect
    .poll(async () => (await saved(page)).at(-1)?.surfaces.find((p) => p.surface === "assistant")?.size)
    .toBeGreaterThan(natural.height + 40);

  await grip.dblclick();
  await expect
    .poll(async () => (await saved(page)).at(-1)?.surfaces.find((p) => p.surface === "assistant")?.size)
    .toBeNull();
  await expect.poll(async () => Math.round((await surface.boundingBox())!.height)).toBeLessThanOrEqual(
    Math.round(natural.height) + 4,
  );
  expect(errorsThrown(page)).toEqual([]);
});
