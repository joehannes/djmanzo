/**
 * §7's last step, and §103's *modularity*: an arrangement you can name.
 *
 * > Make these starting points, not rigid identities. Every preset should
 * > remain editable.
 *
 * That was true except for the end of it. A DJ could pick one of the
 * twenty-three, move what they liked, and the edit survived a restart — but it
 * had no name, so the second layout they built replaced the first. A DJ with a
 * wedding layout and a club layout had one of them and a memory of the other.
 *
 * What a name may be is `cockpit::keep` and is tested there: empty is refused, a
 * name djmanzo ships is refused, one of your own is replaced rather than
 * doubled, and the collection is sorted so the menu settles. These say the three
 * things only a browser can — the row appears, it is kept apart from the shipped
 * ones, and a refusal reaches the screen instead of being swallowed.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const PICKER = "select.workspace-preset";

/** Choose "Save this arrangement…" and type a name. */
async function nameIt(page: Page, name: string) {
  await page.locator(PICKER).selectOption("__keep__");
  const field = page.getByRole("textbox", { name: "Name for this arrangement" });
  await expect(field).toBeVisible();
  await field.fill(name);
}

/** What djmanzo was asked to keep, in order. */
async function kept(page: Page) {
  return page.evaluate(
    () => (window as unknown as { __kept?: { name: string }[] }).__kept ?? [],
  );
}

test.describe("§7's own arrangements", () => {
  /**
   * **The load-bearing one: a named arrangement is kept, and appears as
   * yours.**
   *
   * Both halves. Kept but not shown is a save a DJ cannot find; shown but not
   * kept is a row that disappears on the next restart — and the second is
   * discovered at the worst possible moment, which is the next time they need
   * that layout.
   */
  test("an arrangement saved under a name appears in your own list", async ({
    page,
  }) => {
    await openShell(page, "/");
    await expect(
      page.locator(`${PICKER} optgroup[label="Yours"]`),
      "a fresh install has arrangements of its own already",
    ).toHaveCount(0);

    await nameIt(page, "Saturday Residency");
    await page.getByRole("button", { name: "Keep", exact: true }).click();

    await expect
      .poll(async () => (await kept(page)).map((w) => w.name))
      .toEqual(["Saturday Residency"]);
    await expect(
      page.locator(`${PICKER} optgroup[label="Yours"] option`),
      "djmanzo kept the arrangement and the picker does not show it, so a DJ " +
        "cannot get back to what they saved",
    ).toHaveText(["Saturday Residency"]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * Yours are above djmanzo's, and separately labelled.
   *
   * A DJ who has saved a layout is looking for *that* one. Reading past
   * twenty-three they have never opened to reach it is the thing having saved it
   * was supposed to avoid.
   */
  test("your own arrangements are listed apart from the ones that ship", async ({
    page,
  }) => {
    await openShell(page, "/");
    await nameIt(page, "Back Room");
    await page.getByRole("button", { name: "Keep", exact: true }).click();

    const groups = page.locator(`${PICKER} optgroup`);
    await expect(groups.first()).toHaveAttribute("label", "Yours");
    // Counted rather than looked at: an `option` inside a closed `select` is
    // not visible to anybody, including a DJ, and the claim here is about what
    // the menu contains.
    await expect(
      page.locator(`${PICKER} optgroup[label="djmanzo\u0027s"] option`),
      "the shipped arrangements went missing when a DJ saved one of their own",
    ).not.toHaveCount(0);
  });

  /**
   * A refusal reaches the screen.
   *
   * "djmanzo already ships an arrangement called Perform" is a sentence a DJ
   * acts on in one press. A save that quietly did nothing is one they find out
   * about an hour later, looking for a layout that was never kept — which is the
   * failure this whole feature exists to end.
   */
  test("a name djmanzo already ships is refused, out loud", async ({ page }) => {
    await openShell(page, "/");
    await nameIt(page, "Perform");
    await page.getByRole("button", { name: "Keep", exact: true }).click();

    await expect(
      page.getByRole("alert"),
      "the save was refused in silence, so the DJ believes they have a layout " +
        "they do not have",
    ).toContainText("already ships");
    expect(await kept(page)).toEqual([]);
  });

  /**
   * Escape gives up without keeping anything.
   *
   * A DJ naming a layout between two records has a hand on the keyboard. The
   * description comes back, which is how they know nothing happened.
   */
  test("escape leaves the naming without saving", async ({ page }) => {
    await openShell(page, "/");
    await nameIt(page, "Never Mind");
    await page.keyboard.press("Escape");

    await expect(
      page.getByRole("textbox", { name: "Name for this arrangement" }),
    ).toHaveCount(0);
    expect(await kept(page)).toEqual([]);
  });
});
