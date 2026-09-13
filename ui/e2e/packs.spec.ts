/**
 * §16's domain knowledge packs, as a format.
 *
 * > Make DJ knowledge extensible. At minimum define packs for: General,
 * > Beginner, Open Format, House, Techno, […] **Do not hard-code this logic
 * > into UI components.**
 *
 * What a pack contains, which tables it selects from and that it narrows what
 * the coach teaches are all `dj_assistant::pack` and `dj_assistant::coach`, and
 * are tested there against the tables that own each field. The rule about UI
 * components is held by a Rust test that reads every Svelte file. These say the
 * two things only a browser can.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

async function openPacks(page: Page) {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.locator(".packs")).toBeVisible();
}

const pack = (page: Page, id: string) =>
  page.locator(`.packs li[data-pack="${id}"]`);

test.describe("§16's knowledge packs", () => {
  /**
   * **The load-bearing one: every word in the picker came from Rust, including
   * the number that says what a pack costs.**
   *
   * §16's own instruction is not to hard-code this into UI components, and the
   * failure it guards against is silent: a picker that spelled its own eight
   * would render perfectly while offering a curriculum the coach does not
   * teach from. The `teaches` count is the sharpest evidence available in a
   * browser — it is derived from the technique catalogue on the other side of
   * the workspace, so a number on screen that matches the fixture cannot have
   * been written in the interface.
   */
  test("the packs and their reach are read, not written here", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await openPacks(page);

    const asked = await page.evaluate(
      () => (window as unknown as { __asked?: string[] }).__asked ?? [],
    );
    expect(asked, "the picker never asked Rust for the packs").toContain(
      "knowledge_packs",
    );

    // Eight rows, and the two ends of the range they cover. The general pack
    // teaches the whole catalogue; the beginner pack is the narrowing, and the
    // difference between the two numbers is the whole point of choosing one.
    await expect(page.locator(".packs li")).toHaveCount(8);
    await expect(pack(page, "general").locator(".pack-reach")).toContainText(
      "27 moves",
    );
    await expect(pack(page, "beginner").locator(".pack-reach")).toContainText(
      "7 moves",
    );

    // The families are named rather than invented: House turns on four, and
    // they are dj_core::genre's own spellings.
    await expect(pack(page, "house").locator(".pack-reach")).toContainText(
      "house, tech house, disco, afro house",
    );
    // Open format turns on all of them, so it names none — the picker must not
    // print an empty list where a real answer goes.
    await expect(
      pack(page, "open-format").locator(".pack-reach"),
    ).not.toContainText("·");
    expect(thrown).toEqual([]);
  });

  /**
   * **Choosing one marks it, and pressing it again gives the catalogue back.**
   *
   * A pack is a narrowing, and the only way out of a narrowing that a DJ can
   * find on their own is the control they used to get into it. What the
   * interface shows is what Rust *kept* rather than what was asked for, which
   * is why the second press has to come back through the same round trip.
   */
  test("a pack can be chosen and put down again", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await openPacks(page);

    const techno = pack(page, "techno").getByRole("button");
    await expect(techno).toHaveAttribute("aria-pressed", "false");

    await techno.click();
    await expect(techno).toHaveAttribute("aria-pressed", "true");
    expect(
      await page.evaluate(
        () => (window as unknown as { __pack?: string }).__pack,
      ),
      "the choice never reached Rust",
    ).toBe("techno");

    await techno.click();
    await expect(techno).toHaveAttribute("aria-pressed", "false");
    expect(
      await page.evaluate(
        () => (window as unknown as { __pack?: string }).__pack,
      ),
      "pressing the chosen pack again left it chosen",
    ).toBe("");
    expect(thrown).toEqual([]);
  });
});
