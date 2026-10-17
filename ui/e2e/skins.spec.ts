/**
 * Karaoke beta B6: a skin over the theme — faint, never taking a press,
 * never on the singers' screen. The patterns and which theme each suits
 * are `src/skins/skins.ts`'s, tested beside it; what a browser holds is
 * what a DJ meets: the skin chosen, the controls under it still pressed.
 */
import { expect, test, type Page } from "@playwright/test";

import { openShell } from "./shell";

const wearing = (page: Page, pkg: string, skin?: string) =>
  page.addInitScript(
    ({ pkg, skin }) => {
      localStorage.setItem("djmanzo.themePackage", pkg);
      if (skin) localStorage.setItem("djmanzo.skin", skin);
    },
    { pkg, skin },
  );

const overlay = (page: Page) =>
  page.evaluate(() => {
    const after = getComputedStyle(document.body, "::after");
    return { events: after.pointerEvents, opacity: Number(after.opacity), content: after.content };
  });

async function singersPanel(page: Page) {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.locator("body").click({ position: { x: 700, y: 120 } });
  await page.keyboard.press("Space");
  await page.keyboard.press("O");
  await page.keyboard.press("i");
  await expect(page.locator("[data-singers]")).toBeVisible();
}

test.describe("Karaoke beta B6: skins", () => {
  /**
   * Chosen in the picker, drawn faintly over everything, and every control
   * under it still takes its press: what the pointer finds at a button's
   * centre is the button, never the skin.
   */
  test("are chosen, drawn faintly, and never take a press", async ({ page }) => {
    await wearing(page, "pkg-caribbean");
    await openShell(page, "/");
    await singersPanel(page);
    const picker = page.getByRole("region", { name: "Skin over the theme" });
    await expect(picker.getByRole("radio", { name: "None" })).toBeChecked();
    await picker.getByRole("radio", { name: "Palm shadows" }).check();
    await expect(page.locator("html")).toHaveAttribute("data-skin", "palms");

    const drawn = await overlay(page);
    expect(drawn.events).toBe("none");
    expect(drawn.content).not.toBe("none");
    expect(drawn.opacity).toBeGreaterThanOrEqual(0.1);
    expect(drawn.opacity).toBeLessThanOrEqual(0.18);

    const buttons = page.locator("button:visible");
    const count = Math.min(await buttons.count(), 40);
    for (let i = 0; i < count; i++) {
      const box = await buttons.nth(i).boundingBox();
      if (!box || box.width < 4 || box.height < 4) continue;
      const hit = await page.evaluate(
        ({ x, y }) => {
          const found = document.elementFromPoint(x, y);
          return found ? found.closest("button") !== null || found.tagName !== "BODY" : false;
        },
        { x: box.x + box.width / 2, y: box.y + box.height / 2 },
      );
      expect(hit, `button ${i} is reachable under the skin`).toBe(true);
    }
    // A real press through the skin: None, back again.
    await picker.getByRole("radio", { name: "None" }).click();
    await expect(page.locator("html")).not.toHaveAttribute("data-skin", /.*/);
  });

  /** The singers' screen never wears one, whatever the main window does. */
  test("are never on the singers' screen", async ({ page }) => {
    await wearing(page, "pkg-caribbean", "palms");
    await openShell(page, "/?panel=singers");
    await expect(page.locator("[data-singer-screen]")).toBeVisible();
    await expect(page.locator("html")).not.toHaveAttribute("data-skin", /.*/);
  });

  /** Fainter still under reduced motion; gone with a theme it does not suit. */
  test("are fainter under reduced motion", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await wearing(page, "pkg-caribbean", "palms");
    await openShell(page, "/");
    await expect(page.locator("html")).toHaveAttribute("data-skin", "palms");
    expect((await overlay(page)).opacity).toBeLessThan(0.1);
  });

  /** A skin kept from another theme is not worn with one it does not suit. */
  test("are only worn with a theme they suit", async ({ page }) => {
    await wearing(page, "pkg-wedding", "palms");
    await openShell(page, "/");
    await expect(page.locator("[data-theme]").first()).toBeAttached();
    await expect(page.locator("html")).not.toHaveAttribute("data-skin", /.*/);
  });

  /**
   * **In Settings too**, under Appearance, beside the theme it is worn over;
   * and with the Singers panel open as well, a choice in either picker is
   * what both show — each its own radio group, not one shared by name.
   */
  test("are chosen in Settings as well, and both pickers agree", async ({ page }) => {
    await wearing(page, "pkg-caribbean");
    await openShell(page, "/");
    await singersPanel(page);
    await page.getByRole("button", { name: "Settings", exact: true }).click();
    const pickers = page.getByRole("region", { name: "Skin over the theme" });
    await expect(pickers).toHaveCount(2);
    const inSettings = page.locator(".surface[data-surface='settings']").getByRole("region", { name: "Skin over the theme" });
    const inSingers = page.locator("[data-singers]").getByRole("region", { name: "Skin over the theme" });
    await inSettings.getByRole("radio", { name: "Palm shadows" }).check();
    await expect(page.locator("html")).toHaveAttribute("data-skin", "palms");
    await expect(inSingers.getByRole("radio", { name: "Palm shadows" })).toBeChecked();
    await expect(inSettings.getByRole("radio", { name: "Palm shadows" })).toBeChecked();
    await inSingers.getByRole("radio", { name: "None" }).check();
    await expect(inSettings.getByRole("radio", { name: "None" })).toBeChecked();
    await expect(inSingers.getByRole("radio", { name: "None" })).toBeChecked();
    await expect(page.locator("html")).not.toHaveAttribute("data-skin", /.*/);
  });
});
