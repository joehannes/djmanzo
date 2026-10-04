/**
 * §122: djmanzo's own layers, behind the words on the singers' screen.
 *
 * Whether a layer can be drawn, and what each signal reads, are
 * `./src/looks.ts`'s and tested beside it. What a browser holds is the rest:
 * that a chosen look is drawn, under the veil that keeps the words legible;
 * that the picture answers to the music it says it does, read back off the
 * canvas; and that nothing the layers do — a picture that will not compile,
 * no WebGL at all, a system asking for stillness — takes the words with it.
 */
import { readFileSync } from "node:fs";

import { expect, test, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/** The content rules djmanzo's webview runs under, as `tauri.conf.json` has them. */
const CONTENT_RULES = (
  JSON.parse(readFileSync(new URL("../../crates/dj-app/tauri.conf.json", import.meta.url), "utf8")) as {
    app: { security: { csp: string } };
  }
).app.security.csp;

/**
 * Serve the page under `rules`, as the application's webview does. The
 * browser tests have no content rules of their own, so without this a
 * library that evaluates JavaScript would pass here and fail in djmanzo.
 */
async function underRules(page: Page, rules: string) {
  await page.route("**/*", async (route) => {
    if (route.request().resourceType() !== "document") return route.fallback();
    const response = await route.fetch();
    const body = (await response.text()).replace(
      "<head>",
      `<head><meta http-equiv="Content-Security-Policy" content="${rules}">`,
    );
    return route.fulfill({ response, body });
  });
}

const WORDS = {
  lines: [
    { at: 10, text: "First line", words: [] },
    { at: 30, text: "After a break", words: [] },
  ],
  plain: [],
  instrumental: false,
};

const SCREEN = "[data-singer-screen]";
const CANVAS = `${SCREEN} canvas[data-visuals]`;

/**
 * Keep what each frame draws, so a test can read it back: a canvas's
 * drawing buffer is otherwise cleared once it is shown. And, when asked,
 * make one kind's picture fail to compile, or offer no WebGL at all.
 */
async function rig(page: Page, options: { failKind?: string; noWebgl?: boolean } = {}) {
  await page.addInitScript((options) => {
    const original = HTMLCanvasElement.prototype.getContext;
    // @ts-expect-error -- the overloads are not worth restating for a test.
    HTMLCanvasElement.prototype.getContext = function (kind: string, attributes?: Record<string, unknown>) {
      if (kind === "webgl" || kind === "webgl2" || kind === "experimental-webgl") {
        if (options.noWebgl) return null;
        const gl = original.call(this, kind, { ...attributes, preserveDrawingBuffer: true }) as WebGLRenderingContext | null;
        if (gl) {
          // Every layer drawn, counted, so a test can hold the frame rate.
          const drawArrays = gl.drawArrays.bind(gl);
          gl.drawArrays = (mode, first, count) => {
            const win = window as unknown as { __layersDrawn?: number };
            win.__layersDrawn = (win.__layersDrawn ?? 0) + 1;
            drawArrays(mode, first, count);
          };
        }
        if (gl && options.failKind) {
          const sources = new WeakMap<WebGLShader, string>();
          const shaderSource = gl.shaderSource.bind(gl);
          gl.shaderSource = (shader, source) => {
            sources.set(shader, source);
            shaderSource(shader, source);
          };
          const parameter = gl.getShaderParameter.bind(gl);
          gl.getShaderParameter = (shader, name) =>
            name === gl.COMPILE_STATUS && (sources.get(shader) ?? "").includes(`// djmanzo: ${options.failKind}`)
              ? false
              : parameter(shader, name);
          const log = gl.getShaderInfoLog.bind(gl);
          gl.getShaderInfoLog = (shader) =>
            (sources.get(shader) ?? "").includes(`// djmanzo: ${options.failKind}`) ? "the test said no" : log(shader);
        }
        return gl;
      }
      return original.call(this, kind, attributes);
    };
  }, options);
}

async function open(page: Page, look: string | null) {
  await openShell(page, "/?panel=singers", {}, { singer_lyrics: WORDS, look_chosen: { singers: look } });
  await expect(page.locator(SCREEN)).toBeVisible();
}

/** Deck 2 playing to the singer, with the master and the stems as given. */
async function hear(
  page: Page,
  sound: { loudness?: number; bands?: number[]; stems?: number[]; voice?: number; volume?: number },
) {
  await page.evaluate((sound) => {
    const win = window as unknown as {
      __lastState?: {
        decks: Record<string, unknown>[];
        master: Record<string, unknown> & { vocals: { strips: Record<string, unknown>[] } };
        context: { audio: Record<string, unknown> };
      };
      __emit?: (next: unknown) => void;
    };
    const state = win.__lastState;
    if (!state) throw new Error("no state");
    const decks = state.decks.map((deck, index) =>
      index === 1
        ? {
            ...deck,
            playing: true,
            loaded: true,
            voice: 0,
            position_seconds: 12,
            effective_bpm: 120,
            beat_phase: 0.3,
            volume: sound.volume ?? 1,
            crossfader_assign: "thru",
            stem_levels: sound.stems ?? [0, 0, 0, 0],
          }
        : { ...deck, playing: false },
    );
    const strips = [{ ...(state.master.vocals.strips[0] ?? {}), level: sound.voice ?? 0 }];
    win.__emit?.({
      ...state,
      decks,
      master: { ...state.master, vocals: { ...state.master.vocals, strips } },
      context: {
        ...state.context,
        audio: { loudness: sound.loudness ?? 0, bands: sound.bands ?? [0, 0, 0, 0] },
      },
    });
  }, sound);
}

/** How much the canvas covers, 0..1: its mean alpha, read back off the GPU. */
async function coverage(page: Page): Promise<number> {
  return page.locator(CANVAS).evaluate((element) => {
    const canvas = element as HTMLCanvasElement;
    const gl = canvas.getContext("webgl") as WebGLRenderingContext;
    const { width, height } = canvas;
    const pixels = new Uint8Array(width * height * 4);
    gl.readPixels(0, 0, width, height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
    let sum = 0;
    for (let i = 3; i < pixels.length; i += 4) sum += pixels[i];
    return sum / (pixels.length / 4) / 255;
  });
}

/** The canvas's colour, read back off the GPU: its mean brightness, 0..1. */
async function brightness(page: Page): Promise<number> {
  return page.locator(CANVAS).evaluate((element) => {
    const canvas = element as HTMLCanvasElement;
    const gl = canvas.getContext("webgl") as WebGLRenderingContext;
    const { width, height } = canvas;
    const pixels = new Uint8Array(width * height * 4);
    gl.readPixels(0, 0, width, height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
    let sum = 0;
    for (let i = 0; i < pixels.length; i += 4) sum += pixels[i] + pixels[i + 1] + pixels[i + 2];
    return sum / (pixels.length / 4) / 3 / 255;
  });
}

/** Coverage once the light has followed the sound: a few frames, then steady. */
async function settled(page: Page): Promise<number> {
  await page.waitForTimeout(700);
  return coverage(page);
}

test.describe("§122: layers behind the words", () => {
  /**
   * **The load-bearing one.** A chosen look is drawn — every layer of it —
   * beneath the veil that keeps the words legible, and the words are still
   * there, on top.
   */
  test("the look the DJ picked is drawn behind the words, under their veil", async ({ page }) => {
    await rig(page);
    await open(page, "Everything");
    await hear(page, { loudness: 0.8, bands: [0.6, 0.5, 0.4, 0.7], stems: [0, 0.8, 0.5, 0.6], voice: 0.7 });
    const canvas = page.locator(CANVAS);
    await expect(canvas).toHaveAttribute("data-visuals", "drawing");
    await expect(canvas).toHaveAttribute("data-look", "Everything");
    await expect(canvas).toHaveAttribute("data-layers", "wash pulse ripple sparks halo");
    await expect(canvas).toHaveAttribute("data-failures", "");
    expect(await settled(page)).toBeGreaterThan(0.2);
    // Inside the background, so its veil is drawn over the layers too.
    expect(await canvas.evaluate((c) => c.parentElement?.classList.contains("backdrop"))).toBe(true);
    await expect(page.locator(`${SCREEN} [data-line="now"]`)).toHaveText("First line");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **What it costs.** Half the screen's size each way and thirty frames a
   * second: under a software renderer that took the layers from more than a
   * core of their own to a third of one (docs/VISUALS.md), and soft layers
   * stretched look the same.
   */
  test("draws at half the screen's size, thirty times a second", async ({ page }) => {
    await rig(page);
    await open(page, "Glow");
    await hear(page, { loudness: 0.5 });
    const canvas = page.locator(CANVAS);
    await expect(canvas).toHaveAttribute("data-visuals", "drawing");
    const size = await canvas.evaluate((c) => {
      const element = c as HTMLCanvasElement;
      return { width: element.width, height: element.height, shown: element.clientWidth, tall: element.clientHeight };
    });
    expect(size.width).toBe(Math.min(960, Math.round(size.shown / 2)));
    expect(size.height).toBe(Math.min(540, Math.round(size.tall / 2)));

    const count = () => page.evaluate(() => (window as unknown as { __layersDrawn?: number }).__layersDrawn ?? 0);
    const before = await count();
    await page.waitForTimeout(2_000);
    // Two layers a frame.
    const frames = (await count()) - before;
    expect(frames / 2 / 2).toBeLessThanOrEqual(32);
    expect(frames / 2 / 2).toBeGreaterThan(10);
  });

  /** The wash answers to the room's loudness: dark when quiet, full when loud. */
  test("a layer answers to its signal: the wash to the room's loudness", async ({ page }) => {
    await rig(page);
    await open(page, "Glow");
    await expect(page.locator(CANVAS)).toHaveAttribute("data-visuals", "drawing");
    await hear(page, { loudness: 0 });
    const quiet = await settled(page);
    await hear(page, { loudness: 1 });
    const loud = await settled(page);
    expect(quiet).toBeGreaterThan(0);
    expect(loud).toBeGreaterThan(quiet * 3);
  });

  /**
   * The pulse answers to the drums **as the room hears them**: the stem
   * through its deck's fader. Loudness held at nothing, so the wash beneath
   * stays still and only the pulse can move.
   */
  test("a stem lights its layer through its deck's fader", async ({ page }) => {
    await rig(page);
    await open(page, "Pulse");
    await expect(page.locator(CANVAS)).toHaveAttribute("data-visuals", "drawing");
    await hear(page, { stems: [0, 0, 0, 0] });
    const none = await settled(page);
    await hear(page, { stems: [0, 0.9, 0, 0] });
    const drums = await settled(page);
    await hear(page, { stems: [0, 0.9, 0, 0], volume: 0 });
    const faded = await settled(page);
    expect(drums).toBeGreaterThan(none + 0.01);
    expect(faded).toBeLessThan(drums);
    expect(Math.abs(faded - none)).toBeLessThan(0.005);
  });

  /** A picture that will not compile is left out and named; the rest are drawn. */
  test("a picture that will not compile leaves its layer out, not the look", async ({ page }) => {
    await rig(page, { failKind: "wash" });
    await open(page, "Everything");
    await hear(page, { loudness: 0.8, bands: [0.6, 0.5, 0.4, 0.7], stems: [0, 0.8, 0.5, 0.6], voice: 0.9 });
    const canvas = page.locator(CANVAS);
    await expect(canvas).toHaveAttribute("data-failures", /wash: the test said no/);
    await expect(canvas).toHaveAttribute("data-visuals", "drawing");
    expect(await settled(page)).toBeGreaterThan(0.01);
    await expect(page.locator(`${SCREEN} [data-line="now"]`)).toHaveText("First line");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **MilkDrop as the main layer, under djmanzo's own content rules.**
   * Butterchurn compiles the preset's equations to WebAssembly and evaluates
   * no JavaScript, so it draws with `'wasm-unsafe-eval'` and without
   * `'unsafe-eval'`. What it draws moves: the picture read back twice, a
   * second apart, differs.
   */
  test("MilkDrop draws as the main layer under djmanzo's content rules", async ({ page }) => {
    await rig(page);
    await underRules(page, CONTENT_RULES);
    await open(page, "Drift");
    await hear(page, { loudness: 0.7, bands: [0.6, 0.4, 0.3, 0.3], voice: 0 });
    const canvas = page.locator(CANVAS);
    await expect(canvas).toHaveAttribute("data-milkdrop", "Drift");
    // Butterchurn is loaded on demand and its preset compiled: waited for,
    // not timed, so a busy machine is not a failure.
    await expect(canvas).toHaveAttribute("data-milkdrop-state", "drawing", { timeout: 20_000 });
    await page.waitForTimeout(500);
    await expect(canvas).toHaveAttribute("data-failures", "");
    const before = await brightness(page);
    await page.waitForTimeout(1_000);
    const after = await brightness(page);
    expect(before + after).toBeGreaterThan(0.01);
    expect(Math.abs(after - before)).toBeGreaterThan(0.0005);
    await expect(page.locator(`${SCREEN} [data-line="now"]`)).toHaveText("First line");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **As WebKitGTK is: an offscreen canvas with no WebGL 2.** djmanzo's Linux
   * webview has the `OffscreenCanvas` class and refuses it WebGL 2, and
   * Butterchurn, which draws into one whenever the class exists, failed there
   * at its first framebuffer. Driving it found that; this holds it, by giving
   * Chromium the same refusal.
   */
  test("MilkDrop draws where an offscreen canvas has no WebGL 2", async ({ page }) => {
    await page.addInitScript(() => {
      const original = OffscreenCanvas.prototype.getContext;
      // @ts-expect-error -- the overloads are not worth restating for a test.
      OffscreenCanvas.prototype.getContext = function (kind: string, attributes?: unknown) {
        return kind === "webgl2" ? null : original.call(this, kind as "2d", attributes as never);
      };
    });
    await rig(page);
    await underRules(page, CONTENT_RULES);
    await open(page, "Drift");
    await hear(page, { loudness: 0.7, bands: [0.6, 0.4, 0.3, 0.3], voice: 0 });
    const canvas = page.locator(CANVAS);
    await expect(canvas).toHaveAttribute("data-milkdrop-state", "drawing", { timeout: 20_000 });
    await page.waitForTimeout(500);
    await expect(canvas).toHaveAttribute("data-failures", "");
    const before = await brightness(page);
    await page.waitForTimeout(1_000);
    expect(Math.abs((await brightness(page)) - before)).toBeGreaterThan(0.0005);
    // And the class is back as it was, for anything else that uses it.
    expect(await page.evaluate(() => typeof OffscreenCanvas)).toBe("function");
  });

  /**
   * Without WebAssembly allowed — content rules that are `'self'` alone —
   * MilkDrop cannot run: it is left out and named, and the look's other
   * layers are drawn. This is also what shows the rule is what lets it run.
   */
  test("with WebAssembly refused, MilkDrop is left out and the rest drawn", async ({ page }) => {
    await rig(page);
    await underRules(page, CONTENT_RULES.replace(" 'wasm-unsafe-eval'", ""));
    await open(page, "Bloom");
    await hear(page, { loudness: 0.7, bands: [0.6, 0.4, 0.3, 0.3], voice: 0.9 });
    const canvas = page.locator(CANVAS);
    await expect(canvas).toHaveAttribute("data-failures", /milkdrop:/, { timeout: 20_000 });
    await expect(canvas).toHaveAttribute("data-milkdrop-state", "failed");
    await expect(canvas).toHaveAttribute("data-visuals", "drawing");
    expect(await settled(page)).toBeGreaterThan(0.005);
    await expect(page.locator(`${SCREEN} [data-line="now"]`)).toHaveText("First line");
  });

  /** No WebGL: nothing drawn, and the screen is K1's, as before there were looks. */
  test("with no WebGL the words and the record's background are as they were", async ({ page }) => {
    await rig(page, { noWebgl: true });
    await openShell(
      page,
      "/?panel=singers",
      {},
      {
        singer_lyrics: WORDS,
        look_chosen: { singers: "Everything" },
        singer_backdrop: {
          source: "sound",
          track: "t",
          shades: [{ colour: "rgb(200 40 60)", weight: 1 }],
          pending: false,
        },
      },
    );
    await hear(page, { loudness: 0.8 });
    await expect(page.locator(CANVAS)).toHaveAttribute("data-visuals", "no-webgl");
    await expect(page.locator(`${SCREEN} .glow`)).toHaveCount(1);
    await expect(page.locator(`${SCREEN} [data-line="now"]`)).toHaveText("First line");
    expect(errorsThrown(page)).toEqual([]);
  });

  test("a system asking for stillness gets none of it", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await rig(page);
    await open(page, "Everything");
    await hear(page, { loudness: 0.8 });
    await expect(page.locator(CANVAS)).toHaveAttribute("data-visuals", "still");
    await expect(page.locator(`${SCREEN} [data-line="now"]`)).toHaveText("First line");
  });

  test("off draws nothing at all", async ({ page }) => {
    await rig(page);
    await open(page, null);
    await hear(page, { loudness: 0.8 });
    await expect(page.locator(`${SCREEN} [data-line="now"]`)).toHaveText("First line");
    await expect(page.locator(CANVAS)).toHaveCount(0);
  });

  /** Picked in another window: the screen asks every few seconds and follows. */
  test("the screen follows a look picked elsewhere, and off again", async ({ page }) => {
    await rig(page);
    await open(page, null);
    await page.evaluate(() => {
      (window as unknown as { __lookChosen: unknown }).__lookChosen = { singers: "Waves" };
    });
    await expect(page.locator(CANVAS)).toHaveAttribute("data-look", "Waves", { timeout: 8_000 });
    await page.evaluate(() => {
      (window as unknown as { __lookChosen: unknown }).__lookChosen = { singers: null };
    });
    await expect(page.locator(CANVAS)).toHaveCount(0, { timeout: 8_000 });
  });
});

test.describe("§122: picking the look", () => {
  test("the Singers panel offers djmanzo's looks, says what each is for, and off", async ({ page }) => {
    await openShell(page, "/?panel=karaoke");
    const picker = page.getByRole("combobox", { name: "The look behind the words" });
    await expect(picker).toHaveValue("");
    const names = await picker.locator("option").allTextContents();
    expect(names[0]).toMatch(/^Off/);
    expect(names.slice(1)).toEqual(["Glow", "Pulse", "Waves", "Sparkle", "Drift", "Bloom", "Everything"]);

    await picker.selectOption("Pulse");
    await expect(page.locator("[data-look-description]")).toContainText("every beat");
    await picker.selectOption("");
    await expect(page.locator("[data-look-description]")).toHaveCount(0);
    const asked = await page.evaluate(() => (window as unknown as { __lookChoices: unknown[] }).__lookChoices);
    expect(asked).toEqual([
      { screen: "singers", name: "Pulse" },
      { screen: "singers", name: null },
    ]);
    expect(errorsThrown(page)).toEqual([]);
  });

  test("the panel says when this computer asks for stillness", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await openShell(page, "/?panel=karaoke", {}, { look_chosen: { singers: "Glow" } });
    await expect(page.getByRole("combobox", { name: "The look behind the words" })).toHaveValue("Glow");
    await expect(page.getByText(/asks for reduced motion/)).toBeVisible();
  });
});
