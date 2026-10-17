/**
 * §128 from the Singers panel: *Can't name it?* beside the request form.
 *
 * The questions are `FindSong.svelte`'s and held by `findsong.spec.ts`;
 * the narrowing is `dj_app::find_song`'s. This holds the way in from where
 * a host takes a request: the questions open under the form, a record they
 * find goes to the song box by name, and an assistant's guess goes there
 * as the record when the collection has it — so the request carries the
 * record — and as words when it does not.
 *
 * Rust's `find_song` and `guess_from_description` are answered by a small
 * model installed in the page by this test (the shell is left alone).
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const SINGERS = '.surface[data-surface="karaoke"]';

const HIT = {
  id: "t1",
  path: "/music/hit.mp3",
  title: "Hasta que te conocí",
  artist: "Juan Gabriel",
  duration_seconds: 300,
  bpm: 82,
  key: "4A",
};

async function rust(page: Page) {
  await page.evaluate((hit) => {
    const win = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
      __guessed: string[];
    };
    win.__guessed = [];
    const stub = win.__TAURI_INTERNALS__.invoke;
    win.__TAURI_INTERNALS__.invoke = (cmd, args = {}) => {
      if (cmd === "find_song") {
        const r = (args.remembered ?? {}) as Record<string, unknown>;
        const checked = Boolean(r.decade);
        return Promise.resolve({
          checked,
          near: r.decade === 1980 ? [{ track: hit, because: ["from the 1980s (1984)"] }] : [],
          description: checked ? "A song a singer wants to sing but cannot name: from the 1980s." : "",
        });
      }
      if (cmd === "words_search") {
        win.__guessed.push(`line ${String(args.phrase)}`);
        return Promise.resolve([{ track: hit, line: "no puedo dormir sin ti", line_number: 3 }]);
      }
      if (cmd === "search_sources") {
        win.__guessed.push(`online ${String(args.text)}`);
        const track = {
          provider: "jamendo",
          id: "x1",
          title: "Vivir así",
          artist: "Camilo Sesto",
          album: null,
          duration_seconds: 240,
          bpm: null,
          key: null,
          genre: null,
          artwork_url: null,
          web_url: null,
          playable: true,
        };
        return Promise.resolve([{ provider: "jamendo", label: "Jamendo", tracks: [track], error: null, matched_locally: 0 }]);
      }
      if (cmd === "similar_to") {
        win.__guessed.push(`like ${String(args.track)}`);
        return Promise.resolve([{ track: hit, score: 3, reasons: ["same key (4A)", "slow too"], summary: "" }]);
      }
      if (cmd === "guess_from_description") {
        win.__guessed.push(String(args.description));
        return Promise.resolve([
          { artist: "Camilo Sesto", title: "Vivir así es morir de amor", why: "a 1980s ballad", owned: null },
          { artist: hit.artist, title: hit.title, why: null, owned: hit },
        ]);
      }
      return stub(cmd, args);
    };
  }, HIT);
}

async function openSingers(page: Page) {
  await openShell(page, "/");
  await rust(page);
  await page.getByRole("button", { name: "Activities", exact: true }).click();
  await page.keyboard.press("8");
  const singers = page.locator(SINGERS);
  await expect(singers).toBeVisible();
  return singers;
}

const karaokeAsks = (page: Page) =>
  page.evaluate(() =>
    ((window as unknown as { __karaoke?: Record<string, unknown>[] }).__karaoke ?? []).filter(
      (call) => call.cmd === "karaoke_ask",
    ),
  );

test.describe("§128: a song a singer cannot name, from the Singers panel", () => {
  test("the questions open under the form, and what they find comes back to the song box", async ({ page }) => {
    const singers = await openSingers(page);
    const song = singers.getByRole("textbox", { name: "Song" });
    const open = singers.getByRole("button", { name: "Can't name it?" });
    const find = page.getByRole("region", { name: "Find a song they cannot name" });

    await expect(find).toHaveCount(0);
    await expect(open).toHaveAttribute("aria-expanded", "false");
    await open.click();
    await expect(open).toHaveAttribute("aria-expanded", "true");
    await expect(find.getByRole("region", { name: "What do they remember?" })).toBeVisible();

    // A record of the collection, found by the questions, to the box by name.
    await find.getByRole("button", { name: "80s" }).click();
    await find.getByRole("button", { name: "Find it" }).click();
    await expect(song).toHaveValue("Juan Gabriel – Hasta que te conocí");
    await expect(find).toHaveCount(0);

    // The assistant's guesses: one nobody owns goes in as words …
    await open.click();
    await find.getByRole("button", { name: "80s" }).click();
    await find.getByRole("button", { name: "Ask the assistant with all of this" }).click();
    const guesses = find.getByRole("list", { name: "The assistant's guesses" });
    await expect(guesses.getByRole("listitem")).toHaveCount(2);
    expect(await page.evaluate(() => (window as unknown as { __guessed: string[] }).__guessed)).toEqual([
      "A song a singer wants to sing but cannot name: from the 1980s.",
    ]);
    await expect(guesses.getByRole("listitem").nth(0)).toContainText("a 1980s ballad");
    await guesses.getByRole("button", { name: /Camilo Sesto/ }).click();
    await expect(song).toHaveValue("Camilo Sesto – Vivir así es morir de amor");

    // … and one the collection has goes in as the record, so the request
    // carries it.
    await open.click();
    await find.getByRole("button", { name: "80s" }).click();
    await find.getByRole("button", { name: "Ask the assistant with all of this" }).click();
    await find.getByRole("button", { name: /Hasta que te conocí.*in your collection/ }).click();
    await expect(song).toHaveValue("Juan Gabriel – Hasta que te conocí");
    await singers.getByRole("textbox", { name: "Singer's name" }).fill("Ana");
    await singers.getByRole("button", { name: "Add", exact: true }).click();
    await expect.poll(async () => (await karaokeAsks(page)).at(-1)?.track).toBe("t1");
    expect((await karaokeAsks(page)).at(-1)?.path).toBe("/music/hit.mp3");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Like this one**: for a song picked from the collection, records like
   * it (Rust's `similar_to`, the Library's), with why; one taken goes into
   * the request as the record.
   */
  test("a song picked from the collection offers songs like it", async ({ page }) => {
    const singers = await openSingers(page);
    const song = singers.getByRole("textbox", { name: "Song" });
    const like = singers.getByRole("button", { name: "Like this one" });
    await expect(like).toHaveCount(0);
    await song.fill("Bach");
    await singers.getByRole("button", { name: "Juan Luis Guerra – Bachata Rosa" }).click();
    await like.click();
    const found = page.getByRole("region", { name: "Songs like Juan Luis Guerra – Bachata Rosa" });
    await expect(found.getByRole("listitem")).toHaveCount(1);
    await expect(found).toContainText("same key (4A) · slow too");
    expect(await page.evaluate(() => (window as unknown as { __guessed: string[] }).__guessed)).toEqual([
      `like ${"a".repeat(64)}`,
    ]);
    await found.getByRole("button", { name: "Juan Gabriel – Hasta que te conocí" }).click();
    await expect(song).toHaveValue("Juan Gabriel – Hasta que te conocí");
    await expect(found).toHaveCount(0);
    await singers.getByRole("textbox", { name: "Singer's name" }).fill("Ana");
    await singers.getByRole("button", { name: "Add", exact: true }).click();
    await expect.poll(async () => (await karaokeAsks(page)).at(-1)?.track).toBe("t1");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Fast search modes** (§128): the song box searches the collection by
   * name, the words of its records by a line, or every connected source
   * online — a record found by its words goes in as the record, a song
   * found online as words, and is not searched for again.
   */
  test("the song box searches by name, by a line, or online", async ({ page }) => {
    const singers = await openSingers(page);
    const song = singers.getByRole("textbox", { name: "Song" });
    const modes = singers.getByRole("group", { name: "How to find the song" });
    await expect(modes.getByRole("button", { name: "By name" })).toHaveAttribute("aria-pressed", "true");
    const asked = () => page.evaluate(() => (window as unknown as { __guessed: string[] }).__guessed);

    await modes.getByRole("button", { name: "By a line" }).click();
    await expect(song).toHaveAttribute("placeholder", /A line they remember/);
    await song.fill("no puedo");
    const byLine = singers.getByRole("list", { name: "Songs with that line" });
    await expect(byLine).toContainText("“no puedo dormir sin ti”");
    expect(await asked()).toEqual(["line no puedo"]);
    await byLine.getByRole("button").first().click();
    await expect(song).toHaveValue("Juan Gabriel – Hasta que te conocí");
    await singers.getByRole("textbox", { name: "Singer's name" }).fill("Ana");
    await singers.getByRole("button", { name: "Add", exact: true }).click();
    await expect.poll(async () => (await karaokeAsks(page)).at(-1)?.track).toBe("t1");

    await modes.getByRole("button", { name: "Online" }).click();
    await song.fill("vivir");
    const online = singers.getByRole("list", { name: "Songs found online" });
    await expect(online).toContainText("Jamendo");
    await online.getByRole("button", { name: /Camilo Sesto – Vivir así/ }).click();
    await expect(song).toHaveValue("Camilo Sesto – Vivir así");
    await expect(online).toHaveCount(0);
    await page.waitForTimeout(800);
    expect((await asked()).filter((call) => call.startsWith("online"))).toEqual(["online vivir"]);
    await expect(online).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});
