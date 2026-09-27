/**
 * §123: the karaoke journal.
 *
 * > the current karaoke singer/guest/guest artist shall be remembered in some
 * > kind of useful and versatile and rich karaoke journal ... name of the
 * > guest artist, song sung, age, email, social profile(s), WA/phone nr, home
 * > country/town, nationality, favorite band, favorite musical genre, favorite
 * > song, native language, additional languages
 *
 * What a guest agreed to is `dj_app::guests`' and its Rust tests hold it;
 * this holds that the Singers surface reaches it: a singer marked as sung is
 * in the guest book, the three questions are Rust's own sentences, contact
 * cannot be typed until it is agreed to, a child cannot agree to it, and a
 * guest can be exported and forgotten.
 */
import { expect, test, type Page } from "@playwright/test";

import guestSong from "./guest-song.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const book = (page: Page) => page.getByRole("region", { name: "Guest book" });

const guestCalls = (page: Page) =>
  page.evaluate(() => (window as unknown as { __guests?: { cmd: string }[] }).__guests ?? []);

async function singers(page: Page, answers: Record<string, unknown> = {}) {
  await openShell(page, "/", {}, answers);
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.locator("body").click({ position: { x: 700, y: 120 } });
  await page.keyboard.press("Space");
  // Space O i: the Singers surface, large over the decks.
  await page.keyboard.press("O");
  await page.keyboard.press("i");
  await expect(page.locator("[data-singers]")).toBeVisible();
}

