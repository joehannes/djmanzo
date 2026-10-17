/**
 * A visitor's night on the island, end to end: the MC's questions change the
 * island, the box office sells a (demo) licence at a regional price it has
 * to prove, and djmanzo's device link is approved on the /link page.
 */
import { expect, test } from "@playwright/test";

test("the MC's answers repaint the island and pick the edition", async ({ page }) => {
  await page.goto("/");
  const mc = page.getByRole("complementary", { name: "The MC" });
  await expect(mc).toContainText("Bienvenido");
  await mc.getByRole("textbox", { name: "Your name" }).fill("DJ Rosa");
  await mc.getByRole("button", { name: "That's me" }).click();
  await mc.getByRole("button", { name: /I host karaoke/ }).click();
  const before = await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--secondary"));
  await mc.getByRole("button", { name: /Bachata/ }).click();
  // Bachata's palette, mixed by the hour: the secondary is Bachata Rosa's gold.
  await expect.poll(() => page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--secondary").trim())).toBe("#f6c453");
  expect(before.trim()).not.toBe("#f6c453");
  await expect(mc).toContainText("Bachata Rosa");
  await mc.getByRole("button", { name: "Done" }).click();
  for (let i = 0; i < 3; i++) await mc.getByRole("button", { name: "Skip" }).first().click();
  await mc.getByRole("button", { name: "Not now" }).click();
  await expect(mc).toContainText("Karaoke");
  await mc.getByRole("button", { name: "Go to the box office" }).click();
  const sleeve = page.getByRole("region", { name: "La Taquilla" });
  await expect(sleeve).toBeVisible();
  // The MC's pick is marked on the Karaoke sleeve.
  await expect(sleeve.getByText("The MC's pick for you")).toBeVisible();
});

test("a regional price must be proven, and a licence comes out", async ({ page }) => {
  await page.goto("/#/taquilla");
  const sleeve = page.getByRole("region", { name: "La Taquilla" });
  await expect(sleeve.getByRole("heading", { name: "Pick your edition" })).toBeVisible();
  await sleeve.locator(".slot", { has: page.getByRole("heading", { name: "Pro DJ", exact: true }) }).getByRole("button", { name: "Choose" }).click();
  await sleeve.getByRole("combobox").first().selectOption("DO");
  await expect(sleeve.getByText("−40 %").first()).toBeVisible();
  await sleeve.getByRole("textbox", { name: "Your DJ name" }).fill("Tester");
  await sleeve.getByRole("button", { name: "Sign in for the demo" }).click();
  await expect(sleeve.getByText("Signed in as")).toBeVisible();
  // A US card with a Dominican address does not prove the Dominican price.
  await sleeve.getByRole("combobox", { name: "Card issued in" }).selectOption("US");
  await sleeve.getByRole("button", { name: /Rehearse paying/ }).click();
  await expect(sleeve.getByRole("alert")).toContainText("the higher of the two prices applies");
  await sleeve.getByRole("button", { name: "Not now" }).click();
  await sleeve.getByRole("combobox", { name: "Card issued in" }).selectOption("DO");
  await sleeve.getByRole("button", { name: /Rehearse paying/ }).click();
  await expect(sleeve.getByRole("heading", { name: "It's yours." })).toBeVisible();
  await expect(sleeve.getByText("demo licence")).toBeVisible();
});

test("djmanzo's code is approved on the /link page", async ({ page, request }) => {
  const start = await (await request.post("/api/device/start", { data: { machine: "Booth laptop" } })).json();
  await page.goto("/#/pase");
  const pass = page.getByRole("region", { name: "Tu Pase" });
  await pass.getByRole("textbox", { name: "Your DJ name" }).fill("Linker");
  await pass.getByRole("button", { name: "Sign in for the demo" }).click();
  await page.goto(`/link?code=${start.user_code}`);
  const link = page.getByRole("region", { name: "Tu Pase" });
  await expect(link.getByText("Booth laptop")).toBeVisible();
  await link.getByRole("button", { name: "Yes — link it" }).click();
  await expect(link.getByText("Linked.")).toBeVisible();
  const token = await (await request.post("/api/device/token", { data: { device_code: start.device_code } })).json();
  expect(token.access_token).toMatch(/^djm_/);
  expect(token.user.name).toBe("Linker");
});

test("djmanzo's loopback: the DJ says yes and the browser carries the answer back", async ({ page, request }) => {
  const verifier = "loopback-verifier-0123456789-abcdefghijklmnop";
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier)));
  const challenge = Buffer.from(digest).toString("base64url");
  const port = 53682;
  let back: URL | null = null;
  // djmanzo's own port, played by the test: record where the browser was sent.
  await page.route(`http://127.0.0.1:${port}/**`, (route) => {
    back = new URL(route.request().url());
    return route.fulfill({ status: 200, contentType: "text/html", body: "<p>djmanzo is linked</p>" });
  });
  await page.goto(`/link?port=${port}&challenge=${challenge}&state=state-0123456789&machine=Booth%20laptop`);
  const pass = page.getByRole("region", { name: "Tu Pase" });
  await expect(pass.getByText("Booth laptop").first()).toBeVisible();
  await pass.getByRole("textbox", { name: "Your DJ name" }).fill("Loop");
  await pass.getByRole("button", { name: "Sign in for the demo" }).click();
  await pass.getByRole("button", { name: "Yes — link it" }).click();
  await expect.poll(() => back?.searchParams.get("state")).toBe("state-0123456789");
  const token = await (await request.post("/api/device/exchange", { data: { code: back!.searchParams.get("code"), verifier } })).json();
  expect(token.access_token).toMatch(/^djm_/);
  expect(token.user.name).toBe("Loop");
});

test("the phone gets the same island", async ({ browser }) => {
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  await page.goto("/");
  await expect(page.getByRole("navigation", { name: /Hot cues/ })).toBeVisible();
  // Nothing wider than the screen.
  const overflow = await page.evaluate(() => [...document.querySelectorAll("header *, nav *")].filter((e) => e.getBoundingClientRect().right > innerWidth + 1).length);
  expect(overflow).toBe(0);
  await page.getByRole("button", { name: /2: La Cabina/ }).click();
  await expect(page.getByRole("region", { name: "La Cabina" })).toBeVisible();
});
