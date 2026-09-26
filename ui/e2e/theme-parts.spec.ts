/**
 * §121: themes in parts, and regions whose rims can be seen.
 *
 * > the system is to be extended into package that can be applied
 * > partially: the theme; effect skinning/parts of the theme that are
 * > artistic ...; interactive skinning (... theme items that apply on
 * > specific events); any theme can be selected in the basic or more
 * > parts/fully
 * >
 * > Main regions are similar, edges are distinct and distinguishing
 *
 * `controls/themes/parts.ts` is the rule and its vitest holds it; this holds
 * that the menu reaches it, that the choice is kept, and that the window's
 * edge wears an event the way the theme draws it. The fixture's deck 1 is
 * playing with twenty-two seconds left and nothing after it -- a record
 * running out, which is the first event there is.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";
import snapshot from "./snapshot.json" with { type: "json" };

/** The autopilot on, and not mid-transition. */
const autopilot = { automix: { ...snapshot.master.automix, enabled: true, mixing: false } };

const skin = (page: Page) => page.locator(".event-skin");

async function themes(page: Page) {
  await page.locator(".switcher button.icon").click();
  await expect(page.locator(".switcher .menu")).toBeVisible();
  return page.locator(".switcher .menu");
}

/** The outline of the first themed control: what the shapes part draws. */
const outline = (page: Page) =>
  page.locator(".knob-container svg path").first().getAttribute("d");

test.describe("§121: themes in parts", () => {
  /**
   * **The load-bearing one.** Full wears everything, Basic takes the
   * controls' shapes and the event skin away, Parts takes each part from
   * where it is told -- and the choice is still there after a restart.
   */
  test("Basic, Parts and Full change what is worn, and the choice is kept", async ({ page }) => {
    await openShell(page, "/");
    // Organic, whole: its stones, and its glow on the edge for a record
    // running out.
    await expect(skin(page)).toHaveAttribute("data-event", "out");
    await expect(skin(page)).toHaveAttribute("data-style", "glow");
    const stone = await outline(page);

    const menu = await themes(page);
    const amount = menu.getByRole("radiogroup", { name: "How much of the theme" });
    await expect(amount.getByRole("radio", { name: "Full" })).toHaveAttribute("aria-checked", "true");

    await amount.getByRole("radio", { name: "Basic" }).click();
    await expect(amount.getByRole("radio", { name: "Basic" })).toHaveAttribute("aria-checked", "true");
    await expect(skin(page)).toHaveCount(0);
    const plain = await outline(page);
    expect(plain).not.toBe(stone);
    // The colours stay the chosen theme's: the name on the button is too.
    await expect(page.locator(".switcher button.icon")).toContainText("Organic");

    // Parts: the events from Industrial, drawn as its hazard bands.
    await amount.getByRole("radio", { name: "Parts" }).click();
    await menu.getByRole("combobox", { name: "Events" }).selectOption({ label: "From Industrial Techno (hazard bands)" });
    await expect(skin(page)).toHaveAttribute("data-style", "flash");
    await menu.getByRole("combobox", { name: "Shapes" }).selectOption("none");
    expect(await outline(page)).toBe(plain);
    await expect(amount.getByRole("radio", { name: "Parts" })).toHaveAttribute("aria-checked", "true");

    await page.reload();
    await expect(skin(page)).toHaveAttribute("data-style", "flash");
    expect(await outline(page)).toBe(plain);
    const again = await themes(page);
    await expect(again.getByRole("combobox", { name: "Events" })).toHaveValue("pkg-industrial");

    await again.getByRole("radio", { name: "Full" }).click();
    await expect(skin(page)).toHaveAttribute("data-style", "glow");
    expect(await outline(page)).toBe(stone);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * Each event, the most pressing first: a record running out only when the
   * autopilot will not mix out of it, then a recording, then the autopilot.
   */
  test("a recording and the autopilot each mark the edge, and never take a click", async ({ page }) => {
    await openShell(page, "/", {
      recording: { active: true, seconds: 12, dropped: 0, failed: false },
    });
    // Deck 1 running out outranks the recording…
    await expect(skin(page)).toHaveAttribute("data-event", "out");
    await expect(skin(page)).toHaveCSS("pointer-events", "none");

    // …until the autopilot has the mix, and it will not run out.
    await openShell(page, "/", {
      recording: { active: true, seconds: 12, dropped: 0, failed: false },
      ...autopilot,
    });
    await expect(skin(page)).toHaveAttribute("data-event", "rec");
    // Still: only a record running out moves.
    await expect(skin(page)).toHaveCSS("animation-name", "none");

    await openShell(page, "/", autopilot);
    await expect(skin(page)).toHaveAttribute("data-event", "auto");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Edges distinct, regions calm.** The rim of the bar, a deck and a panel
   * is `--frame`, not the hairline the controls inside use.
   */
  test("the rim of a region is the frame colour, not the hairline inside it", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    const [frame, hairline] = await page.evaluate(() => {
      const probe = document.createElement("div");
      document.body.append(probe);
      probe.style.color = "var(--frame)";
      const frame = getComputedStyle(probe).color;
      probe.style.color = "var(--border)";
      const hairline = getComputedStyle(probe).color;
      probe.remove();
      return [frame, hairline];
    });
    expect(frame).not.toBe(hairline);
    // A deck that is not playing: a playing one's rim says whether it is going
    // out or coming in, which is its own colour.
    for (const region of [".topbar", ".deck:not(.playing)", '.surface[data-surface="library"]']) {
      await expect(page.locator(region).first(), region).toHaveCSS("border-top-color", frame);
    }
    expect(errorsThrown(page)).toEqual([]);
  });
});
