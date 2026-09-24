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
/** The other end: where a mix *into* the fixture's record could begin. */
const IN_OPENS_FRAME = 300_000;
const IN_CLOSES_FRAME = 2_400_000;

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
   * **§75's three: the trajectory, the breakdowns in it and the drops out of
   * them.**
   *
   * These were `Drawn::Nowhere` in §25's table for the life of that table,
   * each with "the analysis does not exist" beside it. It does now, and the
   * only thing worth asserting here is the half Rust cannot: that the marks
   * land where the record puts them. The harness's record thins out in its
   * third quarter and comes back at three quarters through, so the band and
   * the line have a position this test can check rather than a mere presence.
   */
  test("the trajectory, the breakdown and the drop are drawn on the overview", async ({
    page,
  }) => {
    await openShell(page, "/");

    const overview = page.locator(".overview").first();
    await expect(overview).toBeVisible();
    const box = await overview.boundingBox();
    expect(box, "the overview has no box").not.toBeNull();

    // The curve, as columns. One per window, and the fixture has four.
    const columns = overview.locator('[data-layer="energy"]');
    await expect(columns).toHaveCount(4);

    // The breakdown covers the record's third quarter.
    const thin = overview.locator('[data-layer="breakdowns"]');
    await expect(thin).toHaveCount(1);
    const band = await thin.boundingBox();
    expect(band).not.toBeNull();
    const at = (pixels: number) => (pixels - box!.x) / box!.width;
    expect(
      at(band!.x),
      "the breakdown band does not start where the record thins out",
    ).toBeCloseTo(0.5, 1);
    expect(band!.width / box!.width).toBeCloseTo(0.25, 1);

    // And the drop is a line where it comes back.
    const drop = overview.locator('[data-layer="drops"]');
    await expect(drop).toHaveCount(1);
    const mark = await drop.boundingBox();
    expect(mark).not.toBeNull();
    expect(
      at(mark!.x),
      "the drop is not marked where the kick returns",
    ).toBeCloseTo(0.75, 1);

    // **And they are drawn over the tile rather than under it.** The first
    // version of these three sat before the image in the DOM, which on an
    // opaque tile means nothing of them reaches the screen -- the same mistake
    // the runway made once, and one `toBeVisible` cannot see, because it asks
    // whether an element has a box rather than whether anything can be seen of
    // it. This is the assertion that would have caught it.
    const above = await overview.evaluate((box) => {
      const tile = box.querySelector("img");
      const depth = (el: Element | null) => {
        if (!el) return null;
        const value = getComputedStyle(el).zIndex;
        return value === "auto" ? 0 : Number(value);
      };
      const order = Array.from(box.children);
      const paints = (el: Element | null) =>
        el === null
          ? null
          : { z: depth(el), at: order.indexOf(el) };
      return {
        tile: paints(tile),
        energy: paints(box.querySelector('[data-layer="energy"]')),
        breakdown: paints(box.querySelector('[data-layer="breakdowns"]')),
        drop: paints(box.querySelector('[data-layer="drops"]')),
      };
    });
    for (const [what, painted] of Object.entries(above)) {
      if (what === "tile" || painted === null) continue;
      const tile = above.tile!;
      expect(
        painted.z > tile.z || (painted.z === tile.z && painted.at > tile.at),
        `the ${what} layer paints under the waveform it is about, so none of ` +
          `it reaches the screen`,
      ).toBe(true);
    }
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **§25's `vocal`, the eighteenth layer.**
   *
   * `Drawn::Nowhere` for the life of that table with *the analysis does not
   * exist* beside it, while djmanzo shipped a separator that pulls a vocal
   * stem out of a record with no model and no download. `dj_analysis::voice`
   * measures the same thing without reconstructing it.
   *
   * The half Rust cannot check is here: that a window nobody measured draws
   * **nothing** rather than an empty strip, and that the strip's strength
   * follows the reading. The fixture's four windows are `null`, 0.3, 0.25 and
   * 0.02 — one unmeasured, two carrying a voice, one with barely any — so a
   * view that drew a segment per section, or drew them all alike, fails.
   */
  test("the vocal layer is drawn where the voice is, and nowhere else", async ({
    page,
  }) => {
    await openShell(page, "/");

    const overview = page.locator(".overview").first();
    await expect(overview).toBeVisible();
    const box = await overview.boundingBox();
    expect(box, "the overview has no box").not.toBeNull();

    // Three of four: the unmeasured window is absent, not drawn at zero.
    const strips = overview.locator('[data-layer="vocal"]');
    await expect(strips).toHaveCount(3);

    // The first drawn one is the second window, a quarter of the way in.
    const first = await strips.first().boundingBox();
    expect(first).not.toBeNull();
    expect(
      (first!.x - box!.x) / box!.width,
      "the voice is drawn before it enters",
    ).toBeCloseTo(0.25, 1);

    // And the strength follows the reading rather than being flat: the window
    // at 0.02 is far fainter than the one at 0.3.
    const opacities = await strips.evaluateAll((els) =>
      els.map((el) => Number(getComputedStyle(el).opacity)),
    );
    expect(opacities).toHaveLength(3);
    expect(
      opacities[0],
      "a window carrying a voice is not drawn at full strength",
    ).toBeGreaterThan(0.9);
    expect(
      opacities[2],
      "a window with barely any voice is drawn as loudly as one full of it",
    ).toBeLessThan(0.2);

    // Over the tile, like every other overview layer -- the mistake this file
    // records being made twice.
    const above = await overview.evaluate((root) => {
      const order = Array.from(root.children);
      const depth = (el: Element | null) => {
        if (!el) return null;
        const value = getComputedStyle(el).zIndex;
        return value === "auto" ? 0 : Number(value);
      };
      const tile = root.querySelector("img");
      const strip = root.querySelector('[data-layer="vocal"]');
      return {
        tile: { z: depth(tile), at: order.indexOf(tile!) },
        strip: { z: depth(strip), at: order.indexOf(strip!) },
      };
    });
    expect(
      above.strip.z! > above.tile.z! ||
        (above.strip.z === above.tile.z && above.strip.at > above.tile.at),
      "the vocal strip paints under the waveform it is about",
    ).toBe(true);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **§25's `stems`, the nineteenth layer.**
   *
   * The same pass that answers `vocal` answers this, and answers it as a
   * partition: the four shares of a window add to the whole of it. What a
   * browser can prove that Rust cannot is that the band **says which** — one
   * colour per window, the colour of the fader that mutes the current it
   * names.
   *
   * The fixture hands three measured windows carried by three different
   * currents, so a band that drew one colour for the whole record, or read
   * the wrong index of the four, fails here.
   */
  test("the stems band names which current carries each stretch", async ({
    page,
  }) => {
    await openShell(page, "/");

    const overview = page.locator(".overview").first();
    await expect(overview).toBeVisible();

    // The band itself. §116's arrival notches are the same layer drawn in a
    // second form — where each current comes and goes — and `ahead.spec.ts`
    // holds those.
    const bands = overview.locator('.current[data-layer="stems"]');
    // Three, not four: the unmeasured window is absent rather than guessed at.
    await expect(bands).toHaveCount(3);
    expect(
      await bands.evaluateAll((els) =>
        els.map((el) => el.getAttribute("data-current")),
      ),
      "the band is not following the loudest current",
    ).toEqual(["vocal", "other", "drums"]);

    // Three different colours actually reach the screen. The four tokens are
    // held apart in every palette by §30's rule, so if these resolve to one
    // value the band is saying nothing whatever its attributes claim.
    const painted = await bands.evaluateAll((els) =>
      els.map((el) => getComputedStyle(el).backgroundColor),
    );
    expect(new Set(painted).size, `three currents painted ${painted}`).toBe(3);

    // And the hover says all four rather than only the winner: which current
    // is loudest is a summary, and a DJ deciding what to mute wants the mix.
    expect(await bands.first().getAttribute("title")).toContain("Drums 25%");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The waveform asks when something changed, and not otherwise.**
   *
   * `waveform_info`'s own doc says why it is not on the 60 Hz snapshot:
   * *"Sixty times a second for a curve that changes twice a track would be the
   * snapshot pump carrying furniture."* The interface did it anyway, and
   * nothing here could see it.
   *
   * The lane and the overview each ran an `$effect` reading `deck.analysis`
   * and `deck.length_frames`. Those reads look like fine-grained dependencies
   * and are not: `App.svelte` does `snapshot = next` on every frame, so every
   * deck is a new `$state` proxy and every read through it is a new signal.
   * Measured before the guard went in: **forty** calls for ten frames of
   * ordinary playback, two components over two decks, every frame.
   *
   * Both halves are asserted, because either alone is passed by something
   * broken. A component that never asks passes the first; one that asks on
   * every frame passes the second.
   */
  test("the waveform asks on a change and not on every frame", async ({
    page,
  }) => {
    await openShell(page, "/");
    await expect(page.locator(".overview").first()).toBeVisible();
    // The calls at startup are two components over two decks and are not what
    // this measures.
    await page.waitForTimeout(700);

    const reset = () =>
      page.evaluate(() => {
        (window as unknown as Record<string, unknown>).__asked = [];
      });
    const asked = () =>
      page.evaluate(
        () =>
          ((window as unknown as { __asked: string[] }).__asked ?? []).filter(
            (cmd) => cmd === "waveform_info",
          ).length,
      );

    // Ten frames of ordinary playback, spaced so they are not batched into
    // one — which is how the pump really delivers them, and how the first
    // version of this measurement missed the defect entirely.
    await reset();
    await page.evaluate(async () => {
      const win = window as unknown as {
        __lastState: { decks: { position_frames: number }[] };
        __emit: (next: unknown) => void;
      };
      for (let i = 0; i < 10; i += 1) {
        const next = structuredClone(win.__lastState);
        next.decks[0].position_frames += 2048;
        win.__emit(next);
        await new Promise((resolve) => setTimeout(resolve, 30));
      }
    });
    await page.waitForTimeout(400);
    expect(
      await asked(),
      "the playhead moving makes the waveform re-ask Rust for bands that " +
        "are beats counted from a grid the playhead does not touch",
    ).toBe(0);

    // And §25's saved loops: a loop kept mid-set changes nothing else on the
    // frame, so `marks` is the only thing that can say the lane should look
    // again. Two calls, which is the lane and the overview of that one deck.
    await reset();
    await page.evaluate(() => {
      const win = window as unknown as {
        __lastState: { decks: { marks: number }[] };
        __emit: (next: unknown) => void;
      };
      const next = structuredClone(win.__lastState);
      next.decks[0].marks += 1;
      win.__emit(next);
    });
    await expect
      .poll(asked, {
        message: "a loop kept mid-set never reaches the lane",
      })
      .toBe(2);
    expect(errorsThrown(page)).toEqual([]);
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
   * **The mix-in band is drawn at the other end, at the frames Rust chose.**
   *
   * §25's `mix-in` layer. The arithmetic — the first whole phrase, and the
   * record's own first drop — is `plan::mix_in` and is tested in Rust. What a
   * browser can prove is the conversion and, more usefully, that the two
   * windows are drawn as two: they share a colour on purpose, because both are
   * djmanzo saying *could* about the same mix from opposite ends, and a lane
   * that drew one of them twice would look exactly right at a glance.
   */
  test("the mix-in band lands where the record can be joined", async ({ page }) => {
    await openShell(page, "/");

    const runway = await inStrip(page, '[data-layer="runway"]');
    expect(runway, "no runway to measure the record against").not.toBeNull();
    const record = runway!.left + runway!.width;

    const band = await inStrip(page, '[data-layer="mix-in"]');
    expect(band, "the mix-in layer is not drawn at all").not.toBeNull();

    expect(band!.left / record, "the band does not open where Rust said").toBeCloseTo(
      IN_OPENS_FRAME / RECORD_FRAMES,
      2,
    );
    expect(band!.width / record, "the band does not close where Rust said").toBeCloseTo(
      (IN_CLOSES_FRAME - IN_OPENS_FRAME) / RECORD_FRAMES,
      2,
    );

    // And it is at the *other* end from the mix-out band, which is the half a
    // colour they share cannot say for itself.
    const out = await inStrip(page, '[data-layer="mix-out"]');
    expect(out, "no mix-out band to compare against").not.toBeNull();
    expect(
      band!.left + band!.width,
      "the two windows overlap, so one record is claiming it can be joined where it can be left",
    ).toBeLessThan(out!.left);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Saved loops are drawn where they would fire, and say which pad fires
   * them.**
   *
   * §25 has listed `saved-loops` since the table existed and nothing drew them,
   * which made saving one a thing a DJ could do and never see: eight slots, a
   * recall per slot, and no way to know where any of them were without pressing
   * one.
   *
   * The slot number is the assertion that matters: a band in the right place
   * with the wrong number on it sends a DJ to the wrong pad. That the loops
   * arrive in slot order is `saved_loops_of`'s and is tested in Rust — a stub
   * that delivered them sorted could only prove the stub was sorted.
   */
  test("every saved loop is drawn, with the pad that recalls it", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");

    const bands = page.locator('.strip [data-layer="saved-loops"]').first();
    await expect(bands).toBeVisible();
    const slots = await page.evaluate(() =>
      [...document.querySelectorAll('.strip [data-layer="saved-loops"]')]
        .slice(0, 2)
        .map((el) => el.textContent?.trim()),
    );
    expect(slots, "the bands do not carry the slot that recalls them").toEqual(["1", "3"]);

    // And the first one is where Rust put it, not where it arrived.
    const runway = await inStrip(page, '[data-layer="runway"]');
    const record = runway!.left + runway!.width;
    const band = await inStrip(page, '[data-layer="saved-loops"]');
    expect(band!.left / record, "the band is not where the loop was saved").toBeCloseTo(
      4_000_000 / RECORD_FRAMES,
      2,
    );
    expect(thrown).toEqual([]);
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

  /**
   * And the inventory the interface reads is the one Rust publishes.
   *
   * Twenty-one: §25's own twenty, plus `transients`, which §75 asked for and
   * §25 never named a layer for. `layer.rs` holds the directive's twenty by
   * slug and requires anything beyond them to say which section wanted it, so
   * the number here is a consequence rather than a second opinion.
   */
  test("the inventory is the one Rust publishes, and the built ones are named", async ({
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
    expect(published).toHaveLength(21);
    const built = (published as { name: string; drawn: string }[])
      .filter((layer) => layer.drawn !== "nowhere")
      .map((layer) => layer.name);
    expect(built).toContain("runway");
    expect(built).toContain("seam");
    expect(built).toContain("mix-out");
    expect(built).toContain("confidence");
    // §27's ghost, which was the twelfth: the `suggestion` layer §25 reserves
    // for "what djmanzo would do, drawn as a ghost rather than as a fact".
    expect(built).toContain("suggestion");
    // §75's three, which were the last of §25's list waiting on an analysis
    // nobody had written rather than on a decision nobody had made.
    expect(built).toContain("energy");
    expect(built).toContain("breakdowns");
    expect(built).toContain("drops");
    // §25's `mix-in`: the sixteenth, and the other end of the one window
    // djmanzo had already been drawing. It was listed as needing an analysis
    // nobody had written, and what it actually needed was `plan::mix_out` read
    // from the other end plus the first drop `energy::trajectory` already
    // finds.
    expect(built).toContain("mix-in");
    // §25's `saved-loops`: the seventeenth, and the one that needed no
    // analysis at all — `dj_library::StoredLoop` has held them per track for as
    // long as the library has, and nothing carried them to the waveform.
    expect(built).toContain("saved-loops");
    // §25's `vocal`: the eighteenth, and one more that was waiting on nobody.
    // `dj_stems::hpss` has separated a vocal stem with no model and no
    // download for as long as it has existed; what was missing was anything
    // asking it where that voice was, which `dj_analysis::voice` now does
    // without reconstructing the stem at all.
    expect(built).toContain("vocal");
    // §25's `stems`: the nineteenth, and the last one that was waiting on an
    // analysis rather than on hardware. The same pass answers both, which is
    // why they arrived a commit apart rather than a year.
    expect(built).toContain("stems");
    // §75's `transients`: the twentieth, and the first layer here that §25
    // never named. Its reason for being absent was *a curve at a resolution
    // the overview cannot show*, which confused transients with their
    // density: a count over a window is exactly what survives being drawn
    // small. `dj_analysis::strikes` reads it off the onset curve the tempo
    // estimator already builds.
    expect(built).toContain("transients");
    expect(built).toHaveLength(20);
  });
});

/**
 * §75's *transient density*, the last of its nine.
 *
 * How often something is struck, which is not how much of the moment is
 * percussive and not how loud it is. `dj_analysis::strikes` measures it and is
 * tested there; this says the one thing only a browser can — that the band is
 * **drawn**, that it follows the density rather than the energy, and that a
 * window nobody measured draws nothing.
 */
test.describe("§75's transient density", () => {
  /**
   * The bands on the *first* overview.
   *
   * Two decks are on screen and each draws one, so an unscoped selector counts
   * every band twice -- which is how the first version of this read six for
   * three measured windows.
   */
  const bandsOf = (page: import("@playwright/test").Page) =>
    page.locator(".overview").first().locator('[data-layer="transients"]');

  test("draws a band that follows the density, not the energy", async ({ page }) => {
    await openShell(page, "/");

    const bands = bandsOf(page);
    await expect(bands.first()).toBeVisible();

    /*
      The fixture's three measured windows are deliberately anti-correlated
      with energy: the *quietest* window is the densest, which is a shaker
      under a pad. So a component that drew the energy curve into this band
      would put the strongest mark on the wrong window, and this reads the
      opacities back to catch exactly that.
    */
    const strengths = await bands.evaluateAll((nodes) =>
      nodes.map((node) => ({
        left: Math.round(parseFloat((node as HTMLElement).style.left)),
        opacity: Number(getComputedStyle(node).opacity),
      })),
    );
    expect(strengths.length, "one band per measured window").toBe(3);

    const strongest = strengths.reduce((a, b) => (b.opacity > a.opacity ? b : a));
    const weakest = strengths.reduce((a, b) => (b.opacity < a.opacity ? b : a));
    expect(
      strongest.opacity,
      "the densest window should be the darkest band",
    ).toBeGreaterThan(weakest.opacity + 0.3);
    // 11 strikes a second against a ceiling of 12.
    expect(strongest.opacity).toBeGreaterThan(0.8);

    /*
      And it has to be the *right* window, which is the assertion that does the
      work. Without it a component drawing the energy curve into this band
      passes everything above: the fixture's energies are 1.0, 0.35 and 0.95,
      so a strongest-versus-weakest check is satisfied by the wrong answer.

      The fixture's densest window is the breakdown at 6,000,000 -- quiet and
      busy, a shaker under a pad -- which is the second of the three measured
      windows over a 12,000,000-frame record, so its band sits at 50%.
    */
    expect(
      strongest.left,
      "the darkest band is not over the densest window, so this is drawing something else",
    ).toBe(50);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A window nobody measured draws nothing, rather than a silent one.**
   *
   * Absent and *nothing was struck* are different answers, and a band that
   * filled the gaps would claim the second when djmanzo only has the first —
   * the same rule §25's vocal strip follows.
   */
  test("an unmeasured window is absent rather than empty", async ({ page }) => {
    await openShell(page, "/");
    const bands = bandsOf(page);
    // The fixture has four windows and one of them has `strikes: null`.
    await expect(bands).toHaveCount(3);
    expect(errorsThrown(page)).toEqual([]);
  });
});
