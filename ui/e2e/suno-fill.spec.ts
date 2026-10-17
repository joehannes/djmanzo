/**
 * Karaoke beta B5: the script that fills Suno's form
 * (`crates/dj-app/src/suno_fill.js`, run by `dj_app::suno::suno_fill`),
 * against a stand-in for Suno's Create page.
 *
 * Suno's real page cannot be reached or signed in to from here, so this
 * holds the script's own rules on a page built like one: Custom mode
 * pressed when the words box is not showing; each box filled the way typing
 * fills it, so a page that only listens for input events (as React does)
 * takes the value; what it could not find said on the page; and Create never
 * pressed. Whether today's suno.com still names its boxes so is not
 * something a test here can know.
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test, type Page } from "@playwright/test";

const FILL = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "../../crates/dj-app/src/suno_fill.js"),
  "utf8",
);

const WORDS = { lyrics: 'Rosa de la mañana\n"bachata" para ti', style: "bachata, romantic guitar", title: "A song for Ana" };

/**
 * A page like Suno's Create: Simple mode first, Custom showing the words box;
 * a value set straight on a box is lost, as with React — only the native
 * setter and an input event keep it.
 */
async function suno(page: Page, { style = true }: { style?: boolean } = {}) {
  await page.setContent(`
    <main>
      <button id="simple">Simple</button><button id="custom">Custom</button>
      <div id="form"></div>
      <button id="create">Create</button>
    </main>`);
  await page.evaluate((style) => {
    const w = window as unknown as { __heard: Record<string, string>; __created: number };
    w.__heard = {};
    w.__created = 0;
    document.getElementById("create")!.addEventListener("click", () => (w.__created += 1));
    const controlled = (el: HTMLInputElement | HTMLTextAreaElement, key: string) => {
      // As React does: an own `value` that drops what is set on it directly.
      Object.defineProperty(el, "value", { configurable: true, get: () => el.getAttribute("data-v") ?? "", set: () => {} });
      el.addEventListener("input", (e) => {
        const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
        const real = Object.getOwnPropertyDescriptor(proto, "value")!.get!.call(e.target);
        w.__heard[key] = real;
        el.setAttribute("data-v", real);
      });
    };
    document.getElementById("custom")!.addEventListener("click", () => {
      const form = document.getElementById("form")!;
      form.innerHTML = `
        <textarea placeholder="Write some lyrics or a prompt"></textarea>
        ${style ? '<textarea placeholder="Style of Music"></textarea>' : ""}
        <input placeholder="Song Title (Optional)">`;
      form.querySelectorAll("textarea, input").forEach((el, i) =>
        controlled(el as HTMLTextAreaElement, ["lyrics", style ? "style" : "title", "title"][i]),
      );
    });
  }, style);
}

const run = (page: Page, overwrite = true, more: Record<string, unknown> = {}) =>
  page.evaluate(
    ({ data, script }) => {
      (window as unknown as { __djmanzoSuno: unknown }).__djmanzoSuno = data;
      // eslint-disable-next-line no-eval
      (0, eval)(script);
    },
    { data: { ...WORDS, overwrite, ...more }, script: FILL },
  );

const heard = (page: Page) => page.evaluate(() => (window as unknown as { __heard: Record<string, string> }).__heard);

test("fills Suno's Custom form the way typing would, says so, and never presses Create", async ({ page }) => {
  await suno(page);
  await run(page);
  await expect.poll(() => heard(page)).toEqual({ lyrics: WORDS.lyrics, style: WORDS.style, title: WORDS.title });
  await expect(page.getByRole("status")).toHaveText("djmanzo filled in the words and the style. Check them, then press Create.");
  await expect(page.getByRole("status")).toHaveCSS("pointer-events", "none");
  expect(await page.evaluate(() => (window as unknown as { __created: number }).__created)).toBe(0);
});

test("says what it could not find, and fills the rest", async ({ page }) => {
  await suno(page, { style: false });
  await run(page);
  await expect.poll(() => heard(page)).toMatchObject({ lyrics: WORDS.lyrics });
  await expect(page.getByRole("status")).toContainText("could not find the style box");
  expect(await page.evaluate(() => (window as unknown as { __created: number }).__created)).toBe(0);
});

/** A fill that waited for the form (through Suno's sign-in) leaves a box the DJ wrote in alone. */
test("a fill that waited leaves what the DJ wrote, and fills the rest", async ({ page }) => {
  await suno(page);
  await page.click("#custom");
  await page.evaluate(() => document.querySelector("textarea")!.setAttribute("data-v", "the DJ's own words"));
  await run(page, false);
  await expect.poll(() => heard(page)).toMatchObject({ style: WORDS.style });
  expect((await heard(page)).lyrics, "the DJ's words kept").toBeUndefined();
  await expect(page.getByRole("status")).toHaveText("djmanzo filled in the style. Check them, then press Create.");
  // A press of the DJ's own puts the words in.
  await run(page, true);
  await expect.poll(async () => (await heard(page)).lyrics).toBe(WORDS.lyrics);
});

/** No form at all — Suno's sign-in page: what the DJ does next, as this window allows. */
test("with no form to fill, says what to do, as the window allows", async ({ page }) => {
  await page.setContent("<main><h1>Welcome to Suno</h1><button>Continue with Google</button></main>");
  await run(page, false, { waits: true, tries: 1 });
  await expect(page.getByRole("status")).toHaveText(
    "djmanzo is waiting for Suno's song form: sign in and open Create, and djmanzo fills it in then.",
  );
  await run(page, true, { waits: false, tries: 1 });
  await expect(page.getByRole("status")).toContainText("press \u201cFill in Suno's form\u201d again in djmanzo");
});
