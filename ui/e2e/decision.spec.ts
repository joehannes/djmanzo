/**
 * §118a: a decision, when one becomes necessary under pressure.
 *
 * > whenever in a stressful situation a decision becomes necessary a super
 * > easy and quick to identify and select widget can pop up (as unobtrusive
 * > or occupying as necessary and adequate)
 *
 * When a record runs out with nothing after it, and how much room that
 * deserves, are `dj_app::whisper`'s and `dj_app::decide`'s, tested there;
 * which records are offered is the rail's ranking. What the browser holds:
 * a line stays a line until asked, a card comes up by itself, the last
 * seconds take more of the screen without covering the decks, one press
 * loads the chosen record on the free deck, the stall loops the deck running
 * out and leaves the choice up, and Not now is sent as itself.
 */
import { expect, test, type Page } from "@playwright/test";

import decide from "./decide.json" with { type: "json" };
import { ANSWERS, errorsThrown, openShell } from "./shell";

/** The rail's fixture, and a third record, so each direction has its own. */
type Row = { track: Record<string, unknown> } & Record<string, unknown>;
const rail = ANSWERS.suggest_next as Row[];
const RAIL = {
  suggest_next: [
    ...rail,
    {
      ...rail[0],
      track: {
        ...rail[0].track,
        id: "d".repeat(64),
        path: "/music/bachata-rosa.flac",
        title: "Bachata Rosa",
      },
    },
  ],
};

const runningOut = (presence: "line" | "card" | "whole", left = "0:15") => ({
  kind: "running-out",
  says: `Deck 1 ends in ${left} and nothing else is loaded.`,
  offer: "Choose the next record",
  run: "ui show next",
  urgent: true,
  deck: 1,
  presence,
});

/** Deliver a frame carrying `proposal`, as the pump does. */
async function propose(page: Page, proposal: unknown) {
  await page.evaluate((proposal) => {
    const win = window as unknown as {
      __lastState?: Record<string, unknown>;
      __emit?: (next: unknown) => void;
    };
    win.__emit?.({ ...win.__lastState, whisper: proposal });
  }, proposal);
}

const recorded = <T,>(page: Page, name: string) =>
  page.evaluate((name) => ((window as unknown as Record<string, unknown>)[name] ?? []) as T[], name);

const DECISION = '[role="dialog"][aria-label="Choose the next record"]';

