/**
 * §118d: the DJ's own press kit.
 *
 * > the DJ can accumulate his personal stuff in a orchestrated way and on
 * > the occasion has easy and direct access to many share mechanisms that
 * > are prepackaged/optimised for typical practical occasions
 *
 * What each occasion says, what the kit lacks for it, what is refused, and
 * the addresses a way opens are `dj_app::kit`'s, tested there (`kit.json`,
 * Rust's own answers). What the browser holds: the occasions are Rust's
 * words with Rust's ways; the card carries the QR code; an enquiry for a
 * kind of night asks Rust for that night's answer; a way is sent as itself
 * and copying puts the words on the clipboard; an edit is kept whole; a
 * half-typed address waits and a refused one says Rust's sentence only
 * while it stands; photos go in and out by the dialog; and a first kit
 * says it was begun from the welcome.
 */
import { expect, test, type Page } from "@playwright/test";

import kit from "./kit.json" with { type: "json" };
import { errorsThrown, openShell } from "./shell";

const PANEL = '.surface[data-surface="kit"]';

const calls = (page: Page) =>
  page.evaluate(() => ((window as unknown as { __kitCalls?: unknown[] }).__kitCalls ?? []) as Record<string, unknown>[]);

async function withKit(page: Page, answers: Record<string, unknown> = {}) {
  await page.setViewportSize({ width: 1400, height: 900 });
  await openShell(page, "/", {}, {
    cockpit_workspace: {
      workspace: {
        name: "Kit",
        about: "",
        surfaces: [{ surface: "kit", dock: "right", order: 0, size: null, collapsed: false, pinned: false }],
        density: "standard",
        focus: "preparing",
        theme: "",
        decks: 2,
        locked: [],
      },
      notes: [],
      permits: { rearrange: true, resize: true, retheme: true, restyle: true },
    },
    ...answers,
  });
  const panel = page.locator(PANEL);
  await expect(panel.locator(".kit")).toBeVisible();
  return panel;
}

