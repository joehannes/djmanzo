/**
 * §110: the spectrum as light.
 *
 * > i also want you to improve and augment and enhance the waveforms/displays
 * > of the song ... more colors that combine to audiowaves like to light ...
 * > all frequencies = white ... lowest frequencies red ... highest frequencies
 * > violet .. so color code in live
 *
 * The colour itself is Rust's — `dj_render::spectral_light`, tested there down
 * to the pixel, including that everything at once is white and that each band
 * is its own hue. The browser cannot see a tile's pixels (the tiles are served
 * by Rust over a custom scheme), so what is held here is the wiring: every
 * tile a lane and an overview ask for names the colouring, choosing the other
 * one redraws them, and a lane whose colour is still being measured asks
 * again until it has it.
 */
import { expect, test, type Page } from "@playwright/test";

import layers from "./layers.json" with { type: "json" };
import { ANSWERS, errorsThrown, openShell } from "./shell";

/** Every waveform image on screen: the lanes' tiles and the overviews. */
const sources = (page: Page) =>
  page.locator("img.tile, img.whole").evaluateAll((images) =>
    images.map((image) => image.getAttribute("src") ?? ""),
  );

async function openSettings(page: Page) {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.locator('[data-picker="waveform-colouring"]')).toBeVisible();
}

test.describe("§110: the spectrum as light", () => {
  /**
   * **Light unless the DJ chose otherwise.** The owner asked for it, so it is
   * what a fresh install draws, and every tile says so in its URL — tiles are
   * cached by URL for a year, so a colouring not in the URL is a colouring
   * that cannot change.
   */
  test("every waveform is drawn in light by default", async ({ page }) => {
    await openShell(page, "/");
    await expect.poll(() => sources(page)).not.toHaveLength(0);
    for (const src of await sources(page)) {
      expect(src, "a waveform tile that does not name its colouring").toMatch(/\/light$/);
    }
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The load-bearing one: choosing the three bands redraws every lane and
   * overview in them, and choosing light brings it back.** The choice reaches
   * Rust too, so it survives a restart.
   */
  test("choosing the three bands redraws every waveform, and back", async ({ page }) => {
    await openShell(page, "/");
    await openSettings(page);
    const picker = page.locator('[data-picker="waveform-colouring"]');
    await expect(picker.getByRole("radio", { name: /spectrum as light/i })).toBeChecked();

    await picker.getByRole("radio", { name: /three bands/i }).check();
    await expect
      .poll(async () => (await sources(page)).every((src) => src.endsWith("/bands")))
      .toBe(true);

    await picker.getByRole("radio", { name: /spectrum as light/i }).check();
    await expect
      .poll(async () => (await sources(page)).every((src) => src.endsWith("/light")))
      .toBe(true);

    expect(await page.evaluate(() => (window as unknown as { __coloured: string[] }).__coloured)).toEqual([
      "bands",
      "light",
    ]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A lane whose colour is still being measured asks again, and takes it.**
   *
   * The spectrum is measured after the load, off the path that puts a record
   * on a deck, and lands a moment later under a new epoch. Nothing else a
   * lane watches moves when it does — the tempo and key of a record analysed
   * before are there at load — so a lane that did not ask again would draw
   * three bands until the next record. Here the colour lands 600 ms after the
   * first ask, and every tile has to end up on the new epoch.
   */
  test("a waveform whose colour is still pending asks again and takes it", async ({ page }) => {
    await openShell(page, "/", {}, {
      waveform_info: { ...ANSWERS.waveform_info, epoch: 1, colour_pending: true },
      waveform_info_then: { ...ANSWERS.waveform_info, epoch: 2, colour_pending: false },
    });
    // tile/{deck}/{w}/{h}/{start}/{zoom}/{theme}/{epoch}/{grid}/{colouring}
    const epochOf = (src: string) => src.split("/").at(-3);
    await expect.poll(() => sources(page)).not.toHaveLength(0);
    await expect
      .poll(async () => (await sources(page)).map(epochOf), { timeout: 5_000 })
      .toEqual(expect.arrayContaining(["2"]));
    await expect
      .poll(async () => (await sources(page)).every((src) => epochOf(src) === "2"), { timeout: 5_000 })
      .toBe(true);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The picker's count of layers is the table's.** It said "twelve exist"
   * long after nineteen did — a sentence true when written and quietly false
   * since — so it is now read from the same list the picker draws.
   */
  test("the layer picker counts what exists from the table", async ({ page }) => {
    await openShell(page, "/");
    await openSettings(page);
    const drawn = layers.filter((layer: { drawn: string }) => layer.drawn !== "nowhere").length;
    await expect(page.locator(".hint").filter({ hasText: "semantic layers" })).toContainText(
      `${drawn} of ${layers.length} exist`,
    );
  });
});
