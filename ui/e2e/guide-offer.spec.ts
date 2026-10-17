/**
 * Karaoke beta B7: the guide vocal, proposed for a song new to the singer.
 *
 * When it is proposed is Rust's (`dj_app::guide_offer`, tested there); what a
 * browser holds is the up-next card drawing Rust's offer, ticked, the load
 * putting the deck's voice to the guide only while it is ticked, nothing
 * where Rust offers nothing — and the singers' screen saying so, briefly,
 * before the first line.
 *
 * Rust's answer is given by wrapping the page's `invoke` in this test: the
 * karaoke beta's sessions leave `shell.ts` alone (docs/karaoke_beta_release.md §4).
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const ROTATION = {
  singers: [
    { name: "Ana", songs: [{ title: "Bachata Rosa", track: "a".repeat(64), path: "/music/bachata-rosa.flac", key: 0 }], turns: 0 },
  ],
  up_next: "Ana",
  lately: [],
};

const LINE = "New to Ana: the guide voice keeps the original singer quietly underneath.";

/** Answer `karaoke_guide_offer` with `offer`, and count the asks. */
async function offering(page: Page, offer: { line: string } | null) {
  await page.evaluate((offer) => {
    const win = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
      __guideAsks: number;
    };
    win.__guideAsks = 0;
    const stub = win.__TAURI_INTERNALS__.invoke;
    win.__TAURI_INTERNALS__.invoke = (cmd, args = {}) => {
      if (cmd === "karaoke_guide_offer") {
        win.__guideAsks += 1;
        return Promise.resolve(offer);
      }
      return stub(cmd, args);
    };
  }, offer);
}

async function singers(page: Page, offer: { line: string } | null) {
  await openShell(page, "/", {}, { karaoke_rotation: ROTATION });
  await offering(page, offer);
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.locator("body").click({ position: { x: 700, y: 120 } });
  await page.keyboard.press("Space");
  await page.keyboard.press("O");
  await page.keyboard.press("i");
  await expect(page.locator("[data-singers]")).toBeVisible();
  return page.getByRole("region", { name: "Up next" });
}

const voices = (page: Page) =>
  page.evaluate(() =>
    ((window as unknown as { __dispatched?: string[] }).__dispatched ?? []).filter((action) => / voice /.test(action)),
  );

test.describe("Karaoke beta B7: the guide, offered for a song new to the singer", () => {
  test("is offered ticked, and loading puts the deck's voice to the guide", async ({ page }) => {
    const up = await singers(page, { line: LINE });
    const box = up.getByRole("checkbox", { name: /Guide voice/ });
    await expect(box).toBeChecked();
    await expect(up.locator("[data-guide-offer]")).toContainText(LINE);
    await up.getByRole("button", { name: "Load on 2" }).click();
    await expect.poll(() => voices(page)).toEqual(["deck 2 voice 0.25"]);
    expect(errorsThrown(page)).toEqual([]);
  });

  test("left unticked, the load leaves the deck's voice alone", async ({ page }) => {
    const up = await singers(page, { line: LINE });
    const box = up.getByRole("checkbox", { name: /Guide voice/ });
    await box.uncheck();
    await up.getByRole("button", { name: "Load on 2" }).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __karaoke?: { cmd: string }[] }).__karaoke ?? []))
      .toContainEqual(expect.objectContaining({ cmd: "karaoke_load" }));
    expect(await voices(page)).toEqual([]);
  });

  test("nothing is offered where Rust offers nothing", async ({ page }) => {
    const up = await singers(page, null);
    await expect(up).toContainText("Bachata Rosa");
    await expect.poll(() => page.evaluate(() => (window as unknown as { __guideAsks: number }).__guideAsks)).toBeGreaterThan(0);
    await expect(up.locator("[data-guide-offer]")).toHaveCount(0);
    await up.getByRole("button", { name: "Load on 1" }).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __karaoke?: { cmd: string }[] }).__karaoke ?? []))
      .toContainEqual(expect.objectContaining({ cmd: "karaoke_load" }));
    expect(await voices(page)).toEqual([]);
  });
});

const WORDS = {
  lines: [
    { at: 10, text: "First line", words: [] },
    { at: 13, text: "Second line", words: [] },
  ],
  plain: [],
  instrumental: false,
};

/** Deck 2 is the singer's: playing at `seconds`, its voice at `voice`. */
async function playing(page: Page, seconds: number, voice: number) {
  await page.evaluate(
    ({ seconds, voice }) => {
      const win = window as unknown as {
        __lastState?: { decks: Record<string, unknown>[] };
        __emit?: (next: unknown) => void;
      };
      const state = win.__lastState;
      if (!state) throw new Error("no state");
      const decks = state.decks.map((deck, index) =>
        index === 1
          ? { ...deck, playing: true, loaded: true, voice, position_seconds: seconds, effective_bpm: 120, title: "Bachata Rosa" }
          : { ...deck, playing: false, voice: 1 },
      );
      win.__emit?.({ ...state, decks });
    },
    { seconds, voice },
  );
}

test("the singers' screen says the guide is underneath, before the first line only", async ({ page }) => {
  await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, karaoke_rotation: ROTATION });
  const screen = page.locator("[data-singer-screen]");
  await expect(screen).toBeVisible();
  const said = screen.locator("[data-guided]");

  await playing(page, 8.5, 0.25);
  await expect(said).toHaveText("You'll hear the original singer quietly underneath");
  await playing(page, 11, 0.25);
  await expect(said, "gone once they sing").toHaveCount(0);
  await playing(page, 8.5, 0);
  await expect(said, "not with the voice out").toHaveCount(0);
  expect(errorsThrown(page)).toEqual([]);
});
