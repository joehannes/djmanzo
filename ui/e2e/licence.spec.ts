/**
 * §131: the licence, in Settings.
 *
 * One download for every DJ, unlocked by the licence their sign-in fetches
 * (docs/EDITIONS.md). The answers are the shape `dj_app::licence::Status`
 * serialises to; what is measured is that the section says where the DJ
 * stands in words, and that a pasted licence is handed to Rust and its answer
 * shown.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const status = (fields: Record<string, unknown>) => ({
  state: "signed_out",
  edition: null,
  product: null,
  modules: [],
  account: null,
  trial_days_left: null,
  renew_due: false,
  development: false,
  reason: null,
  waiting: false,
  missing: [],
  ...fields,
});

async function openLicence(page: Page, answers: Record<string, unknown>) {
  await openShell(page, "/", {}, answers);
  await page.getByRole("button", { name: /settings/i }).first().click();
  const section = page.locator("[data-licence]");
  await expect(section).toBeVisible();
  return section;
}

test.describe("§131: the licence, in Settings", () => {
  test("a running trial says how many days are left", async ({ page }) => {
    const section = await openLicence(page, {
      licence_status: status({ state: "trial", edition: "full", product: "djmanzo", trial_days_left: 3, account: "github:1" }),
    });
    await expect(section).toHaveAttribute("data-licence", "trial");
    await expect(section).toContainText("Trying djmanzo: 3 days left.");
    // And in the top bar, quietly: the only other place a trial is mentioned.
    await expect(page.locator("[data-trial-days]")).toHaveText("Trial · 3 days");
    expect(errorsThrown(page)).toEqual([]);
  });

  test("a licence that is not accepted says why", async ({ page }) => {
    const section = await openLicence(page, {
      licence_status: status({ state: "refused", reason: "the licence's signature does not match" }),
    });
    await expect(section).toContainText("not accepted: the licence's signature does not match.");
  });

  /** The load-bearing one: what is pasted goes to Rust, and Rust's answer is what shows. */
  test("a pasted licence is handed to djmanzo and its answer shown", async ({ page }) => {
    const section = await openLicence(page, {
      licence_status: status({}),
      licence_install: status({ state: "licensed", edition: "karaoke", product: "djmanzo Karaoke", account: "google:7", modules: ["karaoke"] }),
    });
    await expect(section).toContainText("Not signed in.");
    const use = section.getByRole("button", { name: "Use this licence" });
    await expect(use).toBeDisabled();
    await section.getByLabel("Paste a licence").fill("djmanzo-licence-1.abc.def");
    await use.click();
    await expect(section).toContainText("djmanzo Karaoke, signed in as google:7.");
    await expect(section.getByRole("button", { name: "Sign out" })).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * §130's quiet card: in an edition without the teacher, the Practice panel
   * is a card that says what it is and where it is, drawn where the panel
   * would be. Rust says what is missing; the interface only draws it.
   */
  test("a panel the edition does not carry is a quiet card in its place", async ({ page }) => {
    await openShell(page, "/", {}, {
      licence_status: status({
        state: "licensed",
        edition: "karaoke",
        product: "djmanzo Karaoke",
        modules: ["karaoke"],
        missing: [{ module: "teacher", title: "Teacher", about: "Lessons and a coach.", edition: "djmanzo Apprentice" }],
      }),
    });
    await page.keyboard.press("7");
    const card = page.locator('[data-quiet-card="teacher"]');
    await expect(card).toBeVisible();
    await expect(card).toContainText("In djmanzo Apprentice, and in djmanzo Full.");
    expect(errorsThrown(page)).toEqual([]);
  });

  /** The sign-in buttons say which account, and a trial says which edition. */
  test("signing in and starting a trial reach djmanzo with the provider and the edition", async ({ page }) => {
    const section = await openLicence(page, {
      licence_status: status({}),
      licence_sign_in: status({ state: "trial", edition: "karaoke", product: "djmanzo Karaoke", trial_days_left: 7 }),
    });
    await section.getByLabel("Edition to try").selectOption("karaoke");
    await section.locator('[data-trial-with="github"]').click();
    await expect(section).toContainText("Trying djmanzo Karaoke: 7 days left.");
    const asked = await page.evaluate(() => (window as unknown as { __signIns?: unknown[] }).__signIns ?? []);
    expect(asked).toEqual([{ provider: "github", trial: "karaoke" }]);
    expect(errorsThrown(page)).toEqual([]);
  });
});
