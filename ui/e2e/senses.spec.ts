/**
 * §106: the room surface and the hum, through the browser's own capture path.
 *
 * > for synthetic material, use for example fitting party videos as club
 * > camera input ... live festival music as microphone input ... improvise for
 * > lacking hardware and try the best to deliver bug free so later when
 * > testing with real hardware there'll be no/minimal issues
 *
 * Chromium plays a file as its camera and another as its microphone, through
 * the same `getUserMedia` path a real device takes, so everything between the
 * button and `room_saw` runs here: the constraints, the fallback when a device
 * is missing, the canvas readback, the analyser, and the arithmetic that turns
 * a frame and a window of sound into three numbers. The files are drawn by
 * `senses/media.ts` — what they are, and why they are not downloaded, is
 * written there.
 *
 * # What these tests found, before they were tests
 *
 * The first run of these files through the surface as it stood read a beach
 * party of seventeen dancers at 120 BPM as **perfectly still**, the same club
 * as half as busy when looked at every eight seconds as every two, and a quiet
 * room five decibels quieter than it was. Each assertion below is one of those
 * made to stay fixed.
 *
 * # What they cannot tell you
 *
 * Chromium is not the webview djmanzo ships in, and a drawn club is not a
 * club. The shipped webview's half — whether it opens a device at all — is
 * `dj_app::senses`, tested there and driven under Xvfb with WebKitGTK's mock
 * devices. Nothing here says what a real webcam makes of a real floor.
 */
import { chromium, expect, test, type Browser, type Page } from "@playwright/test";

import {
  ensureMedia,
  expectedLoudness,
  expectedRms,
  footagePath,
  soundPath,
  type Footage,
  type Sound,
} from "./senses/media";
import { errorsThrown, openShell } from "./shell";

const ROOM = '.surface[data-surface="room"]';

interface Reading {
  light: number | null;
  movement: number | null;
  loudness: number | null;
}

// Each test launches its own browser with its own files, and the slowest
// looks at the room every eight seconds.
test.describe.configure({ timeout: 120_000 });
test.beforeAll(() => ensureMedia());

/** A Chromium whose camera is `footage` and whose microphone is `sound`. */
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
 * Open djmanzo in `browser` and press *Look at the room*.
 *
 * `every` is the period the surface is told to look at, which in the
 * application is §48's tier. With `noCamera`, a request that includes video
 * fails the way it does on a machine with no camera; every stream the page is
 * handed is kept, so a test can ask whether it was let go.
 */
async function look(
  browser: Browser,
  every: number,
  { noCamera = false }: { noCamera?: boolean } = {},
): Promise<Page> {
  const context = await browser.newContext({ baseURL: test.info().project.use.baseURL });
  const page = await context.newPage();
  await page.addInitScript((noCamera: boolean) => {
    const devices = navigator.mediaDevices;
    const real = devices.getUserMedia.bind(devices);
    const win = window as unknown as { __streams: MediaStream[] };
    win.__streams = [];
    devices.getUserMedia = async (asked?: MediaStreamConstraints) => {
      if (noCamera && asked?.video) {
        throw new DOMException("Requested device not found", "NotFoundError");
      }
      const stream = await real(asked);
      win.__streams.push(stream);
      return stream;
    };
  }, noCamera);
  await openShell(page, "/", {}, { room_poll_ms: every });
  await page.locator('.mission [data-mission="room"]').click();
  await page.locator(ROOM).getByRole("button", { name: "Look at the room" }).click();
  return page;
}

/** Wait for `count` readings and return them all. */
async function readings(page: Page, count: number): Promise<Reading[]> {
  await page.waitForFunction(
    (n) => ((window as unknown as { __saw?: unknown[] }).__saw ?? []).length >= n,
    count,
    { timeout: 90_000 },
  );
  return page.evaluate(() => (window as unknown as { __saw: Reading[] }).__saw);
}

function median(values: (number | null)[]): number {
  const known = values.filter((v): v is number => typeof v === "number").sort((a, b) => a - b);
  expect(known.length, "a sense that never read anything").toBeGreaterThan(0);
  return known[Math.floor(known.length / 2)];
}

/** The median of each sense over `count` readings of one scene. */
async function scene(footage: Footage, sound: Sound, every: number, count: number) {
  const browser = await launch(footage, sound);
  try {
    const seen = await readings(await look(browser, every), count);
    return {
      light: median(seen.map((r) => r.light)),
      movement: median(seen.map((r) => r.movement)),
      loudness: median(seen.map((r) => r.loudness)),
      seen,
    };
  } finally {
    await browser.close();
  }
}

