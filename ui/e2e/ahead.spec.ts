/**
 * §116: seeing the record coming.
 *
 * > i'd love to also make the frequency and intensity visible .. maybe you can
 * > investigate as of typical instruments and arrange some kind of individual
 * > visualization so DJs can see instruments coming via the waveform ahead of
 * > time, via their shape or colorcoding .. .also use the bg of the display of
 * > a song control and waveform somehow to visualize rhythm and melody,
 * > intensity, amplitude, evolution of the song (rising, setting ...)
 *
 * Which currents come and go and where a record builds is
 * `dj_analysis::energy::Trajectory::changes`, tested there; the counting in
 * bars is `./src/ahead.ts`, tested beside it. What a browser can hold is that
 * a DJ sees it: the next change named in the lane with how far away it is,
 * the record's energy as ground under the waveform, and every arrival and
 * departure marked along the overview.
 */
import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

// The fixture's windows are 32 beats of 3 000 000 frames: a bar is 375 000.
const BAR = 375_000;

const lane = (page: Page) => page.locator('.deck[data-deck="1"] .lane');
const overview = (page: Page) => page.locator('.deck[data-deck="1"] .overview');

/** Send the snapshot again with deck 1's playhead somewhere else. */
async function playheadAt(page: Page, frames: number) {
  await page.evaluate((frames) => {
    const win = window as unknown as {
      __lastState?: { decks: Record<string, unknown>[] };
      __emit?: (next: unknown) => void;
    };
    const state = win.__lastState;
    if (!state) throw new Error("the harness delivered no state to start from");
    const decks = state.decks.map((deck, index) =>
      index === 0 ? { ...deck, position_frames: frames, playing: false } : deck,
    );
    win.__emit?.({ ...state, decks });
  }, frames);
}

