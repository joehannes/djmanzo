/**
 * A2: push-to-talk on the assistant's panel (docs/ASSISTANT.md §4).
 *
 * The take, the strip held out of the PA and put back, and a take that runs
 * out are `dj_app`'s and tested there over the engine's own command queue.
 * This holds what the DJ sees and presses: a button held, which sends the
 * same two actions the key and a controller pad do; the key the keyboard map
 * binds, shown beside it; which microphone the commands come from; and what
 * the take came to.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";
import vocals from "./vocals.json" with { type: "json" };

const PUSH_TO_TALK = {
  chord: "keyb",
  label: "Push to talk (hold)",
  group: "Voice",
  held: true,
  press: "voice listen_on",
  release: "voice listen_off",
};

const dispatched = (page: Page) =>
  page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []);

const voice = (page: Page, next: unknown) =>
  page.evaluate((next) => {
    (window as unknown as { __voice: unknown }).__voice = next;
  }, next);

async function openAssistant(page: Page, answers: Record<string, unknown> = {}) {
  await openShell(page, "/", {}, { keyboard_keys: [PUSH_TO_TALK], vocals_state: vocals.open, ...answers });
  await page.getByRole("button", { name: "Assistant", exact: true }).click();
  const panel = page.locator('.surface[data-surface="assistant"]');
  await expect(panel).toBeVisible();
  return panel;
}

test.describe("A2: push-to-talk", () => {
  /**
   * **The load-bearing one: held, it sends `voice listen_on`; let go,
   * `voice listen_off`** — the very lines the key and a pad send — and the
   * panel says what is happening and what the take came to.
   */
  test("the button is held to talk, and says what it took", async ({ page }) => {
    const panel = await openAssistant(page);
    const hold = panel.getByRole("button", { name: "Hold to talk" });
    await expect(panel.locator(".talk kbd"), "the key the map binds is not shown").toHaveText("B");

    await hold.hover();
    await page.mouse.down();
    await expect.poll(() => dispatched(page)).toEqual(["voice listen_on"]);
    await voice(page, { strip: 0, listening: true, seconds: 1.2, last: null, hearing: false, said: null });
    await expect(panel.getByText(/Listening on Mic 1, out of the PA/)).toContainText("1.2 s");
    await expect(panel.getByRole("button", { name: "Listening…" })).toHaveAttribute("aria-pressed", "true");

    await page.mouse.up();
    await expect.poll(() => dispatched(page)).toEqual(["voice listen_on", "voice listen_off"]);
    await voice(page, {
      strip: 0,
      listening: false,
      seconds: 0,
      last: { seconds: 2.4, voiced: 1.8, problem: null },
      hearing: false,
      said: null,
    });
    await expect(panel.getByText(/Took 2\.4 s from Mic 1/)).toContainText("1.8 s of it voice");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A2 step 2: what was heard is shown before what was done** — a
   * mishearing and a wrong answer look the same until the words are there.
   */
  test("the words heard come before what was done with them", async ({ page }) => {
    const said = (over: Record<string, unknown>) => ({
      strip: 0,
      listening: false,
      seconds: 0,
      last: { seconds: 1.6, voiced: 1.1, problem: null },
      hearing: false,
      said: {
        heard: "Pon el dos.",
        language: "es",
        actions: ["deck 2 play"],
        undelivered: [],
        reply: "Playing deck 2",
        source: "local",
        problem: null,
        seconds: 0.8,
        ...over,
      },
    });
    const panel = await openAssistant(page, {
      voice_state: { ...said({}), hearing: true, said: null },
    });
    await expect(panel.getByText("Making out the words…")).toBeVisible();

    await voice(page, said({}));
    const heard = panel.locator(".heard");
    await expect(heard).toHaveText(/^Heard “Pon el dos\.” → deck 2 play\s*$/);
    await expect(heard).toHaveAttribute("data-source", "local");

    await voice(
      page,
      said({ heard: "Make it warmer.", source: "model", actions: ["deck 1 filter -0.3"], undelivered: ["make it golden"] }),
    );
    await expect(heard).toHaveText(/Heard “Make it warmer\.” → deck 1 filter -0\.3 \(your AI\)/);
    await expect(panel.getByText("Not done: make it golden")).toBeVisible();

    const nothing = "Not a command djmanzo knows by itself, and no AI is set up to ask: nothing was done.";
    await voice(page, said({ heard: "Make it warmer.", source: "none", actions: [], problem: nothing }));
    await expect(heard).toHaveText("Heard “Make it warmer.”");
    await expect(panel.getByText(nothing)).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A take with no voice in it says why, in Rust's words. */
  test("a take with nothing in it says why", async ({ page }) => {
    const said = "Nothing was heard on Mic 1: hold the key while speaking, close to the microphone.";
    const panel = await openAssistant(page, {
      voice_state: {
        strip: 0,
        listening: false,
        seconds: 0,
        last: { seconds: 1, voiced: 0, problem: said },
        hearing: false,
        said: null,
      },
    });
    await expect(panel.getByText(said)).toBeVisible();
  });

  /** Commands come from the microphone chosen, one of those open. */
  test("the microphone commands come from is chosen from those open", async ({ page }) => {
    const panel = await openAssistant(page);
    const from = panel.getByLabel("Commands from");
    await expect(from.locator("option")).toHaveText(["Mic 1", "Mic 2"]);
    await from.selectOption("1");
    await expect(from).toHaveValue("1");
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __voice?: { strip: number } }).__voice?.strip))
      .toBe(1);
  });

  /** With the microphones closed, the panel says where to open them. */
  test("with no microphone open, it says where to open one", async ({ page }) => {
    const panel = await openAssistant(page, { vocals_state: vocals.closed });
    await expect(panel.getByText("Open the microphones under Microphones")).toBeVisible();
  });
});
