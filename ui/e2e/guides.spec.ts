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
      await expect(tile).toContainText(guide.title);
      if (guide.not_now === null) {
        // Learning a move: a quiet booth is the time for it.
        await expect(tile).toBeEnabled();
        continue;
      }
      await expect(tile).toBeDisabled();
      await expect(tile.locator(".about")).toHaveText(guide.not_now);
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

  /**
   * **A request waiting** (§118): the most wanted three, in Rust's order, each
   * with how many asked; *Played* and *Not tonight* settle it through the
   * Requests panel's own command and the next comes up; *Find it* searches
   * the collection for it and puts the guide away.
   */
  test("a request waiting offers the most wanted, found or settled", async ({ page }) => {
    const ask = (id: number, text: string, voices: number) => ({
      id,
      text,
      voices,
      first_asked: 1_790_000_000,
      last_asked: 1_790_000_000,
      standing: "waiting",
    });
    const waiting = [ask(1, "Vivir mi vida", 5), ask(2, "Danza kuduro", 3), ask(3, "La bicicleta", 1), ask(4, "Propuesta indecente", 1)];
    await openShell(page, "/", {}, { guides: guides.asked, audience_waiting: waiting });
    const topics = await openGuides(page);
    const tile = topics.locator('[data-topic="request"]');
    await expect(tile).toBeEnabled();
    await tile.click();
    const card = page.locator('[role="dialog"][aria-label="A request waiting"]');
    await expect(card).toBeVisible();
    const rows = card.locator("[data-ask]");
    await expect(rows).toHaveCount(3);
    await expect(rows.nth(0)).toContainText("Vivir mi vida");
    await expect(rows.nth(0)).toContainText("5 asked");
    await expect(rows.nth(2)).toContainText("1 asked");

    await rows.nth(0).getByRole("button", { name: "Played" }).click();
    await expect(rows.nth(0)).toContainText("Danza kuduro");
    await expect(rows).toHaveCount(3);
    await expect(card).toContainText("Propuesta indecente");
    await card.locator('[data-ask="2"]').getByRole("button", { name: "Not tonight" }).click();
    await expect(card.locator('[data-ask="2"]')).toHaveCount(0);
    expect(await recorded(page, "__settled")).toEqual([
      { id: 1, standing: "played" },
      { id: 2, standing: "passed" },
    ]);

    await card.locator('[data-ask="3"]').getByRole("button", { name: "Find it" }).click();
    await expect(card).toHaveCount(0);
    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();
    await expect(page.locator('.surface[data-surface="library"] input[type="search"], .surface[data-surface="library"] input').first()).toHaveValue("La bicicleta");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * §118: the energy of the night. Opened while a record plays, it draws
   * the night as djmanzo reads it -- the phase in words, the level, the arc
   * so far, the first note -- and offers the DJ's word as four choices; a
   * press declares that phase (recorded, as Rust's `night_declare` would
   * receive it) and is marked as the word in force, and *Let the music say*
   * takes it back.
   */
  test("the energy of the night shows where it stands and takes the DJ's word", async ({ page }) => {
    const night = {
      phase: "heat",
      words: "building",
      energy: 0.55,
      certainty: "fair",
      certainty_about: "The music alone, and it has been steady.",
      basis: "measured",
      drift: null,
      time_of_day: "evening",
      declared: null,
      measured: "heat",
      readings: 140,
      still_needed: 0,
      notes: ["Tonight is building, from the music alone."],
      arc: [
        { at_seconds: 1500, phase: "warm_up", words: "a warm-up" },
        { at_seconds: 3720, phase: "heat", words: "building" },
      ],
      warrant: "speak",
    };
    await openShell(page, "/", {}, { guides: guides.playing, night_read: night });
    const topics = await openGuides(page);
    const tile = topics.locator('[data-topic="energy"]');
    await expect(tile).toBeEnabled();
    await tile.click();
    const card = page.locator('[role="dialog"][aria-label="The energy of the night"]');
    await expect(card).toBeVisible();
    await expect(card.locator("header strong")).toHaveText("Tonight is building");
    await expect(card.locator(".level .fill")).toHaveAttribute("style", /width: 55%/);
    await expect(card.locator(".arc li")).toHaveText(["a warm-up · 25 min", "building · 1 h 02"]);
    await expect(card).toContainText("Tonight is building, from the music alone.");
    const words = card.locator("[data-word]");
    await expect(words).toHaveText([/Keep building/, /Hold the peak/, /Bring it down/, /Let the music say/]);
    // Nothing said yet: letting the music say is what is in force.
    await expect(card.locator('[data-word="music"]')).toHaveAttribute("aria-pressed", "true");

    await card.locator('[data-word="cooldown"]').click();
    await expect(card.locator('[data-word="cooldown"]')).toHaveAttribute("aria-pressed", "true");
    await expect(card.locator('[data-word="cooldown"]')).toContainText("Your word");
    await expect(card.locator('[data-word="music"]')).toHaveAttribute("aria-pressed", "false");
    await card.locator('[data-word="music"]').click();
    await expect(card.locator('[data-word="cooldown"]')).toHaveAttribute("aria-pressed", "false");
    expect(await recorded(page, "__declared")).toEqual(["cooldown", null]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** With nothing playing, the energy guide says so in Rust's words. */
  test("the energy guide says when nothing is playing", async ({ page }) => {
    await openShell(page, "/");
    const topics = await openGuides(page);
    const quiet = guides.quiet.find((g) => g.topic === "energy")!;
    await expect(topics.locator('[data-topic="energy"]')).toBeDisabled();
    await expect(topics.locator('[data-topic="energy"] .about')).toHaveText(quiet.not_now!);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * §118: learning a move. Between records it lists the moves the DJ is
   * learning, then the coach's next, each in the coach's words (Rust's,
   * `guides.json`); *I have it now* keeps one as played and it leaves the
   * list, *Learn this one* makes the suggestion theirs, and *Rehearse it*
   * opens the Practice lab.
   */
  test("learning a move lists the moves being learned, kept or rehearsed", async ({ page }) => {
    await openShell(page, "/", {}, { learning_moves: guides.learning });
    const topics = await openGuides(page);
    await topics.locator('[data-topic="learn"]').click();
    const card = page.locator('[role="dialog"][aria-label="Learning a move"]');
    await expect(card).toBeVisible();
    const [theirs, next] = guides.learning;
    const rows = card.locator("[data-move]");
    await expect(rows).toHaveCount(2);
    await expect(rows.nth(0)).toHaveAttribute("data-move", theirs.name);
    await expect(rows.nth(0).locator(".tag")).toHaveText(`You are learning it · ${theirs.difficulty}`);
    await expect(rows.nth(0)).toContainText(theirs.what);
    await expect(rows.nth(0)).toContainText(theirs.metaphor);
    await expect(rows.nth(1).locator(".tag")).toHaveText(`One more to try · ${next.difficulty}`);

    await rows.nth(1).getByRole("button", { name: "Learn this one" }).click();
    await expect(rows.nth(1).locator(".tag")).toContainText("You are learning it");
    await expect(card).toContainText(`Kept: you are learning ${next.name}.`);
    await card.locator(`[data-move="${theirs.name}"]`).getByRole("button", { name: "I have it now" }).click();
    await expect(card.locator(`[data-move="${theirs.name}"]`)).toHaveCount(0);
    await expect(card).toContainText(`Kept: ${theirs.name} is one of your moves now.`);
    expect(await recorded(page, "__marked")).toEqual([
      { name: next.name, have: false },
      { name: theirs.name, have: true },
    ]);

    await rows.nth(0).getByRole("button", { name: "Rehearse it" }).click();
    await expect(card).toHaveCount(0);
    await expect(page.locator('.surface[data-surface="practice"]')).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /** While mixing, learning a move waits, in the coach's words. */
  test("learning a move waits while mixing", async ({ page }) => {
    await openShell(page, "/", {}, { guides: guides.mixing });
    const topics = await openGuides(page);
    const mixing = guides.mixing.find((g) => g.topic === "learn")!;
    await expect(topics.locator('[data-topic="learn"]')).toBeDisabled();
    await expect(topics.locator('[data-topic="learn"] .about')).toHaveText(mixing.not_now!);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** With nothing waiting, the request guide says so in Rust's words. */
  test("the request guide says when nothing is waiting", async ({ page }) => {
    await openShell(page, "/");
    const topics = await openGuides(page);
    const quiet = guides.quiet.find((g) => g.topic === "request")!;
    await expect(topics.locator('[data-topic="request"]')).toBeDisabled();
    await expect(topics.locator('[data-topic="request"] .about')).toHaveText(quiet.not_now!);
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
