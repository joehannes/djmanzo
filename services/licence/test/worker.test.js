import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { handle } from "../src/index.js";

const devKey = JSON.parse(readFileSync(new URL("./dev-key.json", import.meta.url)));
const NOW = 1_792_000_000;

function kv() {
  const map = new Map();
  return { map, get: async (k) => map.get(k) ?? null, put: async (k, v) => void map.set(k, v), delete: async (k) => void map.delete(k) };
}
const env = () => ({ LICENCES: kv(), SIGNING_KEY: devKey.pkcs8, GITHUB_CLIENT_ID: "gh-id", GITHUB_CLIENT_SECRET: "gh-secret", LEMONSQUEEZY_SECRET: "whsec", VARIANTS: JSON.stringify({ 11: { edition: "pro" }, 12: { edition: "custom", modules: "karaoke" } }) });
const b64url = (bytes) => btoa(String.fromCharCode(...bytes)).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
const challengeOf = async (v) => b64url(new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(v))));
const post = (path, body, headers = {}) => new Request(`https://licence.example${path}`, { method: "POST", body: typeof body === "string" ? body : JSON.stringify(body), headers });

/** GitHub, answered from a script rather than the network. */
const github = async (url) => {
  if (String(url).includes("access_token")) return Response.json({ access_token: "tok" });
  if (String(url).endsWith("/user")) return Response.json({ id: 583231 });
  return new Response("", { status: 404 });
};

async function signIn(e, verifier = "v".repeat(50), trial) {
  const state = "s".repeat(20);
  const start = await handle(new Request(`https://licence.example/login/github?state=${state}&code_challenge=${await challengeOf(verifier)}&port=49152`), e, NOW, github);
  assert.equal(start.status, 302);
  assert.match(start.headers.get("location"), /^https:\/\/github\.com\/login\/oauth\/authorize\?client_id=gh-id/);
  const back = await handle(new Request(`https://licence.example/callback/github?code=abc&state=${state}`), e, NOW, github);
  const to = new URL(back.headers.get("location"));
  assert.equal(to.origin, "http://127.0.0.1:49152", "back to djmanzo on this machine only");
  assert.equal(to.pathname, "/", "at the root, where djmanzo's listener answers");
  const reply = await handle(post("/licence", { code: to.searchParams.get("code"), verifier, trial }), e, NOW, github);
  return { status: reply.status, body: await reply.json() };
}

test("signing in with no purchase says so, and a trial can be started once", async () => {
  const e = env();
  const first = await signIn(e);
  assert.equal(first.body.account, "github:583231");
  assert.match(first.body.refused, /no licence yet/);
  const trial = await signIn(e, "w".repeat(50), { edition: "full" });
  assert.match(trial.body.licence, /^djmanzo-licence-1\./);
  const again = await signIn(e, "x".repeat(50), { edition: "pro" });
  assert.match(again.body.refused, /had its trial/);
});

test("a grant is for the djmanzo that asked: the wrong verifier is refused", async () => {
  const e = env();
  const state = "t".repeat(20);
  await handle(new Request(`https://licence.example/login/github?state=${state}&code_challenge=${await challengeOf("right".repeat(10))}&port=50000`), e, NOW, github);
  const back = await handle(new Request(`https://licence.example/callback/github?code=abc&state=${state}`), e, NOW, github);
  const code = new URL(back.headers.get("location")).searchParams.get("code");
  const reply = await handle(post("/licence", { code, verifier: "wrong".repeat(10) }), e, NOW, github);
  assert.equal(reply.status, 400);
  const replay = await handle(post("/licence", { code, verifier: "right".repeat(10) }), e, NOW, github);
  assert.equal(replay.status, 400, "a grant is used once, right or wrong");
});

test("a paid order becomes a licence, a refund takes it back, a forged webhook does nothing", async () => {
  const e = env();
  const { body } = await signIn(e);
  const order = (name, variant) => JSON.stringify({ meta: { event_name: name, custom_data: { account: "github:583231" } }, data: { id: 9001, attributes: { first_order_item: { variant_id: variant } } } });
  const signed = async (raw) => {
    const key = await crypto.subtle.importKey("raw", new TextEncoder().encode("whsec"), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
    return [...new Uint8Array(await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(raw)))].map((b) => b.toString(16).padStart(2, "0")).join("");
  };
  const forged = await handle(post("/webhook/lemonsqueezy", order("order_created", 11), { "x-signature": "00" }), e, NOW);
  assert.equal(forged.status, 401);

  const raw = order("order_created", 11);
  assert.equal((await handle(post("/webhook/lemonsqueezy", raw, { "x-signature": await signed(raw) }), e, NOW)).status, 200);
  const renewed = await (await handle(post("/renew", { renew: body.renew }), e, NOW)).json();
  const payload = JSON.parse(atob(renewed.licence.split(".")[1].replaceAll("-", "+").replaceAll("_", "/")));
  assert.equal(payload.edition, "pro");

  const refund = order("order_refunded", 11);
  await handle(post("/webhook/lemonsqueezy", refund, { "x-signature": await signed(refund) }), e, NOW);
  const after = await (await handle(post("/renew", { renew: body.renew }), e, NOW)).json();
  assert.match(after.refused, /no licence/);
});

test("a damaged sign-in link and an unknown renewal token are refused", async () => {
  const e = env();
  assert.equal((await handle(new Request("https://licence.example/login/github?state=x&code_challenge=y&port=80"), e, NOW)).status, 400);
  assert.equal((await handle(post("/renew", { renew: "nope" }), e, NOW)).status, 401);
  assert.equal((await handle(new Request("https://licence.example/login/myspace?state=x"), e, NOW)).status, 404);
});
