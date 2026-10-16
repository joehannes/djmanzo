/**
 * Karaoke beta B2: emoji from the room on the singers' screen, held back.
 *
 * The rates, the glyphs and what is kept are Rust's (`dj_net::reactions`,
 * tested there); what a browser holds is how the screen draws them — small,
 * in a corner away from the words, a few at once, gone within seconds,
 * nothing while the host has them off — and the host's switch.
 */
import { expect, test, type Page } from "@playwright/test";

import { openShell } from "./shell";


const WORDS = {
  lines: [
    { at: 10, text: "First line", words: [] },
    {
      at: 13,
      text: "So long my friend",
      words: [
        [13, "So"],
        [13.5, "long"],
        [14, "my"],
        [15, "friend"],
      ],
    },
    { at: 30, text: "After a break", words: [] },
  ],
  plain: [],
  instrumental: false,
};

const ROTATION = {
  singers: [
    { name: "Ana", songs: [{ title: "Bachata Rosa", track: null, path: null, key: 0 }], turns: 0 },
    { name: "Ben", songs: [{ title: "Hey Jude", track: null, path: null, key: 0 }], turns: 0 },
  ],
  up_next: "Ana",
  lately: [],
};

const SCREEN = "[data-singer-screen]";

/** Deck 2 is the singer's: playing, vocal out, at `seconds`, at 120 BPM. */
async function singing(page: Page, seconds: number) {
  await page.evaluate((seconds) => {
    const win = window as unknown as {
      __lastState?: { decks: Record<string, unknown>[] };
      __emit?: (next: unknown) => void;
    };
    const state = win.__lastState;
    if (!state) throw new Error("no state");
    const decks = state.decks.map((deck, index) =>
      index === 1
        ? { ...deck, playing: true, loaded: true, voice: 0, position_seconds: seconds, effective_bpm: 120, title: "Bachata Rosa" }
        : { ...deck, playing: index === 0, voice: 1 },
    );
    win.__emit?.({ ...state, decks });
  }, seconds);
}

/**
 * Answer `room_reactions*` as Rust would — the switch, and what is newer
 * than `since` — and keep every call to the switch. `__send(glyph)` is a
 * tap from the room.
 */
async function room(page: Page, on: boolean) {
  await page.evaluate((on) => {
    const win = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
      __emojiCalls: { args: Record<string, unknown> }[];
      __send: (emoji: string) => void;
    };
    const held = { on, next: 0, sent: [] as { id: number; emoji: string }[] };
    win.__emojiCalls = [];
    win.__send = (emoji) => {
      if (held.on) held.sent.push({ id: ++held.next, emoji });
    };
    const answer = (since: number) => ({ on: held.on, reactions: held.sent.filter((r) => r.id > since) });
    const stub = win.__TAURI_INTERNALS__.invoke;
    win.__TAURI_INTERNALS__.invoke = (cmd, args = {}) => {
      if (cmd === "room_reactions") return Promise.resolve(answer(Number(args.since ?? 0)));
      if (cmd === "room_reactions_set") {
        win.__emojiCalls.push({ args: structuredClone(args) });
        held.on = Boolean(args.on);
        if (!held.on) held.sent = [];
        return Promise.resolve(answer(0));
      }
      return stub(cmd, args);
    };
  }, on);
}

const send = (page: Page, emoji: string, times = 1) =>
  page.evaluate(
    ({ emoji, times }) => {
      for (let i = 0; i < times; i++) (window as unknown as { __send: (e: string) => void }).__send(emoji);
    },
    { emoji, times },
  );

test.describe("Karaoke beta B2: emoji from the room", () => {
  /**
   * A tap shows small, in the screen's corner, clear of the line being
   * sung, and is gone within a few seconds.
   */
  test("show small in a corner, clear of the words, and go", async ({ page }) => {
    await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, karaoke_rotation: ROTATION });
    await expect(page.locator(SCREEN)).toBeVisible();
    await room(page, true);
    await singing(page, 14.5);
    await expect(page.locator(`${SCREEN} [data-line="now"]`)).toBeVisible();
    await send(page, "👏");
    const one = page.locator(`${SCREEN} [data-reaction="👏"]`);
    await expect(one).toHaveCount(1, { timeout: 3_000 });

    const view = page.viewportSize()!;
    const glyph = (await one.boundingBox())!;
    expect(glyph.x, "in the right-hand corner").toBeGreaterThan(view.width * 0.75);
    expect(glyph.y, "in the bottom half").toBeGreaterThan(view.height * 0.5);
    expect(glyph.height, "small").toBeLessThan(view.height * 0.08);
    const line = (await page.locator(`${SCREEN} [data-line="now"]`).boundingBox())!;
    const overlaps =
      glyph.x < line.x + line.width && glyph.x + glyph.width > line.x && glyph.y < line.y + line.height && glyph.y + glyph.height > line.y;
    expect(overlaps, "never over the words").toBe(false);
    await expect(page.locator(`${SCREEN} .room-emoji`)).toHaveCSS("pointer-events", "none");

    await expect(one).toHaveCount(0, { timeout: 5_000 });
  });

  /** A burst from the room is a few at once, never a flood. */
  test("draw only a few at once", async ({ page }) => {
    await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, karaoke_rotation: ROTATION });
    await expect(page.locator(SCREEN)).toBeVisible();
    await room(page, true);
    await send(page, "🔥", 12);
    await expect(page.locator(`${SCREEN} [data-reaction]`).first()).toBeVisible({ timeout: 3_000 });
    expect(await page.locator(`${SCREEN} [data-reaction]`).count()).toBeLessThanOrEqual(5);
  });

  test("draw nothing while the host has them off", async ({ page }) => {
    await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, karaoke_rotation: ROTATION });
    await expect(page.locator(SCREEN)).toBeVisible();
    await room(page, false);
    await singing(page, 14.5);
    await send(page, "👏", 3);
    await page.waitForTimeout(3_500);
    await expect(page.locator(`${SCREEN} [data-reaction]`)).toHaveCount(0);
  });

  /** The host's switch, in the Singers panel, reaches Rust and says what it means. */
  test("are turned on and off by the host", async ({ page }) => {
    await openShell(page, "/");
    await room(page, false);
    await page.setViewportSize({ width: 1400, height: 900 });
    await page.locator("body").click({ position: { x: 700, y: 120 } });
    await page.keyboard.press("Space");
    await page.keyboard.press("O");
    await page.keyboard.press("i");
    await expect(page.locator("[data-singers]")).toBeVisible();
    const section = page.getByRole("region", { name: "Emoji from the room" });
    const box = section.getByRole("checkbox", { name: "Emoji from phones on the singers' screen" });
    await expect(box).not.toBeChecked();
    await expect(section).toContainText("Off: the room's page shows no row");
    await box.check();
    await expect(box).toBeChecked();
    await expect(section).toContainText("never over the words");
    const sent = await page.evaluate(
      () => (window as unknown as { __emojiCalls: { args: Record<string, unknown> }[] }).__emojiCalls.map((c) => c.args),
    );
    expect(sent).toEqual([{ on: true }]);
  });
});
