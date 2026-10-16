/**
 * §127: the playlist under each deck (`DeckDrawer.svelte`).
 *
 * The queue's rules — what comes next, when a deck has run out — are
 * `dj_app::deck_queue`'s and tested there. This holds the drawer: a tab on
 * each deck's edge that costs the deck no height, a drawer per deck, the
 * deck's playlist chosen from the DJ's lists (never a folder), the switches
 * and the loads sent as Rust takes them, and what Rust answers drawn — the
 * record on the deck and the one next.
 *
 * Rust's three commands are answered by a small model installed in the page
 * by this test, which also records what was sent: the karaoke beta's
 * sessions leave `shell.ts` alone (docs/karaoke_beta_release.md §4).
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LISTS = [
  { id: 5, name: "Fillers", parent_id: null, kind: "list", track_count: 3, query: null },
  { id: 6, name: "Old sets", parent_id: null, kind: "folder", track_count: 0, query: null },
  { id: 7, name: "Smart", parent_id: null, kind: "smart", track_count: 9, query: "bpm:120" },
];

const track = (n: number, title: string, artist: string, seconds: number) => ({
  position: n,
  id: `t${n}`,
  path: `/music/${title}.mp3`,
  title,
  artist,
  duration_seconds: seconds,
  bpm: 120,
  key: "8A",
});

const FILLERS = [track(1, "Jingle", "djmanzo", 12), track(2, "Merengue break", "Banda", 95), track(3, "tune_rising", "Trío", 130)];

/** Answer `deck_queue*` from a model per deck, and keep every call. */
async function rust(page: Page) {
  await page.evaluate(
    ({ fillers }) => {
      type Queue = { playlist: number | null; at: number | null; advance: boolean; repeat: boolean };
      const win = window as unknown as {
        __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
        __queueCalls: { cmd: string; args: Record<string, unknown> }[];
      };
      const decks: Record<number, Queue> = {};
      const held = (deck: number) => (decks[deck] ??= { playlist: null, at: null, advance: false, repeat: false });
      const answer = (deck: number) => {
        const q = held(deck);
        const entries = q.playlist === 5 ? fillers : [];
        const len = entries.length;
        const next =
          len === 0 ? null : q.at === null ? 0 : q.at + 1 < len ? q.at + 1 : q.repeat ? 0 : null;
        return {
          deck,
          ...q,
          name: q.playlist === 5 ? "Fillers" : null,
          entries,
          next,
          problem: null,
        };
      };
      win.__queueCalls = [];
      const stub = win.__TAURI_INTERNALS__.invoke;
      win.__TAURI_INTERNALS__.invoke = (cmd, args = {}) => {
        if (!cmd.startsWith("deck_queue")) return stub(cmd, args);
        win.__queueCalls.push({ cmd, args: structuredClone(args) });
        const deck = Number(args.deck);
        if (cmd === "deck_queue_set") {
          const q = held(deck);
          if (q.playlist !== args.playlist) q.at = null;
          Object.assign(q, { playlist: args.playlist, advance: args.advance, repeat: args.repeat });
        }
        if (cmd === "deck_queue_load") held(deck).at = Number(args.index);
        return Promise.resolve(answer(deck));
      };
    },
    { fillers: FILLERS },
  );
}

const sent = (page: Page, cmd: string) =>
  page.evaluate(
    (cmd) =>
      (window as unknown as { __queueCalls: { cmd: string; args: Record<string, unknown> }[] }).__queueCalls
        .filter((call) => call.cmd === cmd)
        .map((call) => call.args),
    cmd,
  );

