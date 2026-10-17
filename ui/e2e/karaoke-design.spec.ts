/**
 * Karaoke beta B8: the karaoke screens' design pass — motion where nothing
 * is pressed, a moment at Sang, and the up-next card's hierarchy.
 *
 * ADR-0009's rule is held here: what a host presses never moves; what moves
 * is a name, a dot, a cheer, once and briefly, and only a fade with reduced
 * motion; and nothing the screen adds ever covers the words being sung.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const ROTATION = {
  singers: [
    { name: "Ana", songs: [{ title: "Bachata Rosa", track: null, path: "/music/bachata-rosa.flac", key: 0 }], turns: 0 },
    { name: "Ben", songs: [{ title: "Hey Jude", track: null, path: "/music/hey-jude.flac", key: 0 }], turns: 0 },
  ],
  up_next: "Ana",
  lately: [{ singer: "Dani", title: "Despacito", track: null, key: 0 }],
};

const WORDS = {
  lines: [
    { at: 10, text: "Rosa de la mañana", words: [] },
    { at: 13, text: "bachata para ti", words: [] },
  ],
  plain: [],
  instrumental: false,
};

const SCREEN = "[data-singer-screen]";

/** Deck 2 is the singer's, playing at `seconds` with its vocal out. */
async function at(page: Page, seconds: number) {
  await page.evaluate((seconds) => {
    const win = window as unknown as {
      __lastState?: { decks: Record<string, unknown>[] };
      __emit?: (next: unknown) => void;
    };
    const state = win.__lastState!;
    const decks = state.decks.map((deck, index) =>
      index === 1
        ? { ...deck, playing: true, loaded: true, voice: 0, position_seconds: seconds, effective_bpm: 120, title: "Bachata Rosa" }
        : { ...deck, playing: false, voice: 1 },
    );
    win.__emit?.({ ...state, decks });
  }, seconds);
}

/** The host presses Sang: Ana moves to what was sung, scored. */
async function sang(page: Page) {
  await page.evaluate(() => {
    const win = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
    };
    const stub = win.__TAURI_INTERNALS__.invoke;
    win.__TAURI_INTERNALS__.invoke = async (cmd, args = {}) => {
      const answer = (await stub(cmd, args)) as Record<string, unknown> & { singers: unknown[]; lately: unknown[] };
      if (cmd !== "karaoke_rotation") return answer;
      return {
        ...answer,
        singers: answer.singers.slice(1),
        up_next: "Ben",
        lately: [{ singer: "Ana", title: "Bachata Rosa", track: null, key: 0, score: 87 }, ...answer.lately],
      };
    };
  });
}

const animation = (page: Page, selector: string) =>
  page.locator(selector).first().evaluate((el) => getComputedStyle(el).animationName);

