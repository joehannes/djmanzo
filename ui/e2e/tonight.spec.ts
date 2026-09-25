/**
 * §118: playing a prepared event, and the quick decision.
 *
 * > the live event: narrow focus, specific, prepared ... prepared for
 * > exceptions and emergencies/alternatives
 *
 * > whenever in a stressful situation a decision becomes necessary a super
 * > easy and quick to identify and select widget can pop up
 *
 * Where the night stands against its agreed times, the next moment, and
 * which troubles have a plan are `gig::tonight`'s; the fixture is Rust's own
 * answer for the wedding eighteen minutes in with one plan written
 * (`events.json`).
 */
import { expect, test, type Page } from "@playwright/test";

import events from "./events.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const PILL = ".tonight";

const eventCalls = (page: Page) =>
  page.evaluate(() => ((window as unknown as { __eventCalls?: unknown[] }).__eventCalls ?? []) as Record<string, unknown>[]);

const PLAYING = { live_event: events.wedding.gig.id };

test.describe("§118: the night being played", () => {
  /**
   * **The load-bearing one.** The line says where the night stands and
   * what is next, in Rust's numbers, and it asked with the DJ's own date
   * and time.
   */
  test("the night's line says where it stands and what is next", async ({ page }) => {
    await openShell(page, "/", {}, PLAYING);
    const pill = page.locator(PILL);
    await expect(pill).toHaveAttribute("data-tonight", events.tonight.id);
    await expect(pill).toContainText(events.tonight.title);
    await expect(pill.locator(".standing")).toHaveText("18 min in · 3 h 42 left");
    await expect(pill.locator(".next")).toContainText("First dance 21:30 · in 12 min · At Last");
    // Twelve minutes away is not yet the last ten.
    await expect(pill.locator(".next")).not.toHaveClass(/soon/);

    const asked = (await eventCalls(page)).find((c) => c.cmd === "event_tonight")!;
    const local = await page.evaluate(() => {
      const d = new Date();
      const pad = (n: number) => String(n).padStart(2, "0");
      return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
    });
    expect(asked.id).toBe(events.tonight.id);
    expect(asked.today).toBe(local);
    expect(asked.now as number).toBeGreaterThanOrEqual(0);
    expect(asked.now as number).toBeLessThan(1440);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The next moment's last ten minutes are the one thing that asks by itself. */
  test("the next moment stands out in its last ten minutes", async ({ page }) => {
    await openShell(page, "/", {}, {
      ...PLAYING,
      event_tonight: { ...events.tonight, next: { ...events.tonight.next!, in_minutes: 8 } },
    });
    await expect(page.locator(`${PILL} .next`)).toHaveClass(/soon/);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The quick decision: the DJ's own plan, or an honest none.** A plan
   * written at home is shown as written; a trouble without one says so and
   * offers the usual answer only as a hint.
   */
  test("If… opens every trouble, each with the plan written for it or none", async ({ page }) => {
    await openShell(page, "/", {}, PLAYING);
    await page.locator(PILL).getByRole("button", { name: "If…" }).click();
    const dialog = page.getByRole("dialog", { name: "If something goes wrong" });
    await expect(dialog).toBeVisible();
    for (const decision of events.tonight.decisions) {
      const button = dialog.locator(`[data-trouble="${decision.trouble}"]`);
      await expect(button).toContainText(decision.title);
      await expect(button).toContainText(decision.plan ? "your plan" : "no plan written");
    }

    await dialog.locator('[data-trouble="power"]').click();
    const power = events.tonight.decisions.find((d) => d.trouble === "power")!;
    await expect(dialog.locator(".written")).toHaveText(power.plan!);
    await dialog.getByRole("button", { name: "← Back" }).click();

    await dialog.locator('[data-trouble="microphone"]').click();
    const mic = events.tonight.decisions.find((d) => d.trouble === "microphone")!;
    await expect(dialog.locator(".written")).toHaveCount(0);
    await expect(dialog.locator(".none")).toHaveText("No plan was written for this.");
    await expect(dialog.locator(".usual")).toContainText(mic.usual);

    // Escape steps back, then closes.
    await page.keyboard.press("Escape");
    await expect(dialog.locator(".decisions")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Ending the night asks first, and then says so to Rust. */
  test("ending the night asks first", async ({ page }) => {
    await openShell(page, "/", {}, PLAYING);
    await page.locator(PILL).getByRole("button", { name: "End the night…" }).click();
    expect((await eventCalls(page)).some((c) => c.cmd === "set_live_event")).toBe(false);
    await page.locator(PILL).getByRole("button", { name: "End the night", exact: true }).click();
    await expect.poll(async () => (await eventCalls(page)).find((c) => c.cmd === "set_live_event")).toEqual({
      cmd: "set_live_event",
      id: null,
    });
    await expect(page.locator(PILL)).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Play it live: the narrow focus.** From the prepared event, one press
   * plays it -- the Mix activity's decks and mixer, and the night's line.
   */
  test("Play it live plays the event and narrows to the decks", async ({ page }) => {
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
    const panel = page.locator('.surface[data-surface="event"]');
    await expect(panel.locator(".event")).toHaveAttribute("data-event", events.wedding.gig.id);
    await expect(page.locator(PILL)).toHaveCount(0);
    await panel.getByRole("button", { name: "Play it live" }).click();
    await expect
      .poll(async () => (await eventCalls(page)).find((c) => c.cmd === "set_live_event"))
      .toEqual({ cmd: "set_live_event", id: events.wedding.gig.id });
    await expect(page.locator(PILL)).toBeVisible();
    // Narrowed: the preparation is put away for the night.
    await expect(page.locator('.surface[data-surface="event"]')).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});
