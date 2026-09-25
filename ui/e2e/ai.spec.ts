/**
 * §120: which AI the assistant uses, found where a DJ looks for an account.
 *
 * > I don't know what AI can be used right now, but I want to be able to use
 * > any OpenRouter AI, ChatGPT, Gemini Free and other models, Claude ... so
 * > there shall be a config as well to configure tokens/accounts ... and
 * > enable/select AI
 *
 * djmanzo already spoke to all of them, behind a "Setup" button inside the
 * assistant's panel. It is in Settings now too, drawn from Rust's own rows
 * (`providers.json`, written by `tests/e2e_fixture.rs`: OpenRouter and Google
 * keyed, the local model not running, the rest waiting for a key).
 */
import { expect, test, type Page } from "@playwright/test";

import providers from "./providers.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const WITH_PROVIDERS = {
  list_llm_providers: providers,
  assistant_state: { provider: "openrouter", model: "openrouter/fast", spent_usd: 0, cap_usd: 2, unpriced_calls: 0 },
};

const chosen = (page: Page) =>
  page.evaluate(
    () =>
      ((window as unknown as { __chosenModels?: { provider: string; model: string }[] })
        .__chosenModels ?? []),
  );

async function openSettings(page: Page) {
  await openShell(page, "/", {}, WITH_PROVIDERS);
  await page.getByRole("button", { name: /settings/i }).first().click();
  const section = page.locator('[data-settings="ai"]');
  await expect(section).toBeVisible();
  return section;
}

test.describe("§120: the AI, in Settings", () => {
  /** Every provider Rust offers is there, with what it needs. */
  test("Settings lists every provider, and which one is in use", async ({ page }) => {
    const section = await openSettings(page);
    for (const row of providers) {
      await expect(section.locator(`[data-provider="${row.id}"]`)).toContainText(row.label);
    }
    await expect(section.locator("[data-ai-in-use]")).toContainText("OpenRouter");
    await expect(section.locator('[data-provider="anthropic"]')).toContainText(
      providers.find((p) => p.id === "anthropic")!.status_detail,
    );
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The load-bearing one: a model goes to the provider it was listed
   * from.** With OpenRouter and Google both ready, choosing a model from
   * Google's list must set Google — the old panel set whichever provider was
   * ready first, and sent a Gemini model to OpenRouter.
   */
  test("a model chosen from Google's list is set with Google", async ({ page }) => {
    const section = await openSettings(page);
    const google = section.locator('[data-provider="google"]');
    await google.getByRole("button", { name: "Models" }).click();
    await google.getByRole("button", { name: /google large/ }).click();

    await expect.poll(() => chosen(page)).toEqual([{ provider: "google", model: "google/large" }]);
    await expect(section.locator("[data-ai-in-use]")).toContainText("Google");
    await expect(google).toContainText("in use");
    expect(errorsThrown(page)).toEqual([]);
  });
});
