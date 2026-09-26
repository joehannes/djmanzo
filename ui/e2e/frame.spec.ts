/**
 * §121: the controls on a panel's border.
 *
 * > i also want controls on the widget borders ... grab, move, resize, zoom,
 * > reset size, full size, minimize, pop out ... sticky
 *
 * > let there be a way to open it as a temporary rather big/huge modal
 * > instead of an integrated side widget ... (also via mnemonics, there's got
 * > to be 2 mnemonic ways to open widgets, one as integrated, one as modal)
 *
 * Every panel's header carries the whole set, each a drawing of its own; the
 * title bar carries the panel to another dock or over the decks; and
 * `Space O <letter>` opens a panel large where `Space o <letter>` docks it.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LIBRARY = '.surface[data-surface="library"]';
const ASSISTANT = '.surface[data-surface="assistant"]';

type Saved = {
  surfaces: { surface: string; dock: string; order: number; zoom?: number | null }[];
  docks?: Record<string, number | null>;
};

const saved = (page: Page) =>
  page.evaluate(() => ((window as unknown as { __saved?: unknown[] }).__saved ?? []) as Saved[]);

const placed = async (page: Page, name: string) =>
  (await saved(page)).at(-1)?.surfaces.find((p) => p.surface === name);

async function withLibrary(page: Page) {
  await openShell(page, "/");
  await page.setViewportSize({ width: 1400, height: 860 });
  await page.getByRole("button", { name: "Browse", exact: true }).click();
  await expect(page.locator(LIBRARY)).toBeVisible();
}

/** Carry a panel by its title to (x, y), in steps as a hand would. */
async function carry(page: Page, panel: string, x: number, y: number) {
  const title = page.locator(`${panel} .surface-head h2`);
  const box = (await title.boundingBox())!;
  const fromX = box.x + box.width / 2;
  const fromY = box.y + box.height / 2;
  await page.mouse.move(fromX, fromY);
  await page.mouse.down();
  await page.mouse.move(fromX + 20, fromY + 20, { steps: 3 });
  await page.mouse.move(x, y, { steps: 8 });
}

