/**
 * §7's workspace presets, from the picker.
 *
 * djmanzo held three arrangements in Rust and offered no way to choose one, so
 * twenty-three named starting points were a list nobody could reach. The Rust
 * tests say the table is right; these say pressing a name does what the name
 * says — which is the whole of what the feature is.
 *
 * The four presets the harness answers with are copied from
 * `dj_app::cockpit::workspaces()` and a Rust test
 * (`the_harness_and_rust_agree_about_the_presets`) fails if they drift, so
 * "Open Format gives you four decks" is a claim about the application rather
 * than about this file.
 */
import { expect, test } from "@playwright/test";

import { compositions, errorsThrown, openShell } from "./shell";
// The same table the interface applies, so "wearing the booth theme" is
// measured against djmanzo's own colours rather than against a hex typed here.
import { paletteFor } from "../src/controls/themes/colors";

const DECK = "section.deck[data-deck]";
const PICKER = 'select[aria-label="Workspace"]';

test.describe("the workspace picker", () => {
  /**
   * The load-bearing one: a preset opens its panels, sets its deck count, and
   * is written down.
   *
   * All three parts matter and each has failed on its own. A preset drawn and
   * never saved looks identical until djmanzo is reopened — the defect §89's
   * four-deck configuration found — and a preset that sets the deck count
   * without opening its panels is the "picker does nothing" reading that made
   * §7 worth doing.
   */
  test("choosing a preset opens what it names and remembers it", async ({ page }) => {
    await openShell(page, "/");
    await expect(page.locator(DECK)).toHaveCount(2);
    await expect(page.locator('.surface[data-surface="library"]')).toHaveCount(0);

    await page.locator(PICKER).selectOption("Open Format");

    await expect(
      page.locator(DECK),
      "Open Format asks for four decks and the shell drew two",
    ).toHaveCount(4);
    await expect(
      page.locator('.surface[data-surface="library"]'),
      "Open Format places the collection along the bottom and nothing opened",
    ).toBeVisible();
    await expect(page.locator('.surface[data-surface="next"]')).toBeVisible();

    const saved = await page.evaluate(
      () =>
        (window as unknown as { __saved?: { name: string; decks: number }[] })
          .__saved ?? [],
    );
    expect(
      saved.at(-1)?.name,
      "the arrangement changed on screen and was never written -- it would be " +
        "gone the next time djmanzo opened",
    ).toBe("Open Format");
    expect(saved.at(-1)?.decks).toBe(4);
  });

  /**
   * §7: *starting points, not rigid identities. Every preset should remain
   * editable.*
   *
   * So the panel a DJ opens after choosing one joins the preset rather than
   * being thrown away by it or throwing it away — which is what "editable"
   * has to mean for something with no Save button.
   */
  test("a preset is a starting point and stays editable", async ({ page }) => {
    await openShell(page, "/");
    await page.locator(PICKER).selectOption("Open Format");
    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();

    await page.getByRole("button", { name: "Booth", exact: true }).click();

    await expect(
      page.locator('.surface[data-surface="booth"]'),
      "a panel opened after a preset did not open",
    ).toBeVisible();
    await expect(
      page.locator('.surface[data-surface="library"]'),
      "opening a panel threw away what the preset had opened",
    ).toBeVisible();

    const saved = await page.evaluate(
      () =>
        (window as unknown as {
          __saved?: { name: string; surfaces: { surface: string }[] }[];
        }).__saved ?? [],
    );
    const last = saved.at(-1);
    expect(
      last?.surfaces.map((p) => p.surface).sort(),
      "the edit was drawn but the preset's own panels were dropped from what " +
        "was written",
    ).toEqual(["booth", "library", "next"]);
    expect(
      last?.name,
      "editing a preset renamed the workspace, so the picker would stop " +
        "showing which one the DJ is on",
    ).toBe("Open Format");
  });

  /**
   * A preset still arrives when djmanzo cannot write it down.
   *
   * The shell applies an arrangement optimistically and corrects it from the
   * round trip, which means the press works before the filesystem answers and
   * still works when the filesystem refuses — a locked profile, a read-only
   * home, a full disk. Undoing the DJ's press because a preferences file could
   * not be written is the failure this posture exists to avoid, and until this
   * test the branch that holds it was never once executed.
   */
  test("a preset the disk refuses still arrives", async ({ page }) => {
    await openShell(page, "/", {}, { set_cockpit_workspace: "reject" });

    await page.locator(PICKER).selectOption("Open Format");

    await expect(
      page.locator(DECK),
      "a preset that could not be written was undone under the DJ",
    ).toHaveCount(4);
    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();
  });

  /**
   * The density is the difference between two presets that place nothing.
   *
   * "Minimal" and "Laptop Compact" both open the decks alone; the only thing
   * either does is choose how much fits on the screen. The stored workspace
   * has carried a density since the cockpit existed and nothing ever applied
   * it, so before the picker those two were the same preset with two names.
   *
   * Measured on the root custom property because that is the one number every
   * other measurement in the interface is in `em` of.
   */
  test("a preset that names a density wears it", async ({ page }) => {
    await openShell(page, "/");
    // Taller than the top band's floor, so the window fitting has chosen
    // Relaxed and the preset has to overrule a decision rather than agree with
    // one. `openShell` pins the viewport to the window djmanzo opens at, so
    // this comes after it.
    await page.setViewportSize({ width: 1280, height: 1600 });
    const scale = () =>
      page.evaluate(() =>
        Number(
          getComputedStyle(document.documentElement).getPropertyValue("--density"),
        ),
      );
    await expect
      .poll(scale, { message: "the window fitting never reached the top band" })
      .toBeCloseTo(1.15, 2);

    await page.locator(PICKER).selectOption("Laptop Compact");

    expect(
      await scale(),
      "Laptop Compact asks for Ultra Dense and the interface stayed at the " +
        "size the window fitted",
    ).toBeCloseTo(0.8, 2);
  });

  /**
   * A preset that names a theme wears it; one that does not, leaves it alone.
   *
   * The second half is the part worth a test. A DJ who chose a theme and then
   * picked "Open Format" should still have their theme — a picker that resets
   * the colours every time it is used is one a DJ stops using mid-set.
   */
  test("a preset changes the theme only when it names one", async ({ page }) => {
    await openShell(page, "/");
    // A theme package is not an attribute on the document: it is the palette
    // written onto the root as custom properties, so that is what is read.
    const accent = () =>
      page.evaluate(() =>
        getComputedStyle(document.documentElement)
          .getPropertyValue("--accent")
          .trim(),
      );
    const before = await accent();
    expect(before, "the shell opened with no palette on the root").not.toBe("");

    await page.locator(PICKER).selectOption("Open Format");
    expect(
      await accent(),
      "a preset with no opinion about the theme changed it anyway -- a picker " +
        "that resets the colours every time is one a DJ stops using mid-set",
    ).toBe(before);

    await page.locator(PICKER).selectOption("High Contrast");
    expect(
      await accent(),
      "High Contrast names the booth theme and the colours did not change",
    ).toBe(paletteFor("pkg-booth", "dark")["--accent"]);

    // And the choice was declared, not only painted. §31 reads the night and
    // adapts the theme on a tick; a preset that paints its colours without
    // saying so is overruled by the next tick, which is what "High Contrast"
    // did for four seconds under Xvfb before going back to green.
    expect(
      await page.evaluate(
        () => (window as unknown as { __chosenTheme?: string }).__chosenTheme,
      ),
      "the picker painted the theme and never told djmanzo, so the night " +
        "adaptation will take it straight back",
    ).toBe("pkg-booth");
  });

  /** The picker says what the chosen one is for, not only what it is called. */
  test("the picker says what the arrangement is for", async ({ page }) => {
    await openShell(page, "/");

    await page.locator(PICKER).selectOption("Open Format");

    await expect(page.locator(".preset-about")).toHaveText(
      "Four decks and the whole collection, for a night that goes anywhere.",
    );
  });
});