test.describe("the room, through a camera and a microphone", () => {
  /**
   * **All three senses arrive, as numbers, with nothing to apologise for.**
   *
   * The whole path, once: two devices opened in one prompt, a picture drawn
   * and read back, a window of sound read, three numbers in 0..1 handed to
   * Rust. Before this ran, none of it had.
   */
  test("the camera and the microphone reach djmanzo", async () => {
    const browser = await launch("peak", "peak");
    try {
      const page = await look(browser, 500);
      const seen = await readings(page, 6);
      for (const reading of seen) {
        for (const sense of ["light", "movement", "loudness"] as const) {
          const value = reading[sense];
          expect(typeof value, `${sense} in ${JSON.stringify(reading)}`).toBe("number");
          expect(value).toBeGreaterThanOrEqual(0);
          expect(value).toBeLessThanOrEqual(1);
        }
      }
      await expect(page.locator(ROOM).locator(".error")).toHaveCount(0);
      await expect(page.locator(ROOM).locator(".sources")).toHaveText("camera · microphone");
      expect(errorsThrown(page)).toEqual([]);
    } finally {
      await browser.close();
    }
  });

  /**
   * **The load-bearing one for the microphone: loudness is the room's own
   * level.**
   *
   * Each file's RMS, read back from the file, is what the surface should
   * report. Two things stood between them: a browser microphone left in its
   * call-tuned defaults — gain control and noise suppression — which read a
   * room of chatter at -42 dBFS as 0.21 instead of 0.30; and a 43 ms window,
   * which read the same loud mix anywhere from 0.65 to 0.80 depending on
   * where in the beat each look landed.
   *
   * Both halves are held: the median, which is what §35 compares, and the
   * spread of single readings, which is what the panel shows a DJ. A room
   * whose level is not changing must not read as a number jumping with the
   * kick drum — with the timing now moved around the beat, a 43 ms window
   * still gets the median right and fails this.
   */
  test("loudness is the room's own level, loud or quiet", async () => {
    const [quiet, loud] = await Promise.all([
      scene("empty", "quiet", 500, 6),
      scene("peak", "peak", 500, 6),
    ]);
    expect(Math.abs(quiet.loudness - expectedLoudness("quiet"))).toBeLessThan(0.03);
    expect(Math.abs(loud.loudness - expectedLoudness("peak"))).toBeLessThan(0.03);
    for (const room of [quiet, loud]) {
      const levels = room.seen.map((r) => r.loudness).filter((v): v is number => v !== null);
      const spread = Math.max(...levels) - Math.min(...levels);
      expect(spread, `a steady room read ${levels.map((v) => v.toFixed(3)).join(" ")}`).toBeLessThan(0.05);
    }
  });

  /**
   * **The load-bearing one for the camera: a floor dancing on the beat reads
   * as moving.**
   *
   * Seventeen people bouncing at 120 BPM in daylight, looked at every two
   * seconds — the period djmanzo uses on a healthy machine. Two seconds is
   * exactly four beats, and when movement was the difference between one
   * look's frame and the last one's, every look caught the floor in the same
   * pose and it read 0.000. Movement is now two frames a sixth of a second
   * apart, at a moment moved randomly within each period.
   */
  test("a floor dancing at 120 BPM does not read as still", async () => {
    const [dancing, empty] = await Promise.all([
      scene("daylight", "peak", 2000, 6),
      scene("empty", "quiet", 2000, 6),
    ]);
    expect(dancing.movement).toBeGreaterThan(0.03);
    expect(dancing.movement).toBeGreaterThan(empty.movement * 3);
  });

  /**
   * **The same floor reads the same however often it is looked at.**
   *
   * §48 slows the room to one look every eight seconds on a struggling
   * machine, and a night can cross that line and back. When movement spanned
   * the period, the club read 0.24 at two seconds and 0.10 at eight — so the
   * night's own baseline, which §35 compares every reading with, was built
   * from two different measurements.
   *
   * A guard on the design rather than proof against the old one: movement no
   * longer takes the period as an input, and this holds that it stays so. On
   * one mutation run the old algorithm passed it too — on footage that loops
   * every eight seconds, what it reads at eight seconds depends on where the
   * loop and the timer happen to meet. The test that does catch the old
   * algorithm every time is the one above.
   */
  test("movement does not depend on how often the room is looked at", async () => {
    const [often, seldom] = await Promise.all([
      scene("peak", "peak", 2000, 6),
      scene("peak", "peak", 8000, 4),
    ]);
    const ratio = seldom.movement / often.movement;
    expect(ratio, `${seldom.movement} every 8 s against ${often.movement} every 2 s`).toBeGreaterThan(0.6);
    expect(ratio).toBeLessThan(1.6);
  });

  /**
   * **The readings move the way the rooms differ.**
   *
   * Not a calibration — a drawn club says nothing about a real one — but the
   * direction has to be right: daylight is brighter than a club, a club is
   * brighter than a room with one light left on, and a room nobody is in
   * moves far less than a floor at its peak.
   */
  test("daylight is brighter, and an emptied room is stiller", async () => {
    const [peak, empty, daylight] = await Promise.all([
      scene("peak", "peak", 500, 6),
      scene("empty", "quiet", 500, 6),
      scene("daylight", "peak", 500, 6),
    ]);
    expect(daylight.light).toBeGreaterThan(0.5);
    expect(peak.light).toBeLessThan(0.25);
    expect(empty.light).toBeLessThan(peak.light);
    expect(empty.movement).toBeLessThan(peak.movement / 4);
  });

  /**
   * **A machine with a microphone and no camera still hears the room.**
   *
   * The surface asked for both, then for the camera alone, and gave up — so a
   * booth whose only input was a microphone read nothing and was told there
   * was "no camera or microphone on this machine".
   */
  test("a microphone with no camera still reads loudness", async () => {
    const browser = await launch("peak", "peak");
    try {
      const page = await look(browser, 500, { noCamera: true });
      const seen = await readings(page, 4);
      for (const reading of seen) {
        expect(typeof reading.loudness).toBe("number");
        expect(reading.light).toBeNull();
        expect(reading.movement).toBeNull();
      }
      await expect(page.locator(ROOM).locator(".error")).toHaveCount(0);
      await expect(page.locator(ROOM).locator(".sources")).toHaveText("no camera · microphone");
    } finally {
      await browser.close();
    }
  });

  /**
   * **Stopping lets the devices go, and so does closing the surface.**
   *
   * A camera left open is a light on a webcam in a dark booth, and a
   * microphone left open is a microphone. Both ways out are held: the button,
   * and closing the room surface while it is still looking.
   */
  test("stopping, or closing the surface, releases the camera and microphone", async () => {
    const browser = await launch("peak", "peak");
    try {
      const page = await look(browser, 500);
      await readings(page, 2);
      await page.locator(ROOM).getByRole("button", { name: "Stop looking" }).click();
      const ended = () =>
        page.evaluate(() =>
          (window as unknown as { __streams: MediaStream[] }).__streams.flatMap((s) =>
            s.getTracks().map((t) => t.readyState),
          ),
        );
      expect(await ended()).not.toContain("live");

      await page.locator(ROOM).getByRole("button", { name: "Look at the room" }).click();
      await readings(page, 4);
      expect(await ended()).toContain("live");
      await page.getByRole("button", { name: "Close Room" }).click();
      await expect(page.locator(ROOM)).toHaveCount(0);
      await expect.poll(ended).not.toContain("live");
    } finally {
      await browser.close();
    }
  });
});

