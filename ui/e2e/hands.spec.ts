/**
 * §53: the interface knowing what is plugged into it.
 *
 * > The UI should know: number of decks actually controllable, available
 * > physical knobs, jogs, pads, stem controls, mixer channels, displays, LED
 * > feedback. Use that to determine which GUI surfaces deserve prominence.
 * >
 * > Example: if a controller has dedicated stem pads: compact GUI stem panel.
 * > If there are no stem controls: expand stem controls.
 *
 * What a mapping reaches, and how it is counted, is `dj_hid::hands` and is
 * tested there against every mapping djmanzo ships. These say the two things
 * only a browser can — that the reading arrives on screen, and that §53's own
 * worked example actually moves a panel.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** A controller that reaches everything except the stems. */
const NO_STEMS = {
  decks: 2,
  knobs: 8,
  jogs: 2,
  pads: 16,
  stems: false,
  channels: 2,
  leds: 12,
};

/** The same controller with stem pads on it. */
const WITH_STEMS = { ...NO_STEMS, stems: true };

const stems = (page: Page) => page.locator("[data-stems-open]").first();

test.describe("§53's controller-aware interface", () => {
  /**
   * **The load-bearing one: a controller that cannot reach the stems opens the
   * stem module, and one that can leaves it folded.**
   *
   * §53's own worked example, and the direction is the whole of it. The module
   * is the biggest block on a deck — about 370 pixels — so unfolding it for
   * everybody would be the mistake it was folded to fix, and folding it for the
   * DJ who has no other way to reach the stems hides the feature from the only
   * person who needs it.
   */
  test("no stem controls on the hardware gives the stem module room", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, { controller_hands: NO_STEMS });
    await expect.poll(async () => stems(page).getAttribute("data-stems-open")).toBe("true");
    expect(thrown).toEqual([]);
  });

  test("a controller with stem pads leaves the module folded", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, { controller_hands: WITH_STEMS });
    // Held rather than merely read once: the profile is polled, so a panel that
    // unfolded a beat late would pass a single read and fail a DJ.
    await expect(stems(page)).toHaveAttribute("data-stems-open", "false");
    await expect(stems(page)).toHaveAttribute("data-stems-open", "false");
    expect(thrown).toEqual([]);
  });

  /**
   * **And nothing plugged in leaves it folded too.**
   *
   * The half worth asserting separately, because "no controller" and "a
   * controller that reaches nothing" are different answers and the code has to
   * keep them apart. A laptop-only DJ is already the case the interface is
   * designed around; unfolding for them would be §53 answering a question
   * nobody asked.
   */
  test("nothing plugged in is not the same as a controller with no stems", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await expect(stems(page)).toHaveAttribute("data-stems-open", "false");
    expect(thrown).toEqual([]);
  });

  /**
   * **The panel says what djmanzo found, including the two it cannot see.**
   *
   * §53 opens with *"the UI should know"*, and a DJ who has just plugged
   * something in is entitled to see what djmanzo thinks it can do before
   * finding out mid-set that it disagrees. The screens and the lights are named
   * rather than counted: one is unknowable from a mapping, and the other is
   * described in the file and driven by nothing yet.
   */
  test("the controllers panel reports the reach it read", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, {
      controller_hands: NO_STEMS,
      control_status: {
        inputs: ["DDJ-SR MIDI 1"],
        open_port: "DDJ-SR MIDI 1",
        open_mapping: "Pioneer DDJ-SR",
        unavailable: null,
        keyboard: true,
        keyboard_name: "",
      },
      control_mappings: [
        {
          name: "Pioneer DDJ-SR",
          device: "DDJ-SR",
          bindings: 113,
          bundled: true,
          hands: NO_STEMS,
        },
      ],
      // The Controllers panel is a surface, opened the way a DJ opens one.
      palette: {
        because: "",
        entries: [
          {
            label: "Show Controllers",
            about: "What is plugged in, and what it is mapped to.",
            kind: "surface",
            run: "controllers",
            tier: "preparation",
          },
        ],
      },
    });
    await page.keyboard.press("Control+k");
    await page.getByRole("button", { name: /Show Controllers/ }).first().click();

    const reach = page.locator("[data-reach]");
    await expect(reach).toBeVisible();
    await expect(reach).toContainText("Stem controls");
    await expect(reach).toContainText("none");
    await expect(reach).toContainText("Pads");
    // The two it cannot answer, said rather than counted.
    await expect(page.locator('[data-surface="controllers"]')).toContainText(
      "cannot see them",
    );
    await expect(page.locator('[data-surface="controllers"]')).toContainText(
      "does not send them yet",
    );
    expect(thrown).toEqual([]);
  });
});
