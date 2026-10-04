/**
 * §119: the computer's webcam, inside djmanzo -- the Camera panel.
 *
 * Chromium's own fake camera stands in for a webcam, through the same
 * `getUserMedia` a real one takes. What these hold: the camera is opened only
 * when the DJ asks -- or in a window opened while the DJ has asked this run,
 * which Rust keeps -- and let go when it is turned off; the picture can be
 * mirrored and left alone on the screen for OBS to capture; a camera that
 * cannot be had is said, and nothing is kept as asked.
 *
 * Chromium is not the webview djmanzo ships in. The shipped webview's half --
 * whether it hands the request to a device at all -- is `dj_app::senses`,
 * driven under Xvfb with WebKitGTK's mock camera.
 */
import { chromium, expect, test, type Browser, type Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

test.describe.configure({ timeout: 60_000 });

type CameraWindow = {
  __cameraSet?: boolean[];
  __streams?: MediaStream[];
  __gumCalls?: number;
};

async function launch(): Promise<Browser> {
  return chromium.launch({
    executablePath: process.env.DJMANZO_CHROMIUM || undefined,
    args: ["--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream"],
  });
}

/** The Camera panel in a window of its own; every stream handed out is kept. */
async function openCamera(
  browser: Browser,
  answers: Record<string, unknown> = {},
  { noCamera = false }: { noCamera?: boolean } = {},
): Promise<Page> {
  const context = await browser.newContext({ baseURL: test.info().project.use.baseURL });
  const page = await context.newPage();
  await page.addInitScript((noCamera: boolean) => {
    const devices = navigator.mediaDevices;
    const real = devices.getUserMedia.bind(devices);
    const win = window as unknown as CameraWindow;
    win.__streams = [];
    win.__gumCalls = 0;
    devices.getUserMedia = async (asked?: MediaStreamConstraints) => {
      win.__gumCalls = (win.__gumCalls ?? 0) + 1;
      if (noCamera) throw new DOMException("Requested device not found", "NotFoundError");
      const stream = await real(asked);
      win.__streams!.push(stream);
      return stream;
    };
  }, noCamera);
  await openShell(page, "/?panel=camera", {}, answers);
  await expect(page.locator("[data-camera]")).toBeVisible();
  return page;
}

const seen = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as CameraWindow;
    const video = document.querySelector("[data-camera] video") as HTMLVideoElement | null;
    const shown = video?.srcObject as MediaStream | null;
    return {
      asked: w.__gumCalls ?? 0,
      set: w.__cameraSet ?? [],
      shown: shown?.getVideoTracks()[0]?.readyState ?? null,
      kept: (w.__streams ?? []).map((s) => s.getVideoTracks()[0]?.readyState),
      width: video?.videoWidth ?? 0,
    };
  });

test.describe("§119: the camera", () => {
  /**
   * **Off until asked; asked, it shows; off, it lets go.** No camera is
   * opened when the panel opens; *Show the camera* opens one, shows it, and
   * tells Rust the DJ asked; mirrored, it is drawn turned round; with the
   * controls hidden only the picture is left, and a click on it brings them
   * back; *Turn the camera off* stops the camera itself, not only the
   * picture, and tells Rust.
   */
  test("the camera is opened only when asked, and let go when turned off", async () => {
    const browser = await launch();
    try {
      const page = await openCamera(browser);
      const panel = page.locator("[data-camera]");
      await expect(panel).toContainText("The camera is off");
      await page.waitForTimeout(300);
      expect((await seen(page)).asked).toBe(0);

      await panel.getByRole("button", { name: "Show the camera" }).click();
      await expect.poll(async () => (await seen(page)).shown).toBe("live");
      await expect.poll(async () => (await seen(page)).width).toBeGreaterThan(0);
      expect((await seen(page)).set).toEqual([true]);

      await panel.getByRole("checkbox", { name: "Mirror" }).check();
      await expect(panel.locator("video")).toHaveClass(/mirrored/);

      await panel.getByRole("button", { name: "Hide the controls" }).click();
      await expect(panel.locator(".controls")).toHaveCount(0);
      await expect(panel).toHaveAttribute("class", /clean/);
      await panel.locator("video").click();
      await expect(panel.getByRole("button", { name: "Turn the camera off" })).toBeVisible();

      await panel.getByRole("button", { name: "Turn the camera off" }).click();
      await expect(panel).toContainText("The camera is off");
      // The camera itself let go, not only the picture.
      await expect.poll(async () => (await seen(page)).kept).toEqual(["ended"]);
      expect((await seen(page)).set).toEqual([true, false]);
      expect(errorsThrown(page)).toEqual([]);
    } finally {
      await browser.close();
    }
  });

  /**
   * **A window opened while the DJ has asked shows the camera by itself** --
   * the panel popped out for OBS -- and one opened when the DJ has not asked
   * never opens one.
   */
  test("a window opened while the camera is wanted shows it without asking again", async () => {
    const browser = await launch();
    try {
      const wanted = await openCamera(browser, { camera_wanted: true });
      await expect.poll(async () => (await seen(wanted)).shown).toBe("live");
      expect((await seen(wanted)).asked).toBe(1);

      const not = await openCamera(browser, { camera_wanted: false });
      await not.waitForTimeout(500);
      expect((await seen(not)).asked).toBe(0);
    } finally {
      await browser.close();
    }
  });

  /** **No camera is said, and nothing is kept as asked.** */
  test("a machine with no camera says so", async () => {
    const browser = await launch();
    try {
      const page = await openCamera(browser, {}, { noCamera: true });
      const panel = page.locator("[data-camera]");
      await panel.getByRole("button", { name: "Show the camera" }).click();
      await expect(panel.getByRole("alert")).toHaveText("No camera was found.");
      expect((await seen(page)).set).toEqual([]);
      await expect(panel.getByRole("button", { name: "Show the camera" })).toBeEnabled();
    } finally {
      await browser.close();
    }
  });
});
