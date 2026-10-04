/**
 * §119: what the room and the stream said, on the records they said it about.
 *
 * What a reaction is about, which way it leans, the record and the part of
 * it it lands on, the night read back and the goals answered are
 * `dj_app::crowd`'s, tested there (`crowd.json`, Rust's reading of a night).
 * What the browser holds: the panel draws Rust's reading and nothing of its
 * own -- every reaction with its subject and its place, the numbers, the
 * records and their moments, the goals met or not -- and the DJ's own
 * writes are sent as they are: a goal (a share as a share), a reaction
 * noted, a chat log timed from when the stream began.
 */
import { expect, test, type Page } from "@playwright/test";

import crowd from "./crowd.json" with { type: "json" };
import layers from "./layers.json" with { type: "json" };
import { ANSWERS, errorsThrown, openShell } from "./shell";

const PANEL = '.surface[data-surface="crowd"]';
const view = crowd.view;

const calls = (page: Page) =>
  page.evaluate(() => ((window as unknown as { __crowdCalls?: unknown[] }).__crowdCalls ?? []) as Record<string, unknown>[]);

async function withCrowd(page: Page) {
  await page.setViewportSize({ width: 1400, height: 900 });
  await openShell(page, "/", {}, {
    cockpit_workspace: {
      workspace: {
        name: "Crowd",
        about: "",
        surfaces: [{ surface: "crowd", dock: "right", order: 0, size: null, collapsed: false, pinned: false }],
        density: "standard",
        focus: "preparing",
        theme: "",
        decks: 2,
        locked: [],
      },
      notes: [],
      permits: { rearrange: true, resize: true, retheme: true, restyle: true },
    },
  });
  const panel = page.locator(PANEL);
  await expect(panel.locator(".crowd .tiles")).toBeVisible();
  return panel;
}