test.describe("§123: the karaoke journal", () => {
  /**
   * **The load-bearing one.** Sang puts the singer in the guest book with
   * the song; opened, the questions are Rust's words, the contact fields
   * wait for their question, and what is kept is what was agreed to.
   */
  test("a singer who sang is in the guest book, and only what they agree to is kept", async ({ page }) => {
    await singers(page, {
      karaoke_rotation: {
        singers: [{ name: "Ana", songs: [{ title: "Obsesión", track: null, path: null, key: 0 }], turns: 0 }],
        up_next: "Ana",
        lately: [],
      },
    });
    await expect(book(page).getByText("Nobody yet")).toBeVisible();
    await page.getByRole("button", { name: "Sang", exact: true }).click();
    const ana = book(page).getByRole("list", { name: "Guests" }).getByRole("button", { name: /Ana/ });
    await expect(ana).toContainText("Obsesión");
    await expect(ana).toContainText("tonight");

    await ana.click();
    const form = book(page).getByRole("form", { name: "About Ana" });
    // Rust's sentences, not the interface's own.
    await expect(form.getByRole("checkbox", { name: /Keep my details after tonight/ })).not.toBeChecked();
    const contact = form.getByRole("checkbox", { name: /Keep my email, phone or WhatsApp/ });
    await expect(form.getByRole("textbox", { name: "Email" })).toBeDisabled();
    await contact.check();
    await form.getByRole("textbox", { name: "Email" }).fill("ana@example.org");
    await form.getByRole("textbox", { name: "Phone or WhatsApp" }).fill("+34 600 123 456");
    await form.getByRole("textbox", { name: "Favourite band" }).fill("Aventura");
    await form.getByRole("textbox", { name: "Native language" }).fill("Spanish");
    await form.getByRole("textbox", { name: "Other languages" }).fill("English, Catalan");
    await expect(form.getByRole("list", { name: "Songs Ana sang" })).toContainText("Obsesión");
    await form.getByRole("button", { name: "Keep" }).click();

    await expect(form).toHaveCount(0);
    await expect(ana).toContainText("contact");
    const saved = (await guestCalls(page)).filter((c) => c.cmd === "guests_save") as unknown as {
      guest: { email: string; languages: string[]; favourite_band: string; consent: { contact: boolean; keep: boolean } };
    }[];
    expect(saved.at(-1)!.guest).toMatchObject({
      email: "ana@example.org",
      favourite_band: "Aventura",
      languages: ["English", "Catalan"],
      consent: { contact: true, keep: false },
    });

    // Contact taken back: what was typed is not sent as kept.
    await ana.click();
    await book(page).getByRole("form", { name: "About Ana" }).getByRole("checkbox", { name: /Keep my email/ }).uncheck();
    await book(page).getByRole("form", { name: "About Ana" }).getByRole("button", { name: "Keep" }).click();
    await expect(ana).not.toContainText("contact");
    await ana.click();
    await expect(book(page).getByRole("form", { name: "About Ana" }).getByRole("textbox", { name: "Email" })).toHaveValue("");
    expect(errorsThrown(page)).toEqual([]);
  });

  test("under sixteen, contact and voice cannot be ticked", async ({ page }) => {
    await singers(page);
    await book(page).getByRole("button", { name: "Add a guest" }).click();
    const form = book(page).getByRole("form", { name: "A new guest" });
    await form.getByRole("textbox", { name: "Name" }).fill("Kid");
    await form.getByRole("spinbutton", { name: "Age" }).fill("14");
    await expect(form.getByRole("checkbox", { name: /Keep my email/ })).toBeDisabled();
    await expect(form.getByRole("checkbox", { name: /Record about fifteen seconds/ })).toBeDisabled();
    await expect(form.getByRole("checkbox", { name: /Keep my details/ })).toBeEnabled();
    await expect(form.getByText(/need a parent's agreement/)).toBeVisible();
    await form.getByRole("spinbutton", { name: "Age" }).fill("16");
    await expect(form.getByRole("checkbox", { name: /Keep my email/ })).toBeEnabled();
    expect(errorsThrown(page)).toEqual([]);
  });

  test("a guest gets their own copy, and can be forgotten", async ({ page }) => {
    await singers(page);
    await book(page).getByRole("button", { name: "Add a guest" }).click();
    let form = book(page).getByRole("form", { name: "A new guest" });
    await form.getByRole("textbox", { name: "Name" }).fill("Bo");
    await form.getByRole("checkbox", { name: /Keep my details/ }).check();
    await form.getByRole("button", { name: "Keep" }).click();
    const bo = book(page).getByRole("list", { name: "Guests" }).getByRole("button", { name: /Bo/ });
    await expect(bo).not.toContainText("tonight");

    await page.evaluate(() => {
      (window as unknown as { __dialogAnswer?: string }).__dialogAnswer = "/home/dj/Bo.json";
    });
    await bo.click();
    form = book(page).getByRole("form", { name: "About Bo" });
    await form.getByRole("button", { name: "Their copy" }).click();
    await expect
      .poll(async () => (await guestCalls(page)).find((c) => c.cmd === "guests_export"))
      .toMatchObject({ cmd: "guests_export", path: "/home/dj/Bo.json" });

    await form.getByRole("button", { name: "Forget…" }).click();
    await form.getByRole("button", { name: "Forget Bo and their recordings" }).click();
    await expect(book(page).getByText("Nobody yet")).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **The voice, only once agreed and kept.** From the up-next card to the
   * singer's record; no record button until voice is agreed and saved; the
   * take shows while it runs, then waits on their record for the song, and
   * lands on it when the song is marked as sung.
   */
  /**
   * A song of their own: offered only once they agreed to it and it is kept;
   * the DJ's language and ideas go with the request, and what comes back --
   * Rust's own reading of an answer -- is shown per language, ready to copy.
   */
  test("a guest who agreed gets the words of a song of their own, ready to copy", async ({ page, context }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await singers(page, {
      karaoke_rotation: {
        singers: [{ name: "Ana", songs: [{ title: "Obsesión", track: null, path: null, key: 0 }], turns: 0 }],
        up_next: "Ana",
        lately: [],
      },
    });
    await page.getByRole("button", { name: "Sang", exact: true }).click();
    const ana = book(page).getByRole("list", { name: "Guests" }).getByRole("button", { name: /Ana/ });
    await ana.click();
    let form = book(page).getByRole("form", { name: "About Ana" });
    const song = form.getByRole("group", { name: "A song for Ana" });
    await expect(song).toHaveCount(0);

    // Agreed, but not yet kept: still nothing.
    await form.getByRole("checkbox", { name: /Record about fifteen seconds/ }).check();
    await page.waitForTimeout(300);
    await expect(song).toHaveCount(0);
    await form.getByRole("textbox", { name: "Native language" }).fill("Catalan");
    await form.getByRole("button", { name: "Keep" }).click();

    await ana.click();
    form = book(page).getByRole("form", { name: "About Ana" });
    await expect(song).toContainText("Obsesión");
    await song.getByRole("textbox", { name: "Sung in" }).fill("es");
    await song.getByRole("textbox", { name: "Your ideas" }).fill("first time on stage");
    await song.getByRole("button", { name: "Write the words" }).click();

    await expect(song.locator("[data-style]")).toHaveText(guestSong.style);
    for (const version of guestSong.versions) {
      await expect(song.getByRole("region", { name: `Words in ${version.language}` })).toContainText(
        version.lyrics.split("\n")[1],
      );
    }
    const asked = (await guestCalls(page)).filter((c) => c.cmd === "guests_song") as unknown as {
      language: string;
      keywords: string;
      date: string;
    }[];
    expect(asked).toHaveLength(1);
    expect(asked[0]).toMatchObject({ language: "es", keywords: "first time on stage" });
    expect(asked[0].date).toMatch(/\d/);

    await song.getByRole("button", { name: "Copy the Spanish words" }).click();
    await expect(song.getByRole("button", { name: "Copied" })).toBeVisible();
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(guestSong.versions[0].lyrics);
    await expect(song.getByRole("button", { name: "Write them again" })).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });

  test("the up-next singer's voice is recorded only once they have agreed", async ({ page }) => {
    await singers(page, {
      karaoke_rotation: {
        singers: [{ name: "Ana", songs: [{ title: "Obsesión", track: null, path: null, key: 0 }], turns: 0 }],
        up_next: "Ana",
        lately: [],
      },
    });
    await page.getByRole("region", { name: "Up next" }).getByRole("button", { name: "Guest book" }).click();
    let form = book(page).getByRole("form", { name: "A new guest" });
    await expect(form.getByRole("textbox", { name: "Name" })).toHaveValue("Ana");
    const record = book(page).getByRole("button", { name: /^Record 15 seconds of their voice$/ });
    await expect(record).toHaveCount(0);

    // Written down agreeing to nothing, then asked: ticked but not yet
    // kept, there is still nothing to press -- the button reads what Rust
    // holds, not the form.
    await form.getByRole("button", { name: "Keep" }).click();
    await page.getByRole("region", { name: "Up next" }).getByRole("button", { name: "Guest book" }).click();
    form = book(page).getByRole("form", { name: "About Ana" });
    await form.getByRole("checkbox", { name: /Record about fifteen seconds/ }).check();
    await page.waitForTimeout(300);
    await expect(record).toHaveCount(0);
    await form.getByRole("button", { name: "Keep" }).click();

    await page.getByRole("region", { name: "Up next" }).getByRole("button", { name: "Guest book" }).click();
    form = book(page).getByRole("form", { name: "About Ana" });
    await record.click();
    await expect(form.getByRole("status")).toHaveText(/Recording Ana's voice, 15 seconds/);
    await expect(form.getByText(/it goes on the song once it is marked as sung/)).toBeVisible({ timeout: 5000 });
    expect((await guestCalls(page)).filter((c) => c.cmd === "guests_voice")).toHaveLength(1);

    await form.getByRole("button", { name: "Cancel" }).click();
    await page.getByRole("button", { name: "Sang", exact: true }).click();
    await book(page).getByRole("list", { name: "Guests" }).getByRole("button", { name: /Ana/ }).click();
    await expect(
      book(page).getByRole("list", { name: "Songs Ana sang" }).getByText("their voice"),
    ).toBeVisible();
    expect(errorsThrown(page)).toEqual([]);
  });
});
