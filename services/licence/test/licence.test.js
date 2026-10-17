import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { DAY, editionOf, entitlement, sign, signingKey, startTrial } from "../src/licence.js";

const devKey = JSON.parse(readFileSync(new URL("./dev-key.json", import.meta.url)));
const NOW = 1_792_000_000;

test("writes the shared test vector", async () => {
  // The Rust side reads this file (dj_app::licence, a_licence_from_the_service_reads).
  // Ed25519 is deterministic, so the same payload always signs the same line;
  // `UPDATE=1 npm test` rewrites it after a deliberate format change.
  const payload = entitlement("github:583231", { purchases: [{ edition: "custom", modules: "karaoke,stems" }] }, NOW - 10_000);
  const line = await sign(payload, await signingKey(devKey.pkcs8));
  const path = new URL("./vector.txt", import.meta.url);
  if (process.env.UPDATE) writeFileSync(path, line);
  assert.equal(readFileSync(path, "utf8"), line);
  assert.equal(payload.edition, "custom");
  assert.equal(payload.modules, "stems,karaoke", "in djmanzo's order, whatever order they were bought in");
});

test("the signature verifies with the public key the application carries", async () => {
  const line = await sign({ v: 1, account: "google:1", edition: "full", issued: NOW, renew_by: NOW + DAY }, await signingKey(devKey.pkcs8));
  const [, body, sig] = line.match(/^djmanzo-licence-1\.([^.]+)\.([^.]+)$/);
  const bytes = (s) => Uint8Array.from(atob(s.replaceAll("-", "+").replaceAll("_", "/")), (c) => c.charCodeAt(0));
  const pub = await crypto.subtle.importKey("raw", bytes(devKey.public), { name: "Ed25519" }, false, ["verify"]);
  assert.ok(await crypto.subtle.verify({ name: "Ed25519" }, pub, bytes(sig), bytes(body)));
});

test("purchases add up to the edition they make", () => {
  assert.deepEqual(editionOf(["karaoke"]), { edition: "karaoke" });
  assert.deepEqual(editionOf(["assistant", "teacher"]), { edition: "apprentice" });
  assert.deepEqual(editionOf([]), { edition: "hobbyist" });
  assert.deepEqual(editionOf(["stems", "dvs"]), { edition: "custom", modules: "stems,dvs" });
  // Pro DJ bought, then karaoke and the teacher added: that is Full.
  const pro = ["four_decks", "stems", "autopilot", "assistant", "crowd", "controllers", "dvs", "video", "streaming", "plugins", "sources", "events", "replay"];
  assert.deepEqual(editionOf([...pro, "karaoke", "teacher"]), { edition: "full" });
});

test("a refunded purchase entitles nothing; a live one outranks a trial", () => {
  const refunded = { purchases: [{ edition: "pro", refunded: NOW }] };
  assert.equal(entitlement("google:1", refunded, NOW), null);
  const both = { purchases: [{ edition: "karaoke" }], trial: { edition: "full", ends: NOW + DAY } };
  const p = entitlement("google:1", both, NOW);
  assert.equal(p.edition, "karaoke");
  assert.equal(p.trial, undefined);
});

test("a trial is seven days of any edition, once, and renews by its end", () => {
  const { record } = startTrial(null, "full", NOW);
  assert.equal(record.trial.ends, NOW + 7 * DAY);
  const p = entitlement("google:1", record, NOW + DAY);
  assert.equal(p.edition, "full");
  assert.equal(p.trial, true);
  assert.equal(p.renew_by, NOW + 7 * DAY);
  assert.equal(entitlement("google:1", record, NOW + 7 * DAY), null, "over at its end");
  assert.match(startTrial(record, "pro", NOW + 8 * DAY).refused, /had its trial/);
  assert.match(startTrial({ purchases: [{ edition: "hobbyist" }] }, "full", NOW).refused, /already bought/);
  assert.match(startTrial(null, "platinum", NOW).refused, /no such edition/);
  const custom = startTrial(null, "custom", NOW, "dvs,nonsense,stems").record;
  assert.equal(entitlement("google:2", custom, NOW).modules, "stems,dvs");
});
