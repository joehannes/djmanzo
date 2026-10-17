/**
 * §34 / §36: the floor's other signals, through the browser's own capture
 * path.
 *
 * Chromium plays a drawn picture as its camera and a generated sound as its
 * microphone (`floor/media.ts`), and the room surface measures them exactly as
 * it would a real camera and microphone: the same frames light and movement
 * are read from, the same window of sound loudness is. Each test asserts that
 * one signal moves the way it should when the thing it measures does — the
 * bass in the low band, a room clapping and not a hi-hat in the clicks, a
 * pan moving together and scattered figures not, a lighting cut as a change of
 * scene — and that what crosses to Rust is numbers and nothing else.
 *
 * # What this cannot tell you
 *
 * Chromium is not the shipped webview, and drawn footage is not a floor. None
 * of these thresholds is a claim about a real room; they say the path runs
 * and which way each number moves.
 */
import { chromium, expect, test, type Browser, type Page } from "@playwright/test";

import { ensureMedia, footagePath, soundPath, type Footage, type Sound } from "./floor/media";
import { errorsThrown, openShell } from "./shell";

const ROOM = '.surface[data-surface="room"]';

interface Measured {
  low: number | null;
  mid: number | null;
  high: number | null;
  applause: number | null;
  uneven: number | null;
  together: number | null;
  scene: number | null;
  occupancy: number | null;
  density: number | null;
  lighting: number | null;
  heard: number | null;
}

test.describe.configure({ timeout: 120_000 });
test.beforeAll(() => ensureMedia());

async function launch(footage: Footage, sound: Sound): Promise<Browser> {
  return chromium.launch({
    executablePath: process.env.DJMANZO_CHROMIUM || undefined,
    args: [
      "--use-fake-ui-for-media-stream",
      "--use-fake-device-for-media-stream",
      `--use-file-for-fake-video-capture=${footagePath(footage)}`,
      `--use-file-for-fake-audio-capture=${soundPath(sound)}`,
    ],
  });
}

/**
 * Keep every `floor_saw` the page sends, as sent, and answer `floor_read`
 * with `read` when given. `shell.ts` is shared and answers neither, so this
 * test answers its own, as `deckqueue.spec.ts` does.
 */
async function rust(page: Page, read: unknown = null) {
  await page.evaluate((read) => {
    const win = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
      __floor: unknown[];
    };
    win.__floor = [];
    const stub = win.__TAURI_INTERNALS__.invoke;
    win.__TAURI_INTERNALS__.invoke = (cmd, args = {}) => {
      if (cmd === "floor_saw") {
        win.__floor.push(structuredClone(args.measured));
        return Promise.resolve(null);
      }
      if (cmd === "floor_read") return Promise.resolve(structuredClone(read));
      return stub(cmd, args);
    };
  }, read);
}

async function look(browser: Browser, every = 500): Promise<Page> {
  const context = await browser.newContext({ baseURL: test.info().project.use.baseURL });
  const page = await context.newPage();
  await openShell(page, "/", {}, { room_poll_ms: every });
  await rust(page);
  await page.locator('.mission [data-mission="room"]').click();
  await page.locator(ROOM).getByRole("button", { name: "Look at the room" }).click();
  return page;
}

async function measured(page: Page, count: number): Promise<Measured[]> {
  await page.waitForFunction(
    (n) => ((window as unknown as { __floor?: unknown[] }).__floor ?? []).length >= n,
    count,
    { timeout: 90_000 },
  );
  return page.evaluate(() => (window as unknown as { __floor: Measured[] }).__floor);
}

function median(values: (number | null)[]): number {
  const known = values.filter((v): v is number => typeof v === "number").sort((a, b) => a - b);
  expect(known.length, "a signal that never read anything").toBeGreaterThan(0);
  return known[Math.floor(known.length / 2)];
}

/** Every look of one picture and one sound. */
async function scene(footage: Footage, sound: Sound, count = 14): Promise<Measured[]> {
  const browser = await launch(footage, sound);
  try {
    return await measured(await look(browser), count);
  } finally {
    await browser.close();
  }
}

