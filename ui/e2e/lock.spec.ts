/**
 * §78's Freeze and §79's six locks — the professional safety valve, which had
 * never been connected to anything.
 *
 * > **Freeze Layout.** When enabled: no automatic rearrangement, no automatic
 * > surface resizing, no theme changes unless explicitly permitted, no surprise
 * > visual transitions. This is the professional safety valve.
 *
 * The field existed. `Workspace.frozen` was stored, serialised, round-tripped
 * through the resolver and read by *nothing at all* — which is worse than not
 * having it, because a DJ who believes the layout is pinned stops watching it.
 *
 * What each lock takes away is `cockpit::Lock::stops` and is tested there
 * against readings this container can produce honestly. These say the three
 * things only a browser can: that a lock the DJ has just set stops the change
 * it names, that it stops *only* that one, and that the switches reach the
 * workspace.
 *
 * # What is not here
 *
 * **The theme.** §31 asks the night what to wear on a twenty-second timer, so a
 * browser test of `permits.retheme` would be a twenty-second wait for one
 * assertion. The gate is in the same effect as the interval and the permit
 * itself is tested in Rust; what is not proven here is the wiring, and it is
 * the one of the four that is not.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** Deliver a snapshot with a phase and room to act on it. §17's positive case. */
async function nightReachesPeak(page: Page) {
  await page.evaluate(() => {
    const win = window as unknown as {
      __lastState?: {
        context: { session: Record<string, unknown> | null };
        attention: { reflow: boolean };
      };
      __emit?: (next: unknown) => void;
    };
    const state = win.__lastState;
    if (!state) throw new Error("the harness delivered no state to start from");
    win.__emit?.({
      ...state,
      attention: { ...state.attention, reflow: true },
      context: {
        ...state.context,
        session: {
          phase: "peak",
          energy: 0.8,
          environment: "club",
          certainty: "sure",
          basis: "measured",
          drift: null,
        },
      },
    });
  });
}

/** Open Settings and wait for §79's switches. */
async function openLocks(page: Page) {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.locator(".locks")).toBeVisible();
}

const lock = (page: Page, slug: string) =>
  page.locator(`.locks .lock-list li[data-lock="${slug}"] input`);

/** Every workspace djmanzo was asked to store, in order. */
async function saved(page: Page) {
  return page.evaluate(
    () => (window as unknown as { __saved?: { locked?: string[] }[] }).__saved ?? [],
  );
}

const ROOM = '.surface[data-surface="room"]';

