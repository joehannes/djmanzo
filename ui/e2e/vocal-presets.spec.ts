/**
 * Karaoke beta B4: the karaoke presets in a microphone's menu.
 *
 * What each preset does to the chain is `dj_vocal::Preset`'s (tested there);
 * the menu is drawn from the list Rust sends (`vocals.json`, generated from
 * Rust by `e2e_fixture`), so this holds that the new presets reach it under
 * their names and that choosing one sends it as Rust names it.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const section = (page: Page) => page.getByRole("region", { name: "Microphones" });

const TWO = {
  inputs: 2,
  working: 1,
  starved_frames: 0,
  strips: [
    { level: 0.5, gate_open: true, compression_db: 0, working: true },
    { level: 0, gate_open: false, compression_db: 0, working: false },
  ],
  monitor_music_db: null as number | null,
  settings_rev: 0,
};

test("the karaoke presets are in each microphone's menu, and are sent as Rust names them", async ({ page }) => {
  await openShell(page, "/", {}, {
    list_inputs: [
      { id: "usb", name: "USB interface", channels: 2, sample_rate: 48_000, is_default: true, supports_split_output: false },
    ],
  });
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.locator("body").click({ position: { x: 700, y: 120 } });
  await page.keyboard.press("Space");
  await page.keyboard.press("O");
  await page.keyboard.press("i");
  const here = section(page);
  await expect(here).toBeVisible();
  await here.getByRole("button", { name: "Open the inputs" }).click();
  await page.evaluate((next) => {
    const win = window as unknown as {
      __lastState?: { master: Record<string, unknown> };
      __emit?: (next: unknown) => void;
    };
    const state = win.__lastState;
    if (!state) throw new Error("no state");
    win.__emit?.({ ...state, master: { ...state.master, vocals: next } });
  }, TWO);

  const menu = here.getByRole("combobox", { name: "Mic 1 is for" });
  await expect(menu).toBeVisible();
  const names = await menu.locator("option").allTextContents();
  for (const name of ["Stadium", "Radio voice", "Big echo", "Duet"]) expect(names).toContain(name);
  expect(names).toContain("Singer");

  await menu.selectOption({ label: "Radio voice" });
  const sent = await page.evaluate(
    () =>
      ((window as unknown as { __vocalCalls?: { cmd: string; strip?: number; preset?: string }[] }).__vocalCalls ?? []).filter(
        (call) => call.cmd === "vocal_strip_preset",
      ),
  );
  expect(sent.at(-1)).toMatchObject({ strip: 0, preset: "radio-voice" });
  expect(errorsThrown(page)).toEqual([]);
});
