/**
 * K3: the singers' microphones on the Singers surface.
 *
 * The chains are `dj_vocal`'s and tested there; the engine's mixing is
 * `dj_engine`'s, and the host's opening of the input `dj_app`'s. This holds
 * what the host sees and presses (docs/KARAOKE.md §6): nothing open until they
 * pick an interface and open it; then a row a microphone, each with its
 * switch, fader, level and gate, compression and what it is for; the rig's
 * cost; and the input vanishing said at once.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";
import vocals from "./vocals.json" with { type: "json" };

const section = (page: Page) => page.getByRole("region", { name: "Microphones" });

const calls = (page: Page) =>
  page.evaluate(
    () =>
      (window as unknown as { __vocalCalls?: { cmd: string; strip?: number; preset?: string; deviceId?: string; settings?: { open: boolean; gain_db: number } }[] })
        .__vocalCalls ?? [],
  );

/** The engine's readings, as the snapshot carries them. */
async function readings(page: Page, vocalsState: unknown) {
  await page.evaluate((next) => {
    const win = window as unknown as {
      __lastState?: { master: Record<string, unknown> };
      __emit?: (next: unknown) => void;
    };
    const state = win.__lastState;
    if (!state) throw new Error("no state");
    win.__emit?.({ ...state, master: { ...state.master, vocals: next } });
  }, vocalsState);
}

const TWO = {
  inputs: 2,
  working: 1,
  starved_frames: 0,
  strips: [
    { level: 0.5, gate_open: true, compression_db: 4.2, working: true },
    { level: 0, gate_open: false, compression_db: 0, working: false },
  ],
};

async function singers(page: Page) {
  await openShell(
    page,
    "/",
    {},
    {
      list_inputs: [
        { id: "usb", name: "USB interface", channels: 2, sample_rate: 48_000, is_default: true, supports_split_output: false },
      ],
    },
  );
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.locator("body").click({ position: { x: 700, y: 120 } });
  await page.keyboard.press("Space");
  await page.keyboard.press("O");
  await page.keyboard.press("i");
  await expect(section(page)).toBeVisible();
}

test.describe("K3: the singers' microphones", () => {
  /**
   * **The load-bearing one.** Closed until the host opens an interface; then
   * a row per input from what the engine reports, each showing its level,
   * its gate and its compression, and what Rust says the strip is for — the
   * second made the MC's. A press on a row's switch sends that strip's
   * settings with it opened; choosing what a strip is for sends the preset.
   * The rig says how many are working and what that costs.
   */
  test("an interface is opened, a row a microphone, and the host's presses reach Rust", async ({ page }) => {
    await singers(page);
    const here = section(page);
    await expect(here.getByRole("list", { name: "Singers' microphones" })).toHaveCount(0);
    await expect(here.getByRole("combobox", { name: "Interface for the singers' microphones" })).toContainText(
      "USB interface — 2 inputs",
    );
    await here.getByRole("button", { name: "Open the inputs" }).click();
    expect((await calls(page)).find((c) => c.cmd === "vocals_open")).toMatchObject({ deviceId: "usb" });

    // The engine now holds two strips.
    await readings(page, TWO);
    const rows = here.getByRole("list", { name: "Singers' microphones" }).getByRole("listitem");
    await expect(rows).toHaveCount(2);
    const first = rows.nth(0);
    await expect(first).toHaveAttribute("data-gate", "true");
    await expect(first).toHaveAttribute("data-working", "true");
    await expect(first.locator(".fill")).toHaveAttribute("style", /width: 50%/);
    await expect(first).toContainText("−4 dB");
    await expect(rows.nth(1).getByRole("combobox", { name: "Mic 2 is for" })).toHaveValue("mc");
    await expect(here.getByRole("status")).toContainText("1 of 2 microphones working — about 0.6 % of the audio thread");
    await expect(here.getByRole("status")).toContainText("all 2 at once, 1.2 %");
    await expect(here.getByRole("status")).toContainText("hears themselves about 11 ms late");

    await first.getByRole("button", { name: "Mic 1 closed" }).click();
    const opened = (await calls(page)).find((c) => c.cmd === "vocal_strip_set");
    expect(opened).toMatchObject({ strip: 0, settings: { open: true, gain_db: vocals.open.strips[0].gain_db } });
    await expect(first.getByRole("button", { name: "Mic 1 open" })).toBeVisible();

    await first.getByRole("combobox", { name: "Mic 1 is for" }).selectOption("loud-singer");
    expect((await calls(page)).find((c) => c.cmd === "vocal_strip_preset")).toMatchObject({
      strip: 0,
      preset: "loud-singer",
    });
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **An input that vanishes is said at once, and only while it is gone.**
   * The starvation count climbing for half a second is the interface
   * unplugged; when it stops climbing, the message goes.
   */
  test("a lost input is said while the ring keeps running dry", async ({ page }) => {
    await singers(page);
    const here = section(page);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    await readings(page, TWO);
    await expect(here.getByRole("alert")).toHaveCount(0);
    for (let tick = 1; tick <= 12; tick++) {
      await readings(page, { ...TWO, starved_frames: tick * 4_800 });
      await page.waitForTimeout(60);
    }
    await expect(here.getByRole("alert")).toContainText("Microphones lost — reconnect the interface");
    // The count stops climbing: the input is back.
    await readings(page, { ...TWO, starved_frames: 12 * 4_800 });
    await expect(here.getByRole("alert")).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});
