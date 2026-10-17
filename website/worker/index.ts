/**
 * The djmanzo website's Worker: the API under `/api`, sign-in under
 * `/auth`, and the island (static assets) for everything else.
 */

import { CHEAPEST_WITH, EDITION_INFO, FOUNDATION, MARKET, MODULES, MODULE_PRICE, MODULE_TITLE, NAMED, PLANS, PLAN_INFO, PRO_MONTH, REASON, SEATS, TRIAL_DAYS, listPrice } from "../shared/catalog";
import { FACTS, STATUS_LINE } from "../shared/facts";
import { BAND_NAME, BAND_SHARE, STUDENT_SHARE, band, currencyFor } from "../shared/regions";
import type { AppEnv } from "./env";
import * as auth from "./auth";
import * as blog from "./blog";
import * as checkout from "./checkout";
import * as community from "./community";
import * as device from "./device";
import * as promo from "./promo";
import { keys } from "./licence";
import { HttpError, body, country, fail, json, text } from "./http";
import { get, list, put, saveLicence, type Licence, type Order, type Promo, type User, type Video } from "./store";

type Handler = (ctx: Ctx) => Promise<Response | unknown>;

interface Ctx {
  req: Request;
  env: AppEnv;
  url: URL;
  params: Record<string, string>;
  exec: ExecutionContext;
}

const routes: { method: string; pattern: RegExp; keys: string[]; handler: Handler }[] = [];

/**
 * What djmanzo itself calls, from no web page: the only mutating requests
 * that may arrive from another origin. Approving a link is a signed-in
 * browser's act on this site, so it is not among them.
 */
const APP_CALLS = ["/api/device/start", "/api/device/token", "/api/device/exchange", "/api/app/"];

function route(method: string, path: string, handler: Handler) {
  const keys: string[] = [];
  const pattern = new RegExp(
    "^" +
      path.replace(/:([a-zA-Z]+)/g, (_, k) => {
        keys.push(k);
        return "([^/]+)";
      }) +
      "/?$",
  );
  routes.push({ method, pattern, keys, handler });
}

const user = (c: Ctx) => auth.whoami(c.req, c.env);
const needUser = (c: Ctx) => auth.required(c.req, c.env);
const needAdmin = (c: Ctx) => auth.adminRequired(c.req, c.env);
const bearer = (c: Ctx) => (c.req.headers.get("authorization") || "").replace(/^Bearer /, "");

// --- Sign-in -----------------------------------------------------------------

route("GET", "/auth/start/:provider", (c) => auth.start(c.req, c.env, c.params.provider));
route("GET", "/auth/callback/:provider", (c) => auth.callback(c.req, c.env, c.params.provider));
route("POST", "/auth/callback/:provider", (c) => auth.callback(c.req, c.env, c.params.provider));
route("POST", "/api/auth/demo", async (c) => auth.demoSignIn(c.req, c.env, (await body<{ name?: string }>(c.req)).name ?? ""));
route("POST", "/api/auth/owner", async (c) => auth.ownerSignIn(c.req, c.env, (await body<{ token?: string }>(c.req)).token ?? ""));
route("POST", "/api/auth/signout", (c) => auth.signOut(c.req, c.env));

route("GET", "/api/me", async (c) => {
  const u = await user(c);
  return {
    user: u ? auth.publicUser(u) : null,
    admin: await auth.isAdmin(c.req, c.env, u),
    providers: auth.enabledProviders(c.env),
    demo: auth.demoMode(c.env),
  };
});

route("GET", "/api/me/licences", async (c) => {
  const u = await needUser(c);
  return (await device.appMe(c.env, u)).licences;
});
route("POST", "/api/me/licences/:lid/deactivate", async (c) => {
  const u = await needUser(c);
  return device.deactivate(c.env, u, c.params.lid, String((await body<{ machine?: string }>(c.req)).machine || ""));
});
route("GET", "/api/me/orders", async (c) => {
  const u = await needUser(c);
  const keys = await list<{ u: string }>(c.env.KV, "order:", 5000);
  const mine = keys.filter((k) => k.metadata?.u === u.id).map((k) => k.name);
  return (await Promise.all(mine.map((k) => get<Order>(c.env.KV, k)))).filter(Boolean);
});
route("GET", "/api/me/profile", async (c) => get(c.env.KV, `profile:${(await needUser(c)).id}`));
route("PUT", "/api/me/profile", async (c) => community.saveProfile(c.env, await needUser(c), await body(c.req, 9000)));

