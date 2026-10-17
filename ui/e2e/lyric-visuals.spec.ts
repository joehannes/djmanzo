/**
 * Karaoke beta B3: the singers' screen, between and within the words.
 *
 * Where a break begins and ends, and whose line a duet's line is, are
 * `./lyricBreak`'s (tested there, with `lyricsAt`'s timings); what a browser
 * holds is the screen drawing them — a bar that fills over an instrumental
 * break and says the seconds left, a soft glow on the one word being sung,
 * and in a duet each singer's name and lines in their microphone's colour.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

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
    { at: 30, text: "Ana: after a break", words: [] },
    {
      at: 33,
      text: "Ben: and you",
      words: [
        [33, "Ben:"],
        [33.2, "and"],
        [33.6, "you"],
      ],
    },
  ],
  plain: [],
  instrumental: false,
};

const ROTATION = {
  singers: [{ name: "Ana", songs: [{ title: "Bachata Rosa", track: null, path: null, key: 0 }], turns: 0 }],
  up_next: "Ana",
  lately: [],
};

const SCREEN = "[data-singer-screen]";

/** Deck 2 is the singer's at `seconds`; the microphones' levels are `levels`. */
async function at(page: Page, seconds: number, levels: number[] = []) {
  await page.evaluate(
    ({ seconds, levels }) => {
      const win = window as unknown as {
        __lastState?: { decks: Record<string, unknown>[]; master: Record<string, unknown> };
        __emit?: (next: unknown) => void;
      };
      const state = win.__lastState;
      if (!state) throw new Error("no state");
      const decks = state.decks.map((deck, index) =>
        index === 1
          ? { ...deck, playing: true, loaded: true, voice: 0, position_seconds: seconds, effective_bpm: 120, title: "Bachata Rosa" }
          : { ...deck, playing: false, voice: 1 },
      );
      const vocals =
        levels.length > 0
          ? {
              inputs: levels.length,
              working: levels.length,
              starved_frames: 0,
              strips: levels.map((level) => ({ level, gate_open: level > 0, compression_db: 0, working: true })),
              monitor_music_db: null,
              settings_rev: 0,
            }
          : (state.master.vocals ?? null);
      win.__emit?.({ ...state, decks, master: { ...state.master, vocals } });
    },
    { seconds, levels },
  );
}

/** Rust's `vocals_state`: who is on each microphone. */
async function onMics(page: Page, names: (string | null)[]) {
  await page.evaluate((names) => {
    const win = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
    };
    const stub = win.__TAURI_INTERNALS__.invoke;
    win.__TAURI_INTERNALS__.invoke = async (cmd, args = {}) => {
      if (cmd === "vocals_state") {
        const held = (await stub(cmd, args)) as Record<string, unknown>;
        return { ...held, on: names.map((singer) => (singer ? { singer, kept: null } : null)) };
      }
      return stub(cmd, args);
    };
  }, names);
}

async function screen(page: Page) {
  await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, karaoke_rotation: ROTATION });
  await expect(page.locator(SCREEN)).toBeVisible();
}

