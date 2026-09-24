/**
 * §108: going live — the record on a stream.
 *
 * > improve integration of social networks
 *
 * Which record is named, the overlay page and the file are Rust's, tested in
 * `dj_app::live` and `dj_net::overlay` over a real socket, and were driven in
 * the running application. What the browser holds is the switch: off until
 * pressed, and once on, the two ways into OBS shown — the overlay's address
 * and the file — with what the stream is being told.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

test.describe("§108: going live", () => {
  test("the switch tells the stream, and shows the two ways into OBS", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Settings", exact: true }).click();
    const block = page.locator("[data-live]");
    await block.scrollIntoViewIfNeeded();

    // Off on a fresh install, and saying nothing while off.
    const toggle = block.getByRole("checkbox", { name: "Tell the stream what is playing" });
    await expect(toggle).not.toBeChecked();
    await expect(block.locator("[data-overlay]")).toHaveCount(0);

    await toggle.check();
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __setLive?: boolean[] }).__setLive ?? []))
      .toEqual([true]);
    await expect(block.locator("[data-saying]")).toContainText("Aventura - Obsesión");
    await expect(block.locator("[data-overlay]")).toHaveValue("http://127.0.0.1:7332/");
    await expect(block.locator("[data-file]")).toHaveValue(/now-playing\.txt$/);

    await toggle.uncheck();
    await expect(block.locator("[data-overlay]")).toHaveCount(0);
    expect(await page.evaluate(() => (window as unknown as { __setLive?: boolean[] }).__setLive)).toEqual([
      true,
      false,
    ]);
    expect(errorsThrown(page)).toEqual([]);
  });
});
