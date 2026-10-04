/**
 * §119: a phone as a camera, from djmanzo's own page -- both pages, against
 * the real server.
 *
 * Not the shell: the phone's page and OBS's page are served by Rust
 * (`dj_net::phone`), over HTTPS with djmanzo's own certificate and plain HTTP
 * on this computer, from one port. So the server is the real one, run on its
 * own by `cargo run -p dj-net --example phone_camera`, and the two pages are
 * what it serves. Chromium's fake camera stands in for the phone's; the
 * certificate warning a phone shows once is what `ignoreHTTPSErrors` taps
 * past.
 *
 * What these hold: the phone's picture reaches OBS's page through WebRTC,
 * with djmanzo carrying only the introduction, and goes when the phone stops;
 * a second phone takes over, and the first stops rather than taking it back;
 * the certificate the page offers is the one whose fingerprint it shows.
 *
 * What they cannot: whether a real phone's browser lends the camera to a
 * page reached by tapping past the warning. Chromium here is told to.
 */
import { chromium, expect, test, type Browser, type Page } from "@playwright/test";
import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const binary = path.join(
  root,
  "target",
  "debug",
  "examples",
  process.platform === "win32" ? "phone_camera.exe" : "phone_camera",
);

test.describe.configure({ timeout: 90_000 });

let server: ChildProcess | undefined;
let port = 0;
let fingerprint = "";
/** Every line the server has printed: what it says each side is doing. */
const said: string[] = [];

test.beforeAll(async () => {
  test.setTimeout(600_000);
  // Built here if a step before has not; quick when it has.
  execFileSync("cargo", ["build", "-q", "-p", "dj-net", "--example", "phone_camera"], {
    cwd: root,
    stdio: "inherit",
  });
  server = spawn(binary, ["0", "phonekey", "obskey"], { stdio: ["ignore", "pipe", "inherit"] });
  const lines = createInterface({ input: server.stdout! });
  lines.on("line", (line) => said.push(line));
  await expect
    .poll(() => said.find((l) => l.startsWith("authority: ")), { timeout: 30_000 })
    .toBeTruthy();
  port = Number(said.find((l) => l.startsWith("listening "))!.split(" ")[1]);
  fingerprint = said.find((l) => l.startsWith("authority: "))!.slice("authority: ".length);
});

test.afterAll(() => {
  server?.kill();
});

async function launch(): Promise<Browser> {
  return chromium.launch({
    executablePath: process.env.DJMANZO_CHROMIUM || undefined,
    args: ["--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream"],
  });
}

const phoneAt = () => `https://127.0.0.1:${port}/p/phonekey/`;
const obsAt = () => `http://127.0.0.1:${port}/v/obskey/`;

async function openPhone(browser: Browser): Promise<Page> {
  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  const page = await context.newPage();
  await page.goto(phoneAt());
  return page;
}

async function openObs(browser: Browser): Promise<Page> {
  const page = await (await browser.newContext()).newPage();
  await page.goto(obsAt());
  return page;
}

/** OBS's picture: whether it is shown, and how big what it decodes is. */
function picture(obs: Page) {
  return obs.evaluate(() => {
    const v = document.getElementById("phone") as HTMLVideoElement;
    return { shown: !v.hidden, width: v.videoWidth };
  });
}

test("a phone's picture reaches OBS's page, and goes when the phone stops", async () => {
  const browser = await launch();
  const from = said.length;
  const since = () => said.slice(from);
  try {
    const obs = await openObs(browser);
    // Nothing on the stream before a phone sends: the page is transparent.
    expect(await picture(obs)).toEqual({ shown: false, width: 0 });

    const phone = await openPhone(browser);
    expect(await phone.evaluate(() => window.isSecureContext)).toBe(true);
    await phone.getByRole("button", { name: "Start the camera" }).click();
    await expect(phone.getByRole("status")).toHaveText("Live: OBS is showing this camera.", {
      timeout: 30_000,
    });
    await expect.poll(async () => (await picture(obs)).shown, { timeout: 30_000 }).toBe(true);
    expect((await picture(obs)).width).toBeGreaterThan(0);
    // djmanzo's view of it, which is what its Camera panel says.
    await expect
      .poll(() => since().includes("phone Sending, OBS Showing"), { timeout: 20_000 })
      .toBe(true);

    await phone.getByRole("button", { name: "Stop" }).click();
    await expect(phone.getByRole("status")).toHaveText("Stopped.");
    // OBS lets the picture go at once, told by djmanzo rather than left to
    // notice the connection die.
    await expect.poll(async () => (await picture(obs)).shown, { timeout: 5_000 }).toBe(false);
    await expect
      .poll(() => since()[since().length - 1], { timeout: 10_000 })
      .toBe("phone Open, OBS Open");
  } finally {
    await browser.close();
  }
});

test("a second phone takes over, and the first stops rather than taking it back", async () => {
  const browser = await launch();
  try {
    const obs = await openObs(browser);
    const first = await openPhone(browser);
    await first.getByRole("button", { name: "Start the camera" }).click();
    await expect(first.getByRole("status")).toHaveText("Live: OBS is showing this camera.", {
      timeout: 30_000,
    });

    const second = await openPhone(browser);
    await second.getByRole("button", { name: "Start the camera" }).click();
    await expect(second.getByRole("status")).toHaveText("Live: OBS is showing this camera.", {
      timeout: 30_000,
    });
    await expect(first.getByRole("status")).toHaveText(
      "Another phone took over. Press start to take it back.",
      { timeout: 30_000 },
    );
    // And it stays that way: no turns taken.
    await first.waitForTimeout(4_000);
    await expect(second.getByRole("status")).toHaveText("Live: OBS is showing this camera.");
    await expect(first.getByRole("button", { name: "Start the camera" })).toBeVisible();
    expect((await picture(obs)).shown).toBe(true);
  } finally {
    await browser.close();
  }
});

test("OBS's page reloaded picks the phone up again by itself", async () => {
  const browser = await launch();
  try {
    const obs = await openObs(browser);
    const phone = await openPhone(browser);
    await phone.getByRole("button", { name: "Start the camera" }).click();
    await expect.poll(async () => (await picture(obs)).shown, { timeout: 30_000 }).toBe(true);

    // OBS restarts its browser sources: a new page, the old one gone. The
    // phone notices its link went, finds its offer still the current one,
    // and offers again; the new page answers it.
    await obs.reload();
    expect(await picture(obs)).toEqual({ shown: false, width: 0 });
    await expect.poll(async () => (await picture(obs)).shown, { timeout: 45_000 }).toBe(true);
    await expect(phone.getByRole("status")).toHaveText("Live: OBS is showing this camera.", {
      timeout: 10_000,
    });
  } finally {
    await browser.close();
  }
});

test("the certificate the phone is offered is the one whose fingerprint it shows", async () => {
  const browser = await launch();
  try {
    const phone = await openPhone(browser);
    await phone.getByText("Stop the browser warning for good").click();
    await expect(phone.getByText(fingerprint)).toBeVisible();
    const href = await phone.getByRole("link", { name: "Download djmanzo's certificate" }).getAttribute("href");
    expect(href).toBe("/p/phonekey/djmanzo.crt");
    const downloaded = await phone.evaluate(async (href) => {
      const got = await fetch(href!);
      const digest = await crypto.subtle.digest("SHA-256", await got.arrayBuffer());
      return {
        type: got.headers.get("content-type"),
        print: [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0").toUpperCase()).join(":"),
      };
    }, href);
    expect(downloaded).toEqual({ type: "application/x-x509-ca-cert", print: fingerprint });
  } finally {
    await browser.close();
  }
});
