/**
 * §124: one search, with the other sources a tick away, and typos forgiven.
 *
 * > as of the library ... there shall be no extra view for external sources,
 * > but the regular search shall have a tickable toggle or something to
 * > include external sources ... also it might be some fuzzy search
 *
 * The browser had two tabs, the collection and "Sources", each with its own
 * search box. Now there is the collection's, with a tick that brings the
 * other sources' answers in below its own; and a record found only by the
 * typo-tolerant pass in Rust is marked as close rather than exact.
 */
import { expect, test, type Page } from "@playwright/test";

import { ANSWERS, errorsThrown, openShell } from "./shell";

const LIBRARY = '.surface[data-surface="library"]';

const rows = ANSWERS.library_search as Record<string, unknown>[];

const sources = [
  {
    provider: "bandcamp",
    label: "Bandcamp",
    matched_locally: 0,
    error: null,
    tracks: [
      {
        provider: "bandcamp",
        id: "bc-1",
        title: "Bachata en Fukuoka",
        artist: "Juan Luis Guerra",
        album: null,
        duration_seconds: 231,
        bpm: 126,
        key: null,
        genre: null,
        artwork_url: null,
        web_url: null,
        playable: false,
      },
    ],
  },
];

async function withLibrary(page: Page, answers: Record<string, unknown> = {}) {
  await openShell(page, "/", {}, { search_sources: sources, ...answers });
  await page.setViewportSize({ width: 1400, height: 860 });
  await page.evaluate(() => {
    try {
      localStorage.removeItem("djmanzo.library.outside");
    } catch {
      // Nothing stored.
    }
  });
  await page.getByRole("button", { name: "Browse", exact: true }).click();
  await expect(page.locator(LIBRARY)).toBeVisible();
}

const asked = (page: Page) =>
  page.evaluate(() => ((window as unknown as { __asked?: string[] }).__asked ?? []).filter((c) => c === "search_sources").length);

