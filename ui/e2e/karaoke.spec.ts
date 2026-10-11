/**
 * §107: the singer rotation, as a karaoke host works it.
 *
 * The rules — first come, first served, one song a turn, not ready is the
 * bottom, a singer's key remembered with the song — are `dj_app::karaoke`'s and
 * tested there. What a browser can hold is the wiring: that the song a host
 * picks from the collection reaches Rust with its record, that the key goes
 * where the host sets it, and that loading the up-next song puts it on the
 * deck *before* setting the singer's key, since a key sent first would be
 * applied to whatever was on the deck.
 */
import { expect, test, type Page } from "@playwright/test";

import breaks from "./breaks.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const SINGERS = '.surface[data-surface="karaoke"]';

/** Through the Karaoke activity, which is where a host opens it. */
async function openSingers(page: Page, answers: Record<string, unknown> = {}) {
  await openShell(page, "/", {}, answers);
  await page.getByRole("button", { name: "Activities", exact: true }).click();
  await page.keyboard.press("8");
  await expect(page.locator(SINGERS)).toBeVisible();
}

const calls = (page: Page) =>
  page.evaluate(() => (window as unknown as { __karaoke?: Record<string, unknown>[] }).__karaoke ?? []);

test.describe("§107: the singer rotation", () => {
  /**
   * **The load-bearing one: a request picked from the collection, keyed,
   * loaded in the singer's key, and sung.** The whole of one singer's turn.
   */
  test("a singer's turn, from request to sung", async ({ page }) => {
    await openSingers(page);
    const singers = page.locator(SINGERS);
    await expect(singers.getByText("Nobody is queued")).toBeVisible();

    await singers.getByRole("textbox", { name: "Singer's name" }).fill("Ana");
    await singers.getByRole("textbox", { name: "Song" }).fill("Bach");
    await singers.getByRole("button", { name: "Juan Luis Guerra – Bachata Rosa" }).click();
    await singers.getByRole("button", { name: "Add", exact: true }).click();

    const asked = (await calls(page)).find((call) => call.cmd === "karaoke_ask");
    expect(asked, "the request never reached Rust").toBeTruthy();
    expect(asked?.singer).toBe("Ana");
    expect(asked?.track, "the record picked from the collection was not sent").toBe("a".repeat(64));
    expect(asked?.path).toBe("/music/bachata-rosa.flac");

    const up = singers.getByRole("region", { name: "Up next" });
    await expect(up).toContainText("Ana");
    await expect(up).toContainText("Bachata Rosa");

    await up.getByRole("button", { name: "Raise the key" }).click();
    await expect(up.locator("[data-key]")).toHaveText("key +1");

    // K2: *Load on 2* is Rust's `karaoke load 2` — the record, then its key,
    // then the voice, in that order, held by `karaoke_voice_tests`.
    await up.getByRole("button", { name: "Load on 2" }).click();
    await expect
      .poll(() =>
        page.evaluate(() =>
          ((window as unknown as { __karaoke?: { cmd: string; deck?: number }[] }).__karaoke ?? []).filter(
            (call) => call.cmd === "karaoke_load",
          ),
        ),
      )
      .toEqual([{ cmd: "karaoke_load", deck: 2 }]);

    await up.getByRole("button", { name: "Sang", exact: true }).click();
    await expect(singers.getByText("Nobody is queued")).toBeVisible();
    await singers.getByText("Sung lately").click();
    await expect(singers.getByText("Ana — Juan Luis Guerra – Bachata Rosa (+1)")).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A song nobody has found in the collection yet can still be queued, and says so. */
  test("a song not found yet is queued and says it needs finding", async ({ page }) => {
    await openSingers(page);
    const singers = page.locator(SINGERS);
    await singers.getByRole("textbox", { name: "Singer's name" }).fill("Ben");
    await singers.getByRole("textbox", { name: "Song" }).fill("A song nobody has");
    await singers.getByRole("button", { name: "Add", exact: true }).click();
    const up = singers.getByRole("region", { name: "Up next" });
    await expect(up).toContainText("Not found in the collection yet");
    await expect(up.getByRole("button", { name: /^Load on/ })).toHaveCount(0);
  });

  /**
   * **The vocal, taken out per deck, and said on the deck.** The host chooses
   * on the Singers surface; the deck then carries a chip saying so, because
   * the setting outlives the karaoke screen, and one press on it puts the
   * voice back. The chip must not move the waveform under it (§18).
   */
  test("the host takes the vocal out, the deck says so, and one press puts it back", async ({ page }) => {
    await openSingers(page);
    const singers = page.locator(SINGERS);
    const voice1 = singers.getByRole("group", { name: "Vocal on deck 1" });
    await expect(voice1.getByRole("button", { name: "As recorded" })).toHaveAttribute("aria-pressed", "true");

    await voice1.getByRole("button", { name: "Out" }).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []))
      .toContain("deck 1 voice 0");
    await voice1.getByRole("button", { name: "Guide" }).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []))
      .toContain("deck 1 voice 0.25");

    // The engine answers on the snapshot; the deck shows it without moving.
    const lane = page.locator('.deck[data-deck="1"] .lane');
    const before = await lane.boundingBox();
    await page.evaluate(() => {
      const win = window as unknown as {
        __lastState?: { decks: Record<string, unknown>[] };
        __emit?: (next: unknown) => void;
      };
      const state = win.__lastState;
      if (!state) throw new Error("no state");
      win.__emit?.({ ...state, decks: state.decks.map((d, i) => (i === 0 ? { ...d, voice: 0 } : d)) });
    });
    const chip = page.locator('.deck[data-deck="1"] [data-voice-chip]');
    await expect(chip).toHaveText("Voice out");
    await expect(voice1.getByRole("button", { name: "Out" })).toHaveAttribute("aria-pressed", "true");
    expect((await lane.boundingBox())?.y, "the chip moved the waveform").toBe(before?.y);
    await expect(page.locator('.deck[data-deck="2"] [data-voice-chip]')).toHaveCount(0);

    await chip.click();
    await expect
      .poll(() =>
        page.evaluate(() => ((window as unknown as { __dispatched?: string[] }).__dispatched ?? []).at(-1)),
      )
      .toBe("deck 1 voice 1");
    expect(errorsThrown(page)).toEqual([]);
  });

  /** F8 is the host's own screen: the rotation beside the decks, the collection under them. */
  test("the Karaoke activity opens the rotation and the collection", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Activities", exact: true }).click();
    await page.keyboard.press("8");
    await expect(page.locator('[data-activity="karaoke"]')).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator(SINGERS)).toBeVisible();
    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();
  });
});