/**
 * §5B: an arrangement changes what a deck is made of, not only which panels
 * are open.
 *
 * > Club mode: large central stacked waveforms, compact decks. […] Laptop
 * > compact mode: dense controls optimized for limited screen height.
 *
 * Which arrangement names which composition, and that every name it uses is one
 * that ships, is `dj_app::cockpit` and is tested there against
 * `layout::builtin()`. This says the thing only a browser can: that pressing
 * the preset actually rebuilds the deck.
 */
test.describe("§5B's deck composition", () => {
  /**
   * **The load-bearing one: a preset that names a composition declares it, and
   * does not overwrite what the arrangement itself states.**
   *
   * Until this field an arrangement changed which panels were open, the deck
   * count, the density and the theme, and left the deck at whatever the DJ last
   * chose — so the feature is that the composition is applied *and told to
   * djmanzo*, the same pair the theme needs and for the same reason.
   *
   * The second half is a defect this test found. A layout carries its own deck
   * count and density, and applying one silently overwrote both: `Laptop
   * Compact` says Ultra Dense and the `Performance` composition it names says
   * 0.85, so naming a composition moved a band the arrangement had stated. A
   * layout named by an arrangement is *what a deck is made of*; how many there
   * are and how tightly they are packed stay the arrangement's to say.
   */
  test("a preset that names a deck composition declares it and keeps its own band", async ({
    page,
  }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");

    await page.locator(PICKER).selectOption("Laptop Compact");

    // Told, not only drawn. Without this the composition is painted and
    // forgotten, which is the failure the theme had before `theme_chosen`.
    await expect
      .poll(() =>
        page.evaluate(
          () => (window as unknown as { __asked?: string[] }).__asked ?? [],
        ),
      )
      .toContain("choose_layout");

    // And the arrangement's own band survived the composition it named.
    await expect
      .poll(() =>
        page.evaluate(() =>
          Number(document.documentElement.style.getPropertyValue("--density")),
        ),
      )
      .toBeCloseTo(0.8, 2);
    expect(thrown).toEqual([]);
  });

  /**
   * **An arrangement that names no composition does not touch the deck.**
   *
   * Most of §7's arrangements name none, because rebuilding the deck under a DJ
   * who only asked for the browser is the surprise §78 forbids. The absence has
   * to be real, not merely untested.
   */
  test("a preset with no composition leaves the deck alone", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");

    await page.locator(PICKER).selectOption("Perform");
    const asked = await page.evaluate(
      () => (window as unknown as { __asked?: string[] }).__asked ?? [],
    );
    expect(
      asked.filter((cmd) => cmd === "choose_layout"),
      "an arrangement that names no composition rebuilt the deck anyway",
    ).toEqual([]);
    expect(thrown).toEqual([]);
  });

  /**
   * **The load-bearing pair: §5B's two named compositions build two different
   * decks, and each is the other's control.**
   *
   * > Scratch mode: jog surfaces and turntable-oriented controls expand. […]
   * > Stem performance mode: large stem-aware waveform and stem controls become
   * > first-class.
   *
   * These are the only two compositions that change a control rather than how
   * much is on screen, and both travel as a prop on a placement — a number on
   * `deck.jog` and a flag on `deck.stems`. A prop is the one thing in the
   * layout format that can be set, serialised, resolved, stored in a golden
   * file and still reach nothing: every Rust test would agree the tree is
   * right, and the deck would look exactly as it did before. So the assertion
   * is measured off the screen.
   *
   * The trees come from `compositions.json`, which Rust blesses out of
   * `layout::builtin()` — the deck is built from what `layout_tree` answers,
   * and a tree typed out here would be a third description of a deck.
   */
  test("the scratch composition puts a platter on the deck and leaves the stems folded", async ({
    page,
  }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, { layout_tree: compositions.Scratch });

    const platter = page.locator(`${DECK} .platter`).first();
    await expect(platter).toBeVisible();
    // 200 px, as `layout::builtin()` asks for, against the deck's usual 80.
    // A range rather than the number because the density token scales it and
    // an arrangement may state its own.
    const width = await platter.evaluate(
      (el) => el.getBoundingClientRect().width,
    );
    expect(
      width,
      "the scratch deck's wheel is the ordinary nudge target, so `size` reached nothing",
    ).toBeGreaterThan(150);

    // The control half: this composition says nothing about the stems, and a
    // flag that unfolded them regardless would pass the other test alone.
    await expect(
      page.locator("[data-stems-open]").first(),
    ).toHaveAttribute("data-stems-open", "false");
    expect(thrown).toEqual([]);
  });

  test("the stem composition opens the stem module and leaves the wheel alone", async ({
    page,
  }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, { layout_tree: compositions["Stem Performance"] });

    await expect
      .poll(() =>
        page
          .locator("[data-stems-open]")
          .first()
          .getAttribute("data-stems-open"),
      )
      .toBe("true");

    const platter = page.locator(`${DECK} .platter`).first();
    await expect(platter).toBeVisible();
    const width = await platter.evaluate(
      (el) => el.getBoundingClientRect().width,
    );
    expect(
      width,
      "the stem deck grew a platter it never asked for",
    ).toBeLessThan(150);
    expect(thrown).toEqual([]);
  });
});
