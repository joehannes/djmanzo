/**
 * K4: singers from the room (docs/KARAOKE.md §7).
 *
 * The page a phone sees, the book behind it and the rotation it feeds are
 * Rust's, and `dj_net::front`'s tests drive them over a real socket. This
 * holds the host's half: the switches send what the host chose, the panel
 * says when the page is not being served and how big the songbook is, and a
 * singer who put their own name down — and pressed *I'm ready* — says so in
 * the rotation.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

async function singers(page: Page, answers: Record<string, unknown> = {}) {
  await openShell(page, "/", {}, answers);
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.locator("body").click({ position: { x: 700, y: 120 } });
  await page.keyboard.press("Space");
  await page.keyboard.press("O");
  await page.keyboard.press("i");
  await expect(page.locator("[data-singers]")).toBeVisible();
}

const calls = (page: Page) =>
  page.evaluate(() => (window as unknown as { __singCalls?: Record<string, unknown>[] }).__singCalls ?? []);

const phones = (page: Page) => page.getByRole("region", { name: "Singers from phones" });

test.describe("K4: singers from the room", () => {
  test("the host opens sign-ups and chooses how songs are chosen", async ({ page }) => {
    await singers(page, {
      sing_rules: { open: false, songs_each: 1, songbook: true, key: false, guide: false, songs: 42, serving: false },
    });
    const section = phones(page);
    const open = section.getByRole("checkbox", { name: "Singers put their names down from their phones" });
    await expect(open).not.toBeChecked();
    await expect(section.locator("[data-songbook]")).toHaveText("42 records with words in the songbook.");
    // Closed: no warning about the page yet.
    await expect(section.getByRole("status")).toHaveCount(0);

    await open.check();
    await expect(section.getByRole("status")).toContainText("The room's page is not being served");
    await section.getByRole("group", { name: "Songs waiting per singer" }).getByRole("button", { name: "2" }).click();
    await expect(
      section.getByRole("group", { name: "Songs waiting per singer" }).getByRole("button", { name: "2" }),
    ).toHaveAttribute("aria-pressed", "true");
    await section.getByRole("button", { name: "Written in" }).click();
    await expect(section.locator("[data-songbook]")).toContainText("found in your collection by you");

    await section.getByRole("checkbox", { name: "May ask for a key" }).check();
    await section.getByRole("checkbox", { name: "May ask for the guide vocal" }).check();

    expect(await calls(page)).toEqual([
      { open: true },
      { songsEach: 2 },
      { songbook: false },
      { key: true },
      { guide: true },
    ]);
    expect(errorsThrown(page)).toEqual([]);
  });

  test("a singer from a phone, ready, says so in the rotation", async ({ page }) => {
    await singers(page, {
      sing_rules: { open: true, songs_each: 1, songbook: true, key: true, guide: true, songs: 3, serving: true },
      karaoke_rotation: {
        singers: [
          {
            name: "Ana",
            songs: [{ title: "Aventura — Obsesión", track: "00ff", path: "/m/o.flac", key: 0 }],
            turns: 0,
            from_phone: true,
            ready: true,
          },
          {
            name: "Carla",
            songs: [{ title: "Suavemente", track: null, path: null, key: 0 }],
            turns: 0,
          },
          {
            name: "Bob",
            songs: [{ title: "Toxic", track: null, path: null, key: 0 }],
            turns: 0,
            from_phone: true,
          },
        ],
        up_next: "Ana",
        lately: [],
      },
    });
    const up = page.getByRole("region", { name: "Up next" });
    await expect(up.locator("[data-ready]")).toHaveText("Ready");
    await expect(up.getByText("phone", { exact: true })).toBeVisible();
    const order = page.getByRole("list", { name: "Calling order" });
    await expect(order.getByRole("listitem").filter({ hasText: "Bob" }).getByText("phone")).toBeVisible();
    await expect(order.getByRole("listitem").filter({ hasText: "Carla" }).getByText("phone")).toHaveCount(0);
    // Open, served: nothing to warn about.
    await expect(phones(page).getByRole("status")).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** **K4: the host's line goes to the singers' screen**, kept on Enter. */
  test("the host writes a line for the singers' screen", async ({ page }) => {
    await singers(page);
    const field = page.getByRole("textbox", { name: "On the singers' screen" });
    await field.fill("  Last orders   at one ");
    await field.press("Enter");
    await expect(field).toHaveValue("Last orders at one");
    const karaoke = await page.evaluate(
      () => (window as unknown as { __karaoke?: { cmd: string; text?: string }[] }).__karaoke ?? [],
    );
    expect(karaoke.filter((c) => c.cmd === "karaoke_line")).toEqual([
      { cmd: "karaoke_line", text: "  Last orders   at one " },
    ]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **What a singer asks for waits for the host.** Shown as asked; taken,
   * it becomes the song's key, and loading the song sets the deck's vocal
   * as asked; refused, the song stays as it was.
   */
  test("a key and a guide asked for are taken or refused, and taken is what loads", async ({ page }) => {
    await singers(page, {
      sing_rules: { open: true, songs_each: 1, songbook: true, key: true, guide: true, songs: 3, serving: true },
      karaoke_rotation: {
        singers: [
          {
            name: "Ana",
            songs: [
              {
                title: "Aventura — Obsesión",
                track: "00ff",
                path: "/m/o.flac",
                key: 0,
                asks: { key: -2, voice: "guide" },
              },
            ],
            turns: 0,
            from_phone: true,
          },
          {
            name: "Bob",
            songs: [{ title: "Toxic", track: null, path: null, key: 0, asks: { key: 3, voice: null } }],
            turns: 0,
            from_phone: true,
          },
        ],
        up_next: "Ana",
        lately: [],
      },
    });
    const up = page.getByRole("region", { name: "Up next" });
    const asks = up.getByRole("group", { name: "What Ana asks for" });
    await expect(asks).toContainText("asks key -2 · guide");
    await expect(up.locator("[data-key]")).toHaveText("as recorded", { useInnerText: true });
    await asks.getByRole("button", { name: "Take" }).click();
    await expect(up.locator("[data-key]")).toHaveText("key -2");
    await expect(up.locator("[data-asks]")).toHaveCount(0);

    await up.getByRole("button", { name: "Load on 1" }).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []))
      .toEqual(expect.arrayContaining(["deck 1 key -2", "deck 1 voice 0.25"]));

    const order = page.getByRole("list", { name: "Calling order" });
    await expect(order.locator("[data-asks]")).toHaveText("asks key +3");
    await order.getByRole("button", { name: "Refuse what Bob asks for" }).click();
    await expect(order.locator("[data-asks]")).toHaveCount(0);
    const karaoke = await page.evaluate(
      () => (window as unknown as { __karaoke?: { cmd: string; singer?: string }[] }).__karaoke ?? [],
    );
    expect(karaoke.filter((c) => c.cmd === "karaoke_take" || c.cmd === "karaoke_refuse")).toEqual([
      { cmd: "karaoke_take", singer: "Ana" },
      { cmd: "karaoke_refuse", singer: "Bob" },
    ]);
    expect(errorsThrown(page)).toEqual([]);
  });
});
