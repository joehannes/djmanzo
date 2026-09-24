/**
 * §107: the singers' screen — the words, on a display facing the microphone.
 *
 * Parsing the lyric is `dj_library::lrc`'s and tested there; where in it a
 * moment falls is `./src/lyricsAt.ts`'s and tested beside it. What a browser
 * holds is what a singer sees: the line being sung with the sung part
 * marked, the next line under it, a count-in before the first line, the
 * next singer's name — and that the screen follows the deck the singer is on.
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

async function open(page: Page) {
  await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, karaoke_rotation: ROTATION });
  await expect(page.locator(SCREEN)).toBeVisible();
}

test.describe("§107: the singers' screen", () => {
  /**
   * **The load-bearing one: the line being sung, wiped as it is sung, the
   * next under it, from the deck the singer is on.** Deck 1 is playing too,
   * with its vocal in: the screen must follow deck 2, where the vocal is out.
   */
  test("shows the line being sung, marked as it goes, and the line after", async ({ page }) => {
    await open(page);
    await singing(page, 14.5);
    const now = page.locator(`${SCREEN} [data-line="now"]`);
    await expect(now).toHaveText(/So\s+long\s+my\s+friend/);
    await expect(page.locator(`${SCREEN} [data-line="next"]`)).toHaveText("After a break");
    // Two words sung, the third half way.
    const fills = await now.locator(".word").evaluateAll((words) =>
      words.map((word) => (word as HTMLElement).style.getPropertyValue("--fill")),
    );
    expect(fills).toEqual(["100%", "100%", "50%", "0%"]);
    // The singer's deck, not the one merely playing: its record's title.
    await expect(page.locator(`${SCREEN} .song`)).toHaveText("Bachata Rosa");
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A line with no word times is wiped evenly to where the next line starts. */
  test("wipes an untimed line evenly", async ({ page }) => {
    await open(page);
    await singing(page, 11.5);
    const wipe = page.locator(`${SCREEN} [data-line="now"] .wipe`);
    await expect(wipe).toHaveText("First line");
    expect(await wipe.evaluate((el) => (el as HTMLElement).style.getPropertyValue("--fill"))).toBe("50%");
  });

  /** A bar before the first line, the beats count down. */
  test("counts the singer in", async ({ page }) => {
    await open(page);
    await singing(page, 8.6); // 1.4 s out at half a second a beat: three beats.
    await expect(page.locator(`${SCREEN} [data-count-in]`)).toHaveAttribute("data-count-in", "3");
    await expect(page.locator(`${SCREEN} .dot.lit`)).toHaveCount(3);
    await expect(page.locator(`${SCREEN} [data-line="next"]`)).toHaveText("First line");
  });

  /**
   * **Who is next, while a song is being sung**: the singer on stage is still
   * at the top of the rotation until the host marks the song sung, so the
   * screen names the one after.
   */
  test("names the next singer", async ({ page }) => {
    await open(page);
    await singing(page, 14.5);
    await expect(page.locator(`${SCREEN} [data-up-next]`)).toContainText("Ben");
    await expect(page.locator(`${SCREEN} [data-up-next]`)).toContainText("Hey Jude");
  });
});