test.describe("§121: the controls on a panel's border", () => {
  /** Every one of the owner's verbs is a button on the header, by name. */
  test("a panel's header carries every control the owner listed", async ({ page }) => {
    await withLibrary(page);
    const head = page.locator(`${LIBRARY} .surface-head`);
    for (const name of [
      "Move Library",
      "Zoom Library out",
      "Library at its own zoom",
      "Zoom Library in",
      "Library at its own size",
      "Pop Library out",
      "Pin Library",
      "Collapse Library",
      "Lift Library",
      "Close Library",
    ]) {
      await expect(head.getByRole("button", { name, exact: true })).toHaveCount(1);
    }
    // Each is drawn, not lettered: every button but the percentage holds a
    // drawing, and no two drawings are the same.
    const drawings = await head
      .locator("button:not(.zoom-at) svg path")
      .evaluateAll((paths) => paths.map((p) => p.getAttribute("d")));
    expect(drawings.length).toBe(9);
    expect(new Set(drawings).size).toBe(9);
    // And the resize grip is there, visible rather than six invisible pixels.
    const grip = page.locator(`${LIBRARY} .grip`);
    expect((await grip.boundingBox())!.width).toBeGreaterThanOrEqual(10);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Zoom.** A press grows what the panel draws, is written to the
   * workspace, and "own zoom" puts it back; Ctrl and the wheel over the panel
   * does the same.
   */
  test("zoom grows what a panel draws, is kept, and goes back", async ({ page }) => {
    await withLibrary(page);
    const library = page.locator(LIBRARY);
    const search = library.getByRole("searchbox", { name: "Search the library" });
    const before = (await search.boundingBox())!.height;

    await page.getByRole("button", { name: "Zoom Library in" }).click();
    await page.getByRole("button", { name: "Zoom Library in" }).click();
    await expect(library).toHaveAttribute("data-zoom", "120");
    await expect.poll(async () => (await placed(page, "library"))?.zoom).toBe(120);
    const grown = (await search.boundingBox())!.height;
    expect(grown).toBeGreaterThan(before * 1.15);
    await expect(page.getByRole("button", { name: "Library at its own zoom" })).toHaveText("120%");

    await page.getByRole("button", { name: "Library at its own zoom" }).click();
    await expect(library).toHaveAttribute("data-zoom", "100");
    await expect.poll(async () => (await placed(page, "library"))?.zoom ?? null).toBeNull();
    expect(Math.round((await search.boundingBox())!.height)).toBe(Math.round(before));

    // Ctrl and the wheel, over the panel's contents.
    const body = (await library.locator(".surface-body").boundingBox())!;
    await page.mouse.move(body.x + body.width / 2, body.y + body.height / 2);
    await page.keyboard.down("Control");
    await page.mouse.wheel(0, -120);
    await page.keyboard.up("Control");
    await expect(library).toHaveAttribute("data-zoom", "120");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Move.** Carried by its title to the right edge, the library lands in
   * the right dock -- drawn there and written there -- and the zones drawn
   * while it was carried were the ones it may go to.
   */
  test("a panel carried by its title to the right lands in the right dock", async ({ page }) => {
    await withLibrary(page);
    await expect(page.locator(`.dock.bottom ${LIBRARY}`)).toBeVisible();
    const room = (await page.locator(".cockpit").boundingBox())!;

    await carry(page, LIBRARY, room.x + room.width - 40, room.y + room.height / 3);
    await expect(page.locator('.move-zone[data-zone="right"]')).toHaveClass(/lit/);
    await expect(page.locator(".move-chip")).toContainText("Library: To the right");
    await page.mouse.up();

    await expect(page.locator(".move-zones")).toHaveCount(0);
    await expect(page.locator(`.dock.side.right ${LIBRARY}`)).toBeVisible();
    await expect.poll(async () => (await placed(page, "library"))?.dock).toBe("right");
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A press on the title that does not travel moves nothing. */
  test("a click on the title is not a move", async ({ page }) => {
    await withLibrary(page);
    const before = (await saved(page)).length;
    await page.locator(`${LIBRARY} .surface-head h2`).click();
    await page.waitForTimeout(200);
    expect((await saved(page)).length).toBe(before);
    await expect(page.locator(`.dock.bottom ${LIBRARY}`)).toBeVisible();
  });

  /**
   * **Beside another panel.** Dropped on the upper half of a docked panel,
   * the carried one goes before it in that dock.
   */
  test("a panel dropped on another's upper half goes before it", async ({ page }) => {
    await withLibrary(page);
    await page.getByRole("button", { name: "Assistant", exact: true }).click();
    await expect(page.locator(`.dock.side.right ${ASSISTANT}`)).toBeVisible();
    const assistant = (await page.locator(ASSISTANT).boundingBox())!;

    await carry(page, LIBRARY, assistant.x + assistant.width / 2, assistant.y + assistant.height / 4);
    await expect(page.locator(ASSISTANT)).toHaveClass(/drop-before/);
    await page.mouse.up();

    await expect(page.locator(`.dock.side.right ${LIBRARY}`)).toBeVisible();
    const order = await page
      .locator(".dock.side.right > .surface")
      .evaluateAll((els) => els.map((el) => (el as HTMLElement).dataset.surface));
    expect(order).toEqual(["library", "assistant"]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Dropped in the middle, the panel is lifted over the decks for now. */
  test("a panel dropped over the decks is lifted there", async ({ page }) => {
    await withLibrary(page);
    const stage = (await page.locator(".stage").boundingBox())!;
    await carry(page, LIBRARY, stage.x + stage.width / 2, stage.y + stage.height / 3);
    await expect(page.locator('.move-zone[data-zone="lift"]')).toHaveClass(/lit/);
    await page.mouse.up();
    await expect(page.locator(LIBRARY)).toHaveAttribute("data-lifted", "true");
    // Lifting is for now: the workspace still has it in the bottom dock.
    expect((await placed(page, "library"))?.dock ?? "bottom").toBe("bottom");
  });

  /** The handle moves a panel from the keyboard too. */
  test("the handle's arrow keys move a panel between docks", async ({ page }) => {
    await withLibrary(page);
    await page.getByRole("button", { name: "Move Library" }).focus();
    await page.keyboard.press("ArrowLeft");
    await expect(page.locator(`.dock.side.left ${LIBRARY}`)).toBeVisible();
    await expect.poll(async () => (await placed(page, "library"))?.dock).toBe("left");
    await page.getByRole("button", { name: "Move Library" }).focus();
    await page.keyboard.press("ArrowDown");
    await expect(page.locator(`.dock.bottom ${LIBRARY}`)).toBeVisible();
  });

  /**
   * **Pop out.** The library goes into the window made for it and out of
   * this one; a panel with no window of its own carries no such button.
   */
  test("pop out opens the library's own window and closes it here", async ({ page }) => {
    await withLibrary(page);
    await page.getByRole("button", { name: "Pop Library out" }).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __detached?: string[] }).__detached ?? []))
      .toEqual(["browser"]);
    await expect(page.locator(LIBRARY)).toHaveCount(0);

    await page.getByRole("button", { name: "Night", exact: true }).click().catch(() => {});
    const night = page.locator('.surface[data-surface="night"]');
    if (await night.count()) {
      await expect(night.getByRole("button", { name: /^Pop .* out$/ })).toHaveCount(0);
    }
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Reset size is there only as something to do once a size was given. */
  test("own size is offered once the panel has been sized, and gives its size back", async ({ page }) => {
    await withLibrary(page);
    const reset = page.getByRole("button", { name: "Library at its own size" });
    await expect(reset).toBeDisabled();
    const grip = (await page.locator(`${LIBRARY} .grip`).boundingBox())!;
    await page.mouse.move(grip.x + grip.width / 2, grip.y + grip.height / 2);
    await page.mouse.down();
    await page.mouse.move(grip.x + grip.width / 2 + 120, grip.y + grip.height / 2, { steps: 5 });
    await page.mouse.up();
    await expect(reset).toBeEnabled();
    await reset.click();
    await expect(reset).toBeDisabled();
    await expect
      .poll(async () => (await saved(page)).at(-1)?.surfaces.find((p) => p.surface === "library"))
      .toMatchObject({ surface: "library" });
    expect(
      (await saved(page)).at(-1)?.surfaces.find((p) => p.surface === "library") as { size?: number | null },
    ).toMatchObject({ size: null });
  });

  /**
   * **The two mnemonics.** `Space o l` docks the library; `Space O l` opens
   * it large over the decks, and pressed again leaves it large.
   */
  test("Space Shift+O and a letter opens a panel large over the decks", async ({ page }) => {
    await openShell(page, "/");
    await page.setViewportSize({ width: 1400, height: 860 });
    await page.locator("body").click({ position: { x: 700, y: 120 } });
    await page.keyboard.press("Space");
    await page.keyboard.press("Shift+O");
    await page.keyboard.press("l");
    const library = page.locator(LIBRARY);
    await expect(library).toBeVisible();
    await expect(library).toHaveAttribute("data-lifted", "true");

    // Escape puts it where `Space o l` would have: docked.
    await page.keyboard.press("Escape");
    await expect(library).toHaveAttribute("data-lifted", "false");
    await expect(page.locator(`.dock.bottom ${LIBRARY}`)).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The library taller, quickly.** A double-click on the bottom dock's gap
   * makes it large; another gives it its own share back.
   */
  test("a double-click on the bottom dock's gap makes the library tall, and again gives it back", async ({ page }) => {
    await withLibrary(page);
    const dock = page.locator(".dock.bottom");
    const own = (await dock.boundingBox())!.height;
    await page.locator('[data-dock-grip="bottom"]').dblclick();
    await expect.poll(async () => (await dock.boundingBox())!.height).toBeGreaterThan(own + 80);
    await expect.poll(async () => (await saved(page)).at(-1)?.docks?.bottom ?? null).not.toBeNull();
    await page.locator('[data-dock-grip="bottom"]').dblclick();
    await expect.poll(async () => (await saved(page)).at(-1)?.docks?.bottom ?? null).toBeNull();
    await expect.poll(async () => Math.round((await dock.boundingBox())!.height)).toBeLessThanOrEqual(
      Math.round(own) + 2,
    );
    expect(errorsThrown(page)).toEqual([]);
  });
});
