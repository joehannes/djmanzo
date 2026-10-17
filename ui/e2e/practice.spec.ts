/**
 * The practice lab, and the promise it rests on.
 *
 * §69 asks for a surface where "two tracks can be explored **without altering
 * the live master**". Everything worth testing in a browser is in those last
 * four words: the panel must offer the alternatives, must say which one
 * djmanzo is actually holding, and must not change it when another is tried.
 *
 * What a rehearsal *sounds* like is not a browser question and is not asked
 * here — nor anywhere in this container, which has no audio device. The
 * rendering itself is Rust's and is tested in `dj_app::practice`.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const PRACTICE = '.surface[data-surface="practice"]';
const PAIR = '.surface[data-surface="pair"]';

/** Open the shell with the practice surface docked. */
async function practiceOpen(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Practice", exact: true }).click();
  await expect(page.locator(PRACTICE)).toBeVisible();
}

/** And with a transition held, which is the state it is useful in. */
async function withHeld(page: import("@playwright/test").Page) {
  await page.getByRole("button", { name: "Pair", exact: true }).click();
  await page.getByRole("button", { name: "Compare", exact: true }).click();
  await page.getByRole("button", { name: "Set up", exact: true }).click();
  await expect(page.locator(`${PAIR} .side`)).toHaveCount(2);
}

