/**
 * §8 Level 1: the nine things djmanzo remembers about a DJ.
 *
 * > **Level 1 — Remember.** The software remembers: workspace, density,
 * > library columns, sorting, panel positions, preferred decks, favorite pad
 * > pages, preferred waveform display, preferred controls.
 *
 * Five were already kept when this was written. Three were not, and all three
 * failed the same way: a plain initialiser in a Svelte component, reset on
 * every *mount* — which is every time a panel closes and reopens, not only
 * every launch. That is a defect no type-checker can see and no Rust test can
 * reach, because nothing is wrong with either side. It is only wrong across
 * the join, which is what these specs stand on.
 *
 * What is kept, what each row claims, and what happens to a nonsense choice is
 * `dj_app::remembered`, `dj_app::columns` and `dj_app::at_hand`, and is tested
 * there. These say the three things only a browser can.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const TABLE = ".table-scroll table";

/** Open Settings and wait for §8's block. */
async function openRemembers(page: Page) {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.locator(".remembers")).toBeVisible();
}

const star = (page: Page, name: string) =>
  page.locator(`.remembers [data-picker="pad-pages"] li`).filter({ hasText: name }).locator("input");

const keep = (page: Page, slug: string) =>
  page
    .locator(`.remembers [data-picker="rail-controls"] li`)
    .filter({ has: page.locator(`.pick-name`, { hasText: new RegExp(`^${slug}$`) }) })
    .locator("input");

test.describe("§8 Level 1: what djmanzo remembers", () => {
  /**
   * **The load-bearing one: a sort survives the browser being closed and
   * reopened.**
   *
   * This is the regression, stated exactly. `sortBy` and `ascending` were
   * `$state("artist")` and `$state(true)`, so a DJ who put the table in BPM
   * order to find the slow records got artist A-to-Z back the moment they
   * looked at another panel and came back — and nothing on screen said the
   * preference had been dropped rather than never taken.
   *
   * Both halves matter. The column alone would pass against an interface that
   * remembered the column and forgot the direction, and descending is the half
   * a DJ actually wanted when they clicked twice.
   */
  test("the sort you chose is the sort you come back to", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");

    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await expect(page.locator(TABLE)).toBeVisible();

    // Twice: once to sort by BPM, again to turn it round. Descending is the
    // interesting direction — it is what "show me the fastest" means.
    const heading = page.locator(`${TABLE} th[data-column="bpm"] button`);
    await heading.click();
    await heading.click();
    await expect(page.locator(`${TABLE} th[data-column="bpm"] .arrow`)).toHaveText("▼");

    // Closed and opened again. Not a reload: this is the *mount* that was
    // losing it, and a DJ meets it a hundred times a night — every time the
    // browser is put away to get the decks back.
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await expect(page.locator(TABLE)).toHaveCount(0);
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await expect(page.locator(TABLE)).toBeVisible();

    await expect(page.locator(`${TABLE} th[data-column="bpm"] .arrow`)).toHaveText("▼");
    expect(thrown).toEqual([]);
  });

  /**
   * **Starring a pad page moves it to the front of every deck's pad zone.**
   *
   * §8 asks for *favorite* pad pages, which is not the page the tabs were last
   * left on — the pad zone deliberately does not restore that, because a DJ who
   * left it on roll an hour ago does not want to come back to a deck whose cues
   * are hidden. A favourite is a decision, so it is made in Settings and met on
   * the deck.
   */
  test("a starred pad page is the one a deck opens on", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");

    // Cues first, as it ships.
    const tabs = page.locator(".zone .tabs button");
    await expect(tabs.first()).toHaveText("cues");

    await openRemembers(page);
    await star(page, "loops").check();

    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await expect(tabs.first()).toHaveText("loops");
    // And the page on screen is that one, not merely the first tab.
    await expect(page.locator(".zone .tabs button.active").first()).toHaveText("loops");
    expect(thrown).toEqual([]);
  });

  /**
   * **A control you keep is on §74's rail, and the rail says you asked for it.**
   *
   * The second half is the one worth a browser. Rust decides *whether* a kept
   * control is on the rail and a Rust test proves it is there in every state;
   * what only this can show is that the reason reaches the screen — a rail that
   * silently grew a button is a rail a DJ stops trusting, which is the whole of
   * §74.
   */
  test("a control you keep is marked as yours on the rail", async ({ page }) => {
    const thrown = errorsThrown(page);
    // §74's rail is not on the top bar — it is a surface, opened the way a DJ
    // opens one. The palette's ranking is Rust's and the shared stub answers a
    // fixed set that proves each *kind* of entry runs; this is one answer
    // djmanzo really gives, for one query, which is what the override is for.
    await openShell(page, "/", {}, {
      palette: {
        // Nothing was cut. §18's note is empty unless the budget actually
        // removed something a DJ would otherwise have seen, and a fixture
        // carrying it always would let a test about the quiet palette pass
        // over a loud one.
        because: "",
        entries: [
          {
            label: "Show At hand",
            about: "The four to eight controls that matter on the focused deck right now.",
            kind: "surface",
            run: "athand",
          },
        ],
      },
    });
    await page.keyboard.press("Control+k");
    await page.getByRole("button", { name: /Show At hand/ }).first().click();
    await expect(page.locator(".at-hand")).toBeVisible();

    await openRemembers(page);
    const box = keep(page, "keylock");
    await expect(box).not.toBeChecked();
    await box.check();
    await expect(box).toBeChecked();

    // The stub answers §74's rail with keylock already marked, which is what
    // Rust would return once the preference is stored. What is proven here is
    // that the interface draws the distinction at all.
    const rail = page.locator('.at-hand .controls button[data-reach="keylock"]');
    await expect(rail).toHaveClass(/kept/);
    await expect(rail).toHaveAttribute("title", /kept within reach/);
    await expect(
      page.locator('.at-hand .controls button[data-reach="sync"]'),
    ).not.toHaveClass(/kept/);
    expect(thrown).toEqual([]);
  });

  /**
   * **The list says what djmanzo does not keep, as plainly as what it does.**
   *
   * A list of eight would read as the whole of §8. A DJ who sets their waveform
   * up and finds it back at the default tomorrow has learned something this
   * block could have told them in advance, and "not yet, and here is what has
   * to happen first" is a different answer from "no".
   */
  test("the one thing djmanzo does not remember says so", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await openRemembers(page);

    await expect(page.locator(".remembers .remember-list li")).toHaveCount(9);
    const waveform = page.locator('.remembers li[data-remembers="waveform-display"]');
    await expect(waveform).toHaveClass(/unkept/);
    await expect(waveform.locator(".remember-not")).toContainText("Not yet");

    // And every other row says what losing it would cost, rather than being
    // left to be inferred from its name.
    await expect(page.locator(".remembers .remember-cost")).toHaveCount(8);
    expect(thrown).toEqual([]);
  });
});
