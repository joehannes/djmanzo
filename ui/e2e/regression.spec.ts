/**
 * §89: the layout rules, held in all ten configurations.
 *
 * `budget.spec.ts` measures one arrangement — the one djmanzo opens at — and
 * exists because the controls a DJ performs a mix with drifted below the fold
 * twice, both times found by a human with a screenshot. §89's contribution is
 * that one arrangement is not coverage: a rule that holds at 1280×800 with
 * nothing docked says nothing about four decks on a compact laptop with the
 * browser open.
 *
 * See `configurations.ts` for what is regressed and, more importantly, what is
 * not: pixels are not diffed, because CI and this container rasterise fonts
 * differently and a baseline that has to be re-blessed every run has stopped
 * being a test.
 */
import { type Page, expect, test } from "@playwright/test";

import { CONFIGURATIONS, type Configuration } from "./configurations";
import { SLACK, centreOf, errorsThrown, openShell } from "./shell";

/**
 * A deck, and only a deck.
 *
 * Not `.deck`: that class is also a *label* in the Next rail and in §74's At
 * Hand panel — "Deck 1", the heading saying which deck a row is about. A
 * configuration with either of those docked counted them as decks, which is
 * how this test first read four decks as five. The attribute is on the deck
 * section itself and means nothing else.
 */
const DECK = "section.deck[data-deck]";

/**
 * Open djmanzo in one configuration.
 *
 * The workspace is handed over the way a saved one would be, rather than
 * assembled by clicking: what is under test is whether the arrangement *lays
 * out*, and a test that had to press eight buttons first would be testing the
 * buttons.
 */
async function open(page: Page, config: Configuration) {
  await page.setViewportSize(config.window);
  await openShell(page, "/", {}, {
    cockpit_workspace: {
      workspace: {
        name: config.name,
        about: "",
        surfaces: config.surfaces,
        density: config.density,
        focus: config.focus,
        theme: config.theme,
        decks: config.decks,
        frozen: false,
      },
      notes: [],
    },
  });
  await expect(page.locator(DECK).first()).toBeVisible();
}

/** The controls reached for mid-transition, in the dark, without looking. */
const PERFORMING = [
  { role: "slider" as const, name: "Volume" },
  { role: "slider" as const, name: "Filter" },
  { role: "slider" as const, name: "Crossfader" },
  { role: "slider" as const, name: "Master gain" },
];

/**
 * **The deck count is part of the arrangement, and is remembered.**
 *
 * §89 asks for a *4 deck* configuration, and finding that one was what
 * uncovered this: the workspace already carried a `decks` field, `toggleSurface`
 * wrote it, and **nothing read it back** — so a DJ who set up four decks found
 * two the next time they opened djmanzo, with the saved workspace still
 * claiming four. The save side was wrong too: it wrote the *stored* count
 * rather than the live one, so pressing the toggle never reached the file at
 * all.
 *
 * Both halves, in one test: change it, see what was written, and see it come
 * back.
 */
test("the deck count survives being saved and reopened", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 1000 });
  await openShell(page, "/");
  await expect(page.locator(DECK)).toHaveCount(2);

  // Two becomes four, and the change is written rather than only drawn.
  await page.getByRole("button", { name: "2 decks" }).click();
  await expect(page.locator(DECK)).toHaveCount(4);

  const saved = await page.evaluate(
    () =>
      (window as unknown as { __saved?: { decks: number }[] }).__saved ?? [],
  );
  expect(
    saved.at(-1)?.decks,
    "the deck count was drawn but never written",
  ).toBe(4);

  // And a workspace that says four opens as four.
  await openShell(page, "/", {}, {
    cockpit_workspace: {
      workspace: {
        name: "Four",
        about: "",
        surfaces: [],
        density: "standard",
        focus: "performing",
        theme: "",
        decks: 4,
        frozen: false,
      },
      notes: [],
    },
  });
  await expect(
    page.locator(DECK),
    "a saved four-deck workspace reopened as something else",
  ).toHaveCount(4);
});

for (const config of CONFIGURATIONS) {
  test.describe(config.name, () => {
    /**
     * **Every performing control is on the first screen.**
     *
     * The budget's rule, in this arrangement. A control a DJ has to scroll to
     * find is a control they do not have during a mix — which is the failure
     * this project has shipped twice and caught by hand both times.
     */
    test("every performing control is reachable without scrolling", async ({
      page,
    }) => {
      await open(page, config);

      const offscreen: string[] = [];
      const missing: string[] = [];
      for (const { role, name } of PERFORMING) {
        const centre = await centreOf(page, role, name);
        if (!centre) {
          missing.push(name);
          continue;
        }
        if (
          centre.y > config.window.height + SLACK ||
          centre.x > config.window.width + SLACK
        ) {
          offscreen.push(
            `${name} at (${Math.round(centre.x)}, ${Math.round(centre.y)})`,
          );
        }
      }

      expect(missing, `${config.name}: controls that are not drawn at all`).toEqual([]);
      expect(
        offscreen,
        `${config.name}: past the ${config.window.width}×${config.window.height} window`,
      ).toEqual([]);
    });

    /**
     * **Nothing scrolls sideways.**
     *
     * A horizontal scrollbar means something is wider than the window, and the
     * thing that is wider is usually a deck row that stopped fitting — which
     * is invisible until somebody drags.
     */
    test("nothing is wider than the window", async ({ page }) => {
      await open(page, config);

      const overflow = await page.evaluate(
        () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
      );
      expect(
        overflow,
        `${config.name}: the interface is ${overflow}px wider than its window`,
      ).toBeLessThanOrEqual(1);
    });

    /**
     * **Every deck the workspace asks for is drawn, and inside the window.**
     *
     * Four decks that lay out as two-and-a-half is the defect this catches: the
     * count is right, the arrangement is not, and nothing else notices.
     */
    test("every deck is drawn and inside the window", async ({ page }) => {
      await open(page, config);

      const decks = page.locator(DECK);
      await expect(
        decks,
        `${config.name}: asked for ${config.decks} decks`,
      ).toHaveCount(config.decks);

      const boxes = await decks.evaluateAll((els) =>
        els.map((el) => {
          const box = el.getBoundingClientRect();
          return { right: box.right, bottom: box.bottom, width: box.width };
        }),
      );
      for (const [n, box] of boxes.entries()) {
        expect(box.width, `${config.name}: deck ${n + 1} has no width`).toBeGreaterThan(0);
        expect(
          Math.round(box.right),
          `${config.name}: deck ${n + 1} runs past the right edge`,
        ).toBeLessThanOrEqual(config.window.width + SLACK);
      }
    });

    /**
     * **Every surface the workspace names is on screen.**
     *
     * A workspace that silently drops a panel looks identical to one a DJ
     * closed themselves.
     */
    test("every surface it asks for is drawn", async ({ page }) => {
      await open(page, config);

      for (const placement of config.surfaces) {
        await expect(
          page.locator(`.surface[data-surface="${placement.surface}"]`),
          `${config.name}: ${placement.surface} was asked for and is not there`,
        ).toBeVisible();
      }
    });

    /** And it renders without throwing, which a screenshot cannot tell you. */
    test("renders without throwing", async ({ page }) => {
      await open(page, config);
      await page.waitForTimeout(300);
      expect(errorsThrown(page), `${config.name}: threw while rendering`).toEqual([]);
    });
  });
}
