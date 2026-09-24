/**
 * §108: a night handed to the networks — WhatsApp, X, Bluesky, Threads.
 *
 * > improve integration of social networks
 *
 * What each channel carries is Rust's (`dj_app::share::message_for`, swept
 * across set sizes there), and `share.json` is its output for one fixed night
 * of twelve records. What the browser holds is the sheet: the channels are
 * offered, the preview and the warning are the chosen channel's own — a post
 * on X holds seven of the twelve, and says so — and the button hands the
 * night to the channel chosen, never posting anything itself.
 */
import { expect, test } from "@playwright/test";

import share from "./share.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

test.describe("§108: sharing a night", () => {
  test("each channel previews what it will carry, and the button hands it there", async ({ page }) => {
    await openShell(page, "/", {}, {
      list_sessions: [{ id: "Sábado", tracks: 12, ended_at: 1_700_003_000 }],
      play_history: [],
    });
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await page.getByRole("button", { name: "History", exact: true }).click();
    await page.getByRole("button", { name: /Sábado · 12/ }).click();

    const sheet = page.locator("section.share");
    const channels = sheet.getByRole("radiogroup", { name: "Where to share it" }).getByRole("radio");
    await expect(channels).toHaveText(["WhatsApp", "X", "Bluesky", "Threads"]);
    await expect(channels.first()).toHaveAttribute("aria-checked", "true");

    // WhatsApp holds the whole night.
    const preview = sheet.locator("[data-preview]");
    await expect(preview).toContainText("Joe Veras - Intentalo Tú");
    await expect(sheet.locator("[data-dropped]")).toHaveCount(0);

    // **X holds a handful, and the preview and the warning say which.**
    await sheet.locator('[data-channel="x"]').click();
    await expect(preview).toHaveText(share.messages.x.message);
    await expect(preview).not.toContainText("Joe Veras");
    await expect(sheet.locator("[data-dropped]")).toContainText(
      `${share.messages.x.dropped} of 12 won't fit in a 280-character X post`,
    );

    // The button names the channel and hands the night to it.
    const open = sheet.locator("[data-open]");
    await expect(open).toHaveText("Open X");
    await open.click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __shared?: string[] }).__shared ?? []))
      .toEqual(["x"]);
    // Opened, not posted: a person still presses the button.
    await expect(sheet.locator(".note")).toContainText("X is open with the post written");

    await sheet.locator('[data-channel="bluesky"]').click();
    await expect(open).toHaveText("Open Bluesky");
    await expect(preview).toHaveText(share.messages.bluesky.message);
    await open.click();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __shared?: string[] }).__shared ?? []))
      .toEqual(["x", "bluesky"]);
    expect(errorsThrown(page)).toEqual([]);
  });
});
