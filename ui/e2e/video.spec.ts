/**
 * §122: the picture of a video on a deck — the Video panel.
 *
 * A real video, recorded in the browser before the tests (a VP8 WebM, as a
 * browser's own recorder writes it), is what the panel's `<video>` elements
 * are given: the shell answers `deck_pictures` with an address on this
 * computer, as Rust does, and that address is answered here with the file, in
 * ranges, as Rust's server answers it.
 *
 * What these hold: a picture keeps to its deck — moved where the deck is,
 * playing at its rate, still when it is paused; the pictures crossfade as the
 * sound does and a picture not heard is not decoded; one deck can be pinned;
 * no video and a picture that will not play are each said.
 *
 * The server that serves the file in djmanzo, `dj_app::video`, is held by its
 * own tests; the webview djmanzo ships in, by driving it.
 */
import { chromium, expect, test, type Browser, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

test.describe.configure({ timeout: 90_000 });

/** Where Rust's server would be: never reached, every request answered here. */
const PICTURES = "http://127.0.0.1:47391/key";
const RECORD_ONE = "a".repeat(64);
const RECORD_TWO = "b".repeat(64);

let clip: Buffer;

test.beforeAll(async () => {
  const browser = await chromium.launch({ executablePath: process.env.DJMANZO_CHROMIUM || undefined });
  try {
    const page = await browser.newPage();
    await page.setContent("<canvas width=320 height=180></canvas>");
    const b64 = await page.evaluate(async () => {
      const canvas = document.querySelector("canvas")!;
      const g = canvas.getContext("2d")!;
      const recorder = new MediaRecorder(canvas.captureStream(30), { mimeType: "video/webm;codecs=vp8" });
      const chunks: Blob[] = [];
      recorder.ondataavailable = (e) => chunks.push(e.data);
      const start = performance.now();
      let frame = 0;
      const draw = () => {
        const t = (performance.now() - start) / 1000;
        g.fillStyle = `hsl(${(t * 90) % 360} 70% 40%)`;
        g.fillRect(0, 0, 320, 180);
        frame = requestAnimationFrame(draw);
      };
      draw();
      recorder.start(250);
      await new Promise((r) => setTimeout(r, 4000));
      recorder.stop();
      await new Promise((r) => (recorder.onstop = r));
      cancelAnimationFrame(frame);
      const bytes = new Uint8Array(await new Blob(chunks).arrayBuffer());
      let s = "";
      for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      return btoa(s);
    });
    clip = withDuration(Buffer.from(b64, "base64"), 4000);
  } finally {
    await browser.close();
  }
});

/**
 * `webm` with its length written in.
 *
 * A browser's recorder writes no duration — it records a stream whose end it
 * does not know — and a media element will not seek in a stream with no end.
 * A real music video has one. So it is added to the file's Info element,
 * here, in milliseconds (the recorder's timestamp scale).
 */
function withDuration(webm: Buffer, milliseconds: number): Buffer {
  const INFO = Buffer.from([0x15, 0x49, 0xa9, 0x66]);
  const at = webm.indexOf(INFO);
  if (at < 0) throw new Error("no Info element in the recording");
  // The size after the id: a variable-length integer.
  const first = webm[at + 4];
  let width = 1;
  while (width <= 8 && !(first & (0x80 >> (width - 1)))) width += 1;
  let size = first & (0xff >> width);
  for (let i = 1; i < width; i += 1) size = size * 256 + webm[at + 4 + i];
  const body = webm.subarray(at + 4 + width, at + 4 + width + size);
  const duration = Buffer.alloc(8);
  duration.writeDoubleBE(milliseconds);
  const element = Buffer.concat([Buffer.from([0x44, 0x89, 0x88]), duration]);
  const grown = Buffer.concat([body, element]);
  const length = Buffer.alloc(8);
  length.writeBigUInt64BE(BigInt(grown.length));
  length[0] = 0x01;
  return Buffer.concat([webm.subarray(0, at), INFO, length, grown, webm.subarray(at + 4 + width + size)]);
}

/** Answer a request for the picture as djmanzo's scheme does: in ranges. */
function inRanges(body: Buffer, range: string | undefined) {
  const asked = /^bytes=(\d+)-(\d*)$/.exec(range ?? "bytes=0-");
  const first = asked ? Number(asked[1]) : 0;
  const last = asked && asked[2] ? Math.min(Number(asked[2]), body.length - 1) : body.length - 1;
  return {
    status: 206,
    headers: {
      "Content-Type": "video/webm",
      "Accept-Ranges": "bytes",
      "Content-Range": `bytes ${first}-${last}/${body.length}`,
    },
    body: body.subarray(first, last + 1),
  };
}

async function launch(): Promise<Browser> {
  return chromium.launch({ executablePath: process.env.DJMANZO_CHROMIUM || undefined });
}

type Shown = {
  deck: number;
  opacity: number;
  paused: boolean;
  ended: boolean;
  time: number;
  rate: number;
  width: number;
};

/** Each picture element: how strongly it is drawn, and where it is. */
const shown = (page: Page) =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLVideoElement>("[data-video] video")].map((v) => ({
      deck: Number(v.dataset.deck),
      opacity: Number(v.style.opacity || "0"),
      paused: v.paused,
      ended: v.ended,
      time: v.currentTime,
      rate: v.playbackRate,
      width: v.videoWidth,
    })),
  ) as Promise<Shown[]>;