/**
 * §107's break music. When it fades in and out is `dj_app::breaks`'s, held by
 * its own tests; what a browser holds is that the host's choices reach Rust as
 * they were made, and that what Rust says back is what the host reads.
 */
test.describe("§107: break music between singers", () => {
  /**
   * **The load-bearing one: switched on with a playlist, a deck and a level,
   * each sent as chosen.** The deck is one of those on screen, and the
   * count on screen goes with it so Rust can refuse one that is not.
   */
  test("the host's choices reach Rust as they were made", async ({ page }) => {
    await openSingers(page);
    const row = page.locator(SINGERS).getByRole("region", { name: "Break music" });
    const on = row.getByRole("checkbox", { name: "Break music between singers" });
    await expect(on).not.toBeChecked();
    // The levels are Rust's, by name, with the one set pressed.
    const levels = row.getByRole("group", { name: "Break music level" }).getByRole("button");
    await expect(levels).toHaveText(breaks.off.levels.map(([, name]) => name as string));
    await expect(row.getByRole("button", { name: "Background" })).toHaveAttribute("aria-pressed", "true");
    // Pressed is seen, not only announced: the set level does not look like
    // the others.
    const colour = (name: string) =>
      row.getByRole("button", { name }).evaluate((button) => getComputedStyle(button).color);
    expect(await colour("Background")).not.toBe(await colour("Quiet"));

    await row.getByRole("combobox", { name: "Break music playlist" }).selectOption({ label: "Last orders (3)" });
    await row.getByRole("group", { name: "Break music deck" }).getByRole("button", { name: "1", exact: true }).click();
    await row.getByRole("button", { name: "Full" }).click();
    await on.check();

    const sets = (await calls(page)).filter((call) => call.cmd === "karaoke_breaks_set");
    expect(sets.at(-1)).toMatchObject({ on: true, deck: 1, playlist: 9, level: 1, decks: 2 });
    await expect(on).toBeChecked();
    await expect(row.getByRole("button", { name: "Full" })).toHaveAttribute("aria-pressed", "true");
    await expect(row.getByRole("status")).toHaveText("Waiting for the room to go quiet");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **What stops it is said, and where it has got to follows it.** Rust's
   * words when there is nothing to play from; and while it is on the line
   * is asked again, so a fade that has started shows without a click.
   */
  test("says what stops it, and follows where it has got to", async ({ page }) => {
    await openSingers(page, { karaoke_breaks: breaks.no_playlist });
    const row = page.locator(SINGERS).getByRole("region", { name: "Break music" });
    await expect(row.getByRole("status")).toHaveText(breaks.no_playlist.problem!);

    await page.evaluate((playing) => {
      (window as unknown as { __breaks: unknown }).__breaks = structuredClone(playing);
    }, breaks.playing);
    await expect(row.getByRole("status")).toHaveText("Playing", { timeout: 5000 });
    await expect(row).toHaveAttribute("data-phase", "playing");
    await page.evaluate(() => {
      (window as unknown as { __breaks: { phase: string } }).__breaks.phase = "fading-out";
    });
    await expect(row.getByRole("status")).toHaveText("Fading out for the singer", { timeout: 5000 });
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A deck the layout does not show is not where break music plays unseen. */
  test("a break deck that is not on screen is said", async ({ page }) => {
    await openSingers(page, { karaoke_breaks: { ...breaks.playing, deck: 4 } });
    const row = page.locator(SINGERS).getByRole("region", { name: "Break music" });
    await expect(row.getByRole("status")).toHaveText(
      "Deck 4 is not on screen. Choose one of these for break music.",
    );
    const decks = row.getByRole("group", { name: "Break music deck" }).getByRole("button");
    await expect(decks).toHaveText(["1", "2"]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **K5: what a record was chosen for is said, in the rail's words**, and
   * the switch that chooses is the host's: off is sent, and kept.
   */
  test("says which song a break record leads into, and the choosing can be switched off", async ({ page }) => {
    await openSingers(page, { karaoke_breaks: breaks.chosen });
    const row = page.locator(SINGERS).getByRole("region", { name: "Break music" });
    const chosen = row.getByLabel("Chosen for");
    const said = breaks.chosen.chosen!;
    await expect(chosen).toHaveText(`${said.title} for ${said.singer}'s ${said.song}${said.why}`);
    await expect(chosen.locator(".why")).toHaveText("+2 BPM · 11B");
    await expect(row.getByRole("status")).toHaveText("Playing");

    const lead = row.getByRole("checkbox", { name: "Lead into the next song" });
    await expect(lead).toBeChecked();
    await lead.uncheck();
    const sets = (await calls(page)).filter((call) => call.cmd === "karaoke_breaks_set");
    expect(sets.at(-1)).toMatchObject({ on: true, deck: 2, playlist: 7, lead: false });
    await expect(lead).not.toBeChecked();

    // K5's mix out of each song: off as Rust starts it, sent when switched,
    // and the choosing switch left as it was.
    const out = row.getByRole("checkbox", { name: "Mix out of each song" });
    await expect(out).not.toBeChecked();
    await out.check();
    const after = (await calls(page)).filter((call) => call.cmd === "karaoke_breaks_set");
    expect(after.at(-1)).toMatchObject({ lead: false, mixOut: true });
    await expect(out).toBeChecked();

    // Leading out of the song just sung, when nobody is waiting.
    await page.evaluate((after) => {
      (window as unknown as { __breaks: unknown }).__breaks = after;
    }, { ...breaks.chosen, chosen: { ...said, after: true } });
    await expect(chosen).toContainText(`${said.title} after ${said.singer}'s`, { timeout: 5000 });
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A switched-off break music claims no choice. */
  test("break music switched off says nothing was chosen", async ({ page }) => {
    await openSingers(page, { karaoke_breaks: { ...breaks.chosen, on: false } });
    const row = page.locator(SINGERS).getByRole("region", { name: "Break music" });
    await expect(row.getByRole("checkbox", { name: "Lead into the next song" })).toBeVisible();
    await expect(row.getByLabel("Chosen for")).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});

test.describe("K5: bringing a singer in", () => {
  const rotation = (bring_in: { from: number; to: number; bringing: boolean } | null) => ({
    singers: [
      {
        name: "Ana",
        songs: [{ title: "Song in A minor", track: "aa".repeat(32), path: "/music/song.wav", key: -2 }],
        turns: 0,
      },
    ],
    up_next: "Ana",
    lately: [],
    bring_in,
  });

  /**
   * **Offered when Rust says there is something to bring them in from, sent
   * once, and said until it is done.** The decks are Rust's to choose; the
   * row says which, and the button cannot be pressed twice.
   */
  test("bring them in is offered, sent, and says it is under way", async ({ page }) => {
    await openSingers(page, { karaoke_rotation: rotation({ from: 2, to: 1, bringing: false }) });
    const up = page.locator(SINGERS).getByRole("region", { name: "Up next" });
    const bring = up.getByRole("button", { name: "Bring them in" });
    await expect(bring).toHaveAttribute("title", "Mix deck 2 into Ana's song on deck 1 at the next phrase");
    await bring.click();
    const pressed = up.getByRole("button", { name: "Bringing Ana in from deck 2…" });
    await expect(pressed).toBeDisabled();
    const sent = (await calls(page)).filter((call) => call.cmd === "karaoke_bring_in");
    expect(sent).toHaveLength(1);

    // Done: Rust stops offering it, and the row follows without a press.
    await page.evaluate(() => {
      (window as unknown as { __rotation: { bring_in: null } }).__rotation.bring_in = null;
    });
    await expect(pressed).toHaveCount(0, { timeout: 5000 });
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Nothing to bring them in from: no button, rather than one that fails. */
  test("no song on a deck offers nothing to press", async ({ page }) => {
    await openSingers(page, { karaoke_rotation: rotation(null) });
    const up = page.locator(SINGERS).getByRole("region", { name: "Up next" });
    await expect(up.getByRole("button", { name: "Sang" })).toBeVisible();
    await expect(up.getByRole("button", { name: /Bring/ })).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});

test.describe("K6: the line a song's voice sings", () => {
  const rotation = {
    singers: [
      {
        name: "Ana",
        songs: [{ title: "Song in A minor", track: "aa".repeat(32), path: "/music/song.wav", key: 0 }],
        turns: 0,
      },
    ],
    up_next: "Ana",
    lately: [],
  };

  /**
   * **Not separated, it says it cannot be scored, and asks again until the
   * line arrives** — then draws it, broken where nobody sings.
   */
  test("a song not yet separated says so, then its line is drawn when it arrives", async ({ page }) => {
    await openSingers(page, { karaoke_rotation: rotation });
    const up = page.locator(SINGERS).getByRole("region", { name: "Up next" });
    await expect(up.getByText(/Not scored: this song has not been separated yet/)).toBeVisible();

    await page.evaluate(() => {
      (window as unknown as { __vocalLine: unknown }).__vocalLine = {
        rate: 20,
        separator: "built-in",
        cents: [-1200, -1200, null, null, -500, -500, -300],
        sung: 5 / 7,
        scorable: true,
      };
    });
    const drawn = up.getByRole("img", { name: "The line Song in A minor's voice sings" });
    await expect(drawn).toBeVisible({ timeout: 8000 });
    await expect(up.getByText(/Not scored/)).toHaveCount(0);
    // Two phrases, so two strokes: the rest is not drawn across.
    const d = await drawn.locator("path").getAttribute("d");
    expect(d?.match(/M/g)).toHaveLength(2);
    // A semitone a unit, the highest note at the top: −300 is 0, −1200 nine below.
    expect(d).toBe("M0 9.0L1 9.0M4 2.0L5 2.0L6 0.0");

    // K6: the singers' scores as they go, and the host's switch.
    await page.evaluate(() => {
      const win = window as unknown as { __rotation: Record<string, unknown> };
      win.__rotation.scores = [{ strip: 0, singer: "Ana", percent: 81.6 }];
    });
    await expect(up.locator("[data-scoring]")).toHaveText("Ana 82%", { timeout: 5000 });
    const scoring = up.getByRole("checkbox", { name: "Score the singers" });
    await expect(scoring).toBeChecked();
    await scoring.uncheck();
    const switched = (await calls(page)).filter((call) => call.cmd === "karaoke_scoring");
    expect(switched.at(-1)).toMatchObject({ on: false });
    await expect(scoring).not.toBeChecked();

    // A line with no single voice in it is said, not drawn.
    await page.evaluate(() => {
      const held = (window as unknown as { __vocalLine: { scorable: boolean; sung: number } }).__vocalLine;
      held.scorable = false;
      held.sung = 0.001;
    });
    await page.locator(SINGERS).getByRole("button", { name: "Load on 1" }).click();
    await expect(up.getByText("Not scored: no single voice was found in this song's separated vocal.")).toBeVisible();
    await expect(drawn).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});

test.describe("K6: versus, and the room's verdict", () => {
  const song = (title: string) => [{ title, track: null, path: null, key: 0 }];
  const rotation = {
    singers: [
      { name: "Ana", songs: song("Valerie"), turns: 0 },
      { name: "Bo", songs: song("Jolene"), turns: 0 },
      { name: "Cy", songs: song("Respect"), turns: 0 },
    ],
    up_next: "Ana",
    lately: [],
  };

  /**
   * **The host decides what decides, and names two teams.** The verdict is
   * sent as pressed; a round starts only with a singer on each side, sends
   * the teams as set up, shows where each stands, and ends on a press.
   */
  test("the verdict is the host's, and a versus round is set up, shown and ended", async ({ page }) => {
    await openSingers(page, { karaoke_rotation: rotation });
    const singers = page.locator(SINGERS);
    const verdict = singers.getByRole("group", { name: "What decides tonight's best" });
    await expect(verdict.getByRole("button", { name: "the singing" })).toHaveAttribute("aria-pressed", "true");
    await verdict.getByRole("button", { name: "the room" }).click();
    await expect(verdict.getByRole("button", { name: "the room" })).toHaveAttribute("aria-pressed", "true");
    expect((await calls(page)).filter((c) => c.cmd === "karaoke_verdict").at(-1)).toMatchObject({ verdict: "room" });

    const versus = singers.getByRole("region", { name: "Versus" });
    await versus.getByRole("textbox", { name: "Team 1's name" }).fill("Reds");
    await versus.getByRole("textbox", { name: "Team 2's name" }).fill("Blues");
    const start = versus.getByRole("button", { name: "Start the round" });
    await versus.getByRole("group", { name: "Ana's team" }).getByRole("button", { name: "Reds" }).click();
    await expect(start).toBeDisabled();
    await versus.getByRole("group", { name: "Bo's team" }).getByRole("button", { name: "Blues" }).click();
    await versus.getByRole("group", { name: "Cy's team" }).getByRole("button", { name: "Reds" }).click();
    await start.click();
    expect((await calls(page)).filter((c) => c.cmd === "karaoke_versus").at(-1)).toMatchObject({
      teams: [
        { name: "Reds", singers: ["Ana", "Cy"] },
        { name: "Blues", singers: ["Bo"] },
      ],
    });
    await expect(versus.getByRole("listitem")).toHaveText([/Reds\s+—\s+0 songs: Ana, Cy/, /Blues\s+—\s+0 songs: Bo/]);
    await versus.getByRole("button", { name: "End the round" }).click();
    expect((await calls(page)).filter((c) => c.cmd === "karaoke_versus").at(-1)).toMatchObject({ teams: null });
    await expect(start).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });
});

test.describe("K7: a quiz made from the collection", () => {
  const quizCalls = (page: Page) =>
    page.evaluate(() => (window as unknown as { __quizCalls?: Record<string, unknown>[] }).__quizCalls ?? []);

  /**
   * **A round is set, asked, revealed and ended by the host's presses.**
   * The answer is not shown before the reveal, since the panel is on a
   * screen the room can see.
   */
  test("the host sets a round, asks, reveals and ends it", async ({ page }) => {
    await openSingers(page);
    const quiz = page.locator(SINGERS).getByRole("region", { name: "Quiz" });
    await expect(quiz).toBeVisible();

    await quiz.getByRole("group", { name: "Quiz questions a round" }).getByRole("button", { name: "5", exact: true }).click();
    await quiz.getByRole("group", { name: "Quiz deck" }).getByRole("button", { name: "1", exact: true }).click();
    await quiz.getByRole("button", { name: "Chipmunk" }).click();
    await quiz.getByRole("button", { name: "Stems in" }).click();
    await quiz.getByRole("button", { name: "Finish the line" }).click();
    expect((await quizCalls(page)).filter((c) => c.cmd === "quiz_set").at(-1)).toMatchObject({
      deck: 1,
      count: 5,
      kinds: ["name-it", "backwards", "stems-in", "finish-the-line"],
    });
    await expect(quiz.getByRole("button", { name: "Chipmunk" })).toHaveAttribute("aria-pressed", "false");

    await quiz.getByRole("button", { name: "Start the quiz" }).click();
    await expect(quiz).toContainText("1/5");
    await expect(quiz).toContainText("Backwards");
    const answers = quiz.getByRole("list", { name: "Answers" }).getByRole("listitem");
    await expect(answers).toHaveCount(4);
    await expect(answers.nth(1)).toContainText("BObsesión — Aventura");
    await expect(quiz.locator("[data-prompt]"), "no line shown for a clip to name").toHaveCount(0);
    await expect(quiz.locator("[data-right]"), "the answer was shown before the reveal").toHaveCount(0);

    await quiz.getByRole("button", { name: "Show the answer" }).click();
    await expect(quiz.locator("[data-right]")).toHaveText("BObsesión — Aventura");
    await expect(quiz.getByRole("button", { name: "Show the answer" })).toBeDisabled();

    await quiz.getByRole("button", { name: "Play it again" }).click();
    await quiz.getByRole("button", { name: "Next question" }).click();
    await expect(quiz).toContainText("2/5");
    await expect(quiz.locator("[data-right]")).toHaveCount(0);

    await quiz.getByRole("button", { name: "End the quiz" }).click();
    await expect(quiz.getByRole("button", { name: "Start the quiz" })).toBeVisible();
    expect((await quizCalls(page)).map((c) => c.cmd)).toEqual(
      expect.arrayContaining(["quiz_next", "quiz_again", "quiz_reveal", "quiz_end"]),
    );
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The last kind cannot be switched off: a round of no kind asks nothing. */
  test("the last kind stays on", async ({ page }) => {
    await openSingers(page, {
      quiz: {
        deck: 2,
        kinds: ["chipmunk"],
        count: 10,
        counts: [5, 10, 15],
        asked: 0,
        question: null,
        playing: false,
        problem: null,
        quiet: false,
        phones: { players: 0, answered: 0, counts: null, leaders: [] },
      },
    });
    const quiz = page.locator(SINGERS).getByRole("region", { name: "Quiz" });
    await quiz.getByRole("button", { name: "Chipmunk" }).click();
    expect((await quizCalls(page)).filter((c) => c.cmd === "quiz_set")).toEqual([]);
    await expect(quiz.getByRole("button", { name: "Chipmunk" })).toHaveAttribute("aria-pressed", "true");
  });

  /** What stops a question is said, and so is a fader left down. */
  test("says what stops a question, and a fader left down", async ({ page }) => {
    await openSingers(page, {
      quiz: {
        deck: 3,
        kinds: ["name-it"],
        count: 10,
        counts: [5, 10, 15],
        asked: 0,
        question: null,
        playing: false,
        problem: null,
        quiet: true,
        phones: { players: 0, answered: 0, counts: null, leaders: [] },
      },
    });
    const quiz = page.locator(SINGERS).getByRole("region", { name: "Quiz" });
    await expect(quiz.getByRole("status")).toHaveText("Deck 3's fader is down: the room will not hear the clip.");
  });

  /**
   * **K7 step 2: the phones on the host's panel** — how many play and
   * answered while a question is open, each answer's count only once it is
   * shown, and the top of the night's leaderboard.
   */
  test("the phones playing along are counted, and their answers once shown", async ({ page }) => {
    await openSingers(page, {
      quiz: {
        deck: 2,
        kinds: ["name-it"],
        count: 5,
        counts: [5, 10, 15],
        asked: 0,
        question: null,
        playing: false,
        problem: null,
        quiet: false,
        phones: { players: 3, answered: 2, counts: null, leaders: [["Ana", 1000], ["Bo", 625]] },
      },
      quiz_counts: [0, 2, 1, 0],
    });
    const quiz = page.locator(SINGERS).getByRole("region", { name: "Quiz" });
    await quiz.getByRole("button", { name: "Start the quiz" }).click();
    await expect(quiz.locator("[data-phones]")).toHaveText("3 phones playing, 2 answered");
    await expect(quiz.locator(".answers .count"), "counts before the answer is shown").toHaveCount(0);
    const board = quiz.getByRole("list", { name: "Quiz leaderboard" }).getByRole("listitem");
    await expect(board).toHaveText(["Ana1000", "Bo625"]);

    await quiz.getByRole("button", { name: "Show the answer" }).click();
    const answers = quiz.getByRole("list", { name: "Answers" }).getByRole("listitem");
    await expect(answers.nth(1).locator(".count")).toHaveText(/^2\s+phones$/);
    await expect(answers.nth(2).locator(".count")).toHaveText(/^1\s+phone$/);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The panel keeps counting after the clip ends**: phones answer for ten
   * seconds more, so the count is asked for while the question is open, not
   * only while the clip plays. Found driving it.
   */
  test("answers after the clip are counted while the question is open", async ({ page }) => {
    await openSingers(page, {
      quiz: {
        deck: 2,
        kinds: ["name-it"],
        count: 5,
        counts: [5, 10, 15],
        asked: 0,
        question: null,
        playing: false,
        problem: null,
        quiet: false,
        phones: { players: 1, answered: 0, counts: null, leaders: [] },
      },
    });
    const quiz = page.locator(SINGERS).getByRole("region", { name: "Quiz" });
    await quiz.getByRole("button", { name: "Start the quiz" }).click();
    await expect(quiz.locator("[data-phones]")).toHaveText("1 phone playing, 0 answered");
    type Held = { playing: boolean; phones: { answered: number } };
    // The clip ends, and the panel hears so…
    await page.evaluate(() => {
      ((window as unknown as { __quiz: Held }).__quiz).playing = false;
    });
    await expect(quiz.getByRole("status")).toHaveCount(0);
    // …then a phone answers.
    await page.evaluate(() => {
      ((window as unknown as { __quiz: Held }).__quiz).phones.answered = 1;
    });
    await expect(quiz.locator("[data-phones]")).toHaveText("1 phone playing, 1 answered", { timeout: 4000 });
  });

  /** **Finish the line: the host sees the line shown, above the answers.** */
  test("finish the line shows the host the line", async ({ page }) => {
    await openSingers(page, {
      quiz_question: {
        kind: "finish-the-line",
        words: "Finish the line",
        answers: ["Never letting go of your hand tonight", "B line here", "C line here", "D line here"],
        right: null,
        number: 1,
        prompt: "Under the moonlight we dance",
      },
    });
    const quiz = page.locator(SINGERS).getByRole("region", { name: "Quiz" });
    await quiz.getByRole("button", { name: "Start the quiz" }).click();
    await expect(quiz.locator("[data-prompt]")).toHaveText("Under the moonlight we dance…");
    await expect(quiz).toContainText("Finish the line");
  });
});

test.describe("K8: a singer's range", () => {
  /** **The up-next singer's range, read tonight, is on their row in notes.** */
  test("the up-next singer's range is on their row", async ({ page }) => {
    const song = (title: string) => [{ title, track: null, path: null, key: 0 }];
    await openSingers(page, {
      karaoke_rotation: {
        singers: [
          { name: "Ana", songs: song("Valerie"), turns: 0, range: { low: -2400, high: -500 } },
          { name: "Bo", songs: song("Jolene"), turns: 0 },
        ],
        up_next: "Ana",
        lately: [],
      },
    });
    const up = page.locator(SINGERS).getByRole("region", { name: "Up next" });
    await expect(up.locator("[data-range]")).toHaveText("A2 – E4, an octave and a fifth");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **K8 step 2: a song chosen for a singer with a range is marked with how
   * it fits, and its key comes with it**; *Songs for them* asks for a fitting
   * record in its key.
   */
  test("a song for a singer with a range is marked, and asked for in its key", async ({ page }) => {
    const song = (title: string) => [{ title, track: null, path: null, key: 0 }];
    await openSingers(page, {
      karaoke_rotation: {
        singers: [{ name: "Ana", songs: song("Valerie"), turns: 0, range: { low: -2400, high: -500 } }],
        up_next: "Ana",
        lately: [],
      },
      karaoke_fits: [{ fit: "key", key: -2 }, { fit: "no" }, { fit: "unknown" }],
      karaoke_songs_for: [
        {
          track: "b".repeat(64),
          path: "/music/obsesion.flac",
          title: "Obsesión",
          artist: "Aventura",
          offer: { fit: "as-recorded" },
        },
      ],
    });
    const singers = page.locator(SINGERS);
    await singers.getByRole("textbox", { name: "Singer's name" }).fill("Ana");
    await singers.getByRole("textbox", { name: "Song" }).fill("Bach");
    const first = singers.getByRole("list", { name: "Songs in your collection" }).getByRole("button").first();
    await expect(first).toContainText("fits -2");
    await first.click();
    await singers.getByRole("button", { name: "Add", exact: true }).click();
    const asked = (await calls(page)).filter((call) => call.cmd === "karaoke_ask").at(-1);
    expect(asked?.key, "the key its fit offered").toBe(-2);

    await singers.getByRole("textbox", { name: "Singer's name" }).fill("Ana");
    await singers.getByRole("button", { name: "Songs for Ana" }).click();
    const offered = singers.getByRole("region", { name: "Songs for Ana" }).getByRole("button");
    await expect(offered).toHaveText(/Aventura – Obsesión\s*fits as recorded/);
    await offered.click();
    const again = (await calls(page)).filter((call) => call.cmd === "karaoke_ask").at(-1);
    expect(again).toMatchObject({ singer: "Ana", track: "b".repeat(64), key: 0 });
    expect(errorsThrown(page)).toEqual([]);
  });
});

test.describe("K9: a singer's song, recorded for them", () => {
  const rotation = {
    singers: [{ name: "Ana", songs: [{ title: "Valerie", track: null, path: "/music/valerie.mp3", key: 0 }], turns: 0 }],
    up_next: "Ana",
    lately: [],
    may_record: true,
  };

  /**
   * **Offered only for a singer who agreed, and said while it runs.** The
   * agreement, the microphone and the file are Rust's and tested there
   * (`performance_tests`); the panel offers *Record their song*, sends who,
   * shows the length running, and stops it.
   */
  test("the host records the song of a singer who agreed, and stops it", async ({ page }) => {
    await openSingers(page, { karaoke_rotation: rotation });
    const up = page.locator(SINGERS).getByRole("region", { name: "Up next" });
    await page.evaluate(() => {
      const held = (window as unknown as { __rotation: Record<string, unknown> }).__rotation;
      held.recording = { singer: "Ana", seconds: 65.4 };
    });
    await up.getByRole("button", { name: "Record their song" }).click();
    await expect
      .poll(async () => (await calls(page)).filter((call) => call.cmd === "karaoke_record_song"))
      .toEqual([{ cmd: "karaoke_record_song", singer: "Ana" }]);
    await expect(up.locator("[data-recording]")).toHaveText(/Recording Ana's song — 1:05/);

    await page.evaluate(() => {
      const held = (window as unknown as { __rotation: Record<string, unknown> }).__rotation;
      held.recording = null;
    });
    await up.getByRole("button", { name: "Stop recording" }).click();
    await expect
      .poll(async () => (await calls(page)).filter((call) => call.cmd === "karaoke_record_stop").length)
      .toBe(1);
    await expect(up.locator("[data-recording]")).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A singer who did not agree is not offered it. */
  test("a singer who did not agree is not offered it", async ({ page }) => {
    await openSingers(page, { karaoke_rotation: { ...rotation, may_record: false } });
    const up = page.locator(SINGERS).getByRole("region", { name: "Up next" });
    await expect(up).toContainText("Ana");
    await expect(up.getByRole("button", { name: "Record their song" })).toHaveCount(0);
  });
});
