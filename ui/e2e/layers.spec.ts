/**
 * The waveform's semantic layers, and the rule that keeps the inventory honest.
 *
 * §25 asks for a "multilayer semantic visualization architecture" of twenty
 * named layers, and says the waveform should become instrumentation. The
 * inventory lives in `dj_render::layer` and is checked there: twenty entries,
 * unique names, and §57's rule that no two unrelated layers share a colour
 * meaning.
 *
 * What Rust cannot check is the other direction — that what is actually on
 * screen is in the list. A layer drawn without being declared would be a
 * twenty-first that nobody counted, and the count is the thing anyone reading
 * the status will quote. This is measurement rather than template scraping:
 * every overlay stamps the layer it is, and this reads them back off a rendered
 * page.
 */
import { expect, test } from "@playwright/test";

import layers from "./layers.json" with { type: "json" };
import snapshot from "./snapshot.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const DECLARED = new Set(layers.map((layer) => layer.name));

/**
 * The mix-out window the harness answers `waveform_info` with, and the record
 * it sits in. Kept beside the assertions rather than reached for out of the
 * stub, because what these tests measure is whether the lane draws *those*
 * frames — a test that read the same object the component read could not tell
 * a correct conversion from no conversion at all.
 */
const RECORD_FRAMES = 12_000_000;
const OPENS_FRAME = 10_800_000;
const CLOSES_FRAME = 11_600_000;

/**
 * Where an element sits inside its scrolling strip, in pixels of lane.
 *
 * Against the strip rather than the viewport because the strip is under a
 * transform that moves sixty times a second, and `offsetLeft` is rounded to a
 * whole pixel — which at four minutes of record in a lane is worth several
 * seconds of music.
 */
async function inStrip(page: import("@playwright/test").Page, selector: string) {
  return page.evaluate((sel) => {
    const el = document.querySelector(sel);
    const strip = el?.closest(".strip");
    if (!el || !strip) return null;
    const box = el.getBoundingClientRect();
    return { left: box.left - strip.getBoundingClientRect().left, width: box.width };
  }, selector);
}

