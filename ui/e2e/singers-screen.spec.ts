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
   * **K4: under the words, who is next and with what, the two after them by
   * name only, and the host's line.** Ana is singing (deck 2, her vocal
   * out), so Ben is next; Cleo and Dan follow, their songs not shown.
   */
  test("shows who is next, the two after by name, and the host's line", async ({ page }) => {
    const four = {
      singers: [
        ...ROTATION.singers,
        { name: "Cleo", songs: [{ title: "Secret Song", track: null, path: null, key: 0 }], turns: 0 },
        { name: "Dan", songs: [{ title: "Another Secret", track: null, path: null, key: 0 }], turns: 0 },
        { name: "Eve", songs: [{ title: "Third", track: null, path: null, key: 0 }], turns: 0 },
      ],
      up_next: "Ana",
      lately: [],
      line: "Last orders at one",
    };
    await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, karaoke_rotation: four });
    await expect(page.locator(SCREEN)).toBeVisible();
    await singing(page, 14.5);
    await expect(page.locator(`${SCREEN} [data-up-next]`)).toHaveText("Next: Ben — Hey Jude");
    await expect(page.locator(`${SCREEN} [data-then]`)).toHaveText("then Cleo · Dan");
    await expect(page.locator(SCREEN)).not.toContainText("Secret");
    await expect(page.locator(SCREEN)).not.toContainText("Eve");
    await expect(page.locator(`${SCREEN} [data-host-line]`)).toHaveText("Last orders at one");
    // No logo set, none drawn.
    await expect(page.locator(`${SCREEN} [data-logo]`)).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **K6: the scores while a song is sung, and tonight's best between
   * songs** — under the words, from Rust's rotation; a singer not yet scored
   * (no sung part passed) is not shown with a number.
   */
  test("shows the scores while singing and tonight's best between songs", async ({ page }) => {
    const scored = {
      ...ROTATION,
      scores: [
        { strip: 0, singer: "Ana", percent: 82.4 },
        { strip: 1, singer: "Ben", percent: null },
      ],
      best: [
        { singer: "Cleo", title: "Valerie", track: null, key: 0, score: 91.2 },
        { singer: "Dan", title: "Dancing Queen", track: null, key: -2, score: 77 },
      ],
    };
    await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, karaoke_rotation: scored });
    await expect(page.locator(SCREEN)).toBeVisible();
    // Between songs: tonight's best, highest first.
    const best = page.locator(`${SCREEN} [data-best]`);
    await expect(best).toBeVisible();
    await expect(best.locator("li")).toHaveText(["91 Cleo — Valerie", "77 Dan — Dancing Queen"]);
    await expect(page.locator(`${SCREEN} [data-scores]`)).toHaveCount(0);
    // A song sung: the scores instead.
    await singing(page, 14.5);
    const scores = page.locator(`${SCREEN} [data-scores]`);
    await expect(scores).toHaveText("Ana 82");
    await expect(best).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** **K6: a versus round's two bars**, each team's mean as Rust stands it. */
  test("shows a versus round as two bars", async ({ page }) => {
    const round = {
      ...ROTATION,
      versus: [
        { name: "Reds", singers: ["Ana"], mean: 72.6, songs: 2 },
        { name: "Blues", singers: ["Ben"], mean: null, songs: 0 },
      ],
    };
    await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, karaoke_rotation: round });
    const versus = page.locator(`${SCREEN} [data-versus]`);
    await expect(versus.locator(".team")).toHaveText([/Reds\s*73/, /Blues\s*—/]);
    await expect(versus.locator(".fill").first()).toHaveAttribute("style", /width: 72.6%/);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **K4: the singer's approved photo, while they sing** — and not before:
   * the screen asks for the singer at the top of the rotation only while
   * their deck plays with its vocal down.
   */
  test("shows the singer's approved photo while they sing", async ({ page }) => {
    const url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGM4EaABAAMkAUEdGZ/EAAAAAElFTkSuQmCC";
    await openShell(page, "/?panel=singers", {}, {
      singer_lyrics: WORDS,
      karaoke_rotation: ROTATION,
      singer_photo: url,
    });
    await expect(page.locator(SCREEN)).toBeVisible();
    await expect(page.locator(`${SCREEN} [data-singer-photo]`)).toHaveCount(0);
    await singing(page, 14.5);
    await expect(page.locator(`${SCREEN} [data-singer-photo]`)).toHaveAttribute("src", url);
    expect(errorsThrown(page)).toEqual([]);
  });

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