// --- The catalogue and a quote ---------------------------------------------

route("GET", "/api/catalog", async (c) => {
  const where = country(c.req);
  return {
    editions: EDITION_INFO,
    modules: MODULES.map((m) => ({ id: m, title: MODULE_TITLE[m], price: MODULE_PRICE[m], cheapest: CHEAPEST_WITH[m] })),
    foundation: FOUNDATION,
    plans: PLAN_INFO,
    prices: Object.fromEntries(NAMED.map((e) => [e, Object.fromEntries(PLANS.map((p) => [p, listPrice(e, p)]))])),
    reasons: REASON,
    seats: SEATS,
    facts: FACTS,
    status: STATUS_LINE,
    market: MARKET,
    proMonth: PRO_MONTH,
    bands: { share: BAND_SHARE, name: BAND_NAME },
    studentShare: STUDENT_SHARE,
    trialDays: TRIAL_DAYS,
    visitor: { country: where, band: band(where), currency: currencyFor(where) },
    pay: {
      demo: auth.demoMode(c.env),
      stripe: c.env.STRIPE_PUBLISHABLE_KEY && c.env.STRIPE_SECRET_KEY ? { key: c.env.STRIPE_PUBLISHABLE_KEY } : null,
      paypal: c.env.PAYPAL_CLIENT_ID && c.env.PAYPAL_CLIENT_SECRET ? { clientId: c.env.PAYPAL_CLIENT_ID } : null,
      transfer: !!c.env.BANK_TRANSFER_DETAILS,
    },
    major: Number(c.env.MAJOR_VERSION || 1),
    downloads: c.env.DOWNLOAD_BASE,
  };
});

route("POST", "/api/quote", async (c) => checkout.quote(c.env, await user(c), checkout.checkInput(await body(c.req))));

// --- Paying ------------------------------------------------------------------

route("POST", "/api/checkout/demo", async (c) => {
  const input = await body<Record<string, unknown>>(c.req);
  return checkout.demoCheckout(c.env, await needUser(c), {
    ...checkout.checkInput(input),
    instrument: text(input.instrument, 2).toUpperCase(),
    billing: text(input.billing, 2).toUpperCase(),
    acceptHigher: !!input.acceptHigher,
  });
});
route("POST", "/api/trial", async (c) => checkout.startTrial(c.env, await needUser(c), String((await body<{ edition?: string }>(c.req)).edition || "")));
route("GET", "/api/trial", async (c) => ({ had: await get(c.env.KV, `trial:${(await needUser(c)).id}`) }));
route("POST", "/api/checkout/free", async (c) => checkout.freeCheckout(c.env, await needUser(c), checkout.checkInput(await body(c.req))));
route("POST", "/api/redeem", async (c) => checkout.redeem(c.env, await needUser(c), String((await body<{ code?: string }>(c.req)).code || "")));
route("POST", "/api/checkout/stripe", async (c) => {
  const input = await body<Record<string, unknown>>(c.req);
  return checkout.stripeConfirm(c.env, await needUser(c), c.url.origin, {
    ...checkout.checkInput(input),
    paymentMethod: String(input.paymentMethod || ""),
    acceptHigher: !!input.acceptHigher,
  });
});
route("POST", "/api/checkout/stripe/finalize", async (c) => checkout.stripeFinalize(c.env, await needUser(c), String((await body<{ order?: string }>(c.req)).order || "")));
route("POST", "/api/webhooks/stripe", (c) => checkout.stripeWebhook(c.env, c.req));
route("POST", "/api/checkout/paypal", async (c) => checkout.paypalCreate(c.env, await needUser(c), checkout.checkInput(await body(c.req))));
route("POST", "/api/checkout/paypal/capture", async (c) => {
  const b = await body<{ order?: string; acceptHigher?: boolean }>(c.req);
  return checkout.paypalCapture(c.env, await needUser(c), String(b.order || ""), !!b.acceptHigher);
});
route("POST", "/api/checkout/transfer", async (c) => {
  const input = await body<Record<string, unknown>>(c.req);
  return checkout.transferCheckout(c.env, await needUser(c), { ...checkout.checkInput(input), billing: text(input.billing, 2).toUpperCase() });
});

// --- djmanzo, the application ------------------------------------------------