test.describe("§34 / §36: the floor's other signals", () => {
  /**
   * **The load-bearing one: each signal moves with what it measures, and the
   * others do not follow it.**
   *
   * Three rooms, each drawn to move a different signal. A measurement that
   * read the same thing under every name — loudness as every band, all
   * movement as both uneven and together — passes nothing here: the bass
   * must sit in the low band and the hats in the high one, the clapping must
   * score where the equally sharp hats do not, the pan must move together
   * where the scattered figures do not, and only the lighting cut changes the
   * scene.
   */
  test("each signal moves with what it measures, through the camera and microphone", async () => {
    const sway = await scene("sway", "claps");
    const scatter = await scene("scatter", "hats");
    const cut = await scene("cut", "bass");

    // Sound: the bands.
    expect(median(cut.map((m) => m.low)), "the bass is not in the low band").toBeGreaterThan(
      median(cut.map((m) => m.high)) + 0.2,
    );
    expect(median(scatter.map((m) => m.high)), "the hats are not in the high band").toBeGreaterThan(
      median(scatter.map((m) => m.low)) + 0.2,
    );

    // Sound: applause-like clicks — the clapping, not the equally sharp hats.
    const clapping = median(sway.map((m) => m.applause));
    const hats = median(scatter.map((m) => m.applause));
    expect(clapping, `claps ${clapping} against hats ${hats}`).toBeGreaterThan(0.25);
    expect(hats, "a hi-hat on the grid read as applause").toBeLessThan(0.1);
    expect(median(cut.map((m) => m.applause)), "a bass tone read as clicks").toBeLessThan(0.05);

    // Picture: moving together, and unevenly.
    const together = median(sway.map((m) => m.together));
    const apart = median(scatter.map((m) => m.together));
    expect(together, "a pan did not move together").toBeGreaterThan(0.8);
    expect(apart, "scattered figures moved together").toBeLessThan(0.6);
    expect(median(scatter.map((m) => m.uneven)), "scattered figures moved evenly").toBeGreaterThan(
      median(sway.map((m) => m.uneven)),
    );

    // Picture: the scene changing as a whole.
    const cutScene = Math.max(...cut.map((m) => m.scene ?? 0));
    const swayScene = Math.max(...sway.map((m) => m.scene ?? 0));
    expect(cutScene, "the lights snapping on was not a change of scene").toBeGreaterThan(0.5);
    expect(swayScene, "a pan over the same room read as a new scene").toBeLessThan(0.15);

    // And every number is a number on 0..1, with the microphone's loudness
    // sent alongside for Rust to pair with the music.
    for (const look of [...sway, ...scatter, ...cut]) {
      expect(typeof look.heard).toBe("number");
      for (const [key, value] of Object.entries(look)) {
        if (value === null) continue;
        expect(value, key).toBeGreaterThanOrEqual(0);
        expect(value, key).toBeLessThanOrEqual(1);
      }
    }
  });

  /**
   * **Occupancy, density and lighting each move with what they measure, and
   * not with one another.**
   *
   * Two figures in a large room against thirty milling in the middle of it:
   * more of the picture is occupied, and what is occupied packs tighter. A
   * beam sweeping a still room against the lights snapping on: the lighting
   * moves where the scene does not, and the scene changes where the lighting
   * does not move. A measurement that read movement or brightness under
   * either name passes neither half.
   */
  test("occupancy, density and lighting each move with what they measure", async () => {
    const few = (await scene("few", "bass", 16)).slice(6);
    const crowd = (await scene("crowd", "bass", 16)).slice(6);
    const beam = await scene("beam", "bass", 14);
    const cut = await scene("cut", "bass", 14);

    const fewOccupied = median(few.map((m) => m.occupancy));
    const crowdOccupied = median(crowd.map((m) => m.occupancy));
    expect(crowdOccupied, `crowd ${crowdOccupied} against two figures ${fewOccupied}`).toBeGreaterThan(
      fewOccupied * 3,
    );
    const fewPacked = median(few.map((m) => m.density));
    const crowdPacked = median(crowd.map((m) => m.density));
    expect(crowdPacked, `crowd ${crowdPacked} against two figures ${fewPacked}`).toBeGreaterThan(
      fewPacked + 0.25,
    );

    const sweeping = median(beam.map((m) => m.lighting));
    expect(sweeping, "a sweeping beam did not move the lighting").toBeGreaterThan(0.3);
    expect(Math.max(...beam.map((m) => m.scene ?? 0)), "a sweeping beam read as a new scene").toBeLessThan(
      0.15,
    );
    expect(Math.max(...cut.map((m) => m.scene ?? 0)), "the lights snapping on was not a new scene").toBeGreaterThan(
      0.5,
    );
    const still = cut.map((m) => m.lighting).filter((v): v is number => v !== null);
    expect(Math.max(0, ...still), "a still room's lights read as moving").toBeLessThan(0.1);
  });

  /**
   * **Each look hears its own moment.** An analyser smooths its spectrum
   * across reads by default, which at one read every look would carry the
   * last look's bass into this one's for many looks after the music changed:
   * four seconds of bass then four of hats must read as bass, then as none.
   */
  test("each look's bands are that look's, not an echo of the last", async () => {
    const seen = await scene("sway", "switch", 18);
    const lows = seen.map((m) => m.low ?? 0);
    expect(Math.max(...lows), "the bass half never read as low").toBeGreaterThan(0.5);
    expect(Math.min(...lows), "the low band never let go of the bass").toBeLessThan(0.15);
  });

  /**
   * **Nothing but numbers crosses to Rust.** The camera is the most sensitive
   * thing djmanzo touches; the room surface's rule is that no frame and no
   * sample leaves it, and the floor's signals keep it.
   */
  test("what crosses to Rust is eleven numbers and nothing else", async () => {
    const seen = await scene("scatter", "claps", 4);
    for (const look of seen) {
      expect(Object.keys(look).sort()).toEqual(
        [
          "applause",
          "density",
          "heard",
          "high",
          "lighting",
          "low",
          "mid",
          "occupancy",
          "scene",
          "together",
          "uneven",
        ].sort(),
      );
      expect(JSON.stringify(look).length, "a look carried more than numbers").toBeLessThan(400);
    }
  });
});

