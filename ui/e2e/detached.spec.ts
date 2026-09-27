/**
 * §121: every panel, in a window of its own.
 *
 * > pop out
 *
 * A detached window mounts `Detached.svelte` with `?panel=` and draws that one
 * panel from the same snapshot and commands as the main window. Which panels
 * there are is Rust's list (`monitors::Panel`, answered here from its golden),
 * and a Rust test holds that `Detached.svelte` names each one; this holds that
 * each one actually draws, in a browser, without an error.
 */
import { expect, test } from "@playwright/test";

import panels from "./panels.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

test.describe("§121: a panel in a window of its own", () => {
  for (const panel of panels) {
    test(`${panel.id} draws in its own window`, async ({ page }) => {
      await openShell(page, `/?panel=${panel.id}`);
      const window = page.locator(`main.detached[data-panel="${panel.id}"]`);
      await expect(window).toBeVisible();
      await expect(window).not.toContainText("No panel called");
      await expect(window).not.toContainText("Waiting for the engine");
      // Something of the panel itself, not just the frame around it.
      await expect(window.locator(":scope > *:not(.error)").first()).toBeVisible();
      expect(errorsThrown(page)).toEqual([]);
    });
  }
});
