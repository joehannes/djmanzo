/**
 * §128: the top bar is one line.
 *
 * > make the navbar a one-liner as of its height, and auto-condense the
 * > items into symbol only buttons if they don't fit
 *
 * At djmanzo's own window and at a small laptop's: one row, every control
 * in it inside the bar and on that row, and what folded away (the night's
 * readings, the guides) back on ⋯.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Every visible control in the bar: inside it, and all on one row. */
async function oneRow(page: Page) {
  const bar = page.locator("header.topbar");
  await expect(bar).toBeVisible();
  // Let the fitting settle.
  await page.waitForTimeout(300);
  const box = (await bar.boundingBox())!;
  const buttons = bar.locator(":scope button:visible");
  const count = await buttons.count();
  expect(count).toBeGreaterThan(3);
  const middles: number[] = [];
  for (let i = 0; i < count; i++) {
    const b = await buttons.nth(i).boundingBox();
    if (!b || b.width === 0) continue;
    // Not in the folded row under the bar, which is open on demand only.
    if (b.y > box.y + box.height) continue;
    expect(b.x, `button ${i} starts inside the bar`).toBeGreaterThanOrEqual(box.x - 1);
    expect(b.x + b.width, `button ${i} ends inside the bar`).toBeLessThanOrEqual(box.x + box.width + 1);
    middles.push(b.y + b.height / 2);
  }
  const spread = Math.max(...middles) - Math.min(...middles);
  expect(spread, "every control on one row").toBeLessThan(12);
  expect(box.height, "one row high").toBeLessThan(58);
  return box;
}