route("POST", "/api/device/start", async (c) => device.startLink(c.env, c.url.origin, String((await body<{ machine?: string }>(c.req)).machine || "")));
route("POST", "/api/device/token", async (c) => device.pollLink(c.env, String((await body<{ device_code?: string }>(c.req)).device_code || "")));
route("POST", "/api/device/loopback", async (c) => device.approveLoopback(c.env, await needUser(c), await body(c.req)));
route("POST", "/api/device/exchange", async (c) => {
  const b = await body<{ code?: string; verifier?: string }>(c.req);
  return device.exchangeLoopback(c.env, String(b.code || ""), String(b.verifier || ""));
});
route("GET", "/api/device/lookup", async (c) => {
  await needUser(c);
  return device.lookupLink(c.env, c.url.searchParams.get("code") || "");
});
route("POST", "/api/device/approve", async (c) => {
  const b = await body<{ code?: string; approve?: boolean }>(c.req);
  return device.decideLink(c.env, await needUser(c), String(b.code || ""), b.approve !== false);
});
route("GET", "/api/app/me", async (c) => device.appMe(c.env, await needUser(c)));
route("POST", "/api/app/activate", async (c) => {
  const b = await body<{ licence?: string; machine?: string; name?: string }>(c.req);
  return device.activate(c.env, await needUser(c), String(b.licence || ""), b.machine, b.name);
});
route("POST", "/api/app/deactivate", async (c) => {
  const b = await body<{ licence?: string; machine?: string }>(c.req);
  return device.deactivate(c.env, await needUser(c), String(b.licence || ""), String(b.machine || ""));
});
route("POST", "/api/app/signout", async (c) => device.revokeAppToken(c.env, bearer(c)));
route("GET", "/api/licence/public-key", async (c) => ({ alg: "Ed25519", key: (await keys(c.env)).publicRaw, development: !c.env.LICENCE_PRIVATE_KEY }));

// --- The community -------------------------------------------------------------

route("GET", "/api/wishes", async (c) => {
  c.exec.waitUntil(community.seedWishes(c.env));
  return community.wishes(c.env);
});
route("GET", "/api/wishes/:id", (c) => community.wish(c.env, c.params.id));
route("POST", "/api/wishes", async (c) => community.addWish(c.env, c.req, await user(c), await body(c.req)));
route("POST", "/api/wishes/:id/vote", async (c) => community.vote(c.env, c.req, await user(c), c.params.id));

route("GET", "/api/comments", (c) =>
  community.comments(c.env, (c.url.searchParams.get("kind") as never) || "comment", { station: c.url.searchParams.get("station"), limit: 60 }),
);
route("POST", "/api/comments", async (c) => community.addComment(c.env, c.req, await user(c), await body(c.req)));
route("GET", "/api/videos", async (c) => (await get<Video[]>(c.env.KV, "config:videos")) ?? []);

route("POST", "/api/journey/reward", async (c) => community.claimReward(c.env, await needUser(c), (await body<{ stamps?: string[] }>(c.req)).stamps ?? []));

route("GET", "/api/blog", (c) => blog.posts(c.env, { lang: c.url.searchParams.get("lang") || "en" }));
route("GET", "/api/blog/:slug", (c) => blog.post(c.env, c.params.slug, c.url.searchParams.get("lang") || "en"));
route("GET", "/api/blog-languages", async () => blog.LANGUAGES);

// --- Backstage -----------------------------------------------------------------

route("GET", "/api/admin/overview", async (c) => {
  await needAdmin(c);
  const [orders, licences, users, wishes, pendingT, pendingC, pendingS, promos] = await Promise.all([
    list<{ s: string; t: number; c: string; at: number; e: string; p: string; v: string }>(c.env.KV, "order:", 10000),
    list<{ s: string; d: boolean; e: string }>(c.env.KV, "licence:", 10000),
    list(c.env.KV, "user:", 10000),
    list(c.env.KV, "wish:", 5000),
    community.comments(c.env, "testimonial", { pending: true }),
    community.comments(c.env, "comment", { pending: true }),
    community.comments(c.env, "sample", { pending: true }),
    list(c.env.KV, "promo:", 10000),
  ]);
  const paid = orders.filter((o) => o.metadata?.s === "paid" && o.metadata?.v !== "demo");
  const revenue: Record<string, number> = {};
  for (const o of paid) revenue[o.metadata!.c] = (revenue[o.metadata!.c] || 0) + o.metadata!.t;
  const byEdition: Record<string, number> = {};
  for (const l of licences) if (!l.metadata?.d) byEdition[l.metadata?.e ?? "?"] = (byEdition[l.metadata?.e ?? "?"] || 0) + 1;
  return {
    orders: orders.length,
    paid: paid.length,
    demoOrders: orders.filter((o) => o.metadata?.v === "demo").length,
    pendingTransfers: orders.filter((o) => o.metadata?.s === "pending" && o.metadata?.v === "transfer").length,
    revenue,
    licences: licences.length,
    byEdition,
    users: users.length,
    wishes: wishes.length,
    pending: pendingT.length + pendingC.length + pendingS.length,
    promos: promos.length,
    ai: c.env.ANTHROPIC_API_KEY ? "claude" : "workers-ai",
    signing: c.env.LICENCE_PRIVATE_KEY ? "secret" : "development key in KV",
    providers: auth.enabledProviders(c.env).map((p) => p.id),
    payments: { stripe: !!c.env.STRIPE_SECRET_KEY, paypal: !!c.env.PAYPAL_CLIENT_ID, transfer: !!c.env.BANK_TRANSFER_DETAILS, demo: auth.demoMode(c.env) },
  };
});

