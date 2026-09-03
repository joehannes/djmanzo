/**
 * Phase 2's gate: a new shell with the old functionality intact.
 *
 * `docs/GUI-OVERHAUL.md` §21 states it as *"every surface still reachable"* --
 * no feature may become unreachable during the migration, which is exactly the
 * failure a 1,700-line `App.svelte` invites when its panel slot is rebuilt.
 * This is the measurement of that.
 *
 * It also measures the thing the migration was *for*. The audit's headline
 * finding was structural: one `panel` variable held one of eight names, so a
 * DJ could not look at the room and the library at the same time -- not
 * because anyone decided that, but because the shell was shaped that way. A
 * test that only checked reachability would pass against the old shell too.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/**
 * Every panel button in the top bar, and the surface it opens.
 *
 * The buttons are named by their labels because that is what a DJ reads. The
 * surface names are djmanzo's own, from `dj_app::cockpit::surfaces()`.
 */
const PANELS: { button: string; surface: string }[] = [
  { button: "Browse", surface: "library" },
  { button: "Prepare", surface: "prepare" },
  { button: "Presets", surface: "presets" },
  { button: "Booth", surface: "booth" },
  { button: "Sampler", surface: "sampler" },
  { button: "Assistant", surface: "assistant" },
  { button: "Settings", surface: "settings" },
  { button: "Keys", surface: "keys" },
  { button: "Log", surface: "log" },
];

test.describe("the dock manager", () => {
  test("every panel the top bar offers still opens", async ({ page }) => {
    await openShell(page, "/");

    for (const { button, surface } of PANELS) {
      await page.getByRole("button", { name: button, exact: true }).click();
      await expect(
        page.locator(`.surface[data-surface="${surface}"]`),
        `pressing ${button} did not open the ${surface} surface -- a feature ` +
          "that was one press away before the docks is now unreachable",
      ).toBeVisible();
      // Closed again, so each is measured on its own rather than in the
      // company of everything opened before it.
      await page.getByRole("button", { name: button, exact: true }).click();
      await expect(page.locator(`.surface[data-surface="${surface}"]`)).toHaveCount(0);
    }
  });

  /**
   * The reason the shell was rebuilt at all.
   *
   * Three things on screen at once -- the decks, the library along the bottom,
   * the assistant beside them. Under the old shell opening the second closed
   * the first, and this assertion would have failed on the deck as well as on
   * the panel.
   */
  test("the library and the assistant can be open together, over the decks", async ({
    page,
  }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await page.getByRole("button", { name: "Assistant", exact: true }).click();

    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();
    await expect(page.locator('.surface[data-surface="assistant"]')).toBeVisible();
    await expect(
      page.locator(".deck").first(),
      "the decks went away when two surfaces were opened, which is the old " +
        "problem wearing a new shape",
    ).toBeVisible();
  });

  /**
   * A wide surface goes along the bottom and a tall one beside the decks.
   *
   * Not a table of special cases: the rule is the surface's own preferred size,
   * which Rust publishes. The library prefers 900x380 and lands under the
   * decks; the assistant is taller than it is wide and stands beside them.
   * Asserted geometrically rather than by class name, because "below" and
   * "beside" are the claims a DJ can actually check.
   */
  test("the library lands below the decks and the assistant beside them", async ({
    page,
  }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await page.getByRole("button", { name: "Assistant", exact: true }).click();

    const deck = await page.locator(".deck").first().boundingBox();
    const library = await page.locator('.surface[data-surface="library"]').boundingBox();
    const assistant = await page
      .locator('.surface[data-surface="assistant"]')
      .boundingBox();
    expect(deck).not.toBeNull();
    expect(library).not.toBeNull();
    expect(assistant).not.toBeNull();

    expect(
      library!.y,
      "the library is not below the decks, so it is not in the bottom dock",
    ).toBeGreaterThan(deck!.y);
    expect(
      assistant!.x,
      "the assistant is not to the right of the decks, so it is not in the side dock",
    ).toBeGreaterThan(deck!.x + deck!.width - 1);
  });

  /**
   * The directive's §21: Prepare is first class, and the gesture still works.
   *
   * Prepare used to be a child of the browser -- mounted inside the library
   * panel and handed a track by a prop, so it could only exist where the
   * library was and only while the library was open. Making it a surface of
   * its own is what "first class" means, and the risk of the move is exactly
   * what §21 warns about: an inconsistent Prepare gesture, which is the thing
   * Engine DJ users complain about.
   *
   * Two halves could break. They must open together over the decks, which is
   * the point of moving it; and the handoff must survive the two being
   * siblings rather than parent and child, which is what
   * `prepare.svelte.ts` carries.
   */
  test("the library and Prepare are separate surfaces that open together", async ({
    page,
  }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await page.getByRole("button", { name: "Prepare", exact: true }).click();

    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();
    await expect(page.locator('.surface[data-surface="prepare"]')).toBeVisible();
    await expect(
      page.locator(".deck").first(),
      "the decks went away when the library and Prepare were both opened",
    ).toBeVisible();
  });

  /**
   * **The gesture itself**, which is what §21 is actually about.
   *
   * The two tests around this one prove the surfaces open. They do not prove
   * that pressing → in the browser reaches Prepare, and the difference is not
   * academic: replacing the browser's handler with an empty function broke
   * nothing, because the fixture had no rows to press. A workflow test needs
   * something to act on, so `shell.ts` now answers `library_search` with one
   * record.
   *
   * What is asserted is the command, not a rendered list: the sidelist's
   * contents come from Rust and this harness answers with an empty one, so the
   * honest claim is that the press crossed from the browser to the Prepare
   * space and asked for the right track.
   */
  test("setting a track aside from the browser reaches Prepare", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await page.getByRole("button", { name: "Prepare", exact: true }).click();

    await page.evaluate(() => ((window as unknown as Record<string, unknown>).__asked = []));
    await page.getByRole("button", { name: "Set aside Bachata Rosa" }).click();

    await expect
      .poll(
        () =>
          page.evaluate(
            () => (window as unknown as { __asked: string[] }).__asked,
          ),
        {
          message:
            "pressing the browser's set-aside button did not reach the Prepare " +
            "space -- the two are sibling surfaces now, and the handoff between " +
            "them is `prepare.svelte.ts`",
        },
      )
      .toContain("sidelist_add");
  });

  /**
   * Prepare opens on its own, with no library in sight.
   *
   * The thing that was impossible before, and the whole of §21: planning a set
   * is a different activity from browsing, and it should not require the
   * browser to be on screen.
   */
  test("Prepare opens without the library", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Prepare", exact: true }).click();

    await expect(page.locator('.surface[data-surface="prepare"]')).toBeVisible();
    await expect(page.locator('.surface[data-surface="library"]')).toHaveCount(0);
  });

  /** A surface closes from its own header, not only from the top bar. */
  test("a surface closes from its own header", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();

    await page.getByRole("button", { name: "Close Library" }).click();
    await expect(page.locator('.surface[data-surface="library"]')).toHaveCount(0);
  });

  /** Nothing may throw while the docks are being opened. See `budget.spec.ts`. */
  test("opening every surface throws nothing", async ({ page }) => {
    await openShell(page, "/");
    for (const { button } of PANELS) {
      await page.getByRole("button", { name: button, exact: true }).click();
    }
    await expect(page.locator(".surface").first()).toBeVisible();
    expect(
      errorsThrown(page),
      "a surface threw while it was being opened, so part of the cockpit did " +
        "not finish rendering",
    ).toEqual([]);
  });
});
