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
import { errorsThrown, openShell } from "./shell";

const DECLARED = new Set(layers.map((layer) => layer.name));

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
  });
});