const whole = (page: Page) =>
  page.evaluate(() =>
    Number((document.querySelector("[data-stage]") as HTMLElement).style.getPropertyValue("--whole")),
  );

/** The Video panel in a window of its own, with `pictures` on the decks. */
async function openVideo(
  browser: Browser,
  pictures: { deck: number; record: string; kind: string }[],
  body: Buffer | null = null,
): Promise<Page> {
  const context = await browser.newContext({ baseURL: test.info().project.use.baseURL });
  await context.route(`${PICTURES}/**`, (route) =>
    body === null
      ? route.fulfill(inRanges(clip, route.request().headers()["range"]))
      : route.fulfill({ status: 200, contentType: "video/mp4", body }),
  );
  const page = await context.newPage();
  await openShell(page, "/?panel=video", {}, {
    deck_pictures: pictures.map((p) => ({ ...p, url: `${PICTURES}/${p.deck}/${p.record}` })),
  });
  await expect(page.locator("[data-video]")).toBeVisible();
  return page;
}

type Patch = {
  decks?: Record<number, Record<string, unknown>>;
  master?: Record<string, unknown>;
};

/** Deliver another snapshot, the last one with `patch` laid over it. */
async function emit(page: Page, patch: Patch) {
  await page.evaluate((patch: Patch) => {
    const w = window as unknown as { __lastState: unknown; __emit: (s: unknown) => void };
    const next = structuredClone(w.__lastState) as {
      decks: Record<string, unknown>[];
      master: Record<string, unknown>;
    };
    for (const [index, fields] of Object.entries(patch.decks ?? {})) Object.assign(next.decks[Number(index)], fields);
    Object.assign(next.master, patch.master ?? {});
    w.__emit(next);
  }, patch);
}

