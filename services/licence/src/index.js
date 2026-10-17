// djmanzo's licence service: a Cloudflare Worker over one KV namespace.
//
// docs/EDITIONS.md ("One download, unlocked by signing in") and
// docs/adr/0011-licence-by-sign-in.md are the design. In short:
//
//   GET  /login/:provider     the application opens this in the system browser
//   GET  /callback/:provider  the provider sends the DJ back here
//   POST /licence             the application trades its one-time grant for a licence
//   POST /renew               ... or its renewal token, in the background
//   POST /signout             forgets a renewal token
//   POST /webhook/lemonsqueezy  the merchant of record says a payment cleared or was refunded
//
// KV keys:
//   account:<provider>:<id>  { purchases: [...], trial?: {...} }  -- the record
//   login:<state>            the application's PKCE challenge and port, 10 minutes
//   grant:<code>             a signed-in account waiting for the application, 2 minutes
//   renew:<token>            the account a renewal token belongs to
//
// Nothing here stores a password, an e-mail address or a payment detail: the
// provider holds the first two, the merchant of record the third.

import { entitlement, sign, signingKey, startTrial, base64url } from "./licence.js";

/** Sign-in providers: where to send the DJ, and how to learn who came back. */
export const PROVIDERS = {
  google: {
    authorize: "https://accounts.google.com/o/oauth2/v2/auth",
    token: "https://oauth2.googleapis.com/token",
    scope: "openid",
    who: async (tokens) => idTokenSubject(tokens.id_token),
  },
  microsoft: {
    authorize: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
    token: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    scope: "openid",
    who: async (tokens) => idTokenSubject(tokens.id_token),
  },
  apple: {
    authorize: "https://appleid.apple.com/auth/authorize",
    token: "https://appleid.apple.com/auth/token",
    // No scope, so Apple may answer by query string like everyone else; djmanzo
    // needs no name and no e-mail.
    scope: "",
    who: async (tokens) => idTokenSubject(tokens.id_token),
  },
  github: {
    authorize: "https://github.com/login/oauth/authorize",
    token: "https://github.com/login/oauth/access_token",
    scope: "",
    who: async (tokens, fetcher) => {
      const reply = await fetcher("https://api.github.com/user", {
        headers: { authorization: `Bearer ${tokens.access_token}`, "user-agent": "djmanzo-licence", accept: "application/json" },
      });
      return reply.ok ? String((await reply.json()).id ?? "") : "";
    },
  },
  discord: {
    authorize: "https://discord.com/oauth2/authorize",
    token: "https://discord.com/api/oauth2/token",
    scope: "identify",
    who: async (tokens, fetcher) => {
      const reply = await fetcher("https://discord.com/api/users/@me", {
        headers: { authorization: `Bearer ${tokens.access_token}` },
      });
      return reply.ok ? String((await reply.json()).id ?? "") : "";
    },
  },
};

/**
 * The subject of an ID token. Its signature is not checked, and need not be:
 * it came straight from the provider's token endpoint over TLS in answer to
 * this Worker's own client secret (OpenID Connect Core §3.1.3.7).
 */
function idTokenSubject(idToken) {
  try {
    const part = idToken.split(".")[1].replaceAll("-", "+").replaceAll("_", "/");
    return String(JSON.parse(atob(part)).sub ?? "");
  } catch {
    return "";
  }
}

const text = (body, status = 200) => new Response(body, { status, headers: { "content-type": "text/plain; charset=utf-8" } });
const json = (body, status = 200) => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

function random() {
  return base64url(crypto.getRandomValues(new Uint8Array(32)));
}

async function sha256url(value) {
  return base64url(new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value))));
}

async function readJson(kv, key) {
  const value = await kv.get(key);
  return value == null ? null : JSON.parse(value);
}

/** The licence for `account` now, or a refusal, signed with the store key. */
async function licenceFor(env, account, now, trial) {
  let record = await readJson(env.LICENCES, `account:${account}`);
  if (trial) {
    const started = startTrial(record, trial.edition, now, trial.modules);
    if (started.refused) return { refused: started.refused };
    record = started.record;
    await env.LICENCES.put(`account:${account}`, JSON.stringify(record));
  }
  const payload = entitlement(account, record, now);
  if (!payload) return { refused: record?.trial ? "the trial has ended" : "this account has no licence yet", trialUsed: Boolean(record?.trial) };
  return { licence: await sign(payload, await signingKey(env.SIGNING_KEY)) };
}

