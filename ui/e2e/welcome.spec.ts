/**
 * §118b: the welcome -- djmanzo getting to know the DJ, and setting itself up.
 *
 * > a new user is welcomed by a guide that introduces to the capabilities
 * > and metaphorical usage and navigation and features and workflow of the
 * > app ... he asks him for his favorite genres/music/artists/songs/bpm/
 * > styles/combinations/tricks/transitions/techniques ... and automatically
 * > creates activities and personalised preset-packages for him ...
 *
 * What setting up will do is `welcome::plan`'s, in Rust's sentences
 * (`welcome.json`). The browser proves the welcome opens on a first run and
 * only then, that every answer is kept as it is given, that the plan shown
 * is Rust's, and that setting up leaves the DJ's own activities on the strip
 * and their name where the wordmark was.
 */
import { expect, test, type Page } from "@playwright/test";

import activities from "./activities.json" with { type: "json" };
import welcome from "./welcome.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const DIALOG = '[role="dialog"][aria-label="Welcome to djmanzo"]';

const calls = (page: Page) =>
  page.evaluate(
    () => ((window as unknown as { __welcomeCalls?: unknown[] }).__welcomeCalls ?? []) as {
      cmd: string;
      answers: Record<string, unknown>;
    }[],
  );

const FIRST_RUN = { welcome_state: { seen: false, answers: welcome.fresh } };

const next = (page: Page) => page.locator(DIALOG).getByRole("button", { name: "Next", exact: true }).click();

