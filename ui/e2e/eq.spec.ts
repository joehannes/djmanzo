/**
 * The deck's EQ knobs turn; they do not switch.
 *
 * §120, the owner: *"the frequency filters (hi low mid) are not to be
 * controlled in (fine) steps, but fully on/off only it seems"*.
 *
 * Each knob sat in a `<label>` with its kill button, and a click anywhere in
 * a label is also a click on its first control — the kill button. Every drag
 * ends with the click that follows the pointer's release, so every drag was
 * followed by a kill, or by putting a killed band back to unity: the knob
 * appeared to go only fully off or fully on. The filter knob's label held no
 * button, which is why it turned.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const dispatched = (page: Page) =>
  page.evaluate(() => (window as unknown as { __dispatched?: string[] }).__dispatched ?? []);

for (const band of ["eq_high", "eq_mid", "eq_low"]) {
  test(`a drag on ${band} sends only where the knob went`, async ({ page }) => {
    await openShell(page, "/");
    const knob = page.locator(`[data-band="${band}"] [role="slider"]`).first();
    await expect(knob).toBeVisible();
    const box = await knob.boundingBox();
    if (!box) throw new Error(`the ${band} knob is not drawn`);
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;

    const before = (await dispatched(page)).length;
    await page.mouse.move(x, y);
    await page.mouse.down();
    // Ten pixels down: a quarter of the way from unity to the kill on a
    // 0..4 knob that travels its whole range in a hundred.
    await page.mouse.move(x, y + 5, { steps: 3 });
    await page.mouse.move(x, y + 10, { steps: 3 });
    await page.mouse.up();
    // Whatever a stray click would send arrives on the same tick.
    await page.waitForTimeout(200);

    const sent = (await dispatched(page)).slice(before);
    const values = sent
      .filter((action) => action.startsWith(`deck 1 ${band} `))
      .map((action) => Number(action.split(" ")[3]));
    expect(values.length, `nothing was sent for ${band}: ${sent.join(" | ")}`).toBeGreaterThan(0);
    // Every value on the way, and the last one above all, is near where the
    // hand is — never the kill (0) or a snap back to unity after it.
    expect(values.every((v) => v > 0.4 && v < 1), sent.join(" | ")).toBe(true);
    expect(values[values.length - 1]).toBeCloseTo(0.6, 1);
    expect(errorsThrown(page)).toEqual([]);
  });
}

/** The kill buttons still kill, on their own press. */
test("a band's kill button still kills it", async ({ page }) => {
  await openShell(page, "/");
  const before = (await dispatched(page)).length;
  await page.getByRole("button", { name: "Kill LOW" }).first().click();
  expect((await dispatched(page)).slice(before)).toEqual(["deck 1 eq_low 0"]);
});
