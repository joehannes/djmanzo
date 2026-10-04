/**
 * §119: YouTube's live chat -- the section in the settings.
 *
 * The reading is Rust's (`dj_app::youtube`, tested there against a scripted
 * Google: the sign-in's PKCE, the loopback address, the pages of chat, the
 * quota). What a browser can prove is the other half: what the DJ sets is
 * what is sent, the sign-in is offered only once there is a client to sign
 * in as, and what Rust says the reading is doing is what is drawn.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

type YouTubeWindow = {
  __youtubeAsked?: Record<string, unknown>[];
  __youtubeStatus?: Record<string, unknown>;
  __youtubeSignInFails?: string;
};

async function openYouTube(page: Page, answers: Record<string, unknown> = {}) {
  await openShell(page, "/?panel=settings", {}, answers);
  const block = page.locator("[data-youtube]");
  await expect(block).toBeVisible();
  return block;
}

const asked = (page: Page) =>
  page.evaluate(() => (window as unknown as YouTubeWindow).__youtubeAsked ?? []);

test.describe("§119: YouTube's live chat", () => {
  /**
   * **No client, no sign-in**: until a Google client id is set the block
   * says what to register, and offers no sign-in; the id and the switch
   * typed are sent as typed, trimmed; then the sign-in is offered.
   */
  test("the sign-in is offered once there is a client to sign in as", async ({ page }) => {
    const block = await openYouTube(page);
    await expect(block.locator("[data-youtube-how]")).toContainText("Desktop app");
    await expect(block.getByRole("button", { name: "Sign in with Google" })).toHaveCount(0);

    await block.getByRole("textbox", { name: "Google client id" }).fill(" 123-abc.apps.googleusercontent.com ");
    await block.getByRole("textbox", { name: "Google client id" }).press("Tab");
    await block.getByRole("checkbox", { name: "Read my YouTube live chat into the crowd" }).check();
    await expect
      .poll(async () => (await asked(page)).filter((a) => a.cmd === "youtube_settings_set").at(-1))
      .toEqual({ cmd: "youtube_settings_set", on: true, clientId: "123-abc.apps.googleusercontent.com", clientSecret: "" });
    await expect(block.locator("[data-youtube-how]")).toHaveCount(0);

    await block.getByRole("button", { name: "Sign in with Google" }).click();
    await expect.poll(async () => (await asked(page)).some((a) => a.cmd === "youtube_sign_in")).toBe(true);
    await expect(block.getByRole("button", { name: "Sign out" })).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Signed in, it says what it is reading, or why not**, in Rust's words;
   * signing out is the DJ's; a sign-in that fails is said.
   */
  test("signed in, what the reading is doing is said in Rust's words", async ({ page }) => {
    const block = await openYouTube(page, {
      youtube_settings: { on: true, client_id: "id", client_secret: "" },
      youtube_status: {
        on: true,
        client: true,
        signed_in: true,
        reading: "Saturday night at La Terraza",
        read: 42,
        note: null,
      },
    });
    const state = block.locator("[data-youtube-state]");
    await expect(state).toHaveAttribute("data-youtube-state", "reading");
    await expect(state).toContainText("Reading the chat of Saturday night at La Terraza · 42 brought in");

    // Rust says the broadcast ended.
    await page.evaluate(() => {
      const w = window as unknown as YouTubeWindow;
      Object.assign(w.__youtubeStatus!, { reading: null, note: "No live broadcast on your channel right now — djmanzo looks again every half minute." });
    });
    await expect(state).toHaveAttribute("data-youtube-state", "waiting");
    await expect(block.locator("[data-youtube-note]")).toContainText("No live broadcast");

    await block.getByRole("button", { name: "Sign out" }).click();
    await expect(block.getByRole("button", { name: "Sign in with Google" })).toBeVisible();
    await page.evaluate(() => {
      (window as unknown as YouTubeWindow).__youtubeSignInFails = "the sign-in was declined";
    });
    await block.getByRole("button", { name: "Sign in with Google" }).click();
    await expect(block.getByRole("alert")).toContainText("the sign-in was declined");
    await expect(block.getByRole("button", { name: "Sign in with Google" })).toBeEnabled();
    expect(errorsThrown(page)).toEqual([]);
  });
});