test.describe("§128: a one-line top bar", () => {
  /** djmanzo's default (§117): no toolbars. */
  const SLIM = { interface_settings: { toolbars: false, uses: {} } };

  test("one row at djmanzo's own window, with its words", async ({ page }) => {
    await openShell(page, "/", {}, SLIM);
    await page.setViewportSize({ width: 1400, height: 900 });
    await oneRow(page);
    await page.screenshot({ path: "test-results/navbar-1400.png" });
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * The full toolbars (§117's opt-in) keep every panel and stage control —
   * folding Activities or the workspace away would hide what that mode is
   * for — so they keep the rows they always had, and nothing is cut off.
   */
  test("with the full toolbars, nothing is folded away or cut off", async ({ page }) => {
    await openShell(page, "/");
    await page.setViewportSize({ width: 1280, height: 800 });
    const bar = page.locator("header.topbar");
    await page.waitForTimeout(300);
    for (const name of ["Activities", "Browse", "Settings"]) {
      await expect(bar.getByRole("button", { name, exact: true })).toBeVisible();
    }
    await expect(bar.getByRole("combobox", { name: "Layout" })).toBeVisible();
    const box = (await bar.boundingBox())!;
    expect(box.height, "no more rows than before").toBeLessThan(150);
    const buttons = bar.locator(":scope button:visible");
    for (let i = 0; i < (await buttons.count()); i++) {
      const b = await buttons.nth(i).boundingBox();
      if (!b || b.width === 0) continue;
      expect(b.x + b.width).toBeLessThanOrEqual(box.x + box.width + 1);
    }
    await page.screenshot({ path: "test-results/navbar-1280-toolbars.png" });
    expect(errorsThrown(page)).toEqual([]);
  });

  test("on a small laptop it condenses and folds, and ⋯ brings the rest back", async ({ page }) => {
    await openShell(page, "/", {}, SLIM);
    await page.setViewportSize({ width: 1024, height: 768 });
    await oneRow(page);
    const bar = page.locator("header.topbar");
    expect(Number(await bar.getAttribute("data-fit"))).toBeGreaterThanOrEqual(1);
    // Words gone, names kept.
    await expect(bar.getByRole("button", { name: "Launcher" })).toBeVisible();
    await expect(bar.getByRole("button", { name: "Keys" })).toBeVisible();
    await page.screenshot({ path: "test-results/navbar-1024.png" });
    const more = bar.getByRole("button", { name: "The night's readings and the guides" });
    if ((await bar.getAttribute("data-fit")) !== "1") {
      await expect(bar.locator(".status")).toBeHidden();
      await more.click();
      await expect(bar.locator(".status")).toBeVisible();
      // Inside the bar, which has grown to hold it.
      const under = (await bar.locator(".status").boundingBox())!;
      const box = (await bar.boundingBox())!;
      expect(under.y + under.height).toBeLessThanOrEqual(box.y + box.height + 1);
      await page.screenshot({ path: "test-results/navbar-1024-open.png" });
      await more.click();
      await expect(bar.locator(".status")).toBeHidden();
      await oneRow(page);
    }
    // Wide again: the words come back.
    await page.setViewportSize({ width: 1600, height: 900 });
    await expect.poll(async () => bar.getAttribute("data-fit")).toBe("0");
    await oneRow(page);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **§128's ⚙: every source, by what it is for.** One press in the bar opens
   * the board; Rust's six jobs in its order, each source under the jobs it
   * does, whether it works now; *Set up →* takes the host to Settings.
   */
  test("the cogwheel opens every source by what it is for, and Set up goes to Settings", async ({ page }) => {
    const row = (job: string, name: string, ready: string, setup: string | null) => ({
      job,
      name,
      what: `${name} does it`,
      ready,
      detail: ready === "ready" ? "Ready" : "Needs a key",
      setup,
    });
    const board = {
      jobs: [
        { job: "search", title: "Finding songs", rows: [row("search", "Local library", "ready", null), row("search", "Spotify", "needs_setup", "sources")] },
        { job: "play", title: "Playing them", rows: [row("play", "Local library", "ready", null)] },
        { job: "buy", title: "Buying them", rows: [row("buy", "Bandcamp", "ready", null)] },
        { job: "lyrics", title: "Lyrics", rows: [row("lyrics", "LRCLIB", "ready", null)] },
        { job: "similar", title: "Songs like this one", rows: [row("similar", "Your AI assistant", "needs_setup", "assistant")] },
        { job: "imagery", title: "Covers and artists", rows: [row("imagery", "Cover Art Archive", "ready", null)] },
      ],
    };
    await openShell(page, "/", {}, { ...SLIM, sources_board: board });
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.locator("header.topbar").getByRole("button", { name: "Sources and connections" }).click();
    const dialog = page.getByRole("dialog", { name: "Sources and connections" });
    await expect(dialog).toBeVisible();
    await expect(dialog.locator("h3")).toHaveText([
      "🔎 Finding songs",
      "▶ Playing them",
      "🛒 Buying them",
      "🎤 Lyrics",
      "✨ Songs like this one",
      "🖼 Covers and artists",
    ]);
    const search = dialog.getByRole("region", { name: "Finding songs" });
    await expect(search.locator('li[data-ready="ready"]')).toContainText("Local library");
    await expect(search.locator('li[data-ready="needs_setup"]')).toContainText("Spotify");
    // Only what needs setting up offers it.
    await expect(dialog.getByRole("button", { name: "Set up →" })).toHaveCount(2);
    await search.getByRole("button", { name: "Set up →" }).click();
    await expect(dialog).toHaveCount(0);
    // Settings, at its Sources block — not at its top.
    await expect(page.locator('[data-settings="sources"]')).toBeInViewport();
    // And the assistant's row goes to the AI block.
    await page.locator("header.topbar").getByRole("button", { name: "Sources and connections" }).click();
    await dialog.getByRole("region", { name: "Songs like this one" }).getByRole("button", { name: "Set up →" }).click();
    await expect(page.locator('[data-settings="ai"]')).toBeInViewport();
    // And closes on its ✕.
    await page.locator("header.topbar").getByRole("button", { name: "Sources and connections" }).click();
    await page.getByRole("button", { name: "Close sources and connections" }).click();
    await expect(page.getByRole("dialog", { name: "Sources and connections" })).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});