test.describe("§118a: a record running out", () => {
  /** A line while there is time: nothing comes up until the DJ asks. */
  test("with time left it is a line, and the choice opens when asked", async ({ page }) => {
    await openShell(page, "/", {}, RAIL);
    await propose(page, runningOut("line", "0:26"));
    const line = page.locator('.whisper[data-whisper="running-out"]');
    await expect(line.locator(".says")).toContainText("0:26");
    await page.waitForTimeout(200);
    await expect(page.locator(DECISION)).toHaveCount(0);

    await line.getByRole("button", { name: "Choose the next record" }).click();
    await expect(page.locator(DECISION)).toBeVisible();
    // Asking opened it; it ran nothing and answered nothing.
    expect(await recorded(page, "__dispatched")).toEqual([]);
    expect(await recorded(page, "__whisperAnswered")).toEqual([]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The load-bearing one.** Once the seconds call for it the choice comes
   * up by itself: three records, one per direction in Rust's words and
   * order, told apart by direction; one press loads the chosen record on the
   * free deck, answers the whisper, and the choice goes.
   */
  test("a card comes up by itself, and one press loads the record on the free deck", async ({ page }) => {
    await openShell(page, "/", {}, RAIL);
    await propose(page, runningOut("card"));
    const decision = page.locator(DECISION);
    await expect(decision).toBeVisible();
    await expect(decision).toHaveAttribute("data-presence", "card");
    await expect.poll(() => recorded(page, "__decisionAsked")).toEqual([1]);

    const choices = decision.locator(".choice");
    await expect(choices).toHaveCount(3);
    expect(await choices.evaluateAll((all) => all.map((c) => c.getAttribute("data-direction")))).toEqual(
      decide.directions.map((d) => d.direction),
    );
    for (const [i, d] of decide.directions.entries()) {
      await expect(choices.nth(i).locator(".way")).toHaveText(d.says);
    }
    // Told apart before they are read: each direction its own colour.
    const colours = await choices.evaluateAll((all) => all.map((c) => getComputedStyle(c).borderTopColor));
    expect(new Set(colours).size, colours.join(" ")).toBe(3);

    // Under the top bar, never over it: REC and SAFE stay where they are.
    const bar = (await page.locator(".topbar").boundingBox())!;
    expect((await decision.boundingBox())!.y).toBeGreaterThanOrEqual(bar.y + bar.height);

    // The frames keep coming while it is up, and it is asked once.
    await propose(page, runningOut("card", "0:14"));
    await propose(page, runningOut("card", "0:13"));
    await expect(decision.locator(".says")).toContainText("0:13");
    expect(await recorded(page, "__decisionAsked")).toEqual([1]);

    const second = choices.nth(1);
    const title = (await second.locator(".title").textContent())!.trim();
    await second.click();
    // The record pressed, on the deck with nothing on it -- the hold
    // direction's own record, which is the rail's second.
    const held = RAIL.suggest_next[1].track;
    expect(title).toBe(held.title);
    await expect
      .poll(() => recorded<{ deck: number; path: string }>(page, "__loadedTracks"))
      .toEqual([{ deck: 2, path: held.path }]);
    await expect.poll(() => recorded(page, "__whisperAnswered")).toEqual([{ kind: "running-out", answer: "taken" }]);
    await expect(decision).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * The last seconds take more of the screen, and still leave the decks
   * reachable: a DJ in trouble may reach for the loop first.
   */
  test("the last seconds take more of the screen, and the decks stay reachable", async ({ page }) => {
    await openShell(page, "/", {}, RAIL);
    await propose(page, runningOut("card"));
    const decision = page.locator(DECISION);
    await expect(decision.locator(".choice")).toHaveCount(3);
    const card = await decision.boundingBox();
    const cardTile = await decision.locator(".choice").first().boundingBox();

    await propose(page, runningOut("whole", "0:06"));
    await expect(decision).toHaveAttribute("data-presence", "whole");
    await expect
      .poll(async () => (await decision.boundingBox())!.width * (await decision.boundingBox())!.height)
      .toBeGreaterThan(card!.width * card!.height * 1.3);
    const wholeTile = await decision.locator(".choice").first().boundingBox();
    expect(wholeTile!.height).toBeGreaterThan(cardTile!.height);

    // Nothing laid over the rest of the booth: a control outside the card
    // is still what a click there reaches -- no scrim, no invisible layer.
    const around = page.getByRole("button", { name: "Toggle limiter" });
    await expect(around).toBeVisible();
    const card2 = (await decision.boundingBox())!;
    const box = (await around.boundingBox())!;
    const clear =
      box.y > card2.y + card2.height || box.y + box.height < card2.y || box.x > card2.x + card2.width || box.x + box.width < card2.x;
    expect(clear, "the limiter sits outside the card, so it can say something").toBe(true);
    expect(
      await around.evaluate((el) => {
        const r = el.getBoundingClientRect();
        const hit = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
        return hit !== null && (hit === el || el.contains(hit));
      }),
    ).toBe(true);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Buying time loops the deck running out, and leaves the choice up. */
  test("the stall loops the deck running out and keeps the choice", async ({ page }) => {
    await openShell(page, "/", {}, RAIL);
    await propose(page, runningOut("whole", "0:05"));
    const decision = page.locator(DECISION);
    await decision.getByRole("button", { name: decide.stall.says }).click();
    await expect.poll(() => recorded(page, "__dispatched")).toContain(decide.stall.run);
    await expect(decision).toBeVisible();
    expect(await recorded(page, "__whisperAnswered")).toEqual([]);

    // Not now is its own answer, and takes it away.
    await decision.getByRole("button", { name: "Not now" }).click();
    await expect.poll(() => recorded(page, "__whisperAnswered")).toEqual([{ kind: "running-out", answer: "not-now" }]);
    await expect(decision).toHaveCount(0);
    expect(await recorded(page, "__loadedTracks")).toEqual([]);
    expect(errorsThrown(page)).toEqual([]);
  });
});
