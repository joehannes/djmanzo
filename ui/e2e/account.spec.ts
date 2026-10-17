/**
 * The account on djmanzo's website -- the block in the settings.
 *
 * The verifying is Rust's (`dj_app::account`, tested there against tokens
 * the website's own code signed). What a browser can prove is the other
 * half: signing in shows the code the DJ must find in the browser, a
 * licence Rust stands behind is named with what it covers, a lapsed one says
 * why, and the beta's promise -- nothing switched off -- is on screen.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

type AccountWindow = {
  __accountAsked?: Record<string, unknown>[];
  __accountStatus?: Record<string, unknown>;
};

async function openAccount(page: Page, answers: Record<string, unknown> = {}) {
  await openShell(page, "/?panel=settings", {}, answers);
  const block = page.locator("[data-account]");
  await expect(block).toBeVisible();
  return block;
}

const base = {
  site: "https://djmanzo.example",
  signed_in: true,
  name: "Yolanda",
  email: "yolanda@example.com",
  licences: [],
  pending: null,
  enforced: false,
  machine_name: "Booth laptop (Linux)",
  note: null,
};

test.describe("The account and its licence", () => {
  test("signing in shows the code to look for in the browser", async ({ page }) => {
    const block = await openAccount(page);
    await expect(block.locator("[data-account-state]")).toHaveAttribute("data-account-state", "signed-out");
    await expect(block.locator("[data-account-beta]")).toContainText("every feature is open");
    await block.getByRole("button", { name: "Sign in" }).click();
    await expect.poll(async () => page.evaluate(() => ((window as unknown as AccountWindow).__accountAsked ?? []).map((a) => a.cmd))).toContain("account_link");
    await expect(block.locator("[data-account-code]")).toHaveText("ABCD-EFGH");
    expect(errorsThrown(page)).toEqual([]);
  });

  test("a licence Rust stands behind is named, with how long it works offline", async ({ page }) => {
    const block = await openAccount(page, {
      account_status: {
        ...base,
        standing: { kind: "licensed", edition: "karaoke", plan: "yearly", until: 1831536000, offline_until: 1802592000, demo: false },
      },
    });
    const licence = block.locator("[data-account-licence]");
    await expect(licence).toContainText("djmanzo Karaoke, yearly");
    await expect(licence).toContainText("Works offline until");
    await block.getByRole("button", { name: "Sign out on this computer" }).click();
    await expect(block.locator("[data-account-state]")).toHaveAttribute("data-account-state", "signed-out");
    expect(errorsThrown(page)).toEqual([]);
  });

  test("a lapsed licence says why, in Rust's words", async ({ page }) => {
    const block = await openAccount(page, {
      account_status: { ...base, standing: { kind: "lapsed", reason: "djmanzo has not reached the website for thirty days — connect once to renew." } },
    });
    await expect(block.locator("[data-account-licence]")).toContainText("thirty days");
    expect(errorsThrown(page)).toEqual([]);
  });
});