test.describe("the waveform's layers", () => {
  /**
   * **Nothing is drawn that djmanzo has not declared.**
   *
   * The pair view is opened because that is where the most layers are on
   * screen at once — two lanes, cues, the loop band, the seam and its marks.
   */
  test("every layer on screen is one djmanzo declares", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Pair", exact: true }).click();
    await page.getByRole("button", { name: "Compare", exact: true }).click();
    await page.getByRole("button", { name: "Set up", exact: true }).click();

    const drawn = await page.evaluate(() =>
      Array.from(document.querySelectorAll("[data-layer]")).map((el) =>
        el.getAttribute("data-layer"),
      ),
    );
    expect(drawn.length, "no layer stamped itself at all").toBeGreaterThan(0);

    const undeclared = [...new Set(drawn)].filter(
      (name) => !DECLARED.has(name as string),
    );
    expect(undeclared, "a layer is drawn that djmanzo does not declare").toEqual(
      [],
    );
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * The two layers this brings: how much record is left, and what the mix
   * covers. Both answer questions §25 puts to the waveform — "what is about to
   * happen" and "what will happen if I do it" — that two lines and an
   * amplitude could not.
   */
  test("the runway and the seam region are drawn", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Pair", exact: true }).click();
    await page.getByRole("button", { name: "Compare", exact: true }).click();
    await page.getByRole("button", { name: "Set up", exact: true }).click();

    await expect(page.locator('[data-layer="runway"]').first()).toBeVisible();
    const seam = page.locator('.seam-band[data-layer="seam"]');
    await expect(seam.first()).toBeVisible();
    // It covers ground rather than being a line: the whole point of a region.
    const box = await seam.first().boundingBox();
    expect(box, "the seam region has no box").not.toBeNull();
    expect(box!.width).toBeGreaterThan(1);
  });

  /**
   * **A wash under an opaque waveform is not a layer.**
   *
   * The runway shipped with `z-index: 0` against tiles at `z-index: auto` that
   * come later in the DOM, so it painted underneath the waveform and was
   * invisible in the application — while this file's `toBeVisible` passed,
   * because Playwright asks whether an element has a box, not whether anything
   * can be seen of it. Found by looking at the running application; this is
   * the guard so it is not found that way twice.
   */
  test("every layer paints above the record it is about", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Pair", exact: true }).click();
    await page.getByRole("button", { name: "Compare", exact: true }).click();
    await page.getByRole("button", { name: "Set up", exact: true }).click();

    const order = await page.evaluate(() => {
      // Scoped to a waveform's own strip. `.mark` is also the top bar's Mark
      // button, and an unscoped query picked that up — a reminder that a class
      // name is not an identity, which is why the layers carry `data-layer`.
      // The lane that actually has a seam on it — the pair view's outgoing
      // one. The first `.strip` on the page is a deck's own lane, which has no
      // transition drawn on it and so nothing to compare.
      const strip = document.querySelector(".seam-band")?.closest(".strip");
      const depth = (selector: string) => {
        const el = strip?.querySelector(selector);
        if (!el) return null;
        const value = getComputedStyle(el).zIndex;
        return value === "auto" ? 0 : Number(value);
      };
      return {
        tile: depth(".tile"),
        runway: depth('[data-layer="runway"]'),
        seam: depth(".seam-band"),
        mark: depth('.mark[data-layer="seam"]'),
      };
    });

    expect(order.tile, "no tile to compare against").not.toBeNull();
    for (const [name, value] of Object.entries(order)) {
      if (name === "tile") continue;
      expect(value, `${name} is missing`).not.toBeNull();
      expect(
        value as number,
        `${name} paints under the waveform, so nothing can be seen of it`,
      ).toBeGreaterThan(order.tile as number);
    }
    // And the order among themselves: a wash under a position under a line.
    expect(order.runway as number).toBeLessThan(order.seam as number);
    expect(order.seam as number).toBeLessThan(order.mark as number);
  });

  /**
   * **The mix-out band is drawn at the frames Rust chose.**
   *
   * §25's `mix-out` layer, and the one thing a browser can prove about it: the
   * arithmetic that decides *where* a record can be left is `plan::mix_out`,
   * tested in Rust against the planner's own lengths and tail margin. What can
   * go wrong here is the conversion — drawing the closing frame as the left
   * edge, or forgetting to divide by the zoom — and either would put the band
   * somewhere plausible-looking and wrong.
   *
   * Measured as a fraction of the whole record so it holds at any zoom. The
   * record's width in pixels comes from the runway, whose right-hand edge is
   * the end of the file by construction.
   */
  test("the mix-out band lands where the record can be left", async ({
    page,
  }) => {
    await openShell(page, "/");

    const runway = await inStrip(page, '[data-layer="runway"]');
    expect(runway, "no runway to measure the record against").not.toBeNull();
    const record = runway!.left + runway!.width;

    const band = await inStrip(page, '[data-layer="mix-out"]');
    expect(band, "the mix-out layer is not drawn at all").not.toBeNull();

    expect(band!.left / record, "the band does not open where Rust said").toBeCloseTo(
      OPENS_FRAME / RECORD_FRAMES,
      2,
    );
    expect(band!.width / record, "the band does not close where Rust said").toBeCloseTo(
      (CLOSES_FRAME - OPENS_FRAME) / RECORD_FRAMES,
      2,
    );
    // It sits inside the runway, which is not a coincidence: a record is left
    // near its end, and the band saying *where* belongs inside the wash saying
    // *how long is left*.
    expect(band!.left).toBeGreaterThan(runway!.left);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The lane says when the grid under it is a guess.**
   *
   * The rasteriser has always faded beat lines by the grid's confidence, and
   * that fade cannot be read: at overview zoom the grid is suppressed entirely
   * for density, so faint and absent look identical and neither says whether
   * the analyser was unsure. This is the layer that says so.
   *
   * The fixture's records both have a certain grid, which is the state worth
   * having as the default — so the absence is asserted first, and then the
   * same lane is sent a deck djmanzo really can send: loaded, playing, and on
   * a grid Sync will not touch.
   */
  test("an uncertain grid is drawn as one", async ({ page }) => {
    await openShell(page, "/");
    await expect(page.locator('[data-layer="confidence"]')).toHaveCount(0);

    const unsure = {
      ...snapshot,
      decks: snapshot.decks.map((deck, index) =>
        index === 0 ? { ...deck, can_sync: false, grid_confidence: 0.31 } : deck,
      ),
    };
    await page.evaluate((state) => {
      const win = window as unknown as Record<string, unknown>;
      const id = (win.__handlers as Map<string, number>).get("snapshot");
      (win[`_${id}`] as (event: unknown) => void)({
        event: "snapshot",
        id: 0,
        payload: state,
      });
    }, unsure);

    const hatch = page.locator('[data-layer="confidence"]').first();
    await expect(hatch).toBeVisible();
    await expect(hatch).toHaveAttribute("title", /31% confidence/);

    // It covers the record and nothing more: the claim is about this file, not
    // about the lane it happens to be in.
    const runway = await inStrip(page, '[data-layer="runway"]');
    const strip = await inStrip(page, '[data-layer="confidence"]');
    expect(strip!.width).toBeCloseTo(runway!.left + runway!.width, 0);
    expect(strip!.left).toBeCloseTo(0, 0);

    // And it is above the record rather than under it -- the mistake the
    // runway shipped with, which `toBeVisible` could not see.
    const depths = await page.evaluate(() => {
      const el = document.querySelector('[data-layer="confidence"]');
      const strip = el?.closest(".strip");
      const z = (node: Element | null | undefined) => {
        if (!node) return null;
        const value = getComputedStyle(node).zIndex;
        return value === "auto" ? 0 : Number(value);
      };
      return { hatch: z(el), tile: z(strip?.querySelector(".tile")) };
    });
    expect(depths.hatch).not.toBeNull();
    expect(depths.hatch as number).toBeGreaterThan(depths.tile as number);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **And it is drawn over the whole record, which is the view it is for.**
   *
   * The band was written for the scrolling lane first, where it is almost never
   * on screen: a lane runs at a couple of hundred frames per pixel — two
   * seconds of record — so a window twenty beats from the end is invisible
   * until the playhead is already inside it, which answers "what is about to
   * happen" far too late to be worth anything. The overview is where a
   * whole-record fact belongs, and this is the assertion that keeps it there.
   */
  test("the overview draws the window over the whole record", async ({
    page,
  }) => {
    await openShell(page, "/");

    const band = await page.evaluate(() => {
      const el = document.querySelector('.overview [data-layer="mix-out"]');
      const strip = el?.closest(".overview");
      if (!el || !strip) return null;
      const box = el.getBoundingClientRect();
      const whole = strip.getBoundingClientRect();
      return {
        left: (box.left - whole.left) / whole.width,
        width: box.width / whole.width,
      };
    });
    expect(band, "the overview draws no mix-out band").not.toBeNull();
    expect(band!.left).toBeCloseTo(OPENS_FRAME / RECORD_FRAMES, 2);
    expect(band!.width).toBeCloseTo((CLOSES_FRAME - OPENS_FRAME) / RECORD_FRAMES, 2);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** And the inventory the interface reads is the one Rust publishes. */
  test("the inventory is twenty, and the built ones are named", async ({
    page,
  }) => {
    await openShell(page, "/");
    const published = await page.evaluate(
      async () =>
        await (
          window as unknown as {
            __TAURI_INTERNALS__: {
              invoke: (cmd: string, args: unknown) => Promise<unknown>;
            };
          }
        ).__TAURI_INTERNALS__.invoke("waveform_layers", {}),
    );
    expect(published).toHaveLength(20);
    const built = (published as { name: string; drawn: string }[])
      .filter((layer) => layer.drawn !== "nowhere")
      .map((layer) => layer.name);
    expect(built).toContain("runway");
    expect(built).toContain("seam");
    expect(built).toContain("mix-out");
    expect(built).toContain("confidence");
    // §27's ghost, which is the twelfth: the `suggestion` layer §25 reserves
    // for "what djmanzo would do, drawn as a ghost rather than as a fact".
    expect(built).toContain("suggestion");
    expect(built).toHaveLength(12);
  });
});
