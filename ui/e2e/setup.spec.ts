/**
 * §54's functional presets: one gesture that configures many systems.
 *
 * > Create presets that configure multiple systems simultaneously. […] The
 * > user can override anything.
 *
 * djmanzo already had presets that configured *one* system — §7's workspaces
 * set the cockpit and stop at the edge of the screen. Everything else §54 lists
 * is a control somewhere else, and a DJ setting up for a wedding had to find
 * all of them.
 *
 * Which presets exist, what each sets and whether every name they use is real
 * is `dj_app::setup`, and is tested there against the tables that own each
 * field. These say the two things only a browser can.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

async function openSetups(page: Page) {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.locator(".setups")).toBeVisible();
}

const night = (page: Page, slug: string) =>
  page.locator(`.setups li[data-setup="${slug}"]`);

test.describe("§54's functional presets", () => {
  /**
   * **The load-bearing one: a preset says what it will do before it does it,
   * and the first press does nothing.**
   *
   * §54's presets touch six systems at once. The rule `dj_app::presets` states
   * about the small ones — *a preset that silently changes eight things is the
   * kind of feature people stop trusting* — applies with more force here, and
   * the only way to keep it is to make reading the list a separate act from
   * applying it.
   */
  test("reading a preset is not applying it", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await openSetups(page);

    const wedding = night(page, "wedding");
    await expect(wedding.locator(".will")).toHaveCount(0);

    await wedding.getByRole("button").click();
    const will = wedding.locator(".will li");
    await expect(will).toHaveCount(6);
    await expect(will.first()).toHaveText("Arrangement: Wedding / Event");
    await expect(wedding).toContainText("Pad pages: cues, saved, sampler");

    // Nothing has been applied: the shell never asked.
    const asked = await page.evaluate(
      () => (window as unknown as { __asked?: string[] }).__asked ?? [],
    );
    expect(
      asked.filter((cmd) => cmd === "apply_setup"),
      "reading the list applied the preset",
    ).toEqual([]);
    expect(thrown).toEqual([]);
  });

  /**
   * **And the second press reaches all six, including the two the shell owns.**
   *
   * Rust sets §25's layers, the pad pages, the assistant's posture and the
   * request page; the arrangement and the theme come back for the interface,
   * because a cockpit is resolved against a window only the shell has measured.
   * What only a browser can show is that the second half arrives — a preset
   * that set four of six and reported all six would be exactly the feature §54
   * asks not to build.
   */
  test("applying one reaches the arrangement and the theme as well", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await openSetups(page);

    const club = night(page, "club");
    await club.getByRole("button").click();
    await club.getByRole("button").click();

    // Rust's half, and that it was asked exactly once.
    const asked = await page.evaluate(
      () => (window as unknown as { __asked?: string[] }).__asked ?? [],
    );
    expect(asked.filter((cmd) => cmd === "apply_setup")).toHaveLength(1);

    // The shell's half: the theme was declared, not merely painted — §31
    // adapts on a timer and un-paints a choice nobody declared.
    await expect
      .poll(async () =>
        page.evaluate(
          () => (window as unknown as { __chosenTheme?: string }).__chosenTheme,
        ),
      )
      .toBe("pkg-industrial");

    // And it says what it did, after the fact as well as before.
    await expect(page.locator(".setups .did")).toContainText("Club");
    await expect(page.locator(".setups .did")).toContainText("Assistant: prepare");
    expect(thrown).toEqual([]);
  });

  /**
   * **The one preset with no look of its own says nothing about the theme.**
   *
   * Open format is the night djmanzo has no palette for, and §31's adaptation
   * is better placed to answer the room than a preset written in advance. A
   * line claiming a theme change that did not happen is what makes the other
   * five lines untrustworthy.
   */
  test("a preset with no theme does not claim one", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await openSetups(page);

    const open = night(page, "open-format");
    await open.getByRole("button").click();
    const will = open.locator(".will li");
    // Five fixed lines plus §16's pack, and no theme line. Open format is the
    // one occasion that names a knowledge pack and no palette, which makes it
    // the sharpest case for the rule: the list has to be the changes, not a
    // fixed shape with a gap in it.
    await expect(will).toHaveCount(6);
    await expect(open.locator(".will")).not.toContainText("Theme:");
    await expect(open.locator(".will")).toContainText("Knowledge: Open Format");
    expect(thrown).toEqual([]);
  });
});
