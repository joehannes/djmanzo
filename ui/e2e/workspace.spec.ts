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

import { openShell } from "./shell";
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