test.describe("§118d: the press kit", () => {
  /**
   * **The load-bearing one.** Every occasion is Rust's words, with Rust's
   * ways; the card carries the contact's QR code; each way is sent as itself
   * and copying puts the very words on the clipboard.
   */
  test("the occasions are Rust's words, and each way is sent as itself", async ({ page, context }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    const panel = await withKit(page);
    for (const composed of kit.full.occasions) {
      const card = panel.locator(`[data-occasion="${composed.occasion}"]`);
      await expect(card.locator("header strong")).toHaveText(composed.title);
      await expect(card.locator(".text")).toHaveText(composed.text);
      expect(await card.locator(".ways button").evaluateAll((all) => all.map((b) => b.getAttribute("data-way")))).toEqual(
        composed.ways.map((w) => w.way),
      );
    }
    await expect(panel.locator('[data-occasion="card"] .qr svg')).toBeVisible();
    await expect(panel.locator('[data-occasion="page"] .missing')).toContainText("Still to add: a photo.");

    await panel.locator('[data-occasion="card"] [data-way="whatsapp"]').click();
    await expect
      .poll(async () => (await calls(page)).find((c) => c.cmd === "kit_send"))
      .toMatchObject({ cmd: "kit_send", occasion: "card", way: "whatsapp", night: null });

    await panel.locator('[data-occasion="card"] [data-way="copy"]').click();
    await expect(panel.locator('[data-occasion="card"] [data-way="copy"]')).toHaveText("Copied");
    const card = kit.full.occasions.find((o) => o.occasion === "card")!;
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(card.text);

    await panel.locator('[data-occasion="page"] [data-way="save"]').click();
    await expect(panel.locator('[data-occasion="page"] .saved')).toContainText("press-kit.html");
    expect(errorsThrown(page)).toEqual([]);
  });

  /** An enquiry for a kind of night is Rust's answer for that night. */
  test("an enquiry for a kind of night is that night's own answer", async ({ page }) => {
    const panel = await withKit(page);
    const enquiry = panel.locator('[data-occasion="enquiry"]');
    await enquiry.getByRole("combobox", { name: "The kind of night asked about" }).selectOption("wedding");
    await expect(enquiry.locator(".text")).toHaveText(kit.wedding.text);
    expect((await calls(page)).find((c) => c.cmd === "kit_compose")).toMatchObject({ occasion: "enquiry", night: "wedding" });
    await enquiry.locator('[data-way="email"]').click();
    await expect
      .poll(async () => (await calls(page)).find((c) => c.cmd === "kit_send"))
      .toMatchObject({ occasion: "enquiry", night: "wedding", way: "email" });
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * An edit is kept whole; a half-typed address waits with a hint rather
   * than being refused, and one Rust refuses says Rust's sentence only while
   * it stands.
   */
  test("an edit is kept whole, and a refusal lasts only while it stands", async ({ page }) => {
    const panel = await withKit(page);
    await panel.getByRole("tab", { name: "Your kit" }).click();
    await panel.getByLabel("In one line").fill("Disco at dawn");
    await expect
      .poll(async () => (await calls(page)).filter((c) => c.cmd === "kit_save").at(-1)?.kit)
      .toMatchObject({ ...kit.full.kit, tagline: "Disco at dawn" });

    const email = panel.getByLabel("E-mail", { exact: true });
    const phone = panel.getByLabel("Phone", { exact: true });
    await email.fill("rosa@");
    await expect(panel.getByRole("alert")).toHaveText("An e-mail address reads name@example.com.");
    await page.waitForTimeout(600);
    expect((await calls(page)).filter((c) => c.cmd === "kit_save").some((c) => (c.kit as { email: string }).email === "rosa@")).toBe(false);
    await email.fill("rosa@example.org");
    await expect(panel.getByRole("alert")).toHaveCount(0);

    // A phone with words in it is not held -- nothing about it is half
    // typed -- so it goes to Rust, which refuses it in its own sentence.
    await phone.fill(kit.refused.phone);
    await expect(panel.getByRole("alert")).toHaveText(kit.refused.message);
    // Another edit, held and so never sent: the refusal was about the kit
    // as it was, and no longer stands -- what is said is the new hint.
    await email.fill("rosa@");
    await expect(panel.getByRole("alert")).toHaveText("An e-mail address reads name@example.com.");
    // Put right, the phone still wrong: Rust says so again.
    await email.fill("rosa@example.org");
    await expect(panel.getByRole("alert")).toHaveText(kit.refused.message);
    await phone.fill("+43 660 765 4321");
    await expect(panel.getByRole("alert")).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /** Photos go in by the dialog and come out by their own button. */
  test("a photo goes in by the dialog and out by its own button", async ({ page }) => {
    const panel = await withKit(page);
    await panel.getByRole("tab", { name: "Your kit" }).click();
    await page.evaluate(() => {
      (window as unknown as { __dialogAnswer?: string }).__dialogAnswer = "/home/dj/photos/rosa on stage.jpg";
    });
    await panel.getByRole("button", { name: "+ A photo" }).click();
    const photo = panel.locator('[data-photo="rosa on stage.jpg"]');
    await expect(photo).toBeVisible();
    expect((await calls(page)).find((c) => c.cmd === "kit_add")).toMatchObject({ kind: "photo", path: "/home/dj/photos/rosa on stage.jpg" });
    // Served on the kit's own scheme, in either of its platform spellings.
    await expect(photo.locator("img")).toHaveAttribute(
      "src",
      /^(kit:\/\/localhost|http:\/\/kit\.localhost)\/rosa%20on%20stage\.jpg\?v=\d+$/,
    );
    await photo.getByRole("button", { name: "Remove" }).click();
    await expect(photo).toHaveCount(0);
    expect((await calls(page)).find((c) => c.cmd === "kit_forget")).toMatchObject({ kind: "photo", file: "rosa on stage.jpg" });
    expect(errorsThrown(page)).toEqual([]);
  });

  /** A first kit is begun from the welcome, and says so. */
  test("a first kit is begun from the welcome", async ({ page }) => {
    const panel = await withKit(page, { kit_view: kit.fresh });
    await expect(panel.locator(".begun")).toBeVisible();
    await expect(panel.locator('[data-occasion="card"] .missing')).toContainText(
      `Still to add: ${kit.fresh.occasions.find((o) => o.occasion === "card")!.missing.join(", ")}.`,
    );
    await expect(panel.locator('[data-occasion="card"] .qr')).toHaveCount(0);
    // What is missing is one press away.
    await panel.locator('[data-occasion="card"] .missing').getByRole("button", { name: "Add it" }).click();
    await expect(panel.getByRole("tab", { name: "Your kit" })).toHaveAttribute("aria-selected", "true");
    await expect(panel.getByLabel("Name", { exact: true })).toHaveValue(kit.fresh.kit.name);
    expect(errorsThrown(page)).toEqual([]);
  });
});