test.describe("§122: the Video panel", () => {
  /**
   * **A picture keeps to its deck**: moved to where the deck is, played at
   * the deck's rate; not seen while the deck is paused, and held still at the
   * deck's frame when pinned.
   */
  test("a picture keeps to its deck", async () => {
    const browser = await launch();
    try {
      const page = await openVideo(browser, [{ deck: 1, record: RECORD_ONE, kind: "video/webm" }]);
      await expect.poll(async () => (await shown(page))[0]?.width ?? 0, { timeout: 15_000 }).toBeGreaterThan(0);

      // The fixture's deck 1 is playing, on the left, the crossfader in the
      // middle and deck 2 silent: deck 1 is all that is heard.
      await emit(page, { decks: { 0: { position_seconds: 2 } } });
      await expect
        .poll(async () => {
          const [one] = await shown(page);
          return !one.paused && Math.abs(one.time - 2) < 0.4 && one.opacity === 1;
        }, { timeout: 10_000 })
        .toBe(true);
      expect(await whole(page)).toBe(1);

      await emit(page, { decks: { 0: { rate: 1.5, position_seconds: 2.2 } } });
      await expect.poll(async () => (await shown(page))[0].rate).toBe(1.5);

      // Paused, it is not heard, so not seen: a cued record's frame never
      // reaches the projector, and is not decoded either.
      await emit(page, { decks: { 0: { playing: false, position_seconds: 1 } } });
      await expect
        .poll(async () => {
          const [one] = await shown(page);
          return [one.paused, one.opacity];
        })
        .toEqual([true, 0]);
      expect(await whole(page)).toBe(0);

      // Pinned, it shows whatever the faders say — still, at the deck's frame.
      await page.getByRole("combobox", { name: "Which picture" }).selectOption("1");
      await expect
        .poll(async () => {
          const [one] = await shown(page);
          return one.paused && one.opacity === 1 && Math.abs(one.time - 1) < 0.05;
        }, { timeout: 10_000 })
        .toBe(true);
      expect(errorsThrown(page)).toEqual([]);
    } finally {
      await browser.close();
    }
  });

  /**
   * **The pictures crossfade as the sound does**: half and half in the
   * middle, one alone at the end, and the one not heard is not playing; a
   * picture fades to black as a record without one takes over; pinned, one
   * deck's picture shows whatever the faders say.
   */
  test("the pictures crossfade as the sound does", async () => {
    const browser = await launch();
    try {
      const page = await openVideo(browser, [
        { deck: 1, record: RECORD_ONE, kind: "video/webm" },
        { deck: 2, record: RECORD_TWO, kind: "video/webm" },
      ]);
      await expect
        .poll(async () => (await shown(page)).every((v) => v.width > 0), { timeout: 15_000 })
        .toBe(true);

      // Both playing, the crossfader in the middle: half and half.
      await emit(page, { decks: { 1: { playing: true, position_seconds: 1 } }, master: { crossfader: 0 } });
      await expect
        .poll(async () =>
          (await shown(page))
            .map((v) => v.opacity)
            .sort()
            .join(","),
        )
        .toBe("0.5,1");

      // Over to the right: deck 2 alone, deck 1 not seen and not playing.
      await emit(page, { decks: { 1: { playing: true } }, master: { crossfader: 1 } });
      // Paused by the panel, not merely run out: the clip is four seconds.
      await expect
        .poll(async () => {
          const [one, two] = await shown(page);
          return [one.opacity, one.paused, one.ended, two.opacity];
        })
        .toEqual([0, true, false, 1]);

      // Pinned to deck 1: shown alone, whatever the faders say.
      await page.getByRole("combobox", { name: "Which picture" }).selectOption("1");
      await expect.poll(async () => (await shown(page)).map((v) => v.opacity)).toEqual([1, 0]);
      await page.getByRole("combobox", { name: "Which picture" }).selectOption("");
      await expect.poll(async () => (await shown(page)).map((v) => v.opacity)).toEqual([0, 1]);
      expect(errorsThrown(page)).toEqual([]);
    } finally {
      await browser.close();
    }
  });

  test("a picture fades to black as a record without one takes over", async () => {
    const browser = await launch();
    try {
      const page = await openVideo(browser, [{ deck: 1, record: RECORD_ONE, kind: "video/webm" }]);
      await expect.poll(async () => (await shown(page))[0]?.width ?? 0, { timeout: 15_000 }).toBeGreaterThan(0);
      // Deck 2 (no picture) playing too, three quarters across to the right:
      // deck 1 is a quarter of what is heard.
      await emit(page, { decks: { 1: { playing: true } }, master: { crossfader: 0.5 } });
      await expect.poll(() => whole(page)).toBeCloseTo(0.25, 5);
      await emit(page, { decks: { 1: { playing: true } }, master: { crossfader: 1 } });
      await expect.poll(() => whole(page)).toBe(0);
    } finally {
      await browser.close();
    }
  });

  /** **No video, and a picture that will not play, are each said.** */
  test("no video and a picture that will not play are said", async () => {
    const browser = await launch();
    try {
      const none = await openVideo(browser, []);
      await expect(none.locator("[data-video-none]")).toContainText("No deck has a video on it");

      const broken = await openVideo(
        browser,
        [{ deck: 1, record: RECORD_ONE, kind: "video/mp4" }],
        Buffer.from("not a video at all"),
      );
      await expect(broken.getByRole("alert")).toContainText("Deck 1's picture will not play");
      await expect(broken.getByRole("alert")).toContainText("its sound plays all the same");
    } finally {
      await browser.close();
    }
  });

  /** The controls hidden leave only the picture; a click brings them back. */
  test("the controls hide for a projector and come back", async () => {
    const browser = await launch();
    try {
      const page = await openVideo(browser, [{ deck: 1, record: RECORD_ONE, kind: "video/webm" }]);
      await page.getByRole("button", { name: "Hide the controls" }).click();
      await expect(page.locator("[data-video] .controls")).toHaveCount(0);
      await expect(page.locator("[data-video]")).toHaveClass(/clean/);
      await page.locator("[data-stage]").click();
      await expect(page.getByRole("button", { name: "Hide the controls" })).toBeVisible();
    } finally {
      await browser.close();
    }
  });
});
