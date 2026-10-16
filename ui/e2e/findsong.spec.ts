/**
 * §128: a song the singer cannot name, found from what they remember
 * (`FindSong.svelte`, in the Library's *From memory*).
 *
 * The narrowing and the sentence are `dj_app::find_song`'s and tested
 * there. This holds the questions: each reply sent as Rust takes it and
 * taken back when pressed again; what Rust found drawn with its reasons;
 * the answers no tag can check said to go to the assistant; and the
 * assistant asked with Rust's own sentence through the description guess
 * the panel already makes.
 *
 * Rust is answered by a small model installed in the page by this test,
 * which also keeps what was sent (docs/karaoke_beta_release.md §4: the
 * sessions leave `shell.ts` alone).
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

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
      __asked: { cmd: string; args: Record<string, unknown> }[];
    };
    win.__asked = [];
    const stub = win.__TAURI_INTERNALS__.invoke;
    win.__TAURI_INTERNALS__.invoke = (cmd, args = {}) => {
      if (cmd === "find_song") {
        const r = (args.remembered ?? {}) as Record<string, unknown>;
        win.__asked.push({ cmd, args: structuredClone(r) });
        const checked = Boolean(r.decade || r.pace || r.genre || r.like);
        const parts = [r.decade ? `from the ${r.decade}s` : null, r.voice ? `a ${r.voice} singing` : null].filter(Boolean);
        return Promise.resolve({
          checked,
          near: checked && r.decade === 1980 ? [{ track: hit, because: ["from the 1980s (1984)", "slow at 82 BPM"] }] : [],
          description: parts.length ? `A song a singer wants to sing but cannot name: ${parts.join(", ")}.` : "",
        });
      }
      if (cmd === "guess_from_description") {
        win.__asked.push({ cmd, args: structuredClone(args) });
        return Promise.resolve([]);
      }
      return stub(cmd, args);
    };
  }, HIT);
}

const asked = (page: Page, cmd: string) =>
  page.evaluate(
    (cmd) =>
      (window as unknown as { __asked: { cmd: string; args: Record<string, unknown> }[] }).__asked
        .filter((call) => call.cmd === cmd)
        .map((call) => call.args),
    cmd,
  );

test.describe("§128: finding the song a singer cannot name", () => {
  test("each answer narrows, a reply pressed again is taken back, and the assistant gets them all", async ({ page }) => {
    await openShell(page, "/");
    await rust(page);
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await page.getByRole("button", { name: "From memory" }).click();
    const panel = page.getByRole("region", { name: "What do they remember?" });
    await expect(panel).toBeVisible();

    // The 80s: sent, and what Rust found drawn with its reasons.
    await panel.getByRole("group", { name: "When is it from?" }).getByRole("button", { name: "80s" }).click();
    await expect.poll(async () => (await asked(page, "find_song")).at(-1)?.decade).toBe(1980);
    const hits = panel.getByRole("list", { name: "In your collection, fitting what they remember" });
    await expect(hits.getByRole("listitem")).toHaveCount(1);
    await expect(hits).toContainText("Juan Gabriel — Hasta que te conocí");
    await expect(hits).toContainText("from the 1980s (1984) · slow at 82 BPM");

    // A woman singing: the assistant's to use, said so.
    await panel.getByRole("group", { name: "Who sings it?" }).getByRole("button", { name: "A woman" }).click();
    await expect(panel.locator("[data-assistant-only]")).toBeVisible();
    await expect.poll(async () => (await asked(page, "find_song")).at(-1)?.voice).toBe("woman");

    // The 80s again: taken back; nothing checkable is left, so nothing is listed.
    await panel.getByRole("button", { name: "80s" }).click();
    await expect.poll(async () => (await asked(page, "find_song")).at(-1)?.decade ?? null).toBeNull();
    await expect(hits).toHaveCount(0);
    await expect(panel).toContainText("narrow your collection");

    // The assistant, with Rust's sentence — through the description box.
    await panel.getByRole("button", { name: "Ask the assistant with all of this" }).click();
    const sentence = "A song a singer wants to sing but cannot name: a woman singing.";
    await expect.poll(async () => (await asked(page, "guess_from_description")).at(-1)?.description).toBe(sentence);
    await expect(page.getByRole("textbox", { name: "A description of the record" })).toHaveValue(sentence);
    expect(errorsThrown(page)).toEqual([]);
  });
});