/** Handle one request. `now` and `fetcher` are passed in so the tests can. */
export async function handle(request, env, now = Math.floor(Date.now() / 1000), fetcher = fetch) {
  const url = new URL(request.url);
  const parts = url.pathname.split("/").filter(Boolean);

  // 1. The application opens this in the DJ's browser.
  if (request.method === "GET" && parts[0] === "login" && PROVIDERS[parts[1]]) {
    const provider = PROVIDERS[parts[1]];
    const state = url.searchParams.get("state") ?? "";
    const challenge = url.searchParams.get("code_challenge") ?? "";
    const port = Number(url.searchParams.get("port"));
    if (!/^[A-Za-z0-9_-]{16,128}$/.test(state) || !/^[A-Za-z0-9_-]{43}$/.test(challenge)) return text("That sign-in link is damaged.", 400);
    if (!Number.isInteger(port) || port < 1024 || port > 65535) return text("That sign-in link is damaged.", 400);
    await env.LICENCES.put(`login:${state}`, JSON.stringify({ challenge, port, provider: parts[1] }), { expirationTtl: 600 });
    const to = new URL(provider.authorize);
    to.searchParams.set("client_id", env[`${parts[1].toUpperCase()}_CLIENT_ID`] ?? "");
    to.searchParams.set("redirect_uri", `${url.origin}/callback/${parts[1]}`);
    to.searchParams.set("response_type", "code");
    if (provider.scope) to.searchParams.set("scope", provider.scope);
    to.searchParams.set("state", state);
    return Response.redirect(to.toString(), 302);
  }

  // 2. The provider sends the DJ back.
  if (request.method === "GET" && parts[0] === "callback" && PROVIDERS[parts[1]]) {
    const state = url.searchParams.get("state") ?? "";
    const login = await readJson(env.LICENCES, `login:${state}`);
    if (!login || login.provider !== parts[1]) return text("That sign-in has expired. Start again from djmanzo.", 400);
    await env.LICENCES.delete(`login:${state}`);
    const code = url.searchParams.get("code");
    if (!code) return text("The sign-in was cancelled. You can close this tab.", 400);
    const name = parts[1].toUpperCase();
    const reply = await fetcher(PROVIDERS[parts[1]].token, {
      method: "POST",
      headers: { "content-type": "application/x-www-form-urlencoded", accept: "application/json" },
      body: new URLSearchParams({
        grant_type: "authorization_code",
        code,
        redirect_uri: `${url.origin}/callback/${parts[1]}`,
        client_id: env[`${name}_CLIENT_ID`] ?? "",
        client_secret: env[`${name}_CLIENT_SECRET`] ?? "",
      }),
    });
    if (!reply.ok) return text("The sign-in could not be finished. Start again from djmanzo.", 502);
    const id = await PROVIDERS[parts[1]].who(await reply.json(), fetcher);
    if (!id) return text("The sign-in could not be finished. Start again from djmanzo.", 502);
    const grant = random();
    await env.LICENCES.put(`grant:${grant}`, JSON.stringify({ account: `${parts[1]}:${id}`, challenge: login.challenge }), { expirationTtl: 120 });
    // The root, where djmanzo's loopback listener answers (dj_app::youtube::callback).
    const back = new URL(`http://127.0.0.1:${login.port}/`);
    back.searchParams.set("code", grant);
    back.searchParams.set("state", state);
    return Response.redirect(back.toString(), 302);
  }

  if (request.method !== "POST") return text("Not here.", 404);

  // 3. The application trades its grant (and PKCE verifier) for a licence.
  if (parts[0] === "licence") {
    const body = await request.json().catch(() => ({}));
    const grant = await readJson(env.LICENCES, `grant:${body.code ?? ""}`);
    if (!grant) return json({ refused: "that sign-in has expired" }, 400);
    await env.LICENCES.delete(`grant:${body.code}`);
    if ((await sha256url(String(body.verifier ?? ""))) !== grant.challenge) return json({ refused: "that sign-in was not this djmanzo's" }, 400);
    const renew = random();
    await env.LICENCES.put(`renew:${renew}`, grant.account);
    const answer = await licenceFor(env, grant.account, now, body.trial);
    return json({ ...answer, renew, account: grant.account });
  }

  // 4. A renewal, in the background.
  if (parts[0] === "renew") {
    const body = await request.json().catch(() => ({}));
    const account = await env.LICENCES.get(`renew:${body.renew ?? ""}`);
    if (!account) return json({ refused: "sign in again" }, 401);
    return json({ ...(await licenceFor(env, account, now, body.trial)), account });
  }

  if (parts[0] === "signout") {
    const body = await request.json().catch(() => ({}));
    if (body.renew) await env.LICENCES.delete(`renew:${body.renew}`);
    return json({ ok: true });
  }

  // 5. The merchant of record: a payment cleared, or was refunded.
  if (parts[0] === "webhook" && parts[1] === "lemonsqueezy") return lemonSqueezy(request, env, now);

  return text("Not here.", 404);
}

/**
 * Lemon Squeezy's webhook. The body is signed with HMAC-SHA256 in `X-Signature`
 * (hex); the checkout carried the account as `custom_data.account`; each
 * product variant is an edition, or a Custom module, by `env.VARIANTS`
 * (`{"<variant id>": {"edition": "pro"} | {"edition": "custom", "modules": "stems"}}`).
 */
async function lemonSqueezy(request, env, now) {
  const raw = await request.text();
  const key = await crypto.subtle.importKey("raw", new TextEncoder().encode(env.LEMONSQUEEZY_SECRET ?? ""), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const mac = new Uint8Array(await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(raw)));
  const expected = [...mac].map((b) => b.toString(16).padStart(2, "0")).join("");
  const given = request.headers.get("x-signature") ?? "";
  if (!env.LEMONSQUEEZY_SECRET || !timingSafeEqual(expected, given)) return text("Bad signature.", 401);

  const event = JSON.parse(raw);
  const name = event?.meta?.event_name;
  const account = event?.meta?.custom_data?.account;
  const order = String(event?.data?.id ?? "");
  if (!account || !/^[a-z]+:[A-Za-z0-9._-]{1,128}$/.test(account)) return text("No account on that order.", 200);
  const record = (await readJson(env.LICENCES, `account:${account}`)) ?? { purchases: [] };
  record.purchases ??= [];

  if (name === "order_created") {
    const variant = String(event?.data?.attributes?.first_order_item?.variant_id ?? "");
    const sold = JSON.parse(env.VARIANTS ?? "{}")[variant];
    if (!sold) return text("Unknown variant.", 200);
    if (!record.purchases.some((p) => p.order === order)) record.purchases.push({ ...sold, order, at: now });
  } else if (name === "order_refunded") {
    for (const p of record.purchases) if (p.order === order) p.refunded = now;
  } else {
    return text("Ignored.", 200);
  }
  await env.LICENCES.put(`account:${account}`, JSON.stringify(record));
  return text("Kept.", 200);
}

function timingSafeEqual(a, b) {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}

export default {
  fetch: (request, env) => handle(request, env),
};
