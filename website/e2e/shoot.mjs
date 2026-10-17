// Screenshots of the island at a given size and address, for looking at, not asserting.
import { chromium } from "@playwright/test";
const [url = "http://127.0.0.1:8787/", out = "shot.png", w = "1400", h = "900", script = ""] = process.argv.slice(2);
const browser = await chromium.launch({ executablePath: process.env.DJMANZO_CHROMIUM || undefined, args: ["--use-gl=swiftshader", "--enable-webgl", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: +w, height: +h }, deviceScaleFactor: 1 });
const logs = [];
page.on("console", (m) => logs.push(`${m.type()}: ${m.text()}`));
page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
await page.goto(url, { waitUntil: "networkidle" });
await page.waitForTimeout(1500);
if (script) {
  for (const step of script.split(";;")) {
    const [kind, ...rest] = step.split("::");
    const arg = rest.join("::");
    if (kind === "click") await page.getByRole("button", { name: arg }).first().click();
    else if (kind === "clicktext") await page.getByText(arg).first().click();
    else if (kind === "key") await page.keyboard.press(arg);
    else if (kind === "type") await page.keyboard.type(arg);
    else if (kind === "fill") { const [sel, val] = arg.split("=>"); await page.locator(sel).first().fill(val); }
    else if (kind === "wait") await page.waitForTimeout(+arg);
    else if (kind === "eval") await page.evaluate(arg);
  }
  await page.waitForTimeout(1200);
}
await page.screenshot({ path: out });
console.log(logs.filter((l) => !l.includes("Download the")).join("\n"));
await browser.close();
