/**
 * The interface's own vocabulary, as the assistant reaches it.
 *
 * §41 asks for the assistant to request *semantic* GUI operations — show
 * Prepare, pin the room panel, focus deck 2 — through a typed vocabulary, and
 * is explicit that it must never emit JavaScript or touch the DOM. The
 * vocabulary and its refusals are Rust's and are tested in `dj_app::uiop`.
 *
 * What a browser can prove is the half that matters here: an operation the DJ
 * did not press arrives through an event and the panel actually appears. That
 * is the whole point of the feature and the one part no Rust test can reach.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Deliver a `cockpit` event the way djmanzo does when an operation lands. */
async function arrive(
  page: import("@playwright/test").Page,
  applied: Record<string, unknown>,
) {
  await page.evaluate((payload) => {
    const win = window as unknown as Record<string, unknown>;
    const id = (win.__handlers as Map<string, number>).get("cockpit");
    if (id === undefined) throw new Error("nothing is listening for cockpit");
    (win[`_${id}`] as (event: unknown) => void)({
      event: "cockpit",
      id: 0,
      payload,
    });
  }, applied);
}

test.describe("the typed interface vocabulary", () => {
  /**
   * **A panel opens without anybody pressing anything.**
   *
   * The assistant asks, Rust applies and stores, and the event is the only way
   * this window would ever find out. Without it §41 is a command that changes a
   * file nobody re-reads.
   */
  test("an operation the DJ did not press still opens the panel", async ({
    page,
  }) => {
    await openShell(page, "/");
    await expect(page.locator('.surface[data-surface="next"]')).toHaveCount(0);

    await arrive(page, {
      workspace: {
        workspace: {
          name: "Custom",
          about: "",
          surfaces: [
            {
              surface: "next",
              dock: "right",
              order: 0,
              collapsed: false,
              pinned: false,
            },
          ],
          density: "standard",
          focus: "performing",
          theme: "",
          decks: 2,
          frozen: false,
        },
        notes: [],
      },
      focus: null,
      what: "opened next",
    });

    await expect(page.locator('.surface[data-surface="next"]')).toBeVisible();
    expect(errorsThrown(page), "the shell threw on a cockpit event").toEqual([]);
  });

  /**
   * Focus marks a deck and then lets go.
   *
   * Latching would teach a DJ to ignore the outline, which costs the next one.
   */
  test("focusing a deck marks it, and it is not stored", async ({ page }) => {
    await openShell(page, "/");
    const focused = page.locator(".deck-slot.looking");
    await expect(focused).toHaveCount(0);

    await arrive(page, {
      workspace: {
        workspace: {
          name: "Perform",
          about: "",
          surfaces: [],
          density: "standard",
          focus: "performing",
          theme: "",
          decks: 2,
          frozen: false,
        },
        notes: [],
      },
      focus: 2,
      what: "focused deck 2",
    });

    await expect(focused).toHaveCount(1);
    // Nothing was docked by it: attention is not an arrangement.
    await expect(page.locator(".surface")).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});
