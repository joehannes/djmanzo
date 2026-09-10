/**
 * The next-track rail, and what it is for.
 *
 * The directive's §22 asks for a *rail*: three to eight candidates, each with
 * its deltas and a one-line reason, and the actions a DJ takes on one --
 * audition, stage, load, reject, pin, more like this. It was a tab inside
 * Prepare, which meant it could be looked at *instead of* the sidelist rather
 * than beside it, and a thing whose value is being glanced at mid-transition
 * is not a thing to go and find.
 *
 * What is measured here is the part that a type-check cannot see: that the
 * line a DJ actually reads is on the screen, that it is deltas rather than
 * values, and that the row's five gestures do the five different things they
 * claim to.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Open the shell with the rail docked. */
async function railOpen(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Next", exact: true }).click();
  await expect(page.locator('.surface[data-surface="next"]')).toBeVisible();
}

/** What the page has asked Rust for since `watch` was called. */
async function watch(page: import("@playwright/test").Page) {
  await page.evaluate(() => ((window as unknown as Record<string, unknown>).__asked = []));
  return () =>
    page.evaluate(() => (window as unknown as { __asked: string[] }).__asked);
}

test.describe("the next-track rail", () => {
  test("opens as a surface of its own, beside the decks", async ({ page }) => {
    await railOpen(page);

    const rail = await page.locator('.surface[data-surface="next"]').boundingBox();
    const deck = await page.locator(".deck").first().boundingBox();
    expect(
      rail!.x,
      "the rail is not to the right of the decks, so it is not a rail beside them",
    ).toBeGreaterThan(deck!.x + deck!.width - 1);
    expect(
      page.locator(".deck").first(),
      "the decks went away when the rail opened",
    ).toBeTruthy();
    expect(errorsThrown(page), "the rail threw while rendering").toEqual([]);
  });

  /**
   * **The line, which is the whole feature.**
   *
   * §22's own example is `+3 BPM · 8A→9A · energy +1`. A rail showing
   * `127 BPM · harmonic (9A)` is answering a question the DJ did not ask --
   * one that needs them to remember what is playing before any of it means
   * anything.
   */
  test("every candidate carries one line of deltas", async ({ page }) => {
    await railOpen(page);

    const lines = page.locator('.surface[data-surface="next"] .why');
    await expect(lines).toHaveCount(2);
    await expect(lines.first()).toHaveText("+3 BPM · 8A→9A · +1 dB");
    await expect(
      lines.nth(1),
      "the second candidate's key clash is not on its line. A suggestion that " +
        "hides its worst feature is one a DJ learns not to trust",
    ).toContainText("clash");
  });

  /** Confidence is drawn, and it differs between a good match and a poor one. */
  test("confidence is visible and is not the same for both", async ({ page }) => {
    await railOpen(page);

    const bars = page.locator('.surface[data-surface="next"] .confidence .fill');
    await expect(bars).toHaveCount(2);
    const widths = await bars.evaluateAll((els) =>
      els.map((el) => el.getBoundingClientRect().width),
    );
    expect(
      widths[0],
      `both candidates drew the same confidence (${widths.join(", ")} px), so ` +
        "the bar is decoration rather than information",
    ).toBeGreaterThan(widths[1] + 1);
  });

  /**
   * **Reject is about this minute.**
   *
   * The row goes, and nothing is written down. "Not that one" while a record
   * is playing is not "never suggest this again", and a rail that quietly
   * learned the first as the second would hide a collection from its owner.
   */
  test("passing on a candidate removes it and asks Rust for nothing", async ({ page }) => {
    await railOpen(page);
    const asked = await watch(page);

    await page.getByRole("button", { name: "Pass on Burbujas de Amor" }).click();

    await expect(page.locator('.surface[data-surface="next"] .why')).toHaveCount(1);
    expect(
      await asked(),
      "passing on a candidate sent something to Rust. It is a decision about " +
        "the next few minutes, not a fact about the collection",
    ).toEqual([]);
  });

  /** Pinning keeps a candidate at the top, whatever the ranking does. */
  test("a pinned candidate rises to the top", async ({ page }) => {
    await railOpen(page);
    const names = page.locator('.surface[data-surface="next"] .name');
    await expect(names.first()).toHaveText("Ojalá Que Llueva Café");

    await page.getByRole("button", { name: "Pin Burbujas de Amor" }).click();
    await expect(
      names.first(),
      "the pinned candidate did not move to the top of the rail",
    ).toHaveText("Burbujas de Amor");
  });

  /** Staging a candidate is the same gesture the browser has, reaching Prepare. */
  test("staging a candidate reaches Prepare", async ({ page }) => {
    await railOpen(page);
    const asked = await watch(page);

    await page.getByRole("button", { name: "Set Ojalá Que Llueva Café aside" }).click();

    await expect.poll(asked, {
      message: "the rail's set-aside gesture did not reach the Prepare space",
    }).toContain("sidelist_add");
  });

  /** "More like this" re-seeds the rail from a record rather than from a deck. */
  test("more like this asks a different question", async ({ page }) => {
    await railOpen(page);
    const asked = await watch(page);

    await page.getByRole("button", { name: "More like Burbujas de Amor" }).click();

    await expect.poll(asked, {
      message: "more-like-this did not re-seed the rail",
    }).toContain("similar_to");
    await expect(
      page.getByRole("button", { name: "Back to what follows the deck" }),
      "the rail is answering a different question and does not say so, so it " +
        "looks like a ranking that stopped following the deck",
    ).toBeVisible();
  });

  /**
   * The tab it replaced is gone, not duplicated.
   *
   * §21 warns against copying a workflow into "awkward parallel systems", and
   * two places that suggest the next record are two places that will disagree.
   */
  test("Prepare no longer carries a second copy of the rail", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Prepare", exact: true }).click();

    await expect(
      page.locator('.surface[data-surface="prepare"]').getByRole("button", { name: "Next" }),
      "Prepare still has a Next tab. The rail moved; it did not get copied",
    ).toHaveCount(0);
  });
});

