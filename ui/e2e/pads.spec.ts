/**
 * §29's pads, and the four of them that sat dark.
 *
 * `dj_core::pads` is a table: each pad carries the action it sends and a
 * `Lit` condition the interface evaluates against the snapshot, so a new page
 * is rows in Rust rather than cases in a component. Eight of the ten
 * conditions were evaluated. `StemMuted` and `StemSolo` returned `false`
 * under a comment saying the pads would not latch *"until the snapshot
 * carries per-stem state"* — which it does, and has for long enough that the
 * stems **panel** has been drawing its mute buttons from the same two fields
 * all along.
 *
 * So four pads a deck sat dark beside a panel showing the very state they
 * were about. The Rust table was right, the snapshot was right, and the one
 * line that reads them was stale.
 *
 * These drive the snapshot directly rather than pressing the pads: what is
 * being checked is that the interface *reads* the state, and a press would
 * measure the round trip instead — which `athand.spec.ts` already does.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Deliver a frame with one deck's stem state changed, as the engine would. */
async function stems(
  page: Page,
  mutes: [boolean, boolean, boolean, boolean],
  soloing: boolean,
) {
  await page.evaluate(
    ({ mutes, soloing }) => {
      const win = window as unknown as {
        __lastState: {
          decks: { stem_mutes: boolean[]; stem_soloing: boolean }[];
        };
        __emit: (next: unknown) => void;
      };
      const next = structuredClone(win.__lastState);
      next.decks[0].stem_mutes = mutes;
      next.decks[0].stem_soloing = soloing;
      win.__emit(next);
    },
    { mutes, soloing },
  );
}

const pad = (page: Page, name: string) =>
  page.getByRole("button", { name, exact: true }).first();

async function stemsPage(page: Page) {
  await openShell(page, "/");
  await pad(page, "Stem pads").click();
  await expect(pad(page, "Vocal mute")).toBeVisible();
}

test.describe("§29's stem pads", () => {
  /**
   * **A muted stem lights its pad.**
   *
   * One stem muted, not all four: a pad that lit for any mute anywhere would
   * pass a test that muted everything, and would be wrong about which stem in
   * exactly the way a DJ reaching for the drums cannot afford.
   */
  test("the mute pads follow the stems that are muted", async ({ page }) => {
    await stemsPage(page);
    await stems(page, [false, true, false, false], false);

    await expect(pad(page, "Drums mute")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    for (const dark of ["Vocal mute", "Bass mute", "Other mute"]) {
      expect(
        await pad(page, dark).getAttribute("aria-pressed"),
        `${dark} lit for a stem that is not muted`,
      ).toBeNull();
    }
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A held solo lights the stem it is auditioning, and only that one.**
   *
   * Which stem is soloed is *derived*: the engine's solo mutes every stem and
   * un-mutes exactly one, keeping the DJ's own pattern aside to restore on
   * release — so the soloed stem is the audible one while a solo is held. A
   * second snapshot field saying which would be a second answer that could
   * disagree with the audio.
   */
  test("a solo lights the stem being auditioned", async ({ page }) => {
    await stemsPage(page);
    // What the engine leaves behind when the vocal is soloed.
    await stems(page, [false, true, true, true], true);

    await expect(pad(page, "Vocal solo")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    for (const dark of ["Drums solo", "Bass solo", "Other solo"]) {
      expect(
        await pad(page, dark).getAttribute("aria-pressed"),
        `${dark} lit while a different stem is soloed`,
      ).toBeNull();
    }

    // And the mute pads agree with the audio — and so with the stems panel,
    // which draws its buttons from the same field. Two surfaces for one state
    // that disagreed would be worse than either being wrong alone.
    await expect(pad(page, "Drums mute")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * The same mutes with no solo held light no solo pad. Without this, a solo
   * pad that ignored `stem_soloing` and lit whenever its stem was audible
   * would pass the test above.
   */
  test("mutes alone light no solo pad", async ({ page }) => {
    await stemsPage(page);
    await stems(page, [false, true, true, true], false);

    for (const dark of ["Vocal solo", "Drums solo", "Bass solo", "Other solo"]) {
      expect(
        await pad(page, dark).getAttribute("aria-pressed"),
        `${dark} lit with no solo held`,
      ).toBeNull();
    }
    expect(errorsThrown(page)).toEqual([]);
  });
});

/**
 * §121: *"make those buttons use only one row of space and be adequately
 * symbolized ... i don't want full wording all the time (on hover only)"*.
 */
test.describe("§121: the pad row, in symbols", () => {
  /**
   * **The load-bearing one: eight pads in one row, the tabs as symbols with
   * their names on hover, and a stem's pad in its own colour.** The words
   * are still there for a screen reader and the hover -- as the accessible
   * name and the title, which is how every test here finds them.
   */
  test("eight pads in one row, the tabs as symbols, the words on hover", async ({ page }) => {
    await stemsPage(page);
    const deck = page.locator("section.deck[data-deck]").first();
    const pads = deck.locator("button.svg-button.pad");
    await expect(pads).toHaveCount(8);
    const tops = await pads.evaluateAll((all) => all.map((el) => Math.round(el.getBoundingClientRect().top)));
    expect(new Set(tops).size, `not one row: ${tops}`).toBe(1);

    const tabs = deck.locator("button.svg-button.tab");
    expect(await tabs.count()).toBeGreaterThanOrEqual(5);
    for (const tab of await tabs.all()) {
      await expect(tab, "a tab still spells its name out").toHaveText("");
      await expect(tab.locator("svg.icon")).toHaveCount(1);
      expect(await tab.getAttribute("title")).toBeTruthy();
    }
    await expect(pad(page, "Stem pads")).toHaveAttribute("title", "Stem pads");

    // A stem pad is its initial, not its sentence, in the stem's own colour.
    await expect(pad(page, "Vocal mute")).toHaveText("V");
    const colour = (name: string) =>
      pad(page, name).locator("b").evaluate((el) => getComputedStyle(el).color);
    expect(await colour("Vocal mute")).not.toBe(await colour("Drums mute"));
    expect(await colour("Vocal mute")).toBe(await colour("Vocal solo"));
    expect(errorsThrown(page)).toEqual([]);
  });
});
