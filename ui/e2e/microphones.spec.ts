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
      (
        window as unknown as {
          __vocalCalls?: {
            cmd: string;
            strip?: number;
            preset?: string;
            deviceId?: string;
            settings?: StripSettings;
            singer?: string | null;
          }[];
        }
      ).__vocalCalls ?? [],
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
  settings_rev: 0,
};

const dispatched = (page: Page) =>
  page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []);

async function singers(page: Page, answers: Record<string, unknown> = {}) {
  await openShell(
    page,
    "/",
    {},
    {
      list_inputs: [
        { id: "usb", name: "USB interface", channels: 2, sample_rate: 48_000, is_default: true, supports_split_output: false },
      ],
      ...answers,
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
   * **K6: the note heard on each microphone**, as Rust reads it: cents above
   * A4 named as a note, how far off it is said when that is more than ten
   * cents, and nothing on a strip nobody is singing into.
   */
  test("each microphone shows the note it hears", async ({ page }) => {
    await singers(page);
    const here = section(page);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    const heard = (pitch: (number | null)[]) => ({
      ...TWO,
      strips: TWO.strips.map((strip, i) => ({ ...strip, pitch: pitch[i] })),
    });
    await readings(page, heard([-1200, null]));
    const rows = here.getByRole("list", { name: "Singers' microphones" }).getByRole("listitem");
    await expect(rows.nth(0).locator("[data-heard]")).toHaveText("A3");
    await expect(rows.nth(1).locator("[data-heard]")).toHaveText("");
    await readings(page, heard([-480, -2400]));
    await expect(rows.nth(0).locator("[data-heard]")).toHaveText("E4 +20");
    await expect(rows.nth(1).locator("[data-heard]")).toHaveText("A2");
    expect(errorsThrown(page)).toEqual([]);
  });

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
  /**
   * **A singer goes on a microphone from the rotation, and what they sound
   * best through is kept for them.** The up-next singer has a button a
   * microphone, the way a record has *Load on 1*: pressed, Rust is asked to
   * put them on that strip, and its row is named for them; pressed on
   * another, they move; pressed again, they come off. Behind the row, who is
   * on it can be anybody in the rotation or nobody, it says where their
   * chain came from, and *Keep for* keeps it. Where the chain comes from and
   * where it is kept is Rust's, and held by `dj_app`'s tests; this holds
   * that every press asks Rust for exactly that.
   */
  test("a singer is put on a microphone from the rotation, and their chain kept", async ({ page }) => {
    const song = (title: string) => ({ title, track: null, path: null, key: 0 });
    await singers(page, {
      karaoke_rotation: {
        singers: [
          { name: "Ana", songs: [song("Dancing Queen")], turns: 0, microphone: null },
          { name: "Ben", songs: [song("Toxic")], turns: 0, microphone: null },
          { name: "Maximiliane von Hohenberg-Lindqvist", songs: [song("Zombie")], turns: 0, microphone: null },
        ],
        up_next: "Ana",
        lately: [],
      },
    });
    const here = section(page);
    const onMic = page.getByRole("group", { name: "Ana on a microphone" });
    await expect(onMic).toHaveCount(0);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    await readings(page, TWO);
    const rows = here.getByRole("list", { name: "Singers' microphones" }).getByRole("listitem");
    const asked = async () => (await calls(page)).filter((c) => c.cmd === "vocal_strip_singer" || c.cmd === "vocal_strip_keep");

    await onMic.getByRole("button", { name: "Mic 2" }).click();
    expect(await asked()).toEqual([{ cmd: "vocal_strip_singer", strip: 1, singer: "Ana" }]);
    await expect(onMic.getByRole("button", { name: "Mic 2" })).toHaveAttribute("aria-pressed", "true");
    await expect(rows.nth(1)).toHaveAttribute("data-singer", "Ana");
    await expect(rows.nth(1).getByRole("button", { name: "Ana", exact: true })).toBeVisible();

    await onMic.getByRole("button", { name: "Mic 1" }).click();
    expect((await asked()).at(-1)).toEqual({ cmd: "vocal_strip_singer", strip: 0, singer: "Ana" });
    await expect(onMic.getByRole("button", { name: "Mic 2" })).toHaveAttribute("aria-pressed", "false");
    await expect(rows.nth(1).getByRole("button", { name: "Mic 2", exact: true })).toBeVisible();

    // Behind Ana's row: who is on it, where her chain is from, and keeping it.
    await rows.nth(0).getByRole("button", { name: "Ana", exact: true }).click();
    const who = here.getByRole("group", { name: "Who is on Mic 1" });
    const picker = who.getByRole("combobox", { name: "Who is singing on Mic 1" });
    await expect(picker).toHaveValue("Ana");
    await expect(who).toContainText("nothing kept for Ana yet");
    await who.getByRole("button", { name: "Keep for Ana" }).click();
    expect((await asked()).at(-1)).toEqual({ cmd: "vocal_strip_keep", strip: 0 });
    await expect(who).toContainText("Ana's own, kept for tonight");

    // A long name on the row and behind it, docked and narrower: inside.
    const list = here.getByRole("list", { name: "Singers' microphones" });
    const overrun = () =>
      list.evaluate((ul) => Array.from(ul.children).map((row) => Math.max(0, row.scrollWidth - row.clientWidth)));
    await picker.selectOption("Maximiliane von Hohenberg-Lindqvist");
    await expect(who.getByRole("button", { name: "Keep for Maximiliane von Hohenberg-Lindqvist" })).toBeVisible();
    expect(await overrun()).toEqual([0, 0]);
    await list.evaluate((ul) => {
      (ul as HTMLElement).style.width = "280px";
    });
    expect(await overrun()).toEqual([0, 0]);
    await list.evaluate((ul) => {
      (ul as HTMLElement).style.width = "";
    });

    await picker.selectOption("Ben");
    expect((await asked()).at(-1)).toEqual({ cmd: "vocal_strip_singer", strip: 0, singer: "Ben" });
    await expect(rows.nth(0)).toHaveAttribute("data-singer", "Ben");
    await expect(onMic.getByRole("button", { name: "Mic 1" })).toHaveAttribute("aria-pressed", "false");
    await picker.selectOption("");
    expect((await asked()).at(-1)).toEqual({ cmd: "vocal_strip_singer", strip: 0, singer: null });
    await expect(rows.nth(0)).toHaveAttribute("data-singer", "");
    await expect(who.getByRole("button", { name: /Keep for/ })).toHaveCount(0);

    // Pressed again, off.
    await onMic.getByRole("button", { name: "Mic 2" }).click();
    await onMic.getByRole("button", { name: "Mic 2" }).click();
    expect((await asked()).slice(-2)).toEqual([
      { cmd: "vocal_strip_singer", strip: 1, singer: "Ana" },
      { cmd: "vocal_strip_singer", strip: 1, singer: null },
    ]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Rust's answer with a singer on a strip reads as it.** Ana on Mic 1,
   * through the soft singer's chain kept on her guest-book record, as
   * `dj_app::commands::vocals_for` answers it: her name on the row, the
   * preset hers, and the chain said to be from the guest book.
   */
  test("a singer on a strip, as Rust answers it", async ({ page }) => {
    await singers(page, { vocals_opened: vocals.singing });
    const here = section(page);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    await readings(page, TWO);
    const first = here.getByRole("list", { name: "Singers' microphones" }).getByRole("listitem").nth(0);
    await expect(first).toHaveAttribute("data-singer", "Ana");
    await expect(first.getByRole("combobox", { name: "Mic 1 is for" })).toHaveValue("soft-singer");
    await first.getByRole("button", { name: "Ana", exact: true }).click();
    await expect(here.getByRole("group", { name: "Who is on Mic 1" })).toContainText("Ana's own, from the guest book");
    // Ana is not in tonight's rotation, and is still who is on it.
    await expect(here.getByRole("combobox", { name: "Who is singing on Mic 1" })).toHaveValue("Ana");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A change made anywhere else reaches the row.** A key, a controller or
   * the Booth's microphone panel working Mic 1 changes a strip in Rust, not
   * through this section; the host says so by moving `settings_rev`, and the
   * section asks again — before, its row went on saying *Closed* over an open
   * microphone, which driving the application found.
   */
  test("a change made elsewhere reaches the row", async ({ page }) => {
    await singers(page);
    const here = section(page);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    await readings(page, TWO);
    const first = here.getByRole("list", { name: "Singers' microphones" }).getByRole("listitem").nth(0);
    await expect(first.getByRole("button", { name: "Mic 1 closed" })).toBeVisible();
    // What `mic on` from the Booth does in Rust: Mic 1 open, the count moved.
    // A new answer, not the old one changed in place, so only asking again
    // can show it.
    await page.evaluate(() => {
      const win = window as unknown as { __vocals: { strips: { open: boolean }[] } };
      const next = structuredClone(win.__vocals);
      next.strips[0].open = true;
      win.__vocals = next;
    });
    const asked = async () => (await calls(page)).filter((c) => c.cmd === "vocals_state").length;
    const before = await asked();
    await readings(page, { ...TWO, settings_rev: 1 });
    await expect(first.getByRole("button", { name: "Mic 1 open" })).toBeVisible();
    expect(await asked()).toBe(before + 1);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A smaller buffer for singing, with what it costs said first.** The
   * section offers the interface's buffer, each size with what it means in
   * milliseconds; choosing one says the trade — sooner for a singer, more
   * for the machine, and that opening the interface again stops the music and
   * empties the decks — and nothing is reopened until the press that says so.
   * Pressed, the interface opens at that size, the device and headphones as
   * they were, and the singers' inputs open again on the same interface.
   * Choosing the size it already has takes the offer away.
   */
  test("a smaller buffer for singing is offered with its cost, and opened on a press", async ({ page }) => {
    await singers(page, {
      open_device: {
        name: "Null output",
        sample_rate: 48_000,
        buffer_frames: 128,
        channels: 2,
        latency_ms: 2.7,
        cue: null,
        cue_error: null,
      },
    });
    const here = section(page);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    await readings(page, TWO);
    const devices = () =>
      page.evaluate(() => (window as unknown as { __devices?: { bufferFrames: number }[] }).__devices ?? []);
    const before = (await devices()).length;
    const picker = here.getByRole("combobox", { name: "Buffer for singing" });
    await expect(picker).toHaveValue("256");
    await expect(picker.locator("option[value='128']")).toHaveText("128 frames — 2.7 ms each way");
    const reopen = here.getByRole("button", { name: /Reopen at/ });
    await expect(reopen).toHaveCount(0);

    await picker.selectOption("128");
    await expect(here.locator("[data-trade]")).toContainText("empties the decks");
    expect((await devices()).length, "nothing reopened by choosing").toBe(before);
    await picker.selectOption("256");
    await expect(reopen).toHaveCount(0);

    await picker.selectOption("128");
    await here.getByRole("button", { name: "Reopen at 128 frames" }).click();
    await expect(reopen).toHaveCount(0);
    const opened = await devices();
    expect(opened.length).toBe(before + 1);
    expect(opened.at(-1)).toMatchObject({ bufferFrames: 128 });
    const reopened = (await calls(page)).filter((c) => c.cmd === "vocals_open");
    expect(reopened.map((c) => c.deviceId)).toEqual(["usb", "usb"]);
    expect(errorsThrown(page)).toEqual([]);
  });

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
   * **K9: pitch correction on a strip** — off until chosen, what it adds to
   * the singer's wedge said before it is, Rust's own starting point and
   * ranges once on, and the key it pulls to as Rust names it: the up-next
   * song's, or a plain word that there is none.
   */
  test("a strip's pitch correction says what it adds and the key it pulls to", async ({ page }) => {
    const song = { title: "Valerie", track: "07".repeat(32), path: null, key: 2 };
    const rotation = {
      singers: [{ name: "Ana", songs: [song], turns: 0, microphone: null }],
      up_next: "Ana",
      lately: [],
    };
    await singers(page, { karaoke_rotation: { ...rotation, sung_key: null } });
    const here = section(page);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    await readings(page, TWO);
    await here.getByRole("button", { name: "Mic 1", exact: true }).click();
    const chain = here.getByRole("group", { name: "Mic 1's chain" });
    const pitch = chain.locator('fieldset[data-stage="correction"]');

    const on = chain.getByRole("checkbox", { name: "Mic 1 Pitch" });
    await expect(on).not.toBeChecked();
    await expect(pitch.locator("legend")).toContainText(`+${Math.round(vocals.open.correction_latency_ms)} ms`);
    await expect(chain.getByRole("slider", { name: "Mic 1 Pitch Strength" })).toHaveCount(0);

    const last = async () => (await calls(page)).filter((c) => c.cmd === "vocal_strip_set").at(-1);
    await on.check();
    await expect.poll(async () => (await last())?.settings?.correction).toEqual(vocals.open.every_stage.correction);
    const limits = vocals.open.limits as Record<string, number[]>;
    const strength = chain.getByRole("slider", { name: "Mic 1 Pitch Strength" });
    await expect(strength).toHaveAttribute("max", String(limits["correction.strength"][1]));
    const speed = chain.getByRole("slider", { name: "Mic 1 Pitch Speed" });
    await expect(speed).toHaveAttribute("min", String(limits["correction.speed_ms"][0]));
    await expect(speed).toHaveAttribute("max", String(limits["correction.speed_ms"][1]));
    await expect(pitch).toContainText("No key to pull to");

    await strength.fill("0.5");
    await expect.poll(async () => (await last())?.settings?.correction?.strength).toBe(0.5);
    await expect(pitch).toContainText("50 %");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **K9: the key pulled to is the rotation's** — Rust's name for the up-next
   * song's key as it will be sung.
   */
  test("pitch correction names the up-next song's key", async ({ page }) => {
    const song = { title: "Valerie", track: "07".repeat(32), path: null, key: 2 };
    await singers(page, {
      karaoke_rotation: {
        singers: [{ name: "Ana", songs: [song], turns: 0, microphone: null }],
        up_next: "Ana",
        lately: [],
        sung_key: "Bm (10A)",
      },
      vocals_opened: { ...vocals.open, strips: [vocals.open.every_stage, vocals.open.strips[1]] },
    });
    const here = section(page);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    await readings(page, TWO);
    await here.getByRole("button", { name: "Mic 1", exact: true }).click();
    const pitch = here.getByRole("group", { name: "Mic 1's chain" }).locator('fieldset[data-stage="correction"]');
    await expect(pitch).toContainText("To Bm (10A)");
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

    // Every stage on, so whichever lands in the last column has its
    // controls: no column is narrower than a stage needs, at any width.
    // (K9's stage put the De-esser in a third column 4 px past the row.)
    const chain = here.getByRole("group", { name: "Mic 1's chain" });
    for (const box of await chain.getByRole("checkbox").all()) await box.check();
    for (let width = 280; width <= 900; width += 15) {
      await list.evaluate((ul, w) => {
        (ul as HTMLElement).style.width = `${w}px`;
      }, width);
      expect([width, ...(await overrun())]).toEqual([width, 0, 0]);
    }
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

  /**
   * **K9: a monitor for each singer** (docs/KARAOKE.md §12). Offered only
   * where Rust says the output has a pair for two or more; asked for as the
   * action a controller would send. Split, each singer's monitor is listed
   * on its outputs with its meter, a microphone past the last pair is told
   * it has none, and what a singer hears in theirs — Rust's mix kept with
   * Ana's chain — is set behind her name and sent with her strip.
   */
  test("each singer can have a monitor of their own, with their own mix", async ({ page }) => {
    await singers(page, { vocals_opened: vocals.singing });
    const here = section(page);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    const split = here.getByRole("checkbox", { name: "A monitor for each singer" });

    // One pair: the shared monitor, and nothing to split.
    await readings(page, { ...TWO, monitor_music_db: 0, monitor_pairs: 1 });
    await expect(here.getByRole("slider", { name: "Music in the singers' monitor" })).toBeVisible();
    await expect(split).toHaveCount(0);

    // Two pairs: offered, and asked for.
    await readings(page, { ...TWO, monitor_music_db: 0, monitor_pairs: 2 });
    await split.check();
    await expect.poll(() => dispatched(page)).toContain("monitor split_on");

    // Rust gives them: Ana on 7–8, Mic 2 on 9–10, each with its meter.
    const given = {
      ...TWO,
      monitor_music_db: 0,
      monitor_pairs: 2,
      monitor_split: true,
      monitor_wedges: 2,
      strips: TWO.strips.map((strip, i) => ({ ...strip, wedge: i === 0 ? 0.4 : 0 })),
    };
    await readings(page, given);
    const monitors = here.getByRole("list", { name: "Each singer's monitor" }).getByRole("listitem");
    await expect(monitors).toHaveCount(2);
    await expect(monitors.nth(0)).toContainText("Ana");
    await expect(monitors.nth(0)).toContainText("outputs 7–8");
    await expect(monitors.nth(0).locator(".fill")).toHaveAttribute("style", /width: 40%/);
    await expect(monitors.nth(1)).toContainText("Mic 2");
    await expect(monitors.nth(1)).toContainText("outputs 9–10");
    // One line each: who, their outputs and their meter side by side — in the
    // application a class shared with the row put them on three.
    for (const part of [".wedge-singer", ".outs", ".meter"]) {
      const [name, other] = await Promise.all([
        monitors.nth(0).locator(".wedge-singer").boundingBox(),
        monitors.nth(0).locator(part).boundingBox(),
      ]);
      expect(name && other && Math.abs(name.y + name.height / 2 - (other.y + other.height / 2))).toBeLessThan(6);
      expect(other?.width ?? 0).toBeGreaterThan(20);
    }
    await expect(here.locator("[data-unwedged]")).toHaveCount(0);

    // Behind Ana's name, her mix as Rust keeps it; a move sends it with her strip.
    const first = here.getByRole("list", { name: "Singers' microphones" }).getByRole("listitem").nth(0);
    await first.getByRole("button", { name: "Ana", exact: true }).click();
    const theirs = first.locator('fieldset[data-stage="wedge"]');
    await expect(theirs).toContainText("Their monitor");
    await expect(theirs).toContainText("outputs 7–8");
    const others = theirs.getByRole("slider", { name: "Mic 1 monitor The others" });
    await expect(others).toHaveValue("0.25");
    await expect(theirs.getByRole("slider", { name: "Mic 1 monitor The music" })).toHaveValue("0.75");
    await others.fill("0.5");
    await expect
      .poll(async () => (await calls(page)).filter((c) => c.cmd === "vocal_strip_set").at(-1)?.settings?.wedge)
      .toEqual({ own: 1, others: 0.5, music: 0.75 });

    // A third microphone, with no pair left for it.
    await readings(page, {
      ...given,
      inputs: 3,
      strips: [...given.strips, { level: 0, gate_open: false, compression_db: 0, working: false, wedge: 0 }],
    });
    await expect(here.locator("[data-unwedged]")).toContainText(
      "Mic 3 has no monitor of their own — the outputs run out at 10",
    );

    // Shared again: no list, and nothing behind the name to set.
    await split.uncheck();
    await expect.poll(() => dispatched(page)).toContain("monitor split_off");
    await readings(page, { ...TWO, monitor_music_db: 0, monitor_pairs: 2 });
    await expect(here.getByRole("list", { name: "Each singer's monitor" })).toHaveCount(0);
    await expect(theirs).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **K8: a singer's range, from the microphone they are on.** Asked for
   * on their row, counted down while they sing, and said in notes once
   * Rust has read it — the reading itself is `dj_app::range`'s.
   */
  test("a singer's range is read from their microphone and said in notes", async ({ page }) => {
    await singers(page, { vocals_opened: vocals.singing });
    const here = section(page);
    await here.getByRole("button", { name: "Open the inputs" }).click();
    await readings(page, TWO);
    const first = here.getByRole("list", { name: "Singers' microphones" }).getByRole("listitem").nth(0);
    await first.getByRole("button", { name: "Ana", exact: true }).click();
    const range = here.getByRole("group", { name: "Ana's range" });
    const rotationAsks = (
      await page.evaluate(() => (window as unknown as { __karaoke?: { cmd: string }[] }).__karaoke ?? [])
    ).filter((c) => c.cmd === "karaoke_rotation").length;
    await range.getByRole("button", { name: "Find Ana's range" }).click();
    await expect(range.locator("[data-range-take]")).toContainText("sing from your lowest note to your highest");
    await expect(range.locator("[data-range-take]")).toContainText("20 s");
    await page.evaluate(() => {
      (window as unknown as { __rangeTake: unknown }).__rangeTake = {
        singer: "Ana",
        left: 7.2,
        voiced: 9.6,
        done: false,
        problem: null,
        range: null,
        in_guest_book: false,
      };
    });
    await expect(range.locator("[data-range-take]")).toContainText("8 s");
    await expect(range.locator("[data-range-take]")).toContainText("9 s of voice heard");
    await page.evaluate(() => {
      (window as unknown as { __rangeTake: unknown }).__rangeTake = {
        singer: "Ana",
        left: 0,
        voiced: 16,
        done: true,
        problem: null,
        range: { low: -2400, high: -500 },
        in_guest_book: true,
      };
    });
    await expect(range.locator("[data-range]")).toHaveText("A2 – E4, an octave and a fifth");
    await expect(range.locator("[data-range-kept]")).toHaveText("kept in Ana's guest-book record");
    // The rotation is asked again, so the range reaches the singer's row.
    await expect
      .poll(async () => (await page.evaluate(() => (window as unknown as { __karaoke?: { cmd: string }[] }).__karaoke ?? [])).filter((c) => c.cmd === "karaoke_rotation").length)
      .toBeGreaterThan(rotationAsks);
    await expect(range.getByRole("button", { name: "Find Ana's range" })).toBeEnabled();
    const asked = await page.evaluate(
      () => (window as unknown as { __rangeCalls?: Record<string, unknown>[] }).__rangeCalls ?? [],
    );
    expect(asked[0]).toMatchObject({ cmd: "karaoke_range_start", singer: "Ana" });
    expect(errorsThrown(page)).toEqual([]);
  });
});
