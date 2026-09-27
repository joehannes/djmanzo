/**
 * §114: a sampler slot draws the shape of what is in it.
 *
 * > i'd also like to see the forms and shapes of the controls and widgets to
 * > be individual and useful and resembling nature and functionality
 *
 * The shape is `dj_app::outline`'s — here a synthetic kick, answered from the
 * Rust golden `sample-outline.json` — and what a browser holds is that it
 * reaches the slot: drawn as bars, loudest at the kick's start, filled as far
 * as the sample has played, and absent for an empty slot.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Deliver a frame with the sampler's first slot as given. */
async function slot(page: Page, change: Record<string, unknown>) {
  await page.evaluate((change) => {
    const win = window as unknown as {
      __lastState: { master: { sampler: { slots: Record<string, unknown>[] } } };
      __emit: (next: unknown) => void;
    };
    const next = structuredClone(win.__lastState);
    next.master.sampler.slots[0] = { ...next.master.sampler.slots[0], ...change };
    win.__emit(next);
  }, change);
}

/** Each bar's height, in the order drawn. */
const heights = (d: string) => [...d.matchAll(/v ([\d.]+)/g)].map((m) => Number(m[1]));

test("a sampler slot draws its sample's shape, filled as far as it has played", async ({ page }) => {
  const thrown = errorsThrown(page);
  await openShell(page, "/");
  await page.getByRole("button", { name: "Sampler", exact: true }).click();
  const surface = page.locator('.surface[data-surface="sampler"]');
  await expect(surface).toBeVisible();
  await expect(surface.locator("svg.shape")).toHaveCount(0);

  await slot(page, { loaded: true, name: "kick", progress: 0 });
  const shape = surface.locator("svg.shape").first();
  await expect(shape).toHaveAttribute("data-steps", "48");
  const rest = (await shape.locator("path.rest").getAttribute("d")) ?? "";
  const bars = heights(rest);
  expect(bars).toHaveLength(48);
  expect(bars[0], "a kick is loudest at its start").toBeGreaterThan(bars[47] * 5);
  expect((await shape.locator("path.played").getAttribute("d")) ?? "").toBe("");

  await slot(page, { loaded: true, name: "kick", progress: 0.5, playing: true });
  await expect.poll(async () => heights((await shape.locator("path.played").getAttribute("d")) ?? "").length).toBe(24);

  await slot(page, { loaded: false, name: null, progress: 0, playing: false });
  await expect(surface.locator("svg.shape")).toHaveCount(0);
  expect(thrown).toEqual([]);
});
