/**
 * §107: the singer rotation, as a karaoke host works it.
 *
 * The rules — first come, first served, one song a turn, not ready is the
 * bottom, a singer's key remembered with the song — are `dj_app::karaoke`'s and
 * tested there. What a browser can hold is the wiring: that the song a host
 * picks from the collection reaches Rust with its record, that the key goes
 * where the host sets it, and that loading the up-next song puts it on the
 * deck *before* setting the singer's key, since a key sent first would be
 * applied to whatever was on the deck.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const SINGERS = '.surface[data-surface="karaoke"]';

/** Through the Karaoke activity, which is where a host opens it. */
async function openSingers(page: Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Activities", exact: true }).click();
  await page.keyboard.press("8");
  await expect(page.locator(SINGERS)).toBeVisible();
}

const calls = (page: Page) =>
  page.evaluate(() => (window as unknown as { __karaoke?: Record<string, unknown>[] }).__karaoke ?? []);

test.describe("§107: the singer rotation", () => {
  /**
   * **The load-bearing one: a request picked from the collection, keyed,
   * loaded in the singer's key, and sung.** The whole of one singer's turn.
   */
  test("a singer's turn, from request to sung", async ({ page }) => {
    await openSingers(page);
    const singers = page.locator(SINGERS);
    await expect(singers.getByText("Nobody is queued")).toBeVisible();

    await singers.getByRole("textbox", { name: "Singer's name" }).fill("Ana");
    await singers.getByRole("textbox", { name: "Song" }).fill("Bach");
    await singers.getByRole("button", { name: "Juan Luis Guerra – Bachata Rosa" }).click();
    await singers.getByRole("button", { name: "Add", exact: true }).click();

    const asked = (await calls(page)).find((call) => call.cmd === "karaoke_ask");
    expect(asked, "the request never reached Rust").toBeTruthy();
    expect(asked?.singer).toBe("Ana");
    expect(asked?.track, "the record picked from the collection was not sent").toBe("a".repeat(64));
    expect(asked?.path).toBe("/music/bachata-rosa.flac");

    const up = singers.getByRole("region", { name: "Up next" });
    await expect(up).toContainText("Ana");
    await expect(up).toContainText("Bachata Rosa");

    await up.getByRole("button", { name: "Raise the key" }).click();
    await expect(up.locator("[data-key]")).toHaveText("key +1");

    await up.getByRole("button", { name: "Load on 2" }).click();
    // The record, then its key — in that order.
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []))
      .toContain("deck 2 key 1");
    const order = await page.evaluate(() => {
      const asked = (window as unknown as { __asked: string[] }).__asked;
      return { load: asked.lastIndexOf("load_track"), key: asked.lastIndexOf("dispatch") };
    });
    expect(order.load, "the song was never loaded").toBeGreaterThanOrEqual(0);
    expect(order.load, "the key was sent before the record arrived").toBeLessThan(order.key);

    await up.getByRole("button", { name: "Sang", exact: true }).click();
    await expect(singers.getByText("Nobody is queued")).toBeVisible();
    await singers.getByText("Sung lately").click();
    await expect(singers.getByText("Ana — Juan Luis Guerra – Bachata Rosa (+1)")).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A song nobody has found in the collection yet can still be queued, and says so. */
  test("a song not found yet is queued and says it needs finding", async ({ page }) => {
    await openSingers(page);
    const singers = page.locator(SINGERS);
    await singers.getByRole("textbox", { name: "Singer's name" }).fill("Ben");
    await singers.getByRole("textbox", { name: "Song" }).fill("A song nobody has");
    await singers.getByRole("button", { name: "Add", exact: true }).click();
    const up = singers.getByRole("region", { name: "Up next" });
    await expect(up).toContainText("Not found in the collection yet");
    await expect(up.getByRole("button", { name: /^Load on/ })).toHaveCount(0);
  });

  /**
   * **The vocal, taken out per deck, and said on the deck.** The host chooses
   * on the Singers surface; the deck then carries a chip saying so, because
   * the setting outlives the karaoke screen, and one press on it puts the
   * voice back. The chip must not move the waveform under it (§18).
   */
  test("the host takes the vocal out, the deck says so, and one press puts it back", async ({ page }) => {
    await openSingers(page);
    const singers = page.locator(SINGERS);
    const voice1 = singers.getByRole("group", { name: "Vocal on deck 1" });
    await expect(voice1.getByRole("button", { name: "As recorded" })).toHaveAttribute("aria-pressed", "true");

    await voice1.getByRole("button", { name: "Out" }).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []))
      .toContain("deck 1 voice 0");
    await voice1.getByRole("button", { name: "Guide" }).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []))
      .toContain("deck 1 voice 0.25");

    // The engine answers on the snapshot; the deck shows it without moving.
    const lane = page.locator('.deck[data-deck="1"] .lane');
    const before = await lane.boundingBox();
    await page.evaluate(() => {
      const win = window as unknown as {
        __lastState?: { decks: Record<string, unknown>[] };
        __emit?: (next: unknown) => void;
      };
      const state = win.__lastState;
      if (!state) throw new Error("no state");
      win.__emit?.({ ...state, decks: state.decks.map((d, i) => (i === 0 ? { ...d, voice: 0 } : d)) });
    });
    const chip = page.locator('.deck[data-deck="1"] [data-voice-chip]');
    await expect(chip).toHaveText("Voice out");
    await expect(voice1.getByRole("button", { name: "Out" })).toHaveAttribute("aria-pressed", "true");
    expect((await lane.boundingBox())?.y, "the chip moved the waveform").toBe(before?.y);
    await expect(page.locator('.deck[data-deck="2"] [data-voice-chip]')).toHaveCount(0);

    await chip.click();
    await expect
      .poll(() =>
        page.evaluate(() => ((window as unknown as { __dispatched?: string[] }).__dispatched ?? []).at(-1)),
      )
      .toBe("deck 1 voice 1");
    expect(errorsThrown(page)).toEqual([]);
  });

  /** F8 is the host's own screen: the rotation beside the decks, the collection under them. */
  test("the Karaoke activity opens the rotation and the collection", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Activities", exact: true }).click();
    await page.keyboard.press("8");
    await expect(page.locator('[data-activity="karaoke"]')).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator(SINGERS)).toBeVisible();
    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();
  });
});