route("GET", "/api/admin/promos", async (c) => {
  await needAdmin(c);
  const keys = await list<ReturnType<typeof promo.meta>>(c.env.KV, "promo:", 10000);
  return keys.map((k) => ({ code: k.name.slice(6), ...k.metadata }));
});
route("GET", "/api/admin/promos/:code", async (c) => {
  await needAdmin(c);
  return get<Promo>(c.env.KV, `promo:${promo.normalise(c.params.code)}`);
});
route("POST", "/api/admin/promos", async (c) => {
  const admin = await needAdmin(c);
  const b = await body<promo.PromoSpec & { count?: number; prefix?: string; code?: string; boundTo?: string[] | string }>(c.req);
  const boundTo = typeof b.boundTo === "string" ? b.boundTo.split(/[\s,;]+/) : b.boundTo;
  const made = await promo.mint(c.env, b, { count: b.count, prefix: b.prefix, code: b.code, boundTo, by: admin.name });
  return { made: made.length, codes: made.map((p) => ({ code: p.code, boundTo: p.boundTo })) };
});
route("PATCH", "/api/admin/promos/:code", async (c) => {
  await needAdmin(c);
  const p = await get<Promo>(c.env.KV, `promo:${promo.normalise(c.params.code)}`);
  if (!p) fail(404, "No such code.");
  const b = await body<{ active?: boolean; maxUses?: number | null; expires?: number | null; note?: string }>(c.req);
  if (b.active !== undefined) p.active = !!b.active;
  if (b.maxUses !== undefined) p.maxUses = b.maxUses;
  if (b.expires !== undefined) p.expires = b.expires;
  if (b.note !== undefined) p.note = text(b.note, 200);
  await put(c.env.KV, `promo:${p.code}`, p, undefined, promo.meta(p));
  return p;
});
route("DELETE", "/api/admin/promos/:code", async (c) => {
  await needAdmin(c);
  await c.env.KV.delete(`promo:${promo.normalise(c.params.code)}`);
  return { ok: true };
});

route("PATCH", "/api/admin/wishes/:id", async (c) => {
  await needAdmin(c);
  const b = await body<{ status?: string; note?: string }>(c.req);
  return community.setWishStatus(c.env, c.params.id, b.status as never, b.note);
});
route("DELETE", "/api/admin/wishes/:id", async (c) => {
  await needAdmin(c);
  await c.env.KV.delete(`wish:${c.params.id}`);
  return { ok: true };
});

route("GET", "/api/admin/comments", async (c) => {
  await needAdmin(c);
  const [t, m, s] = await Promise.all((["testimonial", "comment", "sample"] as const).map((k) => community.comments(c.env, k, { pending: true })));
  return [...t, ...m, ...s];
});
route("POST", "/api/admin/comments/moderate", async (c) => {
  await needAdmin(c);
  const b = await body<{ key?: string; approve?: boolean }>(c.req);
  return community.moderate(c.env, String(b.key || ""), !!b.approve);
});