test.describe("§78's freeze and §79's locks", () => {
  /**
   * **The load-bearing one: a frozen cockpit does not rearrange itself.**
   *
   * The comparison is `phase.spec.ts`'s own positive case — the same snapshot,
   * the same phase, the same room to think — where the room panel *does* open.
   * Without that pair this would pass against a shell that had simply stopped
   * promoting anything, which is how a lock comes to look like it works.
   */
  test("a frozen cockpit sits still through a phase that would have moved it", async ({
    page,
  }) => {
    await openShell(page, "/");
    await openLocks(page);
    await page.locator(".locks .freeze").click();
    await expect(page.locator(".locks .freeze")).toHaveAttribute("aria-pressed", "true");

    await nightReachesPeak(page);
    // Given a moment to do the wrong thing, then asserted: a count of zero that
    // passes because nothing has happened yet proves nothing.
    await page.waitForTimeout(400);
    await expect(
      page.locator(ROOM),
      "the night reached peak and the cockpit rearranged itself anyway -- §78 " +
        "calls this the professional safety valve, and a valve that does not " +
        "close is worse than none",
    ).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * And the same night, with nothing locked, moves — so the test above is
   * measuring the lock rather than a shell that stopped adapting.
   */
  test("the same night with nothing locked still opens what the phase asks for", async ({
    page,
  }) => {
    await openShell(page, "/");
    await nightReachesPeak(page);
    await expect(page.locator(ROOM)).toBeVisible();
  });

  /**
   * §78's second bullet: *no automatic surface resizing*.
   *
   * The density band is the only thing in djmanzo that resizes a surface
   * without being asked, and a DJ locks it for a specific reason — the booth
   * screen is the one they learned the layout on, and a window edge caught by a
   * sleeve should not re-scale the night.
   */
  test("a locked density stops following the window", async ({ page }) => {
    await openShell(page, "/");
    const scale = () =>
      page.evaluate(() =>
        getComputedStyle(document.documentElement).getPropertyValue("--density").trim(),
      );

    await page.setViewportSize({ width: 1280, height: 1200 });
    const roomy = await scale();
    await page.setViewportSize({ width: 1280, height: 1000 });
    const cramped = await scale();
    expect(
      cramped,
      "the density does not follow the window at all, so there is nothing here " +
        "for a lock to stop",
    ).not.toBe(roomy);

    await page.setViewportSize({ width: 1280, height: 1200 });
    await openLocks(page);
    await lock(page, "density").check();
    await expect(page.locator(".locks .freeze")).toHaveAttribute("aria-pressed", "false");

    const before = await scale();
    await page.setViewportSize({ width: 1280, height: 1000 });
    await page.waitForTimeout(200);
    expect(
      await scale(),
      "the interface resized itself under a DJ who had told it not to",
    ).toBe(before);
  });

  /**
   * §78's fourth bullet: *no surprise visual transitions*.
   *
   * These six custom properties are the whole of how djmanzo answers the audio
   * — every control reads them from static CSS — so locking the waveform is the
   * interface sitting still. At rest rather than held: freezing them where they
   * were would pin one random frame of the music onto a booth screen all night.
   */
  test("a locked waveform stops the interface answering the audio", async ({ page }) => {
    await openShell(page, "/");
    const loudness = () =>
      page.evaluate(() =>
        getComputedStyle(document.documentElement).getPropertyValue("--audio-loudness").trim(),
      );

    expect(
      Number(await loudness()),
      "nothing is answering the audio, so there is nothing here for a lock to " +
        "stop",
    ).toBeGreaterThan(0);

    await openLocks(page);
    await lock(page, "waveform").check();
    await page.waitForTimeout(200);
    expect(
      Number(await loudness()),
      "the controls kept pulsing under a DJ who had told them not to",
    ).toBe(0);
  });

  /**
   * One lock takes one thing away.
   *
   * §79's whole point is that the six are separate: a DJ who wants the palette
   * to stop moving has not asked for the browser to stop opening itself, and a
   * "lock the theme" that froze everything would be Freeze under a narrower
   * name — which a DJ would discover mid-set.
   */
  test("locking one thing leaves the others alone", async ({ page }) => {
    await openShell(page, "/");
    await openLocks(page);
    await lock(page, "theme").check();

    await nightReachesPeak(page);
    await expect(
      page.locator(ROOM),
      "locking the theme stopped the cockpit adapting as well",
    ).toBeVisible();
  });

  /**
   * The switches reach the workspace, and they reach it as themselves.
   *
   * A lock is a field of the workspace — stored with it, restored with it, and
   * carried by a preset a DJ shares — so what is proven here is that ticking
   * one writes that one slug and no others.
   */
  test("ticking a lock stores that lock and nothing else", async ({ page }) => {
    await openShell(page, "/");
    await openLocks(page);
    await lock(page, "arrangement").check();

    await expect
      .poll(async () => (await saved(page)).at(-1)?.locked)
      .toEqual(["arrangement"]);

    await lock(page, "theme").check();
    await expect
      .poll(async () => (await saved(page)).at(-1)?.locked?.slice().sort())
      .toEqual(["arrangement", "theme"]);

    await lock(page, "arrangement").uncheck();
    await expect.poll(async () => (await saved(page)).at(-1)?.locked).toEqual(["theme"]);
  });

  /** Freeze is every lock, and pressing it again is none. */
  test("freeze sets all six and pressing it again clears them", async ({ page }) => {
    await openShell(page, "/");
    await openLocks(page);

    const boxes = page.locator(".locks .lock-list input");
    await expect(boxes, "§79 names six things a DJ may lock").toHaveCount(6);
    await expect(page.locator(".locks .lock-list input:checked")).toHaveCount(0);

    await page.locator(".locks .freeze").click();
    await expect(
      page.locator(".locks .lock-list input:checked"),
      "§78's Freeze left something djmanzo could still change by itself",
    ).toHaveCount(6);

    await page.locator(".locks .freeze").click();
    await expect(page.locator(".locks .lock-list input:checked")).toHaveCount(0);
    await expect(page.locator(".locks .freeze")).toHaveAttribute("aria-pressed", "false");
  });
});
