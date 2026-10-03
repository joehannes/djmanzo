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

/**
 * **The session log, popped out, keeps up with the set.** A log on a screen
 * of its own beside the set being played that stopped at the moment it was
 * opened would be a record of nothing; it asks again every two seconds.
 */
test("the session log in its own window keeps up with the set", async ({ page }) => {
  await openShell(page, "/?panel=log");
  const log = page.getByRole("region", { name: "Session log" });
  await expect(log).toContainText("(nothing yet)");
  await page.evaluate(() => {
    (window as unknown as { __sessionLog: string[] }).__sessionLog = ["   1.250  deck 1 play", "   9.500  mic on"];
  });
  await expect(log).toContainText("deck 1 play", { timeout: 5_000 });
  await expect(log).toContainText("mic on");
  expect(errorsThrown(page)).toEqual([]);
});
