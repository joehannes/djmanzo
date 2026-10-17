/**
 * §130/§131: an edition draws only what it carries.
 *
 * Rust decides what each edition carries and refuses the rest at the door
 * (`crate::licence`), and the lists the interface is drawn from — panels,
 * activities, the palette, the pad pages, the tree under Space — already
 * leave it out; those are tested in Rust. These say the part only a browser
 * can: that the few controls the interface draws without asking Rust for a
 * list first are not drawn as buttons that would be refused, and that the
 * edition's name is the one a DJ is welcomed by.
 */
import { expect, test } from "@playwright/test";

import welcome from "./welcome.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const HOBBYIST = {
  edition: "hobbyist",
  product_name: "djmanzo Hobbyist",
  modules: [],
  missing: [
    { module: "stems", title: "Stems", edition: "djmanzo Pro DJ" },
    { module: "sources", title: "Online sources", edition: "djmanzo Pro DJ" },
  ],
  development: true,
};

test.describe("§130/§131: an edition draws only what it carries", () => {
  test("a Hobbyist deck draws no stem controls, and Full's does", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, { edition: HOBBYIST });
    await expect(page.locator("section.deck[data-deck]").first()).toBeVisible();
    await expect(page.locator("[data-stems-open]")).toHaveCount(0);

    await openShell(page, "/");
    await expect(page.locator("[data-stems-open]").first()).toBeAttached();
    expect(thrown).toEqual([]);
  });

  test("the library offers no outside sources without the module", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, { edition: HOBBYIST });
    await page.keyboard.press("Control+f");
    await expect(page.locator('[data-surface="library"]')).toBeVisible();
    await expect(page.getByText("Include external sources")).toHaveCount(0);

    await openShell(page, "/");
    await page.keyboard.press("Control+f");
    await expect(page.getByText("Include external sources")).toBeVisible();
    expect(thrown).toEqual([]);
  });

  test("the welcome says the edition's name", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, {
      edition: HOBBYIST,
      welcome_state: { seen: false, answers: welcome.fresh },
    });
    await expect(page.getByRole("heading", { name: "Welcome to djmanzo Hobbyist" })).toBeVisible();
    expect(thrown).toEqual([]);
  });
});
