/**
 * §120: a dock's own size, from the gap beside it.
 *
 * > scrollbars, sizing, layout stickyness and resizing shall be super
 * > comfortable and quickly possible in very useful and quickly applicable
 * > ways
 *
 * A panel could be resized along its dock, but the dock itself was a share of
 * the window the interface chose -- a side dock 30 % of the width, between
 * 320 and 520 px. The gap between a dock and the stage is its handle now:
 * dragged, the size is the workspace's; double-clicked, the dock has its own
 * share back; and an arrangement that says nothing about docks keeps the
 * DJ's, as a pinned panel is kept.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

type Saved = { name: string; surfaces: { surface: string }[]; docks?: Record<string, number | null> };

const saved = (page: Page) =>
  page.evaluate(() => ((window as unknown as { __saved?: unknown[] }).__saved ?? []) as Saved[]);

const RIGHT = ".dock.side.right";

async function withAssistant(page: Page) {
  await page.setViewportSize({ width: 1400, height: 860 });
  await openShell(page, "/");
  await page.getByRole("button", { name: "Assistant", exact: true }).click();
  await expect(page.locator(RIGHT)).toBeVisible();
}

async function dragGrip(page: Page, dock: string, dx: number, dy = 0) {
  const grip = page.locator(`[data-dock-grip="${dock}"]`);
  const box = (await grip.boundingBox())!;
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + dx / 2, y + dy / 2, { steps: 3 });
  await page.mouse.move(x + dx, y + dy, { steps: 3 });
  await page.mouse.up();
}

test.describe("§120: a dock sized by dragging the gap beside it", () => {
  /**
   * **The load-bearing one.** Dragged towards the stage, the right dock
   * grows by what the hand moved, the stage gives it up, and the size is
   * written to the workspace; a double-click gives the dock its own share
   * back and writes that too.
   */
  test("the right dock grows by the drag, is kept, and a double-click gives its share back", async ({ page }) => {
    await withAssistant(page);
    const dock = page.locator(RIGHT);
    const own = (await dock.boundingBox())!;
    const stage = (await page.locator(".stage").boundingBox())!;

    await dragGrip(page, "right", -150);
    await expect.poll(async () => Math.round((await dock.boundingBox())!.width)).toBeGreaterThan(own.width + 140);
    const grown = (await dock.boundingBox())!;
    expect(grown.width).toBeLessThan(own.width + 160);
    // The stage gave the room up rather than the window scrolling.
    const narrower = (await page.locator(".stage").boundingBox())!;
    expect(stage.width - narrower.width).toBeGreaterThan(130);
    await expect
      .poll(async () => (await saved(page)).at(-1)?.docks?.right)
      .toBe(Math.round(grown.width));

    await page.locator('[data-dock-grip="right"]').dblclick();
    await expect.poll(async () => (await saved(page)).at(-1)?.docks?.right ?? null).toBeNull();
    await expect.poll(async () => Math.round((await dock.boundingBox())!.width)).toBeLessThanOrEqual(
      Math.round(own.width) + 2,
    );
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A stored size is drawn.** The drag leaves its own style on the dock,
   * so the test above would pass with a workspace size nothing reads; this
   * is the next start, when the size comes from the file and nothing else.
   */
  test("a dock size in the workspace is the dock's size when it opens", async ({ page }) => {
    await page.setViewportSize({ width: 1400, height: 860 });
    await openShell(page, "/", {}, {
      cockpit_workspace: {
        workspace: {
          name: "Perform",
          about: "",
          surfaces: [
            { surface: "assistant", dock: "right", order: 0, size: null, collapsed: false, pinned: false },
          ],
          density: "standard",
          focus: "performing",
          theme: "",
          decks: 2,
          locked: [],
          docks: { right: 500 },
        },
        notes: [],
        permits: { rearrange: true, resize: true, retheme: true, restyle: true },
      },
    });
    await expect(page.locator(RIGHT)).toBeVisible();
    expect(Math.round((await page.locator(RIGHT).boundingBox())!.width)).toBe(500);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The live drag stops where the saved size will: at the side dock's bounds. */
  test("a dock dragged past its bounds stops at them", async ({ page }) => {
    await withAssistant(page);
    await dragGrip(page, "right", 600);
    await expect.poll(async () => (await saved(page)).at(-1)?.docks?.right).toBe(280);
    expect(Math.round((await page.locator(RIGHT).boundingBox())!.width)).toBe(280);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Sticky.** An arrangement that says nothing about docks keeps the size
   * the DJ dragged -- how wide the side panels are is about the screen, not
   * the activity.
   */
  test("an arrangement that names no dock sizes keeps the DJ's", async ({ page }) => {
    await withAssistant(page);
    await dragGrip(page, "right", -120);
    await expect.poll(async () => (await saved(page)).at(-1)?.docks?.right).toBeGreaterThan(0);
    const size = (await saved(page)).at(-1)!.docks!.right;

    const picker = page.locator("select.workspace-preset");
    const first = await picker.locator("option").nth(1).getAttribute("value");
    await picker.selectOption(first!);
    await expect.poll(async () => (await saved(page)).at(-1)?.name).toBe(first);
    expect((await saved(page)).at(-1)?.docks?.right).toBe(size);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The bottom dock is sized by the gap above it, upwards growing it. */
  test("the bottom dock grows upwards from the gap above it", async ({ page }) => {
    await page.setViewportSize({ width: 1400, height: 860 });
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    const dock = page.locator(".dock.bottom");
    await expect(dock).toBeVisible();
    const own = (await dock.boundingBox())!;
    await dragGrip(page, "bottom", 0, -60);
    await expect.poll(async () => Math.round((await dock.boundingBox())!.height)).toBeGreaterThan(own.height + 50);
    await expect.poll(async () => (await saved(page)).at(-1)?.docks?.bottom).toBeGreaterThan(own.height + 50);
    expect(errorsThrown(page)).toEqual([]);
  });
});