test.describe("§116: seeing the record coming", () => {
  /**
   * **The load-bearing one: eight bars before the breakdown, the lane says
   * so, and counts down.** The DJ reads what is coming and how far away it
   * is without doing the arithmetic.
   */
  test("the lane names what is coming and how many bars away", async ({ page }) => {
    await openShell(page, "/");
    await playheadAt(page, 6_000_000 - 8 * BAR);
    const coming = lane(page).getByRole("list", { name: "Coming up on deck 1" });
    await expect(coming).toBeVisible();
    await expect(coming.locator("li").first()).toHaveText(/Breakdown\s*8 bars/);

    await playheadAt(page, 6_000_000 - 2 * BAR);
    await expect(coming.locator("li").first()).toHaveText(/Breakdown\s*2 bars/);

    // Past the breakdown: the drop, and the drums back with it. Two at most:
    // a readout of everything at once is one nobody reads between two cues.
    await playheadAt(page, 9_000_000 - 4 * BAR);
    await expect(coming.locator("li").first()).toHaveText(/Drop\s*4 bars/);
    await expect(coming.locator("li").nth(1)).toHaveText(/Drums in\s*4 bars/);
    await expect(coming.locator("li")).toHaveCount(2);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A layer the DJ switched off is not read out either. */
  test("the readout keeps to the layers the DJ has on", async ({ page }) => {
    await openShell(page, "/");
    await playheadAt(page, 6_000_000 - 8 * BAR);
    const coming = lane(page).getByRole("list", { name: "Coming up on deck 1" });
    await expect(coming.locator('[data-layer="stems"]')).toHaveCount(1);

    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await expect(page.locator(".remembers")).toBeVisible();
    await page.locator('.remembers [data-layer-row="stems"] input').uncheck();
    await expect(coming.locator('[data-layer="stems"]')).toHaveCount(0);
    await expect(coming.locator('[data-layer="breakdowns"]')).toHaveCount(1);
  });

  /**
   * **The ground under the waveform is the record's energy.** The busiest
   * window stands full height and the breakdown a third of it, so the step
   * down is on screen before the playhead is.
   */
  test("the ground under the lane rises and falls with the record", async ({ page }) => {
    await openShell(page, "/");
    await playheadAt(page, 6_000_000 - BAR / 2);
    const ground = lane(page).locator(".ground");
    await expect(ground).not.toHaveCount(0);
    const heights = await ground.evaluateAll((windows) =>
      windows.map((window) => (window as HTMLElement).style.height),
    );
    // The busiest window, and the breakdown after it.
    expect(heights).toContain("100%");
    expect(heights).toContain("35%");
    // Under the waveform, not over it: every ground window comes before the
    // first tile in the strip.
    const order = await lane(page).locator(".strip > *").evaluateAll((children) =>
      children.map((child) => child.getAttribute("class") ?? ""),
    );
    const lastGround = order.map((name) => name.includes("ground")).lastIndexOf(true);
    const firstTile = order.findIndex((name) => name.includes("tile"));
    expect(lastGround).toBeLessThan(firstTile);
  });

  /**
   * **The melody line is on the lane, placed by pitch and coloured by the
   * step Rust sent**, broken where nothing was pitched, and gone when the DJ
   * switches its layer off.
   */
  test("the lane draws the melody line, and the layer switch takes it away", async ({ page }) => {
    await openShell(page, "/");
    const line = lane(page).locator('svg.melody[data-layer="melody"]');
    await expect(line).toBeVisible();
    const notes = line.locator("path.note");
    await expect(notes).not.toHaveCount(0);
    // More than one path: the rests break the line, and so does each change
    // of colour.
    expect(await notes.count()).toBeGreaterThan(2);
    // A colour from the table sent, by the step sent — not one of the
    // theme's own tokens.
    const strokes = await notes.evaluateAll((paths) => paths.map((path) => path.getAttribute("stroke")));
    expect(strokes).toContain("rgb(100, 155, 128)");
    expect(strokes).toContain("rgb(107, 148, 128)");
    // The higher note sits higher: the step-107 run is above the step-100
    // one. Read at each run's last point — its first is where the run before
    // it ended, which is what keeps the line unbroken across a colour.
    const tops = await notes.evaluateAll((paths) =>
      paths.map((path) => {
        const numbers = (path.getAttribute("d") ?? "").trim().split(/[ ML]+/).filter(Boolean);
        return { stroke: path.getAttribute("stroke"), y: Number(numbers[numbers.length - 1]) };
      }),
    );
    const at = (stroke: string) => tops.filter((t) => t.stroke === stroke).map((t) => t.y);
    expect(Math.max(...at("rgb(107, 148, 128)"))).toBeLessThan(Math.min(...at("rgb(100, 155, 128)")));

    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await expect(page.locator(".remembers")).toBeVisible();
    await page.locator('.remembers [data-layer-row="melody"] input').uncheck();
    await expect(line).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The rhythm is under the waveform as a drum machine reads it**: the
   * kick every beat on the floor row, the hats between, the snare on two and
   * four — each in its band's colour — and gone with its layer switch.
   */
  test("the lane draws the rhythm, a row a voice, and the layer switch takes it away", async ({ page }) => {
    await openShell(page, "/");
    const strip = lane(page).locator('svg.rhythm[data-layer="rhythm"]');
    await expect(strip).toBeVisible();
    const xs = (voice: number) =>
      strip
        .locator(`rect.step[data-voice="${voice}"]`)
        .evaluateAll((rects) => rects.map((r) => Number(r.getAttribute("x")) + Number(r.getAttribute("width")) / 2));
    const kicks = await xs(0);
    const hats = await xs(2);
    const snares = await xs(1);
    expect(kicks.length).toBeGreaterThan(3);
    // A beat apart, evenly; a hat half-way between two kicks; a snare every
    // other beat.
    const beat = kicks[1] - kicks[0];
    expect(beat).toBeGreaterThan(8);
    for (let i = 2; i < kicks.length; i++) expect(kicks[i] - kicks[i - 1]).toBeCloseTo(beat, 3);
    const between = hats.find((x) => x > kicks[0] && x < kicks[1]);
    expect(between).toBeCloseTo((kicks[0] + kicks[1]) / 2, 3);
    expect(snares.length).toBeLessThan(kicks.length);
    expect(snares.length).toBeGreaterThan(0);
    // The floor row is the lows' colour.
    const fill = await strip.locator('rect.step[data-voice="0"]').first().evaluate((r) => getComputedStyle(r).fill);
    const low = await page.evaluate(() => {
      const probe = document.createElement("span");
      probe.style.color = "var(--band-low)";
      document.body.append(probe);
      const value = getComputedStyle(probe).color;
      probe.remove();
      return value;
    });
    expect(fill).toBe(low);

    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await expect(page.locator(".remembers")).toBeVisible();
    await page.locator('.remembers [data-layer-row="rhythm"] input').uncheck();
    await expect(strip).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Every arrival and departure is marked along the overview**, in the
   * colour of the fader that mutes that current, departures hatched.
   */
  test("the overview marks where each current comes and goes", async ({ page }) => {
    await openShell(page, "/");
    const marks = overview(page).locator(".arrival");
    await expect(marks).toHaveCount(4);
    await expect(overview(page).locator(".arrival.leaving")).toHaveCount(2);
    const placed = await marks.evaluateAll((all) =>
      all.map((mark) => ({
        key: mark.getAttribute("data-current"),
        left: (mark as HTMLElement).style.left,
        leaving: mark.classList.contains("leaving"),
      })),
    );
    expect(placed).toContainEqual({ key: "other", left: "50%", leaving: false });
    expect(placed).toContainEqual({ key: "drums", left: "50%", leaving: true });
    expect(placed).toContainEqual({ key: "drums", left: "75%", leaving: false });
    expect(placed).toContainEqual({ key: "vocal", left: "75%", leaving: true });
  });
});
