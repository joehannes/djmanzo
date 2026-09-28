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
   * **K1: never a blank screen.** Behind the words, the record's own colours
   * — strongest first, each glowing at its weight — and over them its cover
   * from `art://`. A cover that will not load leaves the colours; a record
   * with none has the colours alone; the words are there throughout.
   */
  test("draws the record's colours behind the words, and its cover over them", async ({ page }) => {
    const shades = [
      { colour: "rgb(255 60 0)", weight: 1 },
      { colour: "rgb(255 170 0)", weight: 0.8 },
      { colour: "rgb(90 200 255)", weight: 0.4 },
    ];
    await openShell(
      page,
      "/?panel=singers",
      {},
      {
        singer_lyrics: WORDS,
        karaoke_rotation: ROTATION,
        singer_backdrop: { source: "archive", track: "ab".repeat(32), shades, pending: false },
      },
    );
    await singing(page, 14.5);
    const backdrop = page.locator(`${SCREEN} [data-backdrop]`);
    await expect(backdrop).toHaveAttribute("data-backdrop", "archive");
    const glows = backdrop.locator(".glow");
    await expect(glows).toHaveCount(3);
    await expect(glows.first()).toHaveAttribute("data-shade", "rgb(255 60 0)");
    const opacity = await glows.evaluateAll((all) => all.map((g) => Number((g as HTMLElement).style.opacity)));
    expect(opacity[0]).toBeGreaterThan(opacity[2]);
    // The browser here has no `art://`: the cover fails, the colours stay.
    await expect(backdrop.locator("img.cover")).toHaveCount(0);
    await expect(glows).toHaveCount(3);
    await expect(page.locator(`${SCREEN} [data-line="now"]`)).toHaveText(/So\s+long\s+my\s+friend/);
    // The words sit above the background.
    expect(await page.locator(`${SCREEN} .words`).evaluate((el) => getComputedStyle(el).zIndex)).toBe("1");
    expect(await backdrop.evaluate((el) => getComputedStyle(el).zIndex)).toBe("0");
    expect(errorsThrown(page)).toEqual([]);
  });

  test("a record with no cover has its colours alone", async ({ page }) => {
    await openShell(
      page,
      "/?panel=singers",
      {},
      {
        singer_lyrics: WORDS,
        karaoke_rotation: ROTATION,
        singer_backdrop: {
          source: "sound",
          track: "cd".repeat(32),
          shades: [{ colour: "rgb(160 0 255)", weight: 1 }],
          pending: false,
        },
      },
    );
    await singing(page, 14.5);
    const backdrop = page.locator(`${SCREEN} [data-backdrop]`);
    await expect(backdrop).toHaveAttribute("data-backdrop", "sound");
    await expect(backdrop.locator(".glow")).toHaveCount(1);
    await expect(backdrop.locator("img")).toHaveCount(0);
  });

  /**
   * **Colours that were not measured yet are asked for again.** A record's
   * spectrum lands seconds after the record does, so a screen opened with it
   * is told the colours are pending, asks again, and draws them when they
   * arrive — and stops asking once they have.
   */
  test("colours still being measured are asked for again until they land", async ({ page }) => {
    await openShell(
      page,
      "/?panel=singers",
      {},
      {
        singer_lyrics: WORDS,
        karaoke_rotation: ROTATION,
        singer_backdrop: { source: "sound", track: "ef".repeat(32), shades: [], pending: true },
        singer_backdrop_then: {
          source: "sound",
          track: "ef".repeat(32),
          shades: [{ colour: "rgb(0 120 255)", weight: 1 }],
          pending: false,
        },
      },
    );
    await singing(page, 14.5);
    const asks = () =>
      page.evaluate(
        () => (window as unknown as { __backdropAsks?: Record<string, number> }).__backdropAsks?.["2"] ?? 0,
      );
    const backdrop = page.locator(`${SCREEN} [data-backdrop]`);
    await expect.poll(asks).toBe(1);
    await expect(backdrop.locator(".glow")).toHaveCount(0);
    await expect(backdrop.locator(".glow")).toHaveCount(1, { timeout: 8000 });
    await expect(backdrop.locator(".glow")).toHaveAttribute("data-shade", "rgb(0 120 255)");
    expect(await asks()).toBe(2);
    // Landed: not asked a third time.
    await page.waitForTimeout(3_500);
    expect(await asks()).toBe(2);
    expect(errorsThrown(page)).toEqual([]);
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