test.describe("Karaoke beta B8: the singers' screen", () => {
  test("who is singing rises in, the count-in pulses on the beat, the next name rises in", async ({ page }) => {
    await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, karaoke_rotation: ROTATION });
    await expect(page.locator(SCREEN)).toBeVisible();
    await at(page, 8.6);
    const now = page.locator(`${SCREEN} [data-now-singing]`);
    await expect(now).toHaveText("Ana");
    expect(await animation(page, `${SCREEN} [data-now-singing]`)).toContain("rise");
    expect(await animation(page, `${SCREEN} .dot.lit`)).toContain("beat");
    expect(await animation(page, `${SCREEN} [data-up-next]`)).toContain("rise");
    await expect(page.locator(`${SCREEN} [data-up-next]`)).toContainText("Ben");
    // Nothing that moves takes a press.
    await expect(page.locator(`${SCREEN} [data-now-singing]`)).not.toHaveAttribute("role", "button");
    expect(errorsThrown(page)).toEqual([]);
  });

  test("at Sang, a moment for the singer — above the words, briefly, and only for a Sang it saw", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 720 });
    await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, karaoke_rotation: ROTATION });
    const cheer = page.locator(`${SCREEN} [data-cheer]`);
    // Dani's song was sung before the screen opened: not cheered, from the
    // first look at the rotation on.
    for (let look = 0; look < 18; look++) {
      expect(await cheer.count(), "a Sang from before the screen opened was cheered").toBe(0);
      await page.waitForTimeout(200);
    }
    await at(page, 11);

    await sang(page);
    await expect(cheer).toBeVisible({ timeout: 5_000 });
    await expect(cheer).toContainText("¡Bravo, Ana!");
    await expect(cheer).toContainText("Bachata Rosa · 87");
    await expect(cheer).toHaveCSS("pointer-events", "none");
    // Never over the words.
    const box = (await cheer.boundingBox())!;
    for (const line of await page.locator(`${SCREEN} [data-line]`).all()) {
      const l = (await line.boundingBox())!;
      const overlaps = box.x < l.x + l.width && box.x + box.width > l.x && box.y < l.y + l.height && box.y + box.height > l.y;
      expect(overlaps, "the cheer covers a line").toBe(false);
    }
    // The cheer takes the singer's name's place while it shows.
    await expect(page.locator(`${SCREEN} [data-now-singing]`)).toHaveCount(0);
    // Gone within seconds, and not back on the next look at the rotation.
    await expect(cheer).toHaveCount(0, { timeout: 7_000 });
    await page.waitForTimeout(3_500);
    await expect(cheer).toHaveCount(0);
  });

  test("with reduced motion: a fade, no pulse, no confetti", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, karaoke_rotation: ROTATION });
    await at(page, 8.6);
    expect(await animation(page, `${SCREEN} .dot.lit`)).toBe("none");
    expect(await animation(page, `${SCREEN} [data-now-singing]`)).toContain("fade");
    await page.waitForTimeout(3_500);
    await sang(page);
    await expect(page.locator(`${SCREEN} [data-cheer]`)).toBeVisible({ timeout: 5_000 });
    await expect(page.locator(`${SCREEN} [data-cheer] .confetti`)).toHaveCSS("display", "none");
  });
});

test.describe("Karaoke beta B8: the up-next card", () => {
  test("the name leads, Load on N is the strong press, the rest quieter, and Sang is acknowledged", async ({ page }) => {
    await openShell(page, "/", {}, { karaoke_rotation: ROTATION });
    await page.setViewportSize({ width: 1400, height: 900 });
    await page.locator("body").click({ position: { x: 700, y: 120 } });
    await page.keyboard.press("Space");
    await page.keyboard.press("O");
    await page.keyboard.press("i");
    const up = page.getByRole("region", { name: "Up next" });
    await expect(up).toBeVisible();
    await expect(up.locator(".eyebrow")).toHaveText(/up next/i);
    const size = (selector: string) => up.locator(selector).evaluate((el) => parseFloat(getComputedStyle(el).fontSize));
    expect(await size(".who")).toBeGreaterThan((await size(".what")) * 1.4);

    const fill = (name: string) =>
      up.getByRole("button", { name, exact: true }).evaluate((el) => getComputedStyle(el).backgroundColor);
    const load = await fill("Load on 1");
    expect(load, "Load on N filled, unlike a plain button").not.toBe(await fill("Guest book"));
    expect(await fill("Not here"), "Not here quiet").toBe("rgba(0, 0, 0, 0)");
    expect(await fill("Load on 2")).toBe(load);

    // The buttons stay where they are while the name rises in: nothing to press moves.
    const before = await up.getByRole("button", { name: "Sang", exact: true }).boundingBox();
    await page.waitForTimeout(600);
    expect(await up.getByRole("button", { name: "Sang", exact: true }).boundingBox()).toEqual(before);

    await up.getByRole("button", { name: "Sang", exact: true }).click();
    await expect(page.locator("[data-sang-note]")).toHaveText("✓ Ana sang Bachata Rosa");
    await expect(page.locator("[data-sang-note]")).toHaveCount(0, { timeout: 6_000 });
    expect(errorsThrown(page)).toEqual([]);
  });
});