/**
 * §22's *estimated transition type*, the last of its fifteen that djmanzo can
 * answer without a second player.
 *
 * The estimate itself is the planner's and is tested in Rust. What matters
 * here is what the rail does with it: that it is a line about the **mix**
 * rather than another chip about the two records, that a pair djmanzo would
 * cut says so beside a pair it would blend, and that it is worded once — a
 * rail and a ghost panel disagreeing about the same mix is the failure §68's
 * table exists to prevent, at the scale of a sentence.
 */
test.describe("the estimated transition", () => {
  test("says what the mix into each candidate would be", async ({ page }) => {
    await railOpen(page);

    const rows = page.locator('.surface[data-surface="next"] li');
    await expect(rows.first().locator(".mix")).toHaveText("32-beat blend at 3:45");
    // The second candidate clashes, so the planner cuts. Two records the same
    // distance apart with different mixes is the whole value of the line.
    await expect(rows.nth(1).locator(".mix")).toHaveText("8-beat cut at 4:01");
  });

  /**
   * **A line about the mix, under the line about the records.**
   *
   * §22 asks for deltas *and* an estimated transition, and they answer
   * different questions: `+3 BPM · 8A→9A` is about the two records, and
   * `32-beat blend` is about what happens between them. Folded into one line
   * they read as one fact and neither is findable at a glance.
   */
  test("is its own line, below the deltas", async ({ page }) => {
    await railOpen(page);

    const row = page.locator('.surface[data-surface="next"] li').first();
    const why = await row.locator(".why").boundingBox();
    const mix = await row.locator(".mix").boundingBox();
    expect(mix!.y, "the transition is not below the deltas").toBeGreaterThan(why!.y);
    expect(
      await row.locator(".why").textContent(),
      "the deltas line swallowed the transition",
    ).not.toContain("blend");
  });

  /**
   * The rail and §27's ghost panel say the same words about the same mix,
   * because both are handed the phrase rather than composing one.
   */
  test("is worded the same way the ghost panel words it", async ({ page }) => {
    await railOpen(page);

    const row = page.locator('.surface[data-surface="next"] li').first();
    const inRail = await row.locator(".mix").textContent();
    await row.getByRole("button", { name: /^Preview / }).click();
    await expect(row.locator(".ghost-what")).toBeVisible();
    expect(await row.locator(".ghost-what").textContent()).toBe(inRail);
  });
});