test.describe("§119: the crowd", () => {
  /**
   * **The load-bearing one.** Every reaction as Rust read it -- its subject
   * and lean, newest first -- placed on the record and the part Rust
   * placed it on; the numbers and the records with their moments are
   * Rust's summary.
   */
  test("what was said is drawn as Rust read it, on the music it was about", async ({ page }) => {
    const panel = await withCrowd(page);
    const items = panel.locator(".feed li");
    await expect(items).toHaveCount(view.placed.length);
    const newest = [...view.placed].reverse();
    expect(await items.evaluateAll((all) => all.map((li) => [li.getAttribute("data-about"), li.getAttribute("data-lean")]))).toEqual(
      newest.map((p) => [p.about, p.lean]),
    );
    // Placed: the drop, where Rust found it; a question on the record.
    const drop = newest.find((p) => p.part === "drop")!;
    const dropItem = items.filter({ hasText: drop.reaction.text });
    await expect(dropItem.locator(".where")).toContainText(`${view.played[drop.record!].title} · the drop at 1:00`);
    const asked = newest.find((p) => p.about === "track")!;
    await expect(items.filter({ hasText: asked.reaction.text }).locator(".where")).toContainText(
      `${view.played[asked.record!].title} · `,
    );

    await expect(panel.locator('[data-count="reactions"]')).toHaveText(String(view.summary.reactions));
    await expect(panel.locator('[data-count="moments"]')).toHaveText(String(view.summary.moments));
    await expect(panel.locator('[data-count="warmth"]')).toHaveText(`${Math.round(view.summary.warmth! * 100)}%`);
    const records = panel.locator(".records li");
    await expect(records).toHaveCount(view.summary.records.length);
    await expect(records.first().locator("strong")).toHaveText(view.summary.records[0].title);
    // Four: three the rules read, and the bassline the DJ's provider placed.
    await expect(records.first().locator(".moment")).toHaveText(["the drop 1:00 ×4"]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Goals as Rust answered them, and the DJ's own kept -- a share as a share. */
  test("goals are Rust's answers, and a goal of the DJ's own is kept", async ({ page }) => {
    const panel = await withCrowd(page);
    const goals = panel.locator(".goals li");
    await expect(goals).toHaveCount(view.goals.length);
    expect(await goals.evaluateAll((all) => all.map((li) => li.getAttribute("data-met")))).toEqual(
      view.goals.map((g) => (g.met === null ? "unknown" : String(g.met))),
    );
    // What the night reached, then the goal -- never the two run together
    // as if the night had claimed it.
    const first = view.goals[0];
    await expect(goals.first().locator(".value")).toHaveText(
      `${Math.round(first.value! * 100)}% · goal ${first.goal.at_most ? "≤" : "≥"} ${Math.round(first.goal.target * 100)}%`,
    );
    // A past night by when it began, not by its id.
    const past = panel.getByRole("combobox", { name: "Which night" }).locator("option").nth(1);
    await expect(past).not.toHaveText(view.sessions[1]);

    await panel.getByRole("button", { name: "+ A goal of your own" }).click();
    await panel.getByRole("combobox", { name: "What the goal measures" }).selectOption("warmth");
    await panel.getByRole("textbox", { name: "The number" }).fill("80");
    await panel.getByRole("textbox", { name: "The goal in your words" }).fill("Warm all night");
    await panel.getByRole("button", { name: "Add", exact: true }).click();
    await expect
      .poll(async () => (await calls(page)).find((c) => c.cmd === "crowd_settings_save")?.settings)
      .toEqual({
        delay: view.delay,
        goals: [...view.goals.map((g) => g.goal), { name: "Warm all night", measure: "warmth", target: 0.8, at_most: false }],
        // The switch for the provider, kept as it was.
        ai: true,
      });
    await expect(goals).toHaveCount(view.goals.length + 1);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A reaction noted is sent as it was typed; a log is timed from when the stream began. */
  test("a reaction is noted, and a chat log is timed from when the stream began", async ({ page }) => {
    const panel = await withCrowd(page);
    await panel.getByRole("textbox", { name: "Note a reaction" }).fill("big cheer 🔥");
    await panel.getByRole("button", { name: "Note it" }).click();
    await expect
      .poll(async () => (await calls(page)).find((c) => c.cmd === "crowd_add"))
      .toMatchObject({ text: "big cheer 🔥", source: "other" });
    // Pressing the button hands the keyboard back to the field: the next
    // letters typed are the next note, not the booth's shortcuts.
    await expect(panel.getByRole("textbox", { name: "Note a reaction" })).toBeFocused();

    await page.evaluate(() => {
      (window as unknown as { __dialogAnswer?: string }).__dialogAnswer = "/home/dj/stream-chat.txt";
    });
    await panel.getByRole("button", { name: "Bring in a chat log" }).click();
    const began = panel.getByRole("textbox", { name: "When the stream began" });
    // Begun at the first record, on its own date, in the page's own zone.
    const first = new Date(view.played[0].at * 1000);
    const clock = `${String(first.getHours()).padStart(2, "0")}:${String(first.getMinutes()).padStart(2, "0")}`;
    await expect(began).toHaveValue(clock);
    await began.fill("21:30");
    await panel.getByRole("button", { name: "Choose the file" }).click();
    const expected = new Date(view.played[0].at * 1000);
    expected.setHours(21, 30, 0, 0);
    await expect
      .poll(async () => (await calls(page)).find((c) => c.cmd === "crowd_import"))
      .toMatchObject({ path: "/home/dj/stream-chat.txt", start: Math.round(expected.getTime() / 1000), source: "youtube" });
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The night on one line, as Rust drew it** (`crowd::timeline`): every
   * stretch the room heard, every part where it was heard, every reaction
   * when what it was about was heard and leaning as Rust read it -- and a
   * cursor, moved by the keys or the pointer, that names the moment under
   * it: the record, how far in, the part just heard, how busy that minute was.
   */
  test("the night on one line names the moment under the cursor", async ({ page }) => {
    const line = view.timeline!;
    const panel = await withCrowd(page);
    const night = panel.locator("[data-night-line]");
    await expect(night.locator("[data-span]")).toHaveCount(line.spans.length);
    await expect(night.locator("[data-landmark]")).toHaveCount(line.landmarks.length);
    expect(await night.locator("[data-landmark]").evaluateAll((all) => all.map((l) => l.getAttribute("data-landmark")))).toEqual(
      line.landmarks.map((l) => l.part),
    );
    const dots = night.locator(".dot");
    await expect(dots).toHaveCount(line.heard.length);
    expect(await dots.evaluateAll((all) => all.map((d) => d.getAttribute("data-lean")))).toEqual(view.placed.map((p) => p.lean));
    // Placed in proportion: the second record's stretch begins where Rust says.
    const second = line.spans[1];
    const left = await night.locator(`[data-span="${second.record}"]`).evaluate((el) => (el as HTMLElement).style.left);
    expect(parseFloat(left)).toBeCloseTo(((second.from - line.from) / (line.to - line.from)) * 100, 3);

    const first = view.played[line.spans[0].record].title;
    const slider = night.getByRole("slider", { name: "The night, on one line" });
    const moment = night.locator("[data-moment]");
    await slider.focus();
    await slider.press("Home");
    await expect(moment).toContainText(`${first} · 0:00 in`);
    // Six steps of ten seconds: the drop, a minute in.
    for (let i = 0; i < 6; i++) await slider.press("ArrowRight");
    await expect(moment).toContainText(`${first} · 1:00 in · the drop · ${line.per_minute[1]} reactions that minute`);
    await expect(slider).toHaveAttribute("aria-valuetext", (await moment.textContent())!.trim());
    // Four minutes: into the second record, forty seconds in.
    await slider.press("Home");
    for (let i = 0; i < 4; i++) await slider.press("PageUp");
    await expect(moment).toContainText(`${view.played[second.record].title} · 0:40 in`);

    // The pointer: half way along is half way through the night.
    const box = (await night.locator(".lanes").boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    const half = Math.round(line.from + (line.to - line.from) / 2);
    const into = half - second.from + second.into;
    await expect(moment).toContainText(`${view.played[second.record].title} · ${Math.floor(into / 60)}:${String(into % 60).padStart(2, "0")} in`);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Nights side by side, as Rust set them** (`crowd::compare`): the night
   * shown and the one before it picked to begin with, asked for in the
   * list's order; each of the goals' measures for each night, a share as a
   * share; and the records both played, with how each night met them.
   */
  test("nights are set side by side on the goals' measures and the records they share", async ({ page }) => {
    const side = crowd.compare;
    const panel = await withCrowd(page);
    const pick = panel.getByRole("group", { name: "Nights to compare" });
    await expect(pick.getByRole("checkbox")).toHaveCount(view.sessions.length);
    const tonight = pick.getByRole("checkbox", { name: "This night" });
    await expect(tonight).toBeChecked();
    // Picked again, last: the nights are still asked for newest first.
    await tonight.uncheck();
    await tonight.check();
    const compare = pick.getByRole("button", { name: "Compare" });
    await compare.click();
    await expect
      .poll(async () => (await calls(page)).find((c) => c.cmd === "crowd_compare"))
      .toMatchObject({ sessions: view.sessions.slice(0, 2) });

    const table = panel.locator("[data-compare]");
    await expect(table.locator("thead th")).toHaveText(["", "This night", /./]);
    await expect(table.locator('[data-measure="reactions"] td')).toHaveText(side.compared.reactions.map(String));
    const warmth = side.compared.measures.find(([m]) => m === "warmth")![1];
    await expect(table.locator('[data-measure="warmth"] td')).toHaveText(warmth.map((w) => `${Math.round(w! * 100)}%`));
    await expect(table.locator('[data-measure="warmth"] th')).toHaveText(
      side.measures.find((m) => m.measure === "warmth")!.title,
    );
    await expect(table.locator("tbody tr")).toHaveCount(side.compared.measures.length + 1);

    const shared = panel.locator("[data-shared] tbody tr");
    await expect(shared).toHaveCount(side.compared.shared.length);
    const both = side.compared.shared[0];
    await expect(shared.first().locator("td")).toHaveText(
      both.nights.map((m) =>
        [`${m.reactions}`, `${m.up} warm`, ...(m.down ? [`${m.down} cold`] : []), ...(m.moments ? [`${m.moments} moments`] : [])].join(" · "),
      ),
    );
    // Played to silence on the earlier night, and said so.
    const silent = side.compared.shared.findIndex((r) => r.nights.some((m) => m.played && m.reactions === 0));
    expect(silent).toBeGreaterThan(-1);
    await expect(shared.nth(silent).locator("td").nth(1)).toHaveText("played, nothing said");

    // One night is not a comparison.
    await pick.getByRole("checkbox", { name: "This night" }).uncheck();
    await expect(compare).toBeDisabled();
    await expect(table).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **What the rules cannot place, and the DJ's provider.** Said plainly:
   * where it goes, how many it placed, how many wait. A reaction it placed
   * is marked as its reading and no other is; *Read them now* asks for this
   * night; the switch turned off is kept with the goals and the delay as
   * they were, and the count and the button go with it; a provider that
   * fails is said in its words.
   */
  test("what the rules cannot place goes to the DJ's provider, said plainly", async ({ page }) => {
    const panel = await withCrowd(page);
    const ai = panel.locator("[data-crowd-ai]");
    const ask = ai.getByRole("checkbox", { name: `Ask ${view.ai.provider} about what the rules cannot place` });
    await expect(ask).toBeChecked();
    await expect(ai.locator("[data-crowd-ai-count]")).toHaveText(
      `${view.ai.read} placed by it · ${view.ai.waiting} waiting`,
    );

    const byModel = view.placed.filter((p) => p.by_model);
    expect(byModel.length).toBeGreaterThan(0);
    const items = panel.locator(".feed li");
    await expect(items.locator(".by-model")).toHaveCount(byModel.length);
    for (const p of byModel) {
      await expect(items.filter({ hasText: p.reaction.text }).locator(".by-model")).toHaveText("AI");
    }

    await ai.getByRole("button", { name: "Read them now" }).click();
    await expect
      .poll(async () => (await calls(page)).find((c) => c.cmd === "crowd_read_now"))
      .toMatchObject({ session: view.session });

    await ask.uncheck();
    await expect
      .poll(async () => (await calls(page)).find((c) => c.cmd === "crowd_settings_save"))
      .toMatchObject({
        settings: { ai: false, delay: view.delay, goals: view.goals.map((a) => a.goal) },
      });
    await expect(ask).not.toBeChecked();
    await expect(ai.locator("[data-crowd-ai-count]")).toHaveCount(0);
    await expect(ai.getByRole("button", { name: /Read them/ })).toHaveCount(0);

    await ask.check();
    await page.evaluate(() => {
      (window as unknown as { __crowdReadFails?: string }).__crowdReadFails =
        "could not reach Local model: connection refused";
    });
    await ai.getByRole("button", { name: "Read them now" }).click();
    await expect(panel.getByRole("alert")).toContainText("connection refused");
    await expect(ai.getByRole("button", { name: "Read them now" })).toBeEnabled();
    expect(errorsThrown(page)).toEqual([]);
  });
});

/**
 * §25's crowd response, which §119 built: where past crowds reacted, over the
 * whole-record view. Which moments, how many and on how many nights are
 * `crowd::marks`' (the fixture's `marks`, Rust's own for the night above).
 * What the browser holds: a mark lands at its frame, grows with the nights
 * that reacted there, and goes when the layer is switched off.
 */
test.describe("§119: the crowd on the record", () => {
  const info = ANSWERS.waveform_info as { total_frames: number };
  const marks = [
    ...crowd.marks,
    // A second, reacted to on three nights: the size rule is the overview's.
    { ...crowd.marks[0], frame: info.total_frames / 2, nights: 3, count: 7, part: "record", says: "7 reactions, on 3 nights" },
  ];

  test("where past crowds reacted is marked on the record, larger the more nights did", async ({ page }) => {
    await openShell(page, "/", {}, { waveform_info: { ...ANSWERS.waveform_info, crowd: marks } });
    const overview = page.locator(".overview").first();
    await expect(overview).toBeVisible();
    const drawn = overview.locator('[data-layer="crowd"]');
    await expect(drawn).toHaveCount(marks.length);
    const box = (await overview.boundingBox())!;
    for (const [i, mark] of marks.entries()) {
      const el = drawn.nth(i);
      await expect(el).toHaveAttribute("title", mark.says);
      // And said to a screen reader, not only drawn.
      await expect(overview.getByRole("img", { name: mark.says })).toHaveCount(1);
      // The tip points at the moment: the element is centred on it.
      const at = await el.boundingBox();
      const centre = (at!.x + at!.width / 2 - box.x) / box.width;
      expect(centre).toBeCloseTo(mark.frame / info.total_frames, 2);
    }
    const one = (await drawn.nth(0).boundingBox())!;
    const three = (await drawn.nth(1).boundingBox())!;
    expect(three.width, "three nights draw no larger than one").toBeGreaterThan(one.width);
    expect(errorsThrown(page)).toEqual([]);
  });

  test("switched off with the rest of the layers, it is gone", async ({ page }) => {
    await openShell(page, "/", {}, {
      waveform_info: { ...ANSWERS.waveform_info, crowd: marks },
      chosen_layers: (layers as { name: string }[]).map((l) => l.name).filter((name) => name !== "crowd"),
    });
    const overview = page.locator(".overview").first();
    await expect(overview.locator('[data-layer="drops"]').first()).toBeVisible();
    await expect(overview.locator('[data-layer="crowd"]')).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});
