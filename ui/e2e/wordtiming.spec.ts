/**
 * §122: the words of a record placed in time by WhisperX.
 *
 * > I prefer x-whisper, since it also tells the timestamps for the
 * > words/lyrics ... that's necessary for karaoke ... if x-whipser works on
 * > all platforms somehow, do use it.
 *
 * The run itself is `dj_app::wordtimes`' and its Rust tests hold the protocol
 * with WhisperX stood in for. This holds the surface: WhisperX installed only
 * when the DJ says so, its steps shown while it installs, a record timed from
 * the deck it is on, and every run measured against the owner's fifteen
 * seconds -- said plainly when it is over.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";
import wordTiming from "./word-timing.json" with { type: "json" };

const section = (page: Page) => page.getByRole("region", { name: "Words in time" });

const asked = (page: Page) =>
  page.evaluate(() => (window as unknown as { __timing?: { cmd: string; deck?: number; language?: string | null }[] }).__timing ?? []);

async function singers(page: Page, answers: Record<string, unknown> = {}) {
  await openShell(page, "/", {}, answers);
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.locator("body").click({ position: { x: 700, y: 120 } });
  await page.keyboard.press("Space");
  await page.keyboard.press("O");
  await page.keyboard.press("i");
  await expect(section(page)).toBeVisible();
}

test.describe("§122: words in time, by WhisperX", () => {
  /**
   * **The load-bearing one.** Nothing is installed until the DJ asks; the
   * install shows its step; then the record on a deck is timed, with the
   * language given, and the report says it met the budget.
   */
  test("WhisperX is installed on request, and a record is timed against the budget", async ({ page }) => {
    await singers(page);
    const here = section(page);
    await expect(here.getByRole("button", { name: /^Time the words/ })).toHaveCount(0);
    expect((await asked(page)).filter((c) => c.cmd === "word_timing_install")).toHaveLength(0);

    await here.getByRole("button", { name: "Install WhisperX" }).click();
    await expect(here.getByRole("status")).toContainText("Installing WhisperX and PyTorch");
    const deck1 = here.getByRole("button", { name: "Time the words on deck 1" });
    await expect(deck1).toBeVisible({ timeout: 8000 });

    await here.getByRole("textbox", { name: "Language" }).fill("es");
    await deck1.click();
    const report = here.getByRole("status");
    await expect(report).toContainText("212 words placed (es)");
    await expect(report).toContainText("within the 15-second budget");
    await expect(report).toHaveAttribute("data-within", "true");
    await expect(report).toContainText("Heard from the separated vocals.");
    await expect(here.getByRole("list", { name: "How long each stage took" }).getByRole("listitem")).toHaveCount(
      wordTiming.within.stages.length,
    );
    expect((await asked(page)).find((c) => c.cmd === "word_timing_run")).toMatchObject({ deck: 1, language: "es" });
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Over the budget is said as over, and with no language WhisperX listens for it. */
  test("a run over the budget is said to be over it", async ({ page }) => {
    await singers(page, {
      word_timing: { ...wordTiming.status, installed: true },
      word_timing_report: wordTiming.over,
    });
    const here = section(page);
    await here.getByRole("button", { name: "Time the words on deck 1" }).click();
    await expect(here.getByRole("status")).toContainText("over the 15-second budget");
    await expect(here.getByRole("status")).toHaveAttribute("data-within", "false");
    await expect(here.getByRole("status")).toContainText("Heard from the whole mix");
    expect((await asked(page)).find((c) => c.cmd === "word_timing_run")).toMatchObject({ deck: 1, language: null });
    expect(errorsThrown(page)).toEqual([]);
  });
});