test.describe("§127: a playlist under each deck", () => {
  test("each deck's drawer chooses its playlist, goes on by itself, and loads onto that deck", async ({ page }) => {
    await openShell(page, "/", {}, { list_playlists: LISTS });
    await rust(page);
    const deck1 = page.locator('section.deck[data-deck="1"]');
    const deck2 = page.locator('section.deck[data-deck="2"]');
    const tab = deck1.getByRole("button", { name: "Deck 1's playlist" });
    await expect(tab).toBeVisible();
    await expect(deck2.getByRole("button", { name: "Deck 2's playlist" })).toBeVisible();

    // The open drawer moves nothing on the deck: it is drawn over the deck's
    // foot, not in its column. (The deck's own height is its row's, so it is
    // the foot's place that says so.)
    const foot = deck1.locator(".deck-foot");
    const before = await foot.boundingBox();
    await tab.click();
    const drawer = page.getByRole("region", { name: "Deck 1's playlist" });
    await expect(drawer).toBeVisible();
    await expect(drawer).toContainText("Choose a playlist for this deck");
    expect(await foot.boundingBox()).toEqual(before);
    const after = await deck1.boundingBox();
    // The drawer is inside its deck, at its foot.
    const box = await drawer.boundingBox();
    expect((box?.y ?? 0) + (box?.height ?? 0)).toBeLessThanOrEqual((after?.y ?? 0) + (after?.height ?? 0) + 1);
    expect(box?.y ?? 0).toBeGreaterThan(after?.y ?? 0);

    // Lists only: a folder and a smart list are not a deck's playlist.
    const choose = drawer.getByRole("combobox", { name: "Playlist for deck 1" });
    await expect(choose.locator("option")).toHaveText(["No playlist", "Fillers — 3"]);
    await choose.selectOption("5");
    expect((await sent(page, "deck_queue_set")).at(-1)).toEqual({ deck: 1, playlist: 5, advance: false, repeat: false });
    const rows = drawer.getByRole("list", { name: "Records in deck 1's playlist" }).getByRole("listitem");
    await expect(rows).toHaveCount(3);
    await expect(drawer).toContainText("3 records · 3:57");
    await expect(rows.nth(0)).toHaveAttribute("data-next", "true");
    await expect(tab).toContainText("Fillers");

    // A filler deck: on by itself, round again.
    await drawer.getByRole("checkbox", { name: "Deck 1: next when this ends" }).check();
    await drawer.getByRole("checkbox", { name: "Deck 1: repeat" }).check();
    expect((await sent(page, "deck_queue_set")).at(-1)).toEqual({ deck: 1, playlist: 5, advance: true, repeat: true });
    await expect(tab.locator(".badge")).toHaveCount(2);

    // ▶ on the last loads and plays it on this deck; round again, the first is next.
    await drawer.getByRole("button", { name: "Load and play tune_rising on deck 1" }).click();
    expect((await sent(page, "deck_queue_load")).at(-1)).toEqual({ deck: 1, index: 2, play: true });
    // (The fixture's deck 1 holds tune_rising, the record Rust says it loaded.)
    await expect(rows.nth(2)).toHaveAttribute("data-at", "true");
    await expect(rows.nth(2)).toContainText("on the deck");
    await expect(rows.nth(0)).toHaveAttribute("data-next", "true");
    // A press on the name loads without playing. The fixture's deck still
    // holds tune_rising, so the queue's word is not taken for the deck's:
    // nothing reads as on the deck.
    await rows.nth(1).getByTitle("Load Merengue break on deck 1").click();
    expect((await sent(page, "deck_queue_load")).at(-1)).toEqual({ deck: 1, index: 1, play: false });
    await expect(drawer.locator('li[data-at="true"]')).toHaveCount(0);
    await expect(rows.nth(2)).toHaveAttribute("data-next", "true");

    // Deck 2's is its own.
    await deck2.getByRole("button", { name: "Deck 2's playlist" }).click();
    await expect(page.getByRole("region", { name: "Deck 2's playlist" })).toContainText("Choose a playlist");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **One deck slim, the other given the room** (§127). With two decks, a
   * press in a deck's header makes it a strip — its record, what is left,
   * play — and the other deck takes the width and opens its playlist. Play
   * on the strip is the deck's own action. *Full size* gives both their
   * room back. Everything stays inside the window.
   */
  test("one deck slim, the other given the room with its playlist open", async ({ page }) => {
    await openShell(page, "/", {}, { list_playlists: LISTS });
    await page.setViewportSize({ width: 1280, height: 800 });
    await rust(page);
    const deck1 = page.locator('section.deck[data-deck="1"]');
    const deck2 = page.locator('section.deck[data-deck="2"]');
    const width = async (deck: typeof deck1) => (await deck.boundingBox())?.width ?? 0;
    const even = await width(deck1);
    expect(Math.abs(even - (await width(deck2)))).toBeLessThan(3);

    await deck1.getByRole("button", { name: "Make deck 1 slim" }).click();
    await expect(deck1.getByRole("group", { name: "Deck 1, slim" })).toBeVisible();
    await expect(deck1.locator(".deck-body")).toHaveCount(0);
    await expect.poll(() => width(deck1)).toBeLessThan(220);
    await expect.poll(() => width(deck2)).toBeGreaterThan(even * 1.5);
    // The record is still named on the strip.
    await expect(deck1.locator("header .title")).toBeVisible();
    // The deck given the room shows its playlist.
    await expect(page.getByRole("region", { name: "Deck 2's playlist" })).toBeVisible();
    // Inside the window.
    const right = await deck2.boundingBox();
    expect((right?.x ?? 0) + (right?.width ?? 0)).toBeLessThanOrEqual(1280);

    await deck1.getByRole("button", { name: /^(Play|Pause) deck 1$/ }).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []))
      .toContain("deck 1 play_pause");

    await deck1.getByRole("button", { name: "Full size" }).click();
    await expect(deck1.locator(".deck-body")).toHaveCount(1);
    await expect.poll(async () => Math.abs((await width(deck1)) - (await width(deck2)))).toBeLessThan(3);
    expect(errorsThrown(page)).toEqual([]);
  });
});
