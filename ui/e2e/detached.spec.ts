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

type Asking = {
  __windowAsked?: string[];
  __night?: { density?: string };
  __interfaceDensity?: string;
  __densitySaid?: string[];
  __emitEvent: (name: string, payload: unknown) => boolean;
};

/**
 * §121: the night and requests need something of the main window's — the
 * density band it wears, the collection it searches. Popped out, they ask it
 * through Rust (`window_ask`, whose checking is tested there); the main window
 * carries the request out as it would a key.
 */
test.describe("§121: a panel in a window of its own asks the main window", () => {
  /** A profile that runs denser than the main window is wearing. */
  const FITS = {
    night_fits: {
      density: { to: "pro-dense", name: "Pro Dense", doing: "if-asked", because: "You run Pro Dense over 6 club nights." },
      posture: null,
      withheld: [],
    },
  };

  /**
   * **The night, popped out, notes the band the main window wears — and
   * follows it.** It cannot read the main window's band, and a night noted at
   * a band nobody was wearing would be the profile learning from a guess.
   */
  test("the night in its own window notes the main window's density", async ({ page }) => {
    await openShell(page, "/?panel=night", {}, { interface_density: "Compact" });
    const night = page.locator('main.detached[data-panel="night"]');
    await night.locator(".kind button").filter({ hasText: /^Club$/ }).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as Asking).__night?.density))
      .toBe("Compact");
    // The main window moves; the night's next note is at the new band.
    await page.evaluate(() => (window as unknown as Asking).__emitEvent("interface-density", "Pro Dense"));
    await expect
      .poll(() => page.evaluate(() => (window as unknown as Asking).__night?.density), { timeout: 10_000 })
      .toBe("Pro Dense");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Taking the night's density, popped out, asks the main window to wear
   * it** — `--density` is the main window's, so that is where the press goes.
   */
  test("taking the night's density in its own window asks the main window", async ({ page }) => {
    await openShell(page, "/?panel=night", {}, FITS);
    const night = page.locator('main.detached[data-panel="night"]');
    await night.locator(".kind button").filter({ hasText: /^Club$/ }).click();
    const offer = night.locator('[data-fit="density"]');
    await expect(offer).toContainText("over 6 club nights");
    await offer.getByRole("button").click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as Asking).__windowAsked ?? []))
      .toEqual(["density pro-dense"]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** **Find, popped out, asks the main window to search its collection.** */
  test("requests in their own window ask the main window to find one", async ({ page }) => {
    await openShell(page, "/?panel=requests", {}, {
      audience_status: {
        running: true,
        open: true,
        port: 8765,
        heading: "Ask for a song",
        language: "en",
        show_playing: false,
        ways_in: [],
        announcing: false,
        announce_error: null,
        error: null,
        waiting: 1,
      },
      audience_waiting: [
        { id: 1, text: "  Tune Rising ", voices: 3, first_asked: 1_700_000_000, last_asked: 1_700_000_100, standing: "waiting" },
      ],
    });
    await page.getByRole("button", { name: "Find it" }).click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as Asking).__windowAsked ?? []))
      .toEqual(["find Tune Rising"]);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Requests, popped out, have their languages.** A window of its own
   * mounts the panel before its first snapshot, while the engine does not
   * yet read as up; the list was asked for only at mount, and only if it
   * was, so the room's language had nothing to choose from — found in the
   * running application.
   */
  test("requests in their own window offer the room's languages", async ({ page }) => {
    await openShell(page, "/?panel=requests");
    const language = page.getByRole("combobox", { name: "The room's language" });
    await expect(language.locator("option")).toHaveText(["English", "Deutsch"]);
    await expect(language).toHaveValue("en");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The main window carries out what it is asked** — the other half: a
   * band worn and said to every window, a search in its collection.
   */
  test("the main window wears the band and searches its collection when asked", async ({ page }) => {
    await openShell(page, "/");
    const said = () => page.evaluate(() => (window as unknown as Asking).__densitySaid ?? []);
    await expect.poll(said).toContain("Standard");

    await page.evaluate(() => (window as unknown as Asking).__emitEvent("leaf", "density pro-dense"));
    await expect
      .poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue("--density")))
      .toBe("0.86");
    await expect.poll(said).toContain("Pro Dense");

    await page.evaluate(() => (window as unknown as Asking).__emitEvent("leaf", "find Tune Rising"));
    await expect(
      page.locator('.surface[data-surface="library"]').getByPlaceholder(/search/i).first(),
    ).toHaveValue("Tune Rising");
    expect(errorsThrown(page)).toEqual([]);
  });
});
