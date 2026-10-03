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

import type { StripSettings } from "../src/api";
import { errorsThrown, openShell } from "./shell";
import vocals from "./vocals.json" with { type: "json" };

const section = (page: Page) => page.getByRole("region", { name: "Microphones" });

const calls = (page: Page) =>
  page.evaluate(
    () =>
      (window as unknown as { __vocalCalls?: { cmd: string; strip?: number; preset?: string; deviceId?: string; settings?: StripSettings }[] })
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
  monitor_music_db: null as number | null,
};

const dispatched = (page: Page) =>
  page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []);

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
   * **The chain is behind a press on the row, drawn over Rust's ranges, and
   * a change sends the strip's whole settings.** Pressing *Mic 1* opens its
   * chain; every slider's range — the row's fader's too — is the one Rust
   * names for it; switching the echo on sends an echo at Rust's starting
   * point for one; a move of the compressor's ratio sends that ratio with the
   * rest of the strip as it was; switching the room off sends no room and
   * takes its controls away; a second press closes the chain.
   */
  test("a row's chain is behind a press on its name, over Rust's ranges", async ({ page }) => {
    await singers(page);
    const here = section(page);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    await readings(page, TWO);
    const first = here.getByRole("list", { name: "Singers' microphones" }).getByRole("listitem").nth(0);
    const chain = here.getByRole("group", { name: "Mic 1's chain" });
    await expect(chain).toHaveCount(0);
    const name = first.getByRole("button", { name: "Mic 1", exact: true });
    await expect(name).toHaveAttribute("aria-expanded", "false");
    await name.click();
    await expect(chain).toBeVisible();
    await expect(name).toHaveAttribute("aria-expanded", "true");

    // Every range is Rust's.
    const limits = vocals.open.limits as Record<string, number[]>;
    const ratio = chain.getByRole("slider", { name: "Mic 1 Compressor Ratio" });
    await expect(ratio).toHaveAttribute("min", String(limits["compressor.ratio"][0]));
    await expect(ratio).toHaveAttribute("max", String(limits["compressor.ratio"][1]));
    const makeup = chain.getByRole("slider", { name: "Mic 1 Compressor Makeup" });
    await expect(makeup).toHaveAttribute("max", String(limits["compressor.makeup_db"][1]));
    const lowCut = chain.getByRole("slider", { name: "Mic 1 Low cut" });
    await expect(lowCut).toHaveAttribute("max", String(limits.high_pass_hz[1]));
    const level = first.getByRole("slider", { name: "Mic 1 level" });
    await expect(level).toHaveAttribute("min", String(limits.gain_db[0]));
    await expect(level).toHaveAttribute("max", String(limits.gain_db[1]));

    const last = async () => (await calls(page)).filter((c) => c.cmd === "vocal_strip_set").at(-1);
    // No echo on a singer's strip; switched on, it starts where Rust says.
    const echo = chain.getByRole("checkbox", { name: "Mic 1 Echo" });
    await expect(echo).not.toBeChecked();
    await echo.check();
    await expect.poll(async () => (await last())?.settings?.echo).toEqual(vocals.open.every_stage.echo);
    expect((await last())?.strip).toBe(0);
    await expect(chain.getByRole("slider", { name: "Mic 1 Echo Delay" })).toBeVisible();

    // One number moved, the rest of the strip exactly as it was.
    const before = (await last())?.settings;
    if (!before?.compressor) throw new Error("a singer's strip has a compressor");
    await ratio.fill("8");
    await expect.poll(async () => (await last())?.settings?.compressor?.ratio).toBe(8);
    expect((await last())?.settings).toEqual({ ...before, compressor: { ...before.compressor, ratio: 8 } });

    // The room off: none sent, and its controls gone.
    await chain.getByRole("checkbox", { name: "Mic 1 Room" }).uncheck();
    await expect.poll(async () => (await last())?.settings?.reverb).toBeNull();
    await expect(chain.getByRole("slider", { name: "Mic 1 Room Length" })).toHaveCount(0);

    await name.click();
    await expect(chain).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A row stays inside its panel, and so does its chain.** Found by driving
   * the application, not by a test: the Singers surface docked at the side
   * is narrower than a row's six things, and every row ran past the panel's
   * edge with its preset cut off — and the chain, as wide as its row, came
   * out clipped. As docked here, no row is wider than its list, open chain
   * and all; and in a list narrower still, the level and the compression go
   * under the fader rather than off the edge.
   */
  test("a row and its chain stay inside the panel, however narrow", async ({ page }) => {
    await singers(page);
    const here = section(page);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    await readings(page, TWO);
    const list = here.getByRole("list", { name: "Singers' microphones" });
    const first = list.getByRole("listitem").nth(0);
    await first.getByRole("button", { name: "Mic 1", exact: true }).click();
    await expect(here.getByRole("group", { name: "Mic 1's chain" })).toBeVisible();

    /** How far each row runs past its own box, in pixels; 0 is inside. */
    const overrun = () =>
      list.evaluate((ul) =>
        Array.from(ul.children).map((row) => Math.max(0, row.scrollWidth - row.clientWidth)),
      );
    expect(await overrun()).toEqual([0, 0]);

    // A narrower dock: the level goes under the fader, and nothing runs over.
    await list.evaluate((ul) => {
      (ul as HTMLElement).style.width = "280px";
    });
    expect(await overrun()).toEqual([0, 0]);
    const fader = await first.getByRole("slider", { name: "Mic 1 level" }).boundingBox();
    const meter = await first.locator(".meter").boundingBox();
    if (!fader || !meter) throw new Error("a fader and a meter");
    expect(meter.y).toBeGreaterThanOrEqual(fader.y + fader.height - 1);

    // Just too wide for that: one line, the longest preset chosen, and the
    // select gives way rather than the row.
    await first.getByRole("combobox", { name: "Mic 1 is for" }).selectOption("loud-singer");
    await list.evaluate((ul) => {
      (ul as HTMLElement).style.width = "490px";
    });
    const wide = await first.getByRole("slider", { name: "Mic 1 level" }).boundingBox();
    const level = await first.locator(".meter").boundingBox();
    if (!wide || !level) throw new Error("a fader and a meter");
    expect(level.y).toBeLessThan(wide.y + wide.height);
    expect(await overrun()).toEqual([0, 0]);
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

  /**
   * **The singers' monitor is offered only where the output has a pair for
   * it, and its level reaches the engine as the action a controller would
   * send.** Six outputs: no slider, and the host told what would give them
   * one. Eight: the music's level in the wedge, said in dB and as *off* at
   * the bottom, and a move of it sent as `monitor music <dB>`.
   */
  test("the monitor's music is offered on an output with a pair for it", async ({ page }) => {
    await singers(page);
    const here = section(page);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    await readings(page, TWO);
    const slider = here.getByRole("slider", { name: "Music in the singers' monitor" });
    await expect(slider).toHaveCount(0);
    await expect(here).toContainText("an interface with eight outputs or more gives them a wedge of their own");

    await readings(page, { ...TWO, monitor_music_db: -6 });
    await expect(slider).toBeVisible();
    await expect(here.locator(".monitor output")).toHaveText("−6 dB");
    await slider.fill("-15");
    await expect.poll(() => dispatched(page)).toContain("monitor music -15");

    // The engine answers at the bottom of the range: the music is off.
    await readings(page, { ...TWO, monitor_music_db: -60 });
    await expect(here.locator(".monitor output")).toHaveText("off");
    expect(errorsThrown(page)).toEqual([]);
  });
});
