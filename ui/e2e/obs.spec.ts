/**
 * §119: djmanzo drives OBS — the section in the settings.
 *
 * The link is Rust's (`dj_app::obs`, tested there against an OBS in memory;
 * `dj_net::obs` against a fake obs-websocket server over a real socket). What
 * a browser can prove is the other half: what the DJ presses is what is sent,
 * what Rust says is what is drawn, and the password goes one way.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

type ObsWindow = {
  __obsSet?: Record<string, unknown>[];
  __obsAsked?: Record<string, unknown>[];
  __obsStatus?: unknown;
};

const CONNECTED = {
  on: true,
  connected: true,
  error: null,
  streaming: false,
  scenes: { current: "Decks", all: ["Decks", "Crowd", "Singer"] },
};

async function openObs(page: Page, answers: Record<string, unknown> = {}) {
  await openShell(page, "/?panel=settings", {}, answers);
  const obs = page.locator("[data-obs]");
  await expect(obs).toBeVisible();
  return obs;
}

const seen = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as ObsWindow;
    return { set: w.__obsSet ?? [], asked: w.__obsAsked ?? [] };
  });

test.describe("§119: djmanzo drives OBS", () => {
  /**
   * **Off until switched on**, and switching it on sends the address the DJ
   * typed — with no password unless one was typed: the field is empty
   * because the password is never read back, and an empty field must not
   * clear the one Rust keeps.
   */
  test("switching the link on sends the address, and keeps the password unless one is typed", async ({ page }) => {
    const obs = await openObs(page, {
      obs_settings: { on: false, host: "localhost", port: 4455, has_password: true },
    });
    const password = obs.getByLabel("OBS WebSocket password");
    await expect(password).toHaveValue("");
    await expect(password).toHaveAttribute("placeholder", /kept/);
    await expect(obs.locator("[data-obs-state]")).toHaveCount(0);

    await obs.getByRole("textbox", { name: "OBS address" }).fill("192.168.1.20");
    await obs.getByRole("checkbox", { name: "Drive OBS from djmanzo" }).check();
    await expect
      .poll(async () => (await seen(page)).set.at(-1))
      .toEqual({ on: true, host: "192.168.1.20", port: 4455, password: null });

    await password.fill("hunter2");
    await password.press("Tab");
    await expect.poll(async () => (await seen(page)).set.at(-1)?.password).toBe("hunter2");
    // Sent, then emptied: what was typed is not left on the screen.
    await expect(password).toHaveValue("");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Connected, it says what OBS is doing and carries out the DJ's
   * presses**: the stream started, a scene put on air, the scene on air
   * marked as such.
   */
  test("connected, the stream and the scenes are the DJ's to press", async ({ page }) => {
    const obs = await openObs(page, {
      obs_settings: { on: true, host: "localhost", port: 4455, has_password: true },
      obs_status: CONNECTED,
    });
    await expect(obs.locator('[data-obs-state="connected"]')).toContainText("Not streaming");
    await expect(obs.locator('[data-obs-state="connected"]')).toContainText("Decks");
    const scenes = obs.getByRole("group", { name: "OBS scenes" });
    await expect(scenes.getByRole("button", { name: "Decks" })).toHaveAttribute("aria-pressed", "true");
    await expect(scenes.getByRole("button", { name: "Crowd" })).toHaveAttribute("aria-pressed", "false");

    await obs.getByRole("button", { name: "Start the stream" }).click();
    await scenes.getByRole("button", { name: "Crowd" }).click();
    await expect
      .poll(async () => (await seen(page)).asked)
      .toEqual([
        { cmd: "obs_stream", on: true },
        { cmd: "obs_scene", scene: "Crowd" },
      ]);

    // Rust says the stream is on: the button turns into its opposite.
    await page.evaluate(
      (status) => ((window as unknown as ObsWindow).__obsStatus = status),
      { ...CONNECTED, streaming: true, scenes: { ...CONNECTED.scenes, current: "Crowd" } },
    );
    await expect(scenes.getByRole("button", { name: "Crowd" })).toHaveAttribute("aria-pressed", "true");
    await obs.getByRole("button", { name: "Stop the stream" }).click();
    await expect.poll(async () => (await seen(page)).asked.at(-1)).toEqual({ cmd: "obs_stream", on: false });
    expect(errorsThrown(page)).toEqual([]);
  });

  /** **Not reached, it says why, and that it is trying again.** */
  test("an OBS not answering is said in Rust's words", async ({ page }) => {
    const obs = await openObs(page, {
      obs_settings: { on: true, host: "localhost", port: 4455, has_password: false },
      obs_status: {
        ...CONNECTED,
        connected: false,
        error: "OBS refused the password",
        scenes: { current: "", all: [] },
      },
    });
    const state = obs.locator('[data-obs-state="waiting"]');
    await expect(state).toContainText("OBS refused the password");
    await expect(state).toContainText("tries again by itself");
    await expect(obs.getByRole("button", { name: "Start the stream" })).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });
});
