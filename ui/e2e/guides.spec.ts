/**
 * §118a: the assistant's guides, one per topic.
 *
 * > the integrated ai assistant shall have different guides that are
 * > visually represented via floating widgets with visual presentations of
 * > topics and representations of (multi-choice) options that are easy to
 * > identify
 *
 * Which guides open, on which decks, and why the others do not is
 * `dj_app::guide`'s (`guides.json`, Rust's answers for four booths). What the
 * browser holds: the topics say why not in Rust's words; the next record
 * opens the three-way decision on the deck Rust names; the mix draws every
 * style the planner offers and holds the chosen one without moving
 * anything; and trouble opens the night's own quick decision.
 */
import { expect, test, type Page } from "@playwright/test";

import events from "./events.json" with { type: "json" };
import guides from "./guides.json" with { type: "json" };
import styles from "./styles.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const recorded = <T,>(page: Page, name: string) =>
  page.evaluate((name) => ((window as unknown as Record<string, unknown>)[name] ?? []) as T[], name);

const TOPICS = '[role="dialog"][aria-label="The assistant\'s guides"]';

async function openGuides(page: Page) {
  await page.getByRole("button", { name: "Guides" }).click();
  await expect(page.locator(TOPICS)).toBeVisible();
  return page.locator(TOPICS);
}

test.describe("§118a: the assistant's guides", () => {
  /** A guide that cannot open says why, in Rust's words, and does nothing. */
  test("a guide that cannot open says why", async ({ page }) => {
    await openShell(page, "/");
    const topics = await openGuides(page);
    await expect(topics.locator(".topic")).toHaveCount(guides.quiet.length);
    // Asked about the decks on screen: the engine has four, the booth shows two.
    expect(await recorded(page, "__guidesDecks")).toEqual([2]);
    for (const guide of guides.quiet) {
      const tile = topics.locator(`[data-topic="${guide.topic}"]`);
      await expect(tile).toBeDisabled();
      await expect(tile).toContainText(guide.title);
      await expect(tile.locator(".about")).toHaveText(guide.not_now!);
    }
    await page.keyboard.press("Escape");
    await expect(topics).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The next record: the three-way decision, on the deck Rust names. */
  test("the next record opens the three-way choice on the deck the room hears", async ({ page }) => {
    await openShell(page, "/", {}, { guides: guides.playing });
    const topics = await openGuides(page);
    const next = guides.playing.find((g) => g.topic === "next")!;
    await topics.locator('[data-topic="next"]').click();
    const decision = page.locator('[role="dialog"][aria-label="Choose the next record"]');
    await expect(decision).toBeVisible();
    await expect(decision).toHaveAttribute("data-decision", String(next.from));
    await expect.poll(() => recorded(page, "__decisionAsked")).toEqual([next.from]);
    expect(await recorded(page, "__decisionDecks")).toEqual([2]);
    // A guide is closed, not declined: nothing is said to the whisper.
    await decision.getByRole("button", { name: "Close" }).click();
    await expect(decision).toHaveCount(0);
    expect(await recorded(page, "__whisperAnswered")).toEqual([]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The load-bearing one.** The mix draws every style the planner offers,
   * marks the planner's own, and a press holds the mix with that style --
   * held, never performed: nothing is sent to the decks.
   */
  test("the mix draws every style and holds the chosen one without moving anything", async ({ page }) => {
    await openShell(page, "/", {}, { guides: guides.waiting });
    const topics = await openGuides(page);
    await topics.locator('[data-topic="mix"]').click();
    const mix = page.locator('[role="dialog"][aria-label="The mix"]');
    await expect(mix.locator(".style")).toHaveCount(styles.length);
    expect(await mix.locator(".style").evaluateAll((all) => all.map((s) => s.getAttribute("data-style")))).toEqual(
      styles.map((s) => s.name),
    );
    // Each drawn: two lines, the one playing and the one coming in.
    for (const style of await mix.locator(".style").all()) {
      await expect(style.locator("svg path")).toHaveCount(2);
    }
    const planned = await mix.locator(".style.planned").getAttribute("data-style");
    await expect(mix.locator(".style.planned .mark")).toHaveText("djmanzo's plan");

    const other = styles.map((s) => s.name).find((name) => name !== planned)!;
    await mix.locator(`[data-style="${other}"]`).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __transition?: { style: string; armed: boolean } }).__transition))
      .toMatchObject({ style: other, armed: true });
    await expect(mix.locator(`[data-style="${other}"]`)).toHaveAttribute("aria-pressed", "true");
    await expect(mix.getByRole("status")).toContainText(`Held: ${other}`);
    expect(await recorded(page, "__dispatched")).toEqual([]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** While a night is played, trouble opens its own quick decision. */
  test("trouble opens the night's quick decision", async ({ page }) => {
    await openShell(page, "/", {}, { guides: guides.live, live_event: events.wedding.gig.id });
    await expect(page.locator(".tonight")).toBeVisible();
    const topics = await openGuides(page);
    await topics.locator('[data-topic="trouble"]').click();
    await expect(topics).toHaveCount(0);
    const decide = page.locator('[role="dialog"][aria-label="If something goes wrong"]');
    await expect(decide).toBeVisible();
    await expect(decide.locator(".decision")).toHaveCount(events.tonight.decisions.length);
    expect(errorsThrown(page)).toEqual([]);
  });
});
