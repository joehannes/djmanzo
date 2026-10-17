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
});