route("GET", "/api/admin/orders", async (c) => {
  await needAdmin(c);
  const keys = await list<Record<string, unknown>>(c.env.KV, "order:", 10000);
  return keys
    .map((k) => ({ id: k.name.slice(6), ...(k.metadata ?? {}) }) as { id: string; at?: number })
    .sort((a, b) => Number(b.at) - Number(a.at))
    .slice(0, 500);
});
route("GET", "/api/admin/orders/:id", async (c) => {
  await needAdmin(c);
  return get<Order>(c.env.KV, `order:${c.params.id}`);
});
route("POST", "/api/admin/orders/:id/paid", async (c) => {
  await needAdmin(c);
  const b = await body<{ bankCountry?: string }>(c.req);
  return checkout.markPaid(c.env, c.params.id, b.bankCountry ? text(b.bankCountry, 2).toUpperCase() : null);
});
route("POST", "/api/admin/licences/:id/revoke", async (c) => {
  await needAdmin(c);
  const l = await get<Licence>(c.env.KV, `licence:${c.params.id}`);
  if (!l) fail(404, "No such licence.");
  l.status = "revoked";
  await saveLicence(c.env.KV, l);
  return l;
});
route("PATCH", "/api/admin/users/:id", async (c) => {
  await needAdmin(c);
  const u = await get<User>(c.env.KV, `user:${c.params.id}`);
  if (!u) fail(404, "No such account.");
  const b = await body<{ student?: boolean }>(c.req);
  if (typeof b.student === "boolean") u.student = b.student;
  await put(c.env.KV, `user:${u.id}`, u);
  return u;
});
route("GET", "/api/admin/users/:id", async (c) => {
  await needAdmin(c);
  const u = await get<User>(c.env.KV, `user:${c.params.id}`);
  return u ? { ...u, licences: await checkout.userLicences(c.env, u.id) } : null;
});

route("PUT", "/api/admin/videos", async (c) => {
  await needAdmin(c);
  const b = await body<Video[]>(c.req);
  const clean = (Array.isArray(b) ? b : [])
    .filter((v) => /^[A-Za-z0-9_-]{6,20}$/.test(String(v.id)))
    .slice(0, 60)
    .map((v) => ({ id: v.id, title: text(v.title, 120), edition: v.edition ?? null, genre: v.genre ? text(v.genre, 30) : null }));
  await put(c.env.KV, "config:videos", clean);
  return clean;
});

route("GET", "/api/admin/blog", async (c) => {
  await needAdmin(c);
  return blog.posts(c.env, { drafts: true });
});
route("GET", "/api/admin/blog/:slug/:lang", async (c) => {
  await needAdmin(c);
  return blog.post(c.env, c.params.slug, c.params.lang, true);
});
route("POST", "/api/admin/blog/generate", async (c) => {
  const admin = await needAdmin(c);
  return blog.generate(c.env, await body(c.req), admin.name);
});
route("POST", "/api/admin/blog", async (c) => {
  const admin = await needAdmin(c);
  return blog.writePost(c.env, await body(c.req, 128 * 1024), admin.name);
});
route("PATCH", "/api/admin/blog/:slug/:lang", async (c) => {
  await needAdmin(c);
  return blog.editPost(c.env, c.params.slug, c.params.lang, await body(c.req, 128 * 1024));
});
route("DELETE", "/api/admin/blog/:slug/:lang", async (c) => {
  await needAdmin(c);
  return blog.deletePost(c.env, c.params.slug, c.params.lang);
});

// --- Dispatch ------------------------------------------------------------------

export async function handle(req: Request, env: AppEnv, exec: ExecutionContext): Promise<Response> {
  const url = new URL(req.url);
  // Mutating requests from a browser must come from this site.
  if (req.method !== "GET" && req.method !== "HEAD" && !url.pathname.startsWith("/api/webhooks/")) {
    const origin = req.headers.get("origin");
    if (origin && origin !== url.origin && !APP_CALLS.some((p) => url.pathname.startsWith(p))) {
      return json({ error: "Cross-site request refused." }, 403);
    }
  }
  for (const r of routes) {
    if (r.method !== req.method) continue;
    const m = url.pathname.match(r.pattern);
    if (!m) continue;
    const params = Object.fromEntries(r.keys.map((k, i) => [k, decodeURIComponent(m[i + 1])]));
    try {
      const out = await r.handler({ req, env, url, params, exec });
      return out instanceof Response ? out : json(out ?? null);
    } catch (e) {
      if (e instanceof HttpError) return json({ error: e.message, ...(e.extra ?? {}) }, e.status);
      console.error(JSON.stringify({ at: url.pathname, error: String((e as Error)?.stack ?? e) }));
      return json({ error: "Something went wrong on our side." }, 500);
    }
  }
  if (url.pathname.startsWith("/api/") || url.pathname.startsWith("/auth/")) return json({ error: "No such thing." }, 404);
  return env.ASSETS.fetch(req);
}

export default {
  fetch: (req, env, ctx) => handle(req, env as unknown as AppEnv, ctx),
} satisfies ExportedHandler<Env>;
