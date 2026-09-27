/**
 * §122: the words of a record placed in time, by whisper.cpp inside djmanzo.
 *
 * > replace it with the model download for the whisper.cpp version. let it
 * > be a dropdown button for different useful models ... and hint at
 * > download size/time as of network, also at the usefulness as of
 * > recognition/reliability and analysis time per song approx as per the
 * > current machine
 *
 * The listening itself is `dj_app::whispercpp`'s and its Rust tests hold it.
 * This holds the surface: nothing downloaded until the DJ picks a model from
 * a list that says what each costs and how well it hears; the download shown
 * as it goes; then a record timed from the deck it is on, measured against
 * the owner's minute and a half and said plainly when it is over.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";
import wordTiming from "./word-timing.json" with { type: "json" };

const section = (page: Page) => page.getByRole("region", { name: "Words in time" });

const asked = (page: Page) =>
  page.evaluate(
    () =>
      (window as unknown as { __timing?: { cmd: string; deck?: number; language?: string | null; model?: string }[] })
        .__timing ?? [],
  );

async function singers(page: Page, answers: Record<string, unknown> = {}) {
  await openShell(page, "/", {}, answers);
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.locator("body").click({ position: { x: 700, y: 120 } });
  await page.keyboard.press("Space");
  await page.keyboard.press("O");
  await page.keyboard.press("i");
  await expect(section(page)).toBeVisible();
}

test.describe("§122: words in time, by whisper.cpp", () => {
  /**
   * **The load-bearing one.** Nothing is downloaded until the DJ asks. The
   * list says, for every model, its size, how long it would take to fetch
   * here once the connection is measured, how well it hears, and how long a
   * song would take on this machine; the recommended one is marked. The
   * chosen model downloads with its bytes shown, is then in use, and the
   * record on a deck is timed with the language given.
   */
  test("a model is picked from the list, downloaded, and a record timed against the limit", async ({ page }) => {
    await singers(page);
    const here = section(page);
    await expect(here.getByRole("button", { name: /^Time the words/ })).toHaveCount(0);
    expect((await asked(page)).filter((c) => c.cmd === "word_timing_download")).toHaveLength(0);
    await expect(here.getByRole("button", { name: /^Install WhisperX/ })).toHaveCount(0);

    await here.getByText("Choose a model to download").click();
    const list = here.getByRole("list", { name: "Models" });
    await expect(list.getByRole("listitem")).toHaveCount(wordTiming.status.models.length);
    const small = list.locator('[data-model="small"]');
    await expect(small).toContainText("recommended");
    await expect(small).toContainText("190 MB");
    await expect(small).toContainText("most words right");
    // Estimated for this machine, and within the limit.
    await expect(small.locator(".analysis")).toContainText("for a 4-minute song (estimated)");
    await expect(small.locator(".analysis")).toHaveAttribute("data-over", "false");
    // The large ones say they would take longer than the limit.
    await expect(list.locator('[data-model="large-v3-turbo"] .analysis')).toHaveAttribute("data-over", "true");
    // Opening the list measured the connection, so each says how long it would take to fetch.
    await expect(small).toContainText("about 30 s to download here");
    expect((await asked(page)).filter((c) => c.cmd === "word_timing_speed")).toHaveLength(1);

    await small.getByRole("button", { name: "Download Small" }).click();
    await expect(here.getByRole("status")).toContainText("Downloading Small — 63 MB of 190 MB");
    expect((await asked(page)).find((c) => c.cmd === "word_timing_download")).toMatchObject({ model: "small" });
    await expect(small).toContainText("In use", { timeout: 8000 });
    await expect(here.getByText("Model: Small")).toBeVisible();

    const deck1 = here.getByRole("button", { name: "Time the words on deck 1" });
    await expect(deck1).toBeVisible();
    await here.getByRole("textbox", { name: "Language" }).fill("es");
    await deck1.click();
    const report = here.getByRole("status");
    await expect(report).toContainText("212 words placed (es) by Small");
    await expect(report).toContainText("within the 90-second limit");
    await expect(report).toHaveAttribute("data-within", "true");
    await expect(report).toContainText("Heard from the separated vocals.");
    await expect(here.getByRole("list", { name: "How long each stage took" }).getByRole("listitem")).toHaveCount(
      wordTiming.within.stages.length,
    );
    expect((await asked(page)).find((c) => c.cmd === "word_timing_run")).toMatchObject({ deck: 1, language: "es" });
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * Over the limit is said as over; a time measured here says so; another
   * downloaded model can be put in use and one removed; and the WhisperX an
   * earlier djmanzo left behind can be deleted.
   */
  test("a run over the limit is said to be over it, and the list manages what is kept", async ({ page }) => {
    await singers(page, {
      word_timing: wordTiming.ready,
      word_timing_report: wordTiming.over,
    });
    const here = section(page);
    await expect(here.getByRole("note")).toContainText("installed WhisperX here (2.6 GB)");
    await here.getByRole("button", { name: "Delete it" }).click();
    await expect(here.getByRole("note")).toHaveCount(0);
    expect((await asked(page)).filter((c) => c.cmd === "word_timing_forget_whisperx")).toHaveLength(1);

    await here.getByRole("button", { name: "Time the words on deck 1" }).click();
    await expect(here.getByRole("status")).toContainText("over the 90-second limit");
    await expect(here.getByRole("status")).toHaveAttribute("data-within", "false");
    await expect(here.getByRole("status")).toContainText("Heard from the whole mix");
    expect((await asked(page)).find((c) => c.cmd === "word_timing_run")).toMatchObject({ deck: 1, language: null });

    await here.getByText("Model: Small").click();
    const list = here.getByRole("list", { name: "Models" });
    await expect(list.locator('[data-model="small"] .analysis')).toContainText("(measured here)");
    // The connection was measured already: not asked again.
    expect((await asked(page)).filter((c) => c.cmd === "word_timing_speed")).toHaveLength(0);
    await list.getByRole("button", { name: "Use Base" }).click();
    await expect(list.locator('[data-model="base"]')).toContainText("In use");
    await expect(here.getByText("Model: Base")).toBeVisible();
    await list.locator('[data-model="small"]').getByRole("button", { name: "Remove" }).click();
    await expect(list.getByRole("button", { name: "Download Small" })).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });
});
