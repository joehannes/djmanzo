/**
 * §118: preparing one night, step by step, and keeping it for the night.
 *
 * > make a workflow and an activity to specifically prepare a set/show for
 * > an event ... and store every step in a decent way ... and save his
 * > preparation/work to load it for the live event
 *
 * What a step still lacks, which ideas a kind of night is offered and the
 * path from learning the moves to playing the night are `dj_app::gig`'s
 * rules; the fixture is Rust's own answer for a half-prepared wedding
 * (`events.json`). What a browser can prove is that the panel draws those
 * answers rather than its own, that every edit is sent as it is made, and
 * that taking an idea sends Rust's own description of it back.
 */
import { expect, test, type Page } from "@playwright/test";

import events from "./events.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const PANEL = '.surface[data-surface="event"]';

const eventSaves = (page: Page) =>
  page.evaluate(() => ((window as unknown as { __eventSaves?: unknown[] }).__eventSaves ?? []) as Record<string, unknown>[]);
const eventCalls = (page: Page) =>
  page.evaluate(() => ((window as unknown as { __eventCalls?: unknown[] }).__eventCalls ?? []) as Record<string, unknown>[]);

/** The shell, with the event panel docked where the Event activity puts it. */
async function withEventPanel(page: Page) {
  await page.setViewportSize({ width: 1400, height: 900 });
  await openShell(page, "/", {}, {
    cockpit_workspace: {
      workspace: {
        name: "Event",
        about: "",
        surfaces: [{ surface: "event", dock: "right", order: 0, size: null, collapsed: false, pinned: false }],
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
  await expect(panel.locator(".event")).toHaveAttribute("data-event", events.wedding.gig.id);
  return panel;
}

test.describe("§118: an event, prepared step by step", () => {
  /**
   * **The load-bearing one.** Each step says what it still lacks, in Rust's
   * words, and the path marks done exactly the stops Rust marks done.
   */
  test("the steps and the path are Rust's, not the panel's", async ({ page }) => {
    const panel = await withEventPanel(page);
    for (const step of events.wedding.steps) {
      const section = panel.locator(`[data-step="${step.step}"]`);
      await expect(section.locator("summary")).toContainText(step.title);
      if (step.missing.length > 0) {
        await expect(section.locator(".lacks")).toHaveText(`${step.missing.length} to go`);
        await expect(section.locator(".missing")).toContainText(step.missing[0]);
      } else {
        await expect(section.locator(".lacks")).toHaveCount(0);
      }
    }
    for (const stop of events.wedding.path) {
      const li = panel.locator(`.path [data-stop="${stop.stop}"]`);
      await expect(li).toContainText(stop.title);
      await expect(li).toHaveClass(stop.done ? /done/ : /^(?!.*done)/);
    }
    // Rain is a trouble for a night partly outside, and this one is.
    await expect(panel.locator('[data-trouble="rain"]')).toHaveCount(1);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** **Every edit is kept as it is made**, with nothing to press. */
  test("an edit is sent a moment after typing stops", async ({ page }) => {
    const panel = await withEventPanel(page);
    const where = panel.locator('[data-step="event"] label:has-text("Where") input');
    await where.fill("The old mill, by the river");
    await expect
      .poll(async () => (await eventSaves(page)).at(-1)?.place)
      .toBe("The old mill, by the river");
    const saved = (await eventSaves(page)).at(-1)!;
    expect(saved.id).toBe(events.wedding.gig.id);
    // The rest of the event went with it: an edit is the whole event, not a
    // field that loses the others.
    expect(saved.title).toBe(events.wedding.gig.title);
    expect(saved.moments).toEqual(events.wedding.gig.moments);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A half-typed time is held, not sent to be refused.** The fields are
   * text, because WebKitGTK drew an empty native date as today's date; a
   * time is kept once it reads as one, and until then the panel says why.
   */
  test("a half-typed start time waits, and is kept once it reads right", async ({ page }) => {
    const panel = await withEventPanel(page);
    const starts = panel.locator('[data-step="event"] label:has-text("You start") input');
    await expect(starts).toHaveValue(events.wedding.gig.starts);
    const before = (await eventSaves(page)).length;
    await starts.fill("22:");
    await expect(panel.locator(".held")).toContainText("hours:minutes");
    await page.waitForTimeout(700);
    expect((await eventSaves(page)).slice(before)).toEqual([]);

    await starts.fill("22:00");
    await expect(panel.locator(".held")).toHaveCount(0);
    await expect.poll(async () => (await eventSaves(page)).at(-1)?.starts).toBe("22:00");
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A plan for a trouble: the usual one is a hint, never written in for the DJ. */
  test("a trouble's plan is the DJ's, with the usual one only as a hint", async ({ page }) => {
    const panel = await withEventPanel(page);
    const rain = panel.locator('[data-trouble="rain"] textarea');
    const usual = events.wedding.troubles.find((t) => t.trouble === "rain")!.usual;
    await expect(rain).toHaveAttribute("placeholder", usual);
    await expect(rain).toHaveValue("");
    await rain.fill("Decks under the barn roof; the terrace set moves inside.");
    await expect
      .poll(async () => ((await eventSaves(page)).at(-1)?.fallbacks as { trouble: string; plan: string }[] | undefined)?.find((f) => f.trouble === "rain")?.plan)
      .toBe("Decks under the barn roof; the terrace set moves inside.");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **An idea is one press, and sends Rust's own description of it back.**
   * The panel then draws the event as Rust answers it.
   */
  test("taking an idea sends what Rust offered", async ({ page }) => {
    const panel = await withEventPanel(page);
    const first = events.wedding.ideas[0];
    // A step with nothing missing starts folded; its ideas are a press away.
    const section = panel.locator(`[data-step="${first.step}"]`);
    if (!(await section.evaluate((el) => (el as HTMLDetailsElement).open))) {
      await section.locator("summary").click();
    }
    const idea = section.locator(".idea", { hasText: first.text }).first();
    await expect(idea).toHaveAttribute("title", first.because);
    await idea.click();
    await expect
      .poll(async () => (await eventCalls(page)).find((c) => c.cmd === "take_event_idea"))
      .toEqual({ cmd: "take_event_idea", id: events.wedding.gig.id, adds: events.taken.adds });
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A new event starts from a name and a date, and opens at once. */
  test("a new event starts from a name and a date", async ({ page }) => {
    const panel = await withEventPanel(page);
    await panel.getByRole("button", { name: "New event" }).click();
    // The name field has the keyboard at once: typed letters must not reach
    // the page, where they are shortcuts.
    await expect(panel.getByRole("textbox", { name: "Name of the new event" })).toBeFocused();
    await panel.getByRole("textbox", { name: "Name of the new event" }).fill("Summer party");
    await panel.getByRole("textbox", { name: "Date of the new event" }).fill("2026-07-04");
    await panel.getByRole("button", { name: "Start" }).click();
    await expect
      .poll(async () => (await eventCalls(page)).find((c) => c.cmd === "new_event"))
      .toEqual({ cmd: "new_event", title: "Summer party", date: "2026-07-04" });
    await expect(panel.locator(".event")).toHaveAttribute("data-event", events.fresh.gig.id);
    // A fresh event lacks most of what a night needs, and says so.
    await expect(panel.locator('[data-step="event"] .lacks')).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Forgetting takes two presses; the first only asks. */
  test("forgetting an event asks first", async ({ page }) => {
    const panel = await withEventPanel(page);
    await panel.getByRole("button", { name: "Forget this event…" }).click();
    expect((await eventCalls(page)).some((c) => c.cmd === "forget_event")).toBe(false);
    await panel.getByRole("button", { name: "Forget it" }).click();
    await expect
      .poll(async () => (await eventCalls(page)).find((c) => c.cmd === "forget_event"))
      .toEqual({ cmd: "forget_event", id: events.wedding.gig.id });
    expect(errorsThrown(page)).toEqual([]);
  });
});
