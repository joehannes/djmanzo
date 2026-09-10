/**
 * §35's room baseline, and the sentence it exists to refuse.
 *
 * > Never use simplistic global thresholds such as "80 dB = high energy".
 * > Instead establish a venue/session baseline. Compare: current room
 * > activity / recent room activity / earlier room activity / activity at
 * > similar session phases.
 *
 * The arithmetic is Rust's and is tested in `dj_assistant::room` — including
 * the guard that stops a phase being compared with itself, which is the one
 * that would otherwise produce a confident answer from nothing. What is
 * measured here is the part a type-check cannot see: that all four reaches
 * reach the screen, that a reach with nothing behind it reads as *nothing*
 * rather than as "usual", and that the column comparing similar phases names
 * the phase it means.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/*
  The room panel lives inside the Assistant surface, in a fold under the
  occasion — "the one thing djmanzo will say about a camera is that the floor
  is doing something other than the night you set up", so it sits beside the
  control it contradicts. That is why this is `.room` inside `assistant` and
  not a surface of its own.
*/
const ROOM = '.surface[data-surface="assistant"] .room';

/** A room half an hour into a night that has read as peak. */
const READ = {
  watching: true,
  recent: 90,
  enough: true,
  notes: [
    "The room is busier than usual tonight.",
    "The room is stiller than it usually is when the night is at peak.",
  ],
  disagreement: null,
  hour: 1,
  light: 0.42,
  movement: 0.61,
  loudness: 0.55,
  phase: "peak",
  baseline: [
    {
      sense: "movement",
      now: 0.61,
      notes: ["The room is stiller than it usually is when the night is at peak."],
      against: [
        {
          horizon: "recent",
          than: "than it has been for the last twenty minutes",
          against: "higher",
          notable: true,
        },
        { horizon: "tonight", than: "than it has been tonight", against: "higher", notable: true },
        {
          horizon: "phase",
          than: "than it usually is when the night is at peak",
          against: "lower",
          notable: true,
        },
      ],
    },
    {
      sense: "loudness",
      now: 0.55,
      notes: [],
      against: [
        {
          horizon: "recent",
          than: "than it has been for the last twenty minutes",
          against: "usual",
          notable: false,
        },
        { horizon: "tonight", than: "than it has been tonight", against: "usual", notable: false },
      ],
    },
  ],
};

async function roomOpen(
  page: import("@playwright/test").Page,
  read: unknown = READ,
) {
  await openShell(page, "/", {}, { room_read: read });
  await page.getByRole("button", { name: "Assistant", exact: true }).click();
  await page.getByText("The room", { exact: true }).click();
  await expect(page.locator(ROOM)).toBeVisible();
}

