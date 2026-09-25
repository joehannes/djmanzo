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
import { errorsThrown, openShell } from "./shell";

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
    await expect(records.first().locator(".moment")).toHaveText(["the drop 1:00 ×3"]);
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
});