test.describe("§34 / §36: the floor's lines", () => {
  const LINES = {
    watching: true,
    lines: [
      {
        signal: "low",
        title: "Low band",
        now: 0.6,
        against: "usual",
        notable: false,
        confidence: "settled",
        looks: 90,
        says: "Low band: about as usual tonight (90 looks; mostly the bass from the speakers, whatever the room is doing)",
        measures: "the microphone's level below 250 Hz",
      },
      {
        signal: "applause",
        title: "Applause-like clicks",
        now: 0.7,
        against: "highest",
        notable: true,
        confidence: "early",
        looks: 40,
        says: "Applause-like clicks: higher than it has been all night (40 looks, early in the night; a fast, loose percussion part can look the same)",
        measures: "sharp clicks in the sound that fall on no regular beat",
      },
      {
        signal: "over_music",
        title: "Room over the music",
        now: null,
        against: null,
        notable: false,
        confidence: "not_yet",
        looks: 12,
        says: "Room over the music: not yet — it takes two minutes of looking alongside the music",
        measures: "how much louder the microphone is than the music's own level explains",
      },
    ],
  };

  /**
   * **Each signal on its own line, in Rust's words, with its own confidence —
   * and no line that adds them up.**
   */
  test("each signal is drawn on its own line with its own confidence, and nothing sums them", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await rust(page, LINES);
    await page.locator('.mission [data-mission="room"]').click();
    const room = page.locator(ROOM);
    // The surface reads on its own poll; the first read after mounting is the
    // one this answers.
    const list = room.locator(".floor li");
    await expect(list).toHaveCount(3, { timeout: 10_000 });
    for (const [i, line] of LINES.lines.entries()) {
      const item = list.nth(i);
      await expect(item).toHaveText(line.says);
      await expect(item).toHaveAttribute("data-confidence", line.confidence);
      await expect(item).toHaveAttribute("title", `Measured as ${line.measures}`);
    }
    await expect(list.nth(1)).toHaveClass(/notable/);
    await expect(list.nth(2)).toHaveClass(/waiting/);
    const words = (await room.locator(".floor").innerText()).toLowerCase();
    for (const forbidden of ["score", "mood", "crowd is", "%"]) {
      expect(words, `the floor's lines say "${forbidden}"`).not.toContain(forbidden);
    }
    expect(thrown).toEqual([]);
  });

  test("nothing is drawn while nobody is looking", async ({ page }) => {
    await openShell(page, "/");
    await rust(page, { watching: false, lines: [] });
    await page.locator('.mission [data-mission="room"]').click();
    await expect(page.locator(ROOM)).toBeVisible();
    await page.waitForTimeout(3500);
    await expect(page.locator(`${ROOM} .floor`)).toHaveCount(0);
  });
});