test.describe("§35's room baseline", () => {
  /**
   * **All four of §35's comparisons are on the screen at once.**
   *
   * The current activity as a number, and the three reaches it is placed
   * against beside it. Any one of them alone is the threshold §35 opens by
   * refusing: 61% means nothing until it is 61% *of something*.
   */
  test("shows the current activity against all three reaches", async ({ page }) => {
    await roomOpen(page);

    const row = page.locator(`${ROOM} table.baseline tbody tr`).first();
    await expect(row).toBeVisible();
    await expect(row.locator("th")).toHaveText("movement");
    // §35's *current* room activity, as the number the rest are of.
    await expect(row.locator("td").first()).toHaveText("61%");

    const reaches = row.locator("td.reach");
    await expect(reaches).toHaveCount(3);
    await expect(reaches.nth(0)).toHaveAttribute("data-against", "higher");
    await expect(reaches.nth(1)).toHaveAttribute("data-against", "higher");
    await expect(reaches.nth(2)).toHaveAttribute("data-against", "lower");
    expect(errorsThrown(page), "the baseline threw while rendering").toEqual([]);
  });

  /**
   * **A reach with nothing behind it says nothing.**
   *
   * The failure that matters: loudness has never been measured at a peak
   * before, and a cell rendering "usual" there would be djmanzo claiming a
   * comparison it has not got. It has to read as absent, and it has to not
   * read as the low end either.
   */
  test("a reach it cannot compare against is blank, not usual", async ({ page }) => {
    await roomOpen(page);

    const loudness = page.locator(`${ROOM} table.baseline tbody tr`).nth(1);
    await expect(loudness.locator("th")).toHaveText("loudness");
    const phase = loudness.locator("td.reach").nth(2);
    await expect(phase).toHaveText("—");
    await expect(phase).toHaveAttribute("data-against", "unknown");
    await expect(phase).not.toHaveClass(/notable/);
  });

  /**
   * **The phase column names the phase.**
   *
   * A column headed "similar phases" over a cell reading "lower" is a claim
   * with its subject missing. When the night has not read at all it says so
   * generically instead, which is the honest version of the same header.
   */
  test("names the phase it is comparing against", async ({ page }) => {
    await roomOpen(page);
    await expect(
      page.locator(`${ROOM} table.baseline thead th`).nth(4),
    ).toHaveText("At peak");

    await roomOpen(page, { ...READ, phase: null });
    await expect(
      page.locator(`${ROOM} table.baseline thead th`).nth(4),
    ).toHaveText("Similar phases");
  });

  /**
   * The sentences come from Rust and are drawn as they arrive. Worth pinning
   * because the panel's whole discipline is that it never words a comparison
   * itself — a second wording is a second answer.
   */
  test("says what Rust said, including the reach", async ({ page }) => {
    await roomOpen(page);
    const notes = page.locator(`${ROOM} ul.notes li`);
    await expect(notes).toHaveCount(2);
    await expect(notes.nth(1)).toHaveText(
      "The room is stiller than it usually is when the night is at peak.",
    );
  });

  /**
   * A room nothing has looked at draws no table at all — not an empty one.
   * The default fixture is that room, so this also guards every other test in
   * the suite against the panel throwing on it.
   */
  test("a room nobody is watching has no baseline to draw", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Assistant", exact: true }).click();
    await page.getByText("The room", { exact: true }).click();
    await expect(page.locator(ROOM)).toBeVisible();
    await expect(page.locator(`${ROOM} table.baseline`)).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});

/**
 * §37's causal crowd analysis, and the word it must never say.
 *
 * > Correlate changes with DJ actions. […] "This type of transition has
 * > historically improved room response here."
 *
 * The arithmetic — the twelve-to-thirty second window, one vote per night,
 * the threshold below which nothing is said — is Rust's and is tested in
 * `dj_app::response`. What is measured here is what reaches a DJ: that the
 * sentence carries its own evidence, that nothing appears until there is
 * something to say, and that the panel does not turn a correlation into a
 * cause on its way to the screen.
 */
test.describe("§37's history", () => {
  const HISTORY = [
    {
      setting: "club",
      style: "blend",
      sense: "movement",
      nights: 5,
      usually: "rose",
      says: "After a blend at a club night, the floor has picked up — 4 of 5 nights.",
    },
    // Four nights that disagree. It is in the table and it says nothing, which
    // is the case the threshold exists for.
    {
      setting: "club",
      style: "cut",
      sense: "movement",
      nights: 4,
      usually: null,
      says: null,
    },
  ];

  /**
   * **The sentence carries its own evidence.**
   *
   * "Historically" with no count behind it is the kind of claim a DJ cannot
   * weigh and so cannot use. Four of five is something they can disagree with.
   */
  test("says what happened after, and over how many nights", async ({ page }) => {
    await openShell(page, "/", {}, { room_history: HISTORY });
    await page.getByRole("button", { name: "Assistant", exact: true }).click();
    await page.getByText("The room", { exact: true }).click();

    const said = page.locator(`${ROOM} .history li`);
    await expect(said).toHaveCount(1);
    const sentence = (await said.first().textContent()) ?? "";
    expect(sentence).toContain("4 of 5 nights");
    expect(sentence).toContain("club night");
    expect(sentence).toContain("blend");
    // §37 is a correlation. The panel must not promote it.
    expect(sentence, "the panel claimed a cause").not.toContain("because");
    expect(sentence).not.toContain("improved");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Nights that disagree are silent, not shown as a weak finding.**
   *
   * The row is in the answer with `says: null`, and a panel that rendered it
   * anyway — greyed out, or as "no clear pattern" — would be putting a
   * non-finding in front of a DJ mid-set.
   */
  test("shows nothing for the nights that disagree", async ({ page }) => {
    await openShell(page, "/", {}, { room_history: HISTORY });
    await page.getByRole("button", { name: "Assistant", exact: true }).click();
    await page.getByText("The room", { exact: true }).click();

    await expect(page.locator(`${ROOM} .history li`)).toHaveCount(1);
    await expect(page.locator(`${ROOM} .history`)).not.toContainText("cut");
  });

  /** With nothing recorded there is no section at all, not an empty one. */
  test("a djmanzo that has never watched a room says nothing", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Assistant", exact: true }).click();
    await page.getByText("The room", { exact: true }).click();
    await expect(page.locator(ROOM)).toBeVisible();
    await expect(page.locator(`${ROOM} .history`)).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});
