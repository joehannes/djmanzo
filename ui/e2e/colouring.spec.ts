/**
 * §110: the spectrum as light.
 *
 * > i also want you to improve and augment and enhance the waveforms/displays
 * > of the song ... more colors that combine to audiowaves like to light ...
 * > all frequencies = white ... lowest frequencies red ... highest frequencies
 * > violet .. so color code in live
 *
 * and, clarified:
 *
 * > i meant low frequencies of light are red, high frequencies of light are
 * > violet and all the colors lie in between ... i guess white is out of the
 * > picture ... reiterate over the (live) colors and interactive/reactiveness
 * > of the waveform
 *
 * The colour itself is Rust's — `dj_render::frequency_colour` and the stacked
 * column, tested there down to the pixel, including that nothing is white.
 * The browser cannot see a tile's pixels (the tiles are served by Rust over a
 * custom scheme), so what is held here is the wiring: every tile a lane and
 * an overview ask for names the colouring, choosing the other one redraws
 * them, a lane whose colour is still being measured asks again until it has
 * it — and, live, a lane is its three EQ parts, each as bright as its knob.
 */
import { expect, test, type Page } from "@playwright/test";

import layers from "./layers.json" with { type: "json" };
import { GHOST } from "../src/eqLight";
import { ANSWERS, errorsThrown, openShell } from "./shell";

/** Every waveform image on screen: the lanes' tiles and the overviews. */
const sources = (page: Page) =>
  page.locator("img.tile, img.whole").evaluateAll((images) =>
    images.map((image) => image.getAttribute("src") ?? ""),
  );

// tile/{deck}/{w}/{h}/{start}/{zoom}/{theme}/{epoch}/{grid}/{colouring}/{part}
const colouringOf = (src: string) => src.split("/").at(-2);
const partOf = (src: string) => src.split("/").at(-1);
const gridOf = (src: string) => src.split("/").at(-3);

/** Deck 1's lane tiles, with the part each draws and its opacity. */
const lane = (page: Page) =>
  page.locator('.deck[data-deck="1"] img.tile').evaluateAll((images) =>
    images.map((image) => ({
      src: image.getAttribute("src") ?? "",
      part: image.getAttribute("data-part"),
      opacity: getComputedStyle(image).opacity,
    })),
  );

/** Send the snapshot again with deck 1's knobs changed. */
async function knobs(page: Page, change: Record<string, number>) {
  await page.evaluate((change) => {
    const win = window as unknown as {
      __lastState?: { decks: Record<string, unknown>[] };
      __emit?: (next: unknown) => void;
    };
    const state = win.__lastState;
    if (!state) throw new Error("the harness delivered no state to start from");
    const decks = state.decks.map((deck, index) => (index === 0 ? { ...deck, ...change } : deck));
    win.__emit?.({ ...state, decks });
  }, change);
}

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
      expect(colouringOf(src), `${src} does not name its colouring`).toBe("light");
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
      .poll(async () => (await sources(page)).every((src) => colouringOf(src) === "bands"))
      .toBe(true);
    // One colour a column cannot be cut by band: no parts under the bands.
    expect(new Set((await sources(page)).map(partOf))).toEqual(new Set(["all"]));

    await picker.getByRole("radio", { name: /spectrum as light/i }).check();
    await expect
      .poll(async () => (await sources(page)).every((src) => colouringOf(src) === "light"))
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
    const epochOf = (src: string) => src.split("/").at(-4);
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
   * **The load-bearing one for "live": a lane is its three EQ parts, and each
   * is as bright as its knob.** Kill the low and the low part fades to a
   * ghost while the mid and high stay whole; sweep the filter fully down and
   * the high goes too. The grid is its own layer, never dimmed, so a killed
   * band never takes the beat lines with it.
   */
  test("the lane is its EQ parts, each as bright as its knob", async ({ page }) => {
    await openShell(page, "/");
    await expect.poll(async () => (await lane(page)).length).toBeGreaterThan(0);
    const parts = new Set((await lane(page)).map((tile) => tile.part));
    expect(parts, "the lane is not drawn as its parts").toEqual(new Set(["low", "mid", "high", "grid"]));
    for (const tile of await lane(page)) {
      if (tile.part === "grid") {
        expect(gridOf(tile.src), "the grid layer does not carry the DJ's lines").not.toBe("---");
      } else {
        expect(gridOf(tile.src), `the ${tile.part} part draws the grid too`).toBe("---");
      }
    }

    const opacityOf = async (part: string) => {
      const found = (await lane(page)).filter((tile) => tile.part === part).map((tile) => Number(tile.opacity));
      return [Math.min(...found), Math.max(...found)];
    };
    await knobs(page, { eq_low: 1, eq_mid: 1, eq_high: 1, filter: 0 });
    await expect.poll(() => opacityOf("low")).toEqual([1, 1]);

    await knobs(page, { eq_low: 0, eq_mid: 1, eq_high: 1, filter: 0 });
    await expect.poll(() => opacityOf("low")).toEqual([GHOST, GHOST]);
    expect(await opacityOf("mid")).toEqual([1, 1]);
    expect(await opacityOf("high")).toEqual([1, 1]);
    expect(await opacityOf("grid"), "a killed low dimmed the grid").toEqual([1, 1]);

    // The filter, all the way down: the high part is gone, the low is not.
    await knobs(page, { eq_low: 1, eq_mid: 1, eq_high: 1, filter: -1 });
    await expect.poll(() => opacityOf("high")).toEqual([GHOST, GHOST]);
    expect((await opacityOf("low"))[0]).toBeGreaterThan(0.3);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Until the spectrum lands, the lane is one layer.** The parts are cut
   * from the spectrum; before it is measured there is nothing to cut, and a
   * lane of three parts would be three copies of one uncut picture.
   */
  test("a lane whose colour is pending is not split", async ({ page }) => {
    await openShell(page, "/", {}, {
      waveform_info: { ...ANSWERS.waveform_info, epoch: 1, colour_pending: true },
    });
    await expect.poll(async () => (await lane(page)).length).toBeGreaterThan(0);
    expect(new Set((await lane(page)).map((tile) => tile.part))).toEqual(new Set(["all"]));
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
