/**
 * Karaoke beta B5: a guest's song, from Suno's window to WhatsApp, as the
 * song card drives it.
 *
 * What Rust does — the fill script, the song's link read from the window's
 * address, the WhatsApp Web address and its consent — is held in
 * `dj_app::suno` and `dj_app::guests` and in `suno-fill.spec.ts`. This holds
 * the card: *Fill in Suno's form* sends the words chosen and puts them on
 * the clipboard too; the link comes into the card by itself once Suno's
 * window shows the song, and says so; and *In djmanzo's window* opens
 * WhatsApp Web with that link, beside the app's button as it was.
 *
 * Rust's three new commands are answered by wrapping the page's `invoke` in
 * this test: the karaoke beta's sessions leave `shell.ts` alone.
 */
import { expect, test, type Page } from "@playwright/test";

import guestSong from "./guest-song.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const book = (page: Page) => page.getByRole("region", { name: "Guest book" });

/** Suno's window shows a song once `__songMade()` is called. */
async function rust(page: Page) {
  await page.evaluate(() => {
    const win = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
      __sendCalls: { cmd: string; args: Record<string, unknown> }[];
      __songMade: () => void;
    };
    let made = false;
    win.__sendCalls = [];
    win.__songMade = () => (made = true);
    const stub = win.__TAURI_INTERNALS__.invoke;
    win.__TAURI_INTERNALS__.invoke = (cmd, args = {}) => {
      if (cmd === "suno_fill" || cmd === "suno_link" || cmd === "guests_whatsapp_here") {
        win.__sendCalls.push({ cmd, args: structuredClone(args) });
      }
      if (cmd === "suno_fill") return Promise.resolve({ language: String(args.language), waits: true });
      if (cmd === "suno_link") return Promise.resolve(made ? "https://suno.com/song/abc-123" : null);
      if (cmd === "guests_whatsapp_here") return Promise.resolve(`Hi Ana! Here is a song made for you from tonight: ${String(args.link)}`);
      return stub(cmd, args);
    };
  });
}

const sent = (page: Page, cmd: string) =>
  page.evaluate(
    (cmd) =>
      (window as unknown as { __sendCalls: { cmd: string; args: Record<string, unknown> }[] }).__sendCalls
        .filter((call) => call.cmd === cmd)
        .map((call) => call.args),
    cmd,
  );

test("Suno's form is filled, the link comes back by itself, and WhatsApp Web opens with it", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await openShell(page, "/", {}, {
    karaoke_rotation: {
      singers: [{ name: "Ana", songs: [{ title: "Obsesión", track: null, path: null, key: 0 }], turns: 0 }],
      up_next: "Ana",
      lately: [],
    },
  });
  await rust(page);
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.locator("body").click({ position: { x: 700, y: 120 } });
  await page.keyboard.press("Space");
  await page.keyboard.press("O");
  await page.keyboard.press("i");
  await expect(page.locator("[data-singers]")).toBeVisible();
  await page.getByRole("button", { name: "Sang", exact: true }).click();
  const ana = book(page).getByRole("list", { name: "Guests" }).getByRole("button", { name: /Ana/ });
  await ana.click();
  let form = book(page).getByRole("form", { name: "About Ana" });
  await form.getByRole("checkbox", { name: /Record about fifteen seconds/ }).check();
  await form.getByRole("checkbox", { name: /Keep my email, phone or WhatsApp/ }).check();
  await form.getByRole("textbox", { name: "Phone or WhatsApp" }).fill("+34 600 123 456");
  await form.getByRole("button", { name: "Keep" }).click();
  await ana.click();
  form = book(page).getByRole("form", { name: "About Ana" });
  const song = form.getByRole("group", { name: "A song for Ana" });
  await song.getByRole("button", { name: "Write the words" }).click();

  // The words chosen go into Suno's form, and onto the clipboard.
  const make = song.getByRole("group", { name: "Make it on Suno" });
  // Both new buttons are readable: their words not in their own fill colour.
  const legible = (name: string) =>
    song.getByRole("button", { name }).evaluate((el) => {
      const style = getComputedStyle(el);
      return style.color !== style.backgroundColor;
    });
  expect(await legible("Fill in Suno's form"), "the fill's words in its own colour").toBe(true);
  expect(await legible("In djmanzo's window"), "the send's words in its own colour").toBe(true);
  await make.getByRole("combobox", { name: "Words to fill in" }).selectOption("Catalan");
  await make.getByRole("button", { name: "Fill in Suno's form" }).click();
  await expect(make.getByRole("status")).toContainText("Catalan words and the style went to Suno's window");
  await expect(make.getByRole("status")).toContainText("Suno's window says what it filled");
  await expect(make.getByRole("status")).toContainText("the form fills itself once Create opens");
  await expect(make.getByRole("status")).toContainText("press Create");
  expect(await sent(page, "suno_fill")).toEqual([{ id: expect.any(String), language: "Catalan" }]);
  const catalan = guestSong.versions.find((v) => v.language === "Catalan")!;
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(catalan.lyrics);

  // Nothing yet while Suno's window shows the form; the song's link once it shows the song.
  const send = song.getByRole("group", { name: "Send it to Ana" });
  const link = send.getByRole("textbox", { name: "The song's link" });
  await expect.poll(async () => (await sent(page, "suno_link")).length).toBeGreaterThan(0);
  await expect(link).toHaveValue("");
  await page.evaluate(() => (window as unknown as { __songMade: () => void }).__songMade());
  await expect(link).toHaveValue("https://suno.com/song/abc-123", { timeout: 5_000 });
  await expect(send.locator("[data-caught]")).toHaveText("The link came from Suno's window.");

  // WhatsApp Web in djmanzo's window, with that link; the app's button is still there.
  await expect(send.getByRole("button", { name: "WhatsApp", exact: true })).toBeVisible();
  await send.getByRole("button", { name: "In djmanzo's window" }).click();
  expect(await sent(page, "guests_whatsapp_here")).toEqual([{ id: expect.any(String), link: "https://suno.com/song/abc-123" }]);
  await expect(send.getByRole("status")).toContainText("https://suno.com/song/abc-123");

  // A link the DJ typed is theirs: not said to have come from Suno.
  await link.fill("https://suno.com/s/typed");
  await expect(send.locator("[data-caught]")).toHaveCount(0);
  expect(errorsThrown(page)).toEqual([]);
});