test.describe("the practice lab", () => {
  /**
   * There is nothing to rehearse until a pair is held, and the panel says so
   * with what to do about it. An empty panel reads as broken.
   */
  test("opens saying what it needs, rather than empty", async ({ page }) => {
    await practiceOpen(page);

    await expect(page.locator(`${PRACTICE} .empty`)).toContainText(
      "Set a transition up",
    );
    expect(errorsThrown(page), "the practice panel threw").toEqual([]);
  });

  /**
   * **A rehearsal is a file, and the panel says where it is and where the mix
   * is inside it.**
   *
   * A DJ handed a twenty-seven-second file with no idea where the second
   * record arrives has to hunt for it. The two numbers come from Rust with the
   * file, so the mark and the audio cannot disagree.
   */
  test("renders a rehearsal and says where the mix is in it", async ({
    page,
  }) => {
    await practiceOpen(page);
    await withHeld(page);

    await page.locator(`${PRACTICE} button`, { hasText: /^blend$/ }).click();

    const heard = page.getByTestId("practice-heard");
    await expect(heard.locator("li")).toHaveCount(1);
    await expect(heard).toContainText("blend");
    await expect(heard).toContainText("mix at");
    await expect(heard).toContainText(".wav");
    expect(errorsThrown(page), "the practice panel threw while rendering").toEqual([]);
  });

  /**
   * **§69's "hear alternative transitions": four renders of one pair, told
   * apart.**
   *
   * The point of the surface. A panel that rendered each style over the last
   * one would give a DJ one file and no comparison, which is the thing they
   * came here to make.
   */
  test("keeps each alternative rather than replacing the last", async ({
    page,
  }) => {
    await practiceOpen(page);
    await withHeld(page);

    await page.locator(`${PRACTICE} button`, { hasText: /^blend$/ }).click();
    await expect(page.getByTestId("practice-heard").locator("li")).toHaveCount(1);
    await page.locator(`${PRACTICE} button`, { hasText: /^cut$/ }).click();
    await expect(page.getByTestId("practice-heard").locator("li")).toHaveCount(2);

    const heard = page.getByTestId("practice-heard");
    await expect(heard).toContainText("blend");
    await expect(heard).toContainText("cut");
    // And they are different renders, not the same one twice: a cut is a
    // handful of actions and a blend is a thousand fader writes.
    await expect(heard).toContainText("12 actions");
    await expect(heard).toContainText("964 actions");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Hearing an alternative does not change the mix djmanzo is holding.**
   *
   * The whole of "without altering the live master", at the level a browser
   * can check it. A lab that quietly restyled the transition would have handed
   * the DJ's next handover to a button labelled "listen".
   *
   * Checked against the practice panel's *own* re-read rather than the pair
   * view's. The pair view only re-asks when its decks move, so a restyle
   * behind its back leaves it drawing the old answer — which is to say it
   * cannot see this failure, and a test written against it passes under
   * exactly the mutation it exists to catch. This panel polls the held
   * transition, so waiting out a poll and finding the mark unmoved is a fact
   * rather than a stale render.
   */
  test("rehearsing an alternative leaves the held mix alone", async ({
    page,
  }) => {
    await practiceOpen(page);
    await withHeld(page);

    const held = page.locator(`${PRACTICE} .tries button.on`);
    await expect(held).toHaveText("blend");

    await page.locator(`${PRACTICE} button`, { hasText: /^cut$/ }).click();
    await expect(page.getByTestId("practice-heard")).toContainText("cut");

    // Past a poll, so this is what djmanzo answered and not what was on
    // screen before the rehearsal.
    await page.waitForTimeout(2500);
    await expect(held, "a rehearsal moved the held mix").toHaveText("blend");
    await expect(
      page.getByTestId("practice-heard"),
      "the rendered alternatives were dropped, which means what is held moved",
    ).toContainText("cut");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **§69, live: a record in the headphones, looped, its stems taken apart
   * and an effect put on** — each press sent as the deck word a controller
   * uses, to the practice deck and nothing else. A record is found by a
   * search with no pair held, the held pair's two are offered when there is
   * one, and with no headphone pair the panel says the deck is silent.
   */
  test("practises a record live, in the headphones only", async ({ page }) => {
    await practiceOpen(page);
    await page.evaluate(() => {
      const win = window as unknown as {
        __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
        __practice: { cmd: string; args: Record<string, unknown> }[];
        __headphones: boolean;
      };
      win.__practice = [];
      win.__headphones = true;
      let live: Record<string, unknown> = { track: null, title: "", playing: false, looping: false, position_seconds: 0, length_seconds: 0, level: 0, headphones: true };
      const stub = win.__TAURI_INTERNALS__.invoke;
      win.__TAURI_INTERNALS__.invoke = (cmd, args = {}) => {
        if (!cmd.startsWith("practice_live")) return stub(cmd, args);
        win.__practice.push({ cmd, args: structuredClone(args) });
        if (cmd === "practice_live_load") live = { ...live, track: args.track, title: "Juan Luis Guerra – Bachata Rosa", length_seconds: 240 };
        if (cmd === "practice_live_do" && args.action === "play_pause") live = { ...live, playing: !live.playing };
        if (cmd === "practice_live_do" && String(args.action).startsWith("loop ")) live = { ...live, looping: true };
        if (cmd === "practice_live_eject") live = { ...live, track: null, title: "", playing: false };
        return Promise.resolve({ ...live, headphones: win.__headphones });
      };
    });
    const panel = page.locator(PRACTICE);
    const live = panel.getByRole("region", { name: "Practise live, in your headphones" });
    await expect(live).toBeVisible();
    await expect(live.locator("[data-no-headphones]")).toHaveCount(0);
    const said = () =>
      page.evaluate(() =>
        (window as unknown as { __practice: { cmd: string; args: Record<string, unknown> }[] }).__practice
          .filter((c) => c.cmd === "practice_live_do")
          .map((c) => c.args.action),
      );

    await live.getByRole("textbox", { name: "Find a record to practise on" }).fill("Bach");
    await live.getByRole("list", { name: "Records to practise on" }).getByRole("button").first().click();
    const deck = live.locator("[data-practice-live]");
    await expect(deck).toContainText("Juan Luis Guerra – Bachata Rosa");

    await deck.getByRole("button", { name: "Play" }).click();
    await expect(deck.getByRole("button", { name: "Pause" })).toBeVisible();
    await deck.getByRole("group", { name: "Loop" }).getByRole("button", { name: "4", exact: true }).click();
    await expect(deck).toContainText("looping");
    const drums = deck.getByRole("group", { name: "Stems" }).getByRole("button", { name: "Drums" });
    await drums.click();
    await expect(drums).toHaveAttribute("aria-pressed", "false");
    const low = deck.getByRole("group", { name: "EQ" }).getByRole("button", { name: "Low" });
    await low.click();
    await expect(low).toHaveAttribute("aria-pressed", "false");
    await deck.getByRole("combobox", { name: "Practice effect" }).selectOption("echo");
    await deck.getByRole("group", { name: "Effect" }).getByRole("button", { name: "Off" }).click();
    expect(await said()).toEqual(["play_pause", "loop 4", "stem_mute drums", "eq_low 0", "fx 1 echo", "fx 1 on"]);

    await deck.getByRole("button", { name: "Take it off" }).click();
    await expect(deck).toHaveCount(0);

    // No headphone pair: said, not hidden.
    await page.evaluate(() => ((window as unknown as { __headphones: boolean }).__headphones = false));
    await live.getByRole("textbox", { name: "Find a record to practise on" }).fill("Bach");
    await live.getByRole("list", { name: "Records to practise on" }).getByRole("button").first().click();
    await expect(live.locator("[data-no-headphones]")).toContainText("silent");
    expect(errorsThrown(page)).toEqual([]);
  });

  test("offers the held pair's two records to practise on", async ({ page }) => {
    await practiceOpen(page);
    await withHeld(page);
    const live = page.locator(PRACTICE).getByRole("region", { name: "Practise live, in your headphones" });
    await expect(live.getByRole("button", { name: /^Try / })).toHaveCount(2);
  });
});
