/**
 * The stems on a deck: a row that is always there, and the tone a press away.
 *
 * §120: *"i don't like the design of the stem section yet ... also it seems
 * to kind of not be very accessible quickly ... what if i want to quickly
 * mute/unmute some stem??"* — and *"i got an issue with some filter knobs.
 * on trying to manipulate them they are only switching on or off fully"*.
 *
 * The stems were a fold, closed on every deck unless a layout opened it, so
 * muting a vocal took two presses and a scroll; and the tone controls were
 * native sliders a few pixels tall. Now four chips sit under the waveform —
 * a press mutes or brings a stem back, Shift and a press hears it alone —
 * and the tone is a panel that floats over the deck and goes away again,
 * with the deck's own knobs: a hundred pixels of drag for the whole range.
 *
 * Also held here, from the v0.23.0 fix: the panel says which separator is
 * doing the work, and keeps saying it while a model loads.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LOADING = {
  available: true,
  backend: "built-in (harmonic/percussive)",
  reason: "the HTDemucs model is loading; the built-in separator plays until it is ready",
  loading: true,
};
const READY = { available: true, backend: "HTDemucs (ONNX)", reason: null, loading: false };

const deck = (page: Page) => page.locator("section.deck[data-deck]").first();
const chip = (page: Page, stem: string) => deck(page).locator(`.stem-chip[data-stem="${stem}"]`);
/** The tone panel: floating, it lives in the body rather than in the deck. */
const panel = (page: Page) => page.locator(".stems-module");

const dispatched = (page: Page) =>
  page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []);

/** Deliver a frame with deck 1's stem state changed, as the engine would. */
async function stemState(page: Page, mutes: boolean[], soloing: boolean) {
  await page.evaluate(
    ({ mutes, soloing }) => {
      const win = window as unknown as {
        __lastState: { decks: { stem_mutes: boolean[]; stem_soloing: boolean }[] };
        __emit: (next: unknown) => void;
      };
      const next = structuredClone(win.__lastState);
      next.decks[0].stem_mutes = mutes;
      next.decks[0].stem_soloing = soloing;
      win.__emit(next);
    },
    { mutes, soloing },
  );
}

test.describe("§120: the stems, one press away", () => {
  /**
   * **The load-bearing one.** Nothing to open: the four chips are on the
   * deck, a press mutes, the chip shows it, Shift hears one alone, and a
   * press during a solo lets the solo go rather than doing nothing.
   */
  test("a stem mutes with one press, from a row that is always there", async ({ page }) => {
    await openShell(page, "/");
    for (const stem of ["vocal", "drums", "bass", "other"]) {
      await expect(chip(page, stem)).toBeVisible();
    }
    await expect(panel(page)).toHaveCount(0);

    await chip(page, "vocal").click();
    expect(await dispatched(page)).toContain("deck 1 stem_mute vocal");

    await stemState(page, [true, false, false, false], false);
    await expect(chip(page, "vocal")).toHaveAttribute("aria-pressed", "false");
    await expect(chip(page, "drums")).toHaveAttribute("aria-pressed", "true");

    await chip(page, "drums").click({ modifiers: ["Shift"] });
    expect(await dispatched(page)).toContain("deck 1 stem_solo_on drums");

    await stemState(page, [true, false, false, false], true);
    const before = (await dispatched(page)).length;
    await chip(page, "bass").click();
    const after = (await dispatched(page)).slice(before);
    expect(after).toEqual(["deck 1 stem_solo_off drums"]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The tone floats, and gets out of the way.** Opening it moves nothing
   * on the deck; Escape and a press elsewhere put it away.
   */
  test("the tone panel floats over the deck and goes away again", async ({ page }) => {
    await openShell(page, "/");
    // What sits under the stems on the deck: a panel that pushed it down
    // would be taking the deck's own room rather than floating over it.
    const tabs = deck(page).getByRole("button", { name: "cues", exact: true }).first();
    await expect(tabs).toBeVisible();
    const below = async () => (await tabs.boundingBox())?.y ?? 0;
    const resting = await below();

    const tone = deck(page).locator(".stem-more");
    await tone.click();
    await expect(panel(page)).toBeVisible();
    await expect(tone).toHaveAttribute("aria-expanded", "true");
    expect(Math.abs((await below()) - resting)).toBeLessThan(2);

    await page.keyboard.press("Escape");
    await expect(panel(page)).toHaveCount(0);

    await tone.click();
    await expect(panel(page)).toBeVisible();
    await page.locator(".topbar").first().click({ position: { x: 5, y: 5 } });
    await expect(panel(page)).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A small drag is a small change.** The stem filter is the deck's own
   * knob now: ten pixels up is a tenth of the way to full high-pass on a
   * -1..1 knob — 0.2 — however narrow the column, not a jump to the end.
   */
  test("a stem's filter turns gradually", async ({ page }) => {
    await openShell(page, "/");
    await deck(page).locator(".stem-more").click();
    const knob = panel(page).getByRole("slider", { name: "Vocals filter" });
    const box = await knob.boundingBox();
    if (!box) throw new Error("the vocal filter knob is not drawn");
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;
    const before = (await dispatched(page)).length;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x, y - 5, { steps: 3 });
    await page.mouse.move(x, y - 10, { steps: 3 });
    await page.mouse.up();
    const sent = (await dispatched(page))
      .slice(before)
      .filter((action) => action.startsWith("deck 1 stem_filter vocal:"))
      .map((action) => Number(action.split(":")[1]));
    expect(sent.length).toBeGreaterThan(0);
    const last = sent[sent.length - 1];
    expect(last).toBeGreaterThan(0.1);
    expect(last).toBeLessThan(0.3);
  });
});

test("the stems panel follows the model from loading to in use", async ({ page }) => {
  await openShell(page, "/", {}, {
    stems_status: [LOADING, LOADING, READY],
    __sequences: ["stems_status"],
  });

  await deck(page).locator(".stem-more").click();
  const reason = panel(page).locator(".stems-reason");
  await expect(reason).toContainText("Using the built-in (harmonic/percussive) separator");
  await expect(reason).toContainText("model is loading");
  // The stems play meanwhile: the built-in separator is doing the work.
  await expect(chip(page, "vocal")).toBeEnabled();

  // Asked again while loading, and the model has taken over: nothing left
  // to explain.
  await expect(reason).toHaveCount(0, { timeout: 8_000 });
  await expect(chip(page, "vocal")).toBeEnabled();
  expect(errorsThrown(page)).toEqual([]);
});