test.describe("Karaoke beta B3: the singers' screen's lyric visuals", () => {
  test("an instrumental break is a bar that fills, with the seconds to the next line", async ({ page }) => {
    await screen(page);
    const bar = page.locator(`${SCREEN} [data-break]`);
    // "So long my friend" is sung for eight seconds at most: 21 → 30 is a break.
    await at(page, 25);
    await expect(bar).toBeVisible();
    await expect(bar.locator("[data-break-left]")).toHaveText("5s");
    await expect(bar).toHaveAttribute("aria-valuenow", "44");
    const fill = (await bar.locator(".break-fill").boundingBox())!;
    const whole = (await bar.locator(".break-track").boundingBox())!;
    expect(fill.width / whole.width).toBeGreaterThan(0.4);
    expect(fill.width / whole.width).toBeLessThan(0.5);
    // And never over the line it follows.
    const line = (await page.locator(`${SCREEN} [data-line="now"]`).boundingBox())!;
    const all = (await bar.boundingBox())!;
    expect(all.y + all.height <= line.y || all.y >= line.y + line.height, "over the words").toBe(true);

    // A breath between lines is not a break.
    await at(page, 11);
    await expect(bar).toHaveCount(0);
    // The intro is one.
    await at(page, 5);
    await expect(bar.locator("[data-break-left]")).toHaveText("5s");
    expect(errorsThrown(page)).toEqual([]);
  });

  test("the word being sung glows, and only that one", async ({ page }) => {
    await screen(page);
    await at(page, 14.2);
    const singing = page.locator(`${SCREEN} .word.singing`);
    await expect(singing).toHaveCount(1);
    await expect(singing).toHaveText("my");
    await expect(singing).toHaveCSS("filter", /drop-shadow/);
    await expect(page.locator(`${SCREEN} .word`, { hasText: "long" })).toHaveCSS("filter", "none");
    await at(page, 13.2);
    await expect(singing).toHaveText("So");
  });

  test("a duet wears each singer's microphone colour, their name lit while heard", async ({ page }) => {
    await screen(page);
    await onMics(page, ["Ana", "Ben"]);
    await at(page, 31, [0.5, 0]);
    const names = page.locator(`${SCREEN} [data-duet]`);
    await expect(names).toBeVisible({ timeout: 5_000 });
    await expect(names.locator("[data-mic]")).toHaveText(["Ana", "Ben"]);
    await expect(names.locator('[data-mic="1"]')).toHaveClass(/hot/);
    await expect(names.locator('[data-mic="2"]')).not.toHaveClass(/hot/);
    const colour = (mic: number) =>
      names.locator(`[data-mic="${mic}"]`).evaluate((el) => getComputedStyle(el).color);
    // Apart in hue, not merely different: the default theme's two accents are
    // both green.
    const hue = (rgb: string) => {
      const [r, g, b] = (rgb.match(/[\d.]+/g) ?? []).slice(0, 3).map(Number);
      return (Math.atan2(Math.sqrt(3) * (g - b), 2 * r - g - b) * 180) / Math.PI;
    };
    const gap = Math.abs(hue(await colour(1)) - hue(await colour(2))) % 360;
    expect(Math.min(gap, 360 - gap), `${await colour(1)} against ${await colour(2)}`).toBeGreaterThan(60);

    // Ana's line, without her name, in her colour.
    const now = page.locator(`${SCREEN} [data-line="now"]`);
    await expect(now).toHaveAttribute("data-part", "1");
    await expect(now).toHaveText("after a break");
    // The line after it is Ben's, read ahead without his name.
    const next = page.locator(`${SCREEN} [data-line="next"]`);
    await expect(next).toHaveAttribute("data-part", "2");
    await expect(next).toHaveText("and you");
    // Ben's timed line: his name is not a word to sing.
    await at(page, 33.4, [0, 0.6]);
    await expect(now).toHaveAttribute("data-part", "2");
    await expect(now).toHaveText(/^\s*and you\s*$/);
    await expect(names.locator('[data-mic="2"]')).toHaveClass(/hot/);
    const wipe = await now.locator(".word").first().evaluate((el) => getComputedStyle(el).backgroundImage);
    expect(wipe, "Ben's line in his microphone's colour").toContain(await colour(2));
    expect(errorsThrown(page)).toEqual([]);
  });

  test("one singer on a microphone is not a duet", async ({ page }) => {
    await screen(page);
    await onMics(page, ["Ana", null]);
    await at(page, 31, [0.5, 0]);
    await page.waitForTimeout(3_500);
    await expect(page.locator(`${SCREEN} [data-duet]`)).toHaveCount(0);
    await expect(page.locator(`${SCREEN} [data-line="now"]`)).toHaveText("Ana: after a break");
  });
});