test.describe("a hum, through a microphone", () => {
  /**
   * **The hum reaches djmanzo whole, at the rate it says, at its own level.**
   *
   * Eight seconds of a hummed tune from the fake microphone, through Memory's
   * capture — resampled by the browser to the rate the hum is read at — and
   * handed to Rust. What is checked is what Rust would be given: the length
   * matches the rate, and the level is the file's — the tune arrives neither
   * silent nor mangled. (The browser's call-tuned defaults stay on for the
   * hum: tried off, the level was the same, so turning them off fixed
   * nothing.)
   */
  test("a hum arrives as eight seconds at its own level", async () => {
    const browser = await launch("empty", "hum");
    try {
      const context = await browser.newContext({ baseURL: test.info().project.use.baseURL });
      const page = await context.newPage();
      await openShell(page, "/");
      await page.getByRole("button", { name: "Browse", exact: true }).click();
      await page.getByRole("button", { name: "From memory" }).click();
      await page.getByRole("button", { name: "Hum it" }).click();
      await page.waitForFunction(() => (window as unknown as { __hummed?: unknown }).__hummed, null, {
        timeout: 20_000,
      });
      const hummed = await page.evaluate(
        () => (window as unknown as { __hummed: { count: number; rate: number; rms: number } }).__hummed,
      );
      expect(hummed.rate).toBe(22_050);
      const seconds = hummed.count / hummed.rate;
      expect(seconds).toBeGreaterThan(7.5);
      expect(seconds).toBeLessThan(8.6);
      const want = expectedRms("hum");
      expect(hummed.rms / want, `hummed at ${hummed.rms}, file is ${want}`).toBeGreaterThan(0.8);
      expect(hummed.rms / want).toBeLessThan(1.25);
    } finally {
      await browser.close();
    }
  });
});