test.describe("§118b: the welcome", () => {
  /** A first run is welcomed; every run after it is not. */
  test("the welcome opens on a first run, and only then", async ({ page }) => {
    await openShell(page, "/", {}, FIRST_RUN);
    await expect(page.locator(DIALOG)).toBeVisible();
    await expect(page.locator(DIALOG)).toHaveAttribute("data-welcome-step", "hello");

    await openShell(page, "/");
    await page.waitForTimeout(300);
    await expect(page.locator(DIALOG)).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The load-bearing one.** Through every step: each answer is kept as it
   * is given, the plan shown is Rust's word for word, and setting up sends
   * the answers, puts the DJ's name where the wordmark was, and leaves their
   * own activities on the strip.
   */
  test("answered through, it shows Rust's plan and sets djmanzo up", async ({ page }) => {
    // Activity mode already on, so the strip is drawn throughout and shows
    // what setting up hands back, rather than what a later key press asks.
    await openShell(page, "/", {}, {
      ...FIRST_RUN,
      activities: { ...activities, on: true, current: "mix" },
    });
    const dialog = page.locator(DIALOG);
    await dialog.getByRole("textbox").first().fill("Johannes");
    await next(page);
    await expect.poll(async () => (await calls(page)).at(-1)?.answers.name).toBe("Johannes");

    // Where: weddings first, then Latin nights, numbered in that order.
    await expect(dialog).toHaveAttribute("data-welcome-step", "nights");
    await dialog.locator('[data-night="wedding"]').click();
    await dialog.locator('[data-night="latin"]').click();
    await expect(dialog.locator('[data-night="wedding"] .order')).toHaveText("1");
    await expect(dialog.locator('[data-night="latin"] .order')).toHaveText("2");
    await next(page);
    await expect.poll(async () => (await calls(page)).at(-1)?.answers.nights).toEqual(["wedding", "latin"]);

    // Music: a genre and a tempo. A tempo not yet said is an empty field,
    // not a zero that reads as one.
    await expect(dialog.getByRole("spinbutton", { name: "Slowest BPM" })).toHaveValue("");
    await expect(dialog.getByRole("spinbutton", { name: "Fastest BPM" })).toHaveValue("");
    await dialog.getByRole("button", { name: "disco", exact: true }).click();
    await dialog.getByRole("spinbutton", { name: "Slowest BPM" }).fill("100");
    await dialog.getByRole("spinbutton", { name: "Fastest BPM" }).fill("128");
    await next(page);
    const music = (await calls(page)).at(-1)!.answers;
    expect(music.genres).toEqual(["disco"]);
    expect([music.bpm_low, music.bpm_high]).toEqual([100, 128]);

    // Moves: one played, one to learn -- and a move is never both.
    await dialog.getByRole("button", { name: "I play cut" }).click();
    await dialog.getByRole("button", { name: "Learn echo out" }).click();
    await dialog.getByRole("button", { name: "Learn cut" }).click();
    await expect(dialog.getByRole("button", { name: "I play cut" })).toHaveAttribute("aria-pressed", "false");
    await dialog.getByRole("button", { name: "I play cut" }).click();
    await next(page);
    const moves = (await calls(page)).at(-1)!.answers;
    expect(moves.moves).toEqual(["cut"]);
    expect(moves.learn).toEqual(["echo out"]);

    await dialog.locator('[data-level="suggest"]').click();
    await next(page);
    await next(page); // the look, left to the nights

    // Ready: Rust's plan, word for word, with the set-up's own lines.
    await expect(dialog).toHaveAttribute("data-welcome-step", "ready");
    const says = dialog.locator("[data-plan] > li");
    await expect(says).toHaveCount(welcome.plan.says.length);
    for (const [i, line] of welcome.plan.says.entries()) {
      await expect(says.nth(i)).toContainText(line);
    }
    await expect(dialog.locator(".changes li")).toHaveCount(welcome.plan.changes.length);

    await dialog.getByRole("button", { name: "Set it up" }).click();
    await expect(dialog).toHaveCount(0);
    const applied = (await calls(page)).find((c) => c.cmd === "welcome_apply")!.answers;
    expect(applied.name).toBe("Johannes");
    expect(applied.nights).toEqual(["wedding", "latin"]);
    expect(applied.level).toBe("suggest");

    await expect(page.locator("[data-dj-name]")).toHaveText("Johannes");
    // Their own activities, on the strip beside djmanzo's, from the answer
    // setting up gave -- no key pressed to ask again.
    for (const activity of welcome.plan.activities) {
      await expect(page.locator("[data-activity-strip] [data-activity]", { hasText: activity.title })).toHaveCount(1);
    }
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Put away half way: what was said is kept, and nothing is set up. */
  test("Not now keeps what was said and sets nothing up", async ({ page }) => {
    await openShell(page, "/", {}, FIRST_RUN);
    const dialog = page.locator(DIALOG);
    await dialog.getByRole("textbox").first().fill("DJ Rosa");
    await dialog.getByRole("button", { name: "Not now" }).click();
    await expect(dialog).toHaveCount(0);
    const made = await calls(page);
    expect(made.at(-1)).toMatchObject({ cmd: "welcome_save", answers: { name: "DJ Rosa" } });
    expect(made.some((c) => c.cmd === "welcome_apply")).toBe(false);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * An answer Rust cannot use is said in Rust's words, holds the welcome on
   * its step, and stops being called wrong once it is put right.
   */
  test("a refused answer is said, and unsaid once put right", async ({ page }) => {
    await openShell(page, "/", {}, FIRST_RUN);
    const dialog = page.locator(DIALOG);
    await next(page);
    await next(page);
    await expect(dialog).toHaveAttribute("data-welcome-step", "music");
    await dialog.getByRole("spinbutton", { name: "Slowest BPM" }).fill(String(welcome.refused.bpm_low));
    await dialog.getByRole("spinbutton", { name: "Fastest BPM" }).fill(String(welcome.refused.bpm_high));
    await next(page);
    await expect(dialog.getByRole("alert")).toHaveText(welcome.refused.message);
    await expect(dialog).toHaveAttribute("data-welcome-step", "music");

    await dialog.getByRole("spinbutton", { name: "Fastest BPM" }).fill("150");
    await expect(dialog.getByRole("alert")).toHaveCount(0);
    await next(page);
    await expect(dialog).toHaveAttribute("data-welcome-step", "moves");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **While it is open, the booth's keys wait.** A digit typed into the tempo
   * must not switch the activity behind the welcome, and Space must not open
   * the key guide over it.
   */
  test("the booth's shortcuts wait while the welcome is open", async ({ page }) => {
    await openShell(page, "/", {}, FIRST_RUN);
    const dialog = page.locator(DIALOG);
    await expect(dialog).toBeVisible();
    await page.keyboard.press("2");
    await page.keyboard.press("Space");
    await page.waitForTimeout(200);
    const moves = await page.evaluate(
      () => ((window as unknown as { __activityMoves?: string[] }).__activityMoves ?? []),
    );
    expect(moves, "a digit behind the welcome switched the activity").toEqual([]);
    await expect(dialog).toBeVisible();
    // And typing still types.
    await dialog.getByRole("textbox").first().click();
    await page.keyboard.type("DJ 24");
    await expect(dialog.getByRole("textbox").first()).toHaveValue("DJ 24");
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A look is worn as it is chosen, and taken off again when un-chosen. */
  test("a look is worn as it is chosen, and put back if the welcome is left", async ({ page }) => {
    await openShell(page, "/", {}, FIRST_RUN);
    const dialog = page.locator(DIALOG);
    // What is worn is what the page is painted with.
    const worn = () =>
      page.evaluate(() => {
        const style = getComputedStyle(document.documentElement);
        return ["--accent", "--panel", "--bg", "--selected"].map((t) => style.getPropertyValue(t).trim()).join(" ");
      });
    // And what djmanzo was told: §31 puts back, a few seconds later, a look
    // that was only painted.
    const declared = () => page.evaluate(() => (window as unknown as { __chosenTheme?: string }).__chosenTheme);
    const before = await worn();
    const was = await page.evaluate(() => localStorage.getItem("djmanzo.themePackage") ?? "pkg-organic");
    for (let i = 0; i < 5; i++) await next(page);
    await expect(dialog).toHaveAttribute("data-welcome-step", "look");
    const pick = "pkg-industrial";
    await dialog.locator(`[data-theme-choice="${pick}"]`).click();
    await expect.poll(worn).not.toBe(before);
    await expect.poll(declared).toBe(pick);
    // Chosen, then un-chosen, then left: painted as it was, and said so.
    await dialog.locator(`[data-theme-choice="${pick}"]`).click();
    await expect.poll(declared).toBe(was);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect.poll(worn).toBe(before);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** And from Settings, whenever the DJ wants it again. */
  test("the welcome opens again from Settings", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await page.getByRole("button", { name: "Open the welcome" }).click();
    await expect(page.locator(DIALOG)).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });
});
