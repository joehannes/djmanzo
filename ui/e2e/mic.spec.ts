/**
 * K3: the DJ's microphone panel when the microphone is Mic 1 of the singers'.
 *
 * With the singers' microphones open and nothing attached to the DJ's own,
 * the engine's `Mic*` parameters are strip 0's and `master.mic.strip` says so
 * (docs/KARAOKE.md §6). Where the `mic` actions go is the host's, and held by
 * `dj_app`'s test through the real host thread; this holds what the panel
 * shows and sends.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";
import snapshot from "./snapshot.json" with { type: "json" };

const dispatched = (page: import("@playwright/test").Page) =>
  page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []);

test.describe("K3: the microphone is Mic 1", () => {
  /**
   * **The panel says it is Mic 1, and offers only what reaches the strip.**
   * The note names it; the switch, the fader, talkover and the headphones are
   * there and send the `mic` actions they always did; talkover's depth and
   * timing, which are the DJ's own channel's and do not reach a singer's, are
   * not offered, nor is the ducking meter that reads that channel; and a
   * microphone of the DJ's own can still be attached.
   */
  test("the panel says it is Mic 1 and offers what reaches the strip", async ({ page }) => {
    await openShell(page, "/", {
      mic: { ...snapshot.master.mic, present: true, open: false, gain_db: -9, strip: true },
    });
    await page.getByRole("button", { name: "Booth", exact: true }).click();
    const panel = page.locator("section.mic");
    await expect(panel.locator("[data-strip]")).toContainText("Mic 1 of the singers' microphones");
    await expect(panel.getByRole("combobox", { name: "Microphone input" })).toBeVisible();
    await expect(panel.getByRole("button", { name: "Talkover depth and timing" })).toHaveCount(0);
    await expect(panel.getByRole("meter", { name: "How far the music is stepping back" })).toHaveCount(0);
    await expect(panel.getByText("Nothing is attached")).toHaveCount(0);

    await panel.getByRole("button", { name: "Off", exact: true }).click();
    await panel.getByRole("button", { name: "Talkover" }).click();
    expect((await dispatched(page)).slice(-2)).toEqual(["mic on", "mic talkover_off"]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A slipping input is said while it slips, not for ever after.** An input
   * whose first block came after the output's has missed a few hundred frames
   * once; the panel first said so for the rest of the night, which with Mic 1
   * would be every karaoke night. Now: nothing for the count it opens on, the
   * warning while the count climbs, and gone once it has stopped.
   */
  test("a slipping input is said while it slips", async ({ page }) => {
    const mic = { ...snapshot.master.mic, present: true, strip: true, starved_frames: 256 };
    await openShell(page, "/", { mic });
    await page.getByRole("button", { name: "Booth", exact: true }).click();
    const panel = page.locator("section.mic");
    const warning = panel.getByText("The input is not keeping up");
    const missed = (starved_frames: number) =>
      page.evaluate((next) => {
        const win = window as unknown as {
          __lastState?: { master: Record<string, unknown> };
          __emit?: (next: unknown) => void;
        };
        const state = win.__lastState;
        if (!state) throw new Error("no state");
        win.__emit?.({ ...state, master: { ...state.master, mic: next } });
      }, { ...mic, starved_frames });
    await missed(256);
    await expect(panel.locator("[data-strip]")).toBeVisible();
    await expect(warning).toHaveCount(0);
    await missed(1_280);
    await expect(warning).toBeVisible();
    await expect(warning).toContainText("1280 frames");
    await page.waitForTimeout(2_300);
    await missed(1_280);
    await expect(warning).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A microphone of the DJ's own reads as it always did. */
  test("the DJ's own microphone keeps its whole panel", async ({ page }) => {
    await openShell(page, "/", {
      mic: { ...snapshot.master.mic, present: true, strip: false },
    });
    await page.getByRole("button", { name: "Booth", exact: true }).click();
    const panel = page.locator("section.mic");
    await expect(panel.locator("[data-strip]")).toHaveCount(0);
    await expect(panel.getByRole("combobox", { name: "Microphone input" })).toHaveCount(0);
    await expect(panel.getByRole("button", { name: "Talkover depth and timing" })).toBeVisible();
    await expect(panel.getByRole("meter", { name: "How far the music is stepping back" })).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });
});
