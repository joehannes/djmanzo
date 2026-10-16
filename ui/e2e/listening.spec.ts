/**
 * Speech recognition in Settings: automatic or off, and what this computer
 * was found to do.
 *
 * The owner asked for the karaoke build to check whether a friend's Mac can
 * listen and turn it on by itself if it can. Whether it can is Rust's
 * (`dj_app::listening`, by running the helper); every sentence here about the
 * machine is Rust's own, from `listening.json`, which a Rust test blesses.
 */
import { expect, test, type Page } from "@playwright/test";

import statuses from "./listening.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

type Status = (typeof statuses)["helper"];

const recorded = <T,>(page: Page, name: string) =>
  page.evaluate((name) => (window as unknown as Record<string, unknown>)[name] as T, name);

async function settings(page: Page, status: Status, check: Status = status) {
  await openShell(page, "/", {}, { listening_status: status, listening_check: check });
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  const block = page.locator('[data-block="listening"]');
  await block.scrollIntoViewIfNeeded();
  await expect(block).toBeVisible();
  return block;
}

/** Rust's reason as the block writes it: a sentence of its own. */
const sentence = (why: string) => `${why.charAt(0).toUpperCase()}${why.slice(1)}.`;

test.describe("speech recognition in Settings", () => {
  test("a build with whisper.cpp in it listens, and says so", async ({ page }) => {
    const block = await settings(page, statuses.built_in);
    await expect(block.getByRole("radio", { name: "Automatic" })).toBeChecked();
    await expect(block.locator(".listening-found")).toHaveText(
      "This computer listens: speech recognition is part of this build.",
    );
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The karaoke build on a Mac new enough: the helper ran, so it listens. */
  test("the karaoke build on a Mac that runs the helper listens through it", async ({ page }) => {
    const block = await settings(page, statuses.helper);
    await expect(block.getByRole("radio", { name: "Automatic" })).toBeChecked();
    await expect(block.locator(".listening-found")).toHaveText(
      "This computer listens, through djmanzo's speech recognition helper.",
    );
  });

  /** A 2011 Mac's last macOS: Rust's sentence, word for word. */
  test("a Mac that cannot says why, in Rust's words", async ({ page }) => {
    const block = await settings(page, statuses.old_mac);
    await expect(block.locator(".listening-found")).toHaveText(sentence(statuses.old_mac.why!));
    await expect(block.locator(".listening-found")).toContainText("macOS 10.13");
  });

  /**
   * Off is sent as off, and the block says whether this computer could —
   * so a DJ who turned it off knows turning it back on would do something.
   * Automatic is sent back as automatic.
   */
  test("off is kept, and says whether this computer could listen", async ({ page }) => {
    const block = await settings(page, statuses.helper);
    await block.getByRole("radio", { name: "Off" }).check();
    await expect(block.locator(".listening-found")).toHaveText("Off: nothing listens. This computer could.");
    await expect(block.getByRole("radio", { name: "Off" })).toBeChecked();
    await block.getByRole("radio", { name: "Automatic" }).check();
    await expect(block.locator(".listening-found")).toHaveText(
      "This computer listens, through djmanzo's speech recognition helper.",
    );
    expect(await recorded(page, "__listeningSet")).toEqual([true, false]);
  });

  /** Switched off on a computer that could not anyway: off, and nothing more. */
  test("off on a computer that cannot claims nothing about it", async ({ page }) => {
    const block = await settings(page, { ...statuses.old_mac, off: true });
    await expect(block.locator(".listening-found")).toHaveText("Off: nothing listens.");
  });

  /** Check again asks the machine, and shows what it answered. */
  test("check again asks the machine again", async ({ page }) => {
    const block = await settings(page, statuses.old_mac, statuses.helper);
    await block.getByRole("button", { name: "Check again" }).click();
    await expect(block.locator(".listening-found")).toHaveText(
      "This computer listens, through djmanzo's speech recognition helper.",
    );
    expect(await recorded(page, "__listeningChecked")).toBe(1);
  });
});