test.describe("§124: one search for everything", () => {
  /** The second tab is gone: one search box, and no "Sources" view. */
  test("the library has one search box and no separate sources view", async ({ page }) => {
    await withLibrary(page);
    const library = page.locator(LIBRARY);
    await expect(library.getByRole("searchbox")).toHaveCount(1);
    await expect(library.getByRole("searchbox", { name: "Search the library" })).toBeVisible();
    await expect(library.getByRole("tab")).toHaveCount(0);
    await expect(library.getByRole("checkbox", { name: "Include external sources" })).not.toBeChecked();
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The load-bearing one.** Off, a search asks the collection only; ticked,
   * the other sources' answers come below the collection's own, in the same
   * list, and a result that cannot be mixed says so where its button would
   * be. Unticked again, they go.
   */
  test("ticked, the search brings the other sources' answers in below the collection's", async ({ page }) => {
    await withLibrary(page);
    const library = page.locator(LIBRARY);
    await library.getByRole("searchbox", { name: "Search the library" }).fill("bachata");
    await page.waitForTimeout(900);
    expect(await asked(page)).toBe(0);
    await expect(library.getByRole("region", { name: "From your other sources" })).toHaveCount(0);

    await library.getByRole("checkbox", { name: "Include external sources" }).check();
    const outside = library.getByRole("region", { name: "From your other sources" });
    await expect(outside).toBeVisible();
    await expect(outside.getByText("Bachata en Fukuoka")).toBeVisible();
    await expect(outside.getByText("not mixable")).toBeVisible();
    expect(await asked(page)).toBeGreaterThan(0);

    // Below the collection's own answers, in the same scrolling list.
    const table = (await library.locator("table").boundingBox())!;
    const below = (await outside.boundingBox())!;
    expect(below.y).toBeGreaterThanOrEqual(table.y + table.height - 1);
    await expect(library.locator(".table-scroll").getByRole("region", { name: "From your other sources" })).toHaveCount(1);

    // Kept on this machine.
    await page.reload();
    await page.getByRole("button", { name: "Browse", exact: true }).click().catch(() => {});
    await expect(
      page.locator(LIBRARY).getByRole("checkbox", { name: "Include external sources" }),
    ).toBeChecked();

    await page.locator(LIBRARY).getByRole("checkbox", { name: "Include external sources" }).uncheck();
    await expect(page.locator(LIBRARY).getByRole("region", { name: "From your other sources" })).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Fuzzy.** A record Rust found by the typo pass is marked as close, and
   * stays below the exact answers whichever column the table is sorted by.
   */
  test("a record found by the typo pass is marked close and stays below the exact ones", async ({ page }) => {
    const near = { ...rows[1], id: "c".repeat(64), title: "Aaa Close One", near: true };
    await withLibrary(page, { library_search: [rows[0], near] });
    const library = page.locator(LIBRARY);
    await library.getByRole("searchbox", { name: "Search the library" }).fill("bachta");
    const titles = library.locator("tbody td.title");
    await expect(titles).toHaveCount(2);
    await expect(titles.nth(1).locator(".near")).toHaveText("≈");
    await expect(titles.nth(1).locator(".near")).toHaveAttribute("title", /Close to what you typed/);
    await expect(titles.nth(0).locator(".near")).toHaveCount(0);

    // Sorted by title, "Aaa…" would come first; it is a near match, so not.
    await library.getByRole("button", { name: "Title" }).click();
    await expect(titles.nth(0)).toContainText("Bachata Rosa");
    await library.getByRole("button", { name: /^Title/ }).click();
    await expect(titles.nth(0)).toContainText("Bachata Rosa");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Sticky.** With a long list along the bottom, the list scrolls and
   * nothing else does: the search box and the crates stay where they were,
   * and the dock itself does not scroll them away.
   */
  test("a long list scrolls under its search and beside its crates", async ({ page }) => {
    const many = Array.from({ length: 80 }, (_, i) => ({
      ...rows[i % 2],
      id: String(i).padStart(64, "0"),
      title: `Record ${i}`,
    }));
    await withLibrary(page, { library_search: many });
    await page.setViewportSize({ width: 1280, height: 800 });
    const library = page.locator(LIBRARY);
    const search = library.getByRole("searchbox", { name: "Search the library" });
    const crate = library.getByRole("button", { name: /All tracks/ }).first();
    await expect(library.locator("tbody tr")).toHaveCount(80);
    const searchAt = (await search.boundingBox())!;
    const crateAt = (await crate.boundingBox())!;

    // The panel is no taller than its dock…
    const dock = (await page.locator(".dock.bottom").boundingBox())!;
    const panel = (await library.boundingBox())!;
    expect(panel.height).toBeLessThanOrEqual(dock.height + 1);

    // …so a wheel over the rows scrolls the rows.
    const list = library.locator(".table-scroll");
    const box = (await list.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.wheel(0, 3000);
    await expect.poll(() => list.evaluate((el) => el.scrollTop)).toBeGreaterThan(500);
    expect(await page.locator(".dock.bottom").evaluate((el) => el.scrollTop)).toBe(0);
    expect((await search.boundingBox())!.y).toBeCloseTo(searchAt.y, 0);
    expect((await crate.boundingBox())!.y).toBeCloseTo(crateAt.y, 0);
    await expect(library.getByText("Record 79")).toBeInViewport();
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * In a narrow side panel the search row wraps rather than making the whole
   * panel scroll sideways with the crates and the search in it.
   */
  test("in a narrow panel the search keeps its width and nothing scrolls sideways but the table", async ({ page }) => {
    await withLibrary(page);
    await page.getByRole("button", { name: "Move Library" }).focus();
    await page.keyboard.press("ArrowRight");
    const library = page.locator(`.dock.side.right ${LIBRARY}`);
    await expect(library).toBeVisible();
    const body = library.locator(".surface-body");
    const [scroll, client] = await body.evaluate((el) => [el.scrollWidth, el.clientWidth]);
    expect(scroll).toBeLessThanOrEqual(client + 1);
    // At least the twelve rems the box asks for, whatever the density makes
    // a rem: it was squeezed to a few pixels.
    const rem = await page.evaluate(() => parseFloat(getComputedStyle(document.documentElement).fontSize));
    const search = (await library.getByRole("searchbox", { name: "Search the library" }).boundingBox())!;
    expect(search.width).toBeGreaterThanOrEqual(12 * rem - 1);
    await expect(library.getByRole("button", { name: /All tracks/ }).first()).toBeInViewport();
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * The Assistant is the one panel that scrolls as a whole: its own bar and
   * the box to ask in hold to their edges while the rest scrolls between.
   */
  test("the assistant's bar and its ask box stay in view as the panel scrolls", async ({ page }) => {
    await openShell(page, "/");
    await page.setViewportSize({ width: 1280, height: 700 });
    await page.getByRole("button", { name: "Assistant", exact: true }).click();
    const panel = page.locator('.surface[data-surface="assistant"]');
    const body = panel.locator(".surface-body");
    const [scroll, client] = await body.evaluate((el) => [el.scrollHeight, el.clientHeight]);
    expect(scroll, "the panel must overflow for this to show anything").toBeGreaterThan(client + 50);
    const view = (await body.boundingBox())!;
    await body.evaluate((el) => el.scrollTo(0, el.scrollHeight));
    await body.evaluate((el) => el.scrollTo(0, el.scrollHeight / 2));
    const bar = (await panel.getByRole("button", { name: /^(Setup|Hide setup)$/ }).boundingBox())!;
    const ask = (await panel.getByRole("button", { name: "Send" }).boundingBox())!;
    expect(bar.y).toBeGreaterThanOrEqual(view.y - 1);
    expect(bar.y + bar.height).toBeLessThanOrEqual(view.y + view.height / 3);
    expect(ask.y + ask.height).toBeLessThanOrEqual(view.y + view.height + 1);
    expect(ask.y).toBeGreaterThanOrEqual(view.y + (view.height * 2) / 3);
    expect(errorsThrown(page)).toEqual([]);
  });
});
