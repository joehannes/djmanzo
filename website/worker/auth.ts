/**
 * Sign-in with an account a person already has.
 *
 * Six providers over OAuth 2 — Google, Apple, GitHub, Microsoft and Discord,
 * as ADR-0011 names them, and Facebook — with PKCE wherever the provider
 * takes it. A provider is offered only when its keys are set; with none set
 * the site offers a demo sign-in instead, labelled as one, so the site can be
 * walked end to end before the owner has registered a single app.
 *
 * Apple is the odd one: its client secret is a short JWT the Worker signs
 * with the owner's key (ES256), it answers by posting a form back rather than
 * redirecting, and the person's name arrives only on their first sign-in.
 *
 * Accounts are linked by *verified* e-mail address: signing in with Google
 * and later with GitHub under the same address is one person with one set of
 * licences. An address a provider does not vouch for links nothing.
 */

import type { AppEnv } from "./env";
import { cookie, cookies, fail, randomId, same, sha256, b64url, unb64url, text } from "./http";
import { get, put, type Session, type User } from "./store";

export const SESSION_DAYS = 30;

interface Provider {
  id: string;
  name: string;
  authorize: string;
  token: string;
  scope: string;
  /** From the access token, by asking the provider. */
  profile?: (token: string) => Promise<Profile>;
  /** From the token response itself (an OpenID id_token), and the callback's form. */
  fromToken?: (tok: Record<string, unknown>, form: URLSearchParams) => Profile | null;
  /** Answers by posting a form to the callback (`response_mode=form_post`). */
  formPost?: boolean;
  /** Does not take PKCE. */
  noPkce?: boolean;
}

export interface Profile {
  sub: string;
  email: string | null;
  verified: boolean;
  name: string;
  avatar: string | null;
}

const UA = { "user-agent": "djmanzo-website" };

async function getJson(url: string, token: string, extra: Record<string, string> = {}) {
  const r = await fetch(url, { headers: { authorization: `Bearer ${token}`, accept: "application/json", ...UA, ...extra } });
  if (!r.ok) fail(502, `The sign-in provider answered ${r.status}.`);
  return (await r.json()) as Record<string, unknown>;
}

const s = (v: unknown) => (typeof v === "string" && v ? v : null);

export const PROVIDERS: Provider[] = [
  {
    id: "google",
    name: "Google",
    authorize: "https://accounts.google.com/o/oauth2/v2/auth",
    token: "https://oauth2.googleapis.com/token",
    scope: "openid email profile",
    profile: async (t) => {
      const p = await getJson("https://openidconnect.googleapis.com/v1/userinfo", t);
      return { sub: String(p.sub), email: s(p.email), verified: p.email_verified === true, name: s(p.name) ?? "DJ", avatar: s(p.picture) };
    },
  },
  {
    id: "apple",
    name: "Apple",
    authorize: "https://appleid.apple.com/auth/authorize",
    token: "https://appleid.apple.com/auth/token",
    scope: "name email",
    formPost: true,
    noPkce: true,
    // The id_token came straight from Apple's token endpoint over TLS, which
    // OpenID Connect Core §3.1.3.7 accepts in place of checking its signature.
    fromToken: (tok, form) => {
      const claims = jwtClaims(s(tok.id_token));
      if (!claims?.sub) return null;
      let name: string | null = null;
      try {
        const u = JSON.parse(form.get("user") || "null") as { name?: { firstName?: string; lastName?: string } } | null;
        name = [u?.name?.firstName, u?.name?.lastName].filter(Boolean).join(" ") || null;
      } catch {
        name = null;
      }
      const verified = claims.email_verified === true || claims.email_verified === "true";
      return { sub: String(claims.sub), email: s(claims.email), verified, name: name ?? "DJ", avatar: null };
    },
  },
  {
    id: "github",
    name: "GitHub",
    authorize: "https://github.com/login/oauth/authorize",
    token: "https://github.com/login/oauth/access_token",
    scope: "read:user user:email",
    profile: async (t) => {
      const p = await getJson("https://api.github.com/user", t);
      const emails = (await getJson("https://api.github.com/user/emails", t).catch(() => [])) as unknown as {
        email: string;
        primary: boolean;
        verified: boolean;
      }[];
      const primary = Array.isArray(emails) ? emails.find((e) => e.primary && e.verified) : undefined;
      return { sub: String(p.id), email: primary?.email ?? null, verified: !!primary, name: s(p.name) ?? s(p.login) ?? "DJ", avatar: s(p.avatar_url) };
    },
  },
  {
    id: "discord",
    name: "Discord",
    authorize: "https://discord.com/oauth2/authorize",
    token: "https://discord.com/api/oauth2/token",
    scope: "identify email",
    profile: async (t) => {
      const p = await getJson("https://discord.com/api/users/@me", t);
      const avatar = p.avatar ? `https://cdn.discordapp.com/avatars/${p.id}/${p.avatar}.png` : null;
      return { sub: String(p.id), email: s(p.email), verified: p.verified === true, name: s(p.global_name) ?? s(p.username) ?? "DJ", avatar };
    },
  },
  {
    id: "facebook",
    name: "Facebook",
    authorize: "https://www.facebook.com/v21.0/dialog/oauth",
    token: "https://graph.facebook.com/v21.0/oauth/access_token",
    scope: "email public_profile",
    profile: async (t) => {
      const p = await getJson("https://graph.facebook.com/me?fields=id,name,email,picture", t);
      const pic = (p.picture as { data?: { url?: string } } | undefined)?.data?.url ?? null;
      // Facebook only returns an address it has confirmed.
      return { sub: String(p.id), email: s(p.email), verified: !!p.email, name: s(p.name) ?? "DJ", avatar: pic };
    },
  },
  {
    id: "microsoft",
    name: "Microsoft",
    authorize: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
    token: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    scope: "openid email profile",
    profile: async (t) => {
      const p = await getJson("https://graph.microsoft.com/oidc/userinfo", t);
      // Microsoft personal and work accounts do not assert verification; an address links nothing.
      return { sub: String(p.sub), email: s(p.email), verified: false, name: s(p.name) ?? "DJ", avatar: null };
    },
  },
];

/** The claims of a JWT, unchecked: only for one received straight from its issuer. */
function jwtClaims(jwt: string | null): Record<string, unknown> | null {
  const part = jwt?.split(".")[1];
  if (!part) return null;
  try {
    return JSON.parse(new TextDecoder().decode(unb64url(part))) as Record<string, unknown>;
  } catch {
    return null;
  }
}

/**
 * Apple's client secret: an ES256 JWT, signed with the key the owner made in
 * Apple's developer account, good for five minutes.
 */
async function appleSecret(env: AppEnv): Promise<string> {
  const pem = String(env.APPLE_PRIVATE_KEY || "").replace(/-----[^-]+-----/g, "").replace(/\s+/g, "");
  const der = Uint8Array.from(atob(pem), (c) => c.charCodeAt(0));
  const key = await crypto.subtle.importKey("pkcs8", der, { name: "ECDSA", namedCurve: "P-256" }, false, ["sign"]);
  const now = Math.floor(Date.now() / 1000);
  const enc = (o: object) => b64url(new TextEncoder().encode(JSON.stringify(o)));
  const head = enc({ alg: "ES256", kid: env.APPLE_KEY_ID });
  const body = enc({ iss: env.APPLE_TEAM_ID, iat: now, exp: now + 300, aud: "https://appleid.apple.com", sub: env.APPLE_CLIENT_ID });
  const sig = await crypto.subtle.sign({ name: "ECDSA", hash: "SHA-256" }, key, new TextEncoder().encode(`${head}.${body}`));
  return `${head}.${body}.${b64url(sig)}`;
}

function credentials(env: AppEnv, id: string): { clientId: string; secret: string } | null {
  if (id === "apple") {
    return env.APPLE_CLIENT_ID && env.APPLE_TEAM_ID && env.APPLE_KEY_ID && env.APPLE_PRIVATE_KEY ? { clientId: env.APPLE_CLIENT_ID, secret: "" } : null;
  }
  const up = id.toUpperCase();
  const clientId = (env as unknown as Record<string, string | undefined>)[`${up}_CLIENT_ID`];
  const secret = (env as unknown as Record<string, string | undefined>)[`${up}_CLIENT_SECRET`];
  return clientId && secret ? { clientId, secret } : null;
}

export function enabledProviders(env: AppEnv): { id: string; name: string }[] {
  return PROVIDERS.filter((p) => credentials(env, p.id)).map((p) => ({ id: p.id, name: p.name }));
}

/** Demo mode: forced by `DEMO_MODE=on`, or "auto" with no payment provider configured. */
export function demoMode(env: AppEnv): boolean {
  const mode = String(env.DEMO_MODE || "auto");
  if (mode === "on") return true;
  if (mode === "off") return false;
  return !env.STRIPE_SECRET_KEY && !env.PAYPAL_CLIENT_ID;
}

/** Only a path on this site may be returned to, so a sign-in cannot be used as an open redirect. */
export function safeReturn(path: string | null): string {
  return path && path.startsWith("/") && !path.startsWith("//") && !path.startsWith("/\\") ? path.slice(0, 600) : "/";
}

export async function start(req: Request, env: AppEnv, providerId: string): Promise<Response> {
  const provider = PROVIDERS.find((p) => p.id === providerId);
  const creds = provider && credentials(env, provider.id);
  if (!provider || !creds) fail(404, "That sign-in is not offered here.");
  const url = new URL(req.url);
  const state = randomId();
  const verifier = randomId(32);
  const challenge = b64url(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier)));
  await put(env.KV, `oauth:${state}`, { provider: provider.id, verifier, back: safeReturn(url.searchParams.get("return")) }, 600);
  const auth = new URL(provider.authorize);
  auth.searchParams.set("client_id", creds.clientId);
  auth.searchParams.set("redirect_uri", `${url.origin}/auth/callback/${provider.id}`);
  auth.searchParams.set("response_type", "code");
  auth.searchParams.set("scope", provider.scope);
  auth.searchParams.set("state", state);
  if (!provider.noPkce) {
    auth.searchParams.set("code_challenge", challenge);
    auth.searchParams.set("code_challenge_method", "S256");
  }
  if (provider.formPost) auth.searchParams.set("response_mode", "form_post");
  if (provider.id === "google") auth.searchParams.set("prompt", "select_account");
  return Response.redirect(auth.toString(), 302);
}

export async function callback(req: Request, env: AppEnv, providerId: string): Promise<Response> {
  const url = new URL(req.url);
  // Apple posts its answer as a form; everyone else redirects with a query.
  const form = req.method === "POST" ? new URLSearchParams(await req.text()) : url.searchParams;
  const state = form.get("state") || "";
  const code = form.get("code") || "";
  const saved = await get<{ provider: string; verifier: string; back: string }>(env.KV, `oauth:${state}`);
  if (!saved || saved.provider !== providerId || !code) {
    return Response.redirect(`${url.origin}/?signin=failed`, 302);
  }
  await env.KV.delete(`oauth:${state}`);
  const provider = PROVIDERS.find((p) => p.id === providerId)!;
  const creds = credentials(env, providerId)!;
  const exchange = new URLSearchParams({
    grant_type: "authorization_code",
    code,
    redirect_uri: `${url.origin}/auth/callback/${providerId}`,
    client_id: creds.clientId,
    client_secret: providerId === "apple" ? await appleSecret(env) : creds.secret,
  });
  if (!provider.noPkce) exchange.set("code_verifier", saved.verifier);
  const r = await fetch(provider.token, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded", accept: "application/json", ...UA },
    body: exchange,
  });
  const tok = (await r.json().catch(() => ({}))) as Record<string, unknown>;
  if (!r.ok || (!tok.access_token && !tok.id_token)) return Response.redirect(`${url.origin}/?signin=failed`, 302);
  const profile = provider.fromToken ? provider.fromToken(tok, form) : await provider.profile!(String(tok.access_token));
  if (!profile) return Response.redirect(`${url.origin}/?signin=failed`, 302);
  const user = await findOrCreate(env, providerId, profile);
  return signedIn(env, user, `${url.origin}${saved.back}`);
}

export async function findOrCreate(env: AppEnv, provider: string, p: Profile, extra: Partial<User> = {}): Promise<User> {
  const identKey = `ident:${provider}:${p.sub}`;
  let uid = await env.KV.get(identKey);
  const email = p.email ? p.email.toLowerCase() : null;
  if (!uid && email && p.verified) uid = await env.KV.get(`email:${email}`);
  let user = uid ? await get<User>(env.KV, `user:${uid}`) : null;
  if (!user) {
    user = {
      id: randomId(12),
      email: p.verified ? email : null,
      name: text(p.name, 60) || "DJ",
      avatar: p.avatar,
      identities: [],
      createdAt: Date.now(),
      ...extra,
    };
  }
  if (!user.identities.some((i) => i.provider === provider && i.sub === p.sub)) {
    user.identities.push({ provider, sub: p.sub });
  }
  if (!user.email && email && p.verified) user.email = email;
  if (!user.avatar && p.avatar) user.avatar = p.avatar;
  await put(env.KV, `user:${user.id}`, user);
  await env.KV.put(identKey, user.id);
  if (user.email && p.verified) await env.KV.put(`email:${user.email}`, user.id);
  return user;
}

export async function signedIn(env: AppEnv, user: User, to: string): Promise<Response> {
  const sid = randomId(24);
  await put(env.KV, `session:${sid}`, { uid: user.id, createdAt: Date.now() } satisfies Session, SESSION_DAYS * 86400);
  return new Response(null, { status: 302, headers: { location: to, "set-cookie": cookie("djm_sid", sid, SESSION_DAYS * 86400) } });
}

export async function signOut(req: Request, env: AppEnv): Promise<Response> {
  const sid = cookies(req).djm_sid;
  if (sid) await env.KV.delete(`session:${sid}`);
  return new Response(JSON.stringify({ ok: true }), {
    headers: { "content-type": "application/json", "set-cookie": cookie("djm_sid", "", 0) },
  });
}

/** The person behind a request: a browser's session cookie, or djmanzo's bearer token. */
export async function whoami(req: Request, env: AppEnv): Promise<User | null> {
  const auth = req.headers.get("authorization") || "";
  if (auth.startsWith("Bearer djm_")) {
    const rec = await get<{ uid: string }>(env.KV, `apptoken:${await sha256(auth.slice(7))}`);
    return rec ? get<User>(env.KV, `user:${rec.uid}`) : null;
  }
  const sid = cookies(req).djm_sid;
  if (!sid) return null;
  const session = await get<Session>(env.KV, `session:${sid}`);
  return session ? get<User>(env.KV, `user:${session.uid}`) : null;
}

export async function required(req: Request, env: AppEnv): Promise<User> {
  const user = await whoami(req, env);
  if (!user) fail(401, "Sign in first — it is how your licence finds its way into djmanzo.");
  return user;
}

export async function isAdmin(req: Request, env: AppEnv, user: User | null): Promise<boolean> {
  const auth = req.headers.get("authorization") || "";
  if (env.ADMIN_TOKEN && auth.startsWith("Bearer ") && !auth.startsWith("Bearer djm_")) {
    if (await same(auth.slice(7), env.ADMIN_TOKEN)) return true;
  }
  if (!user) return false;
  if (user.owner) return true;
  const admins = String(env.ADMIN_EMAILS || "")
    .split(",")
    .map((e) => e.trim().toLowerCase())
    .filter(Boolean);
  return !!user.email && admins.includes(user.email);
}

export async function adminRequired(req: Request, env: AppEnv): Promise<User | { id: "token"; name: "Owner" }> {
  const user = await whoami(req, env);
  if (!(await isAdmin(req, env, user))) fail(403, "Backstage is for the crew.");
  return user ?? { id: "token", name: "Owner" };
}

/** The owner's way in before any provider is set up: the admin token, exchanged for a session. */
export async function ownerSignIn(req: Request, env: AppEnv, token: string): Promise<Response> {
  if (!env.ADMIN_TOKEN || !(await same(token, env.ADMIN_TOKEN))) fail(403, "That is not the crew's token.");
  const user = await findOrCreate(env, "owner", { sub: "owner", email: null, verified: false, name: "Owner", avatar: null }, { owner: true });
  const res = await signedIn(env, user, new URL(req.url).origin + "/#backstage");
  return new Response(JSON.stringify({ ok: true }), { headers: { "content-type": "application/json", "set-cookie": res.headers.get("set-cookie")! } });
}

/** Demo mode's sign-in: a name, no password, and a flag on the account that keeps it out of real licences. */
export async function demoSignIn(req: Request, env: AppEnv, name: string): Promise<Response> {
  if (!demoMode(env) && enabledProviders(env).length) fail(404, "The demo sign-in is off; use one of the real ones.");
  const user = await findOrCreate(
    env,
    "demo",
    { sub: randomId(9), email: null, verified: false, name: text(name, 40) || "Guest DJ", avatar: null },
    { demo: true },
  );
  const res = await signedIn(env, user, new URL(req.url).origin + "/");
  return new Response(JSON.stringify({ ok: true, user: publicUser(user) }), {
    headers: { "content-type": "application/json", "set-cookie": res.headers.get("set-cookie")! },
  });
}

export function publicUser(u: User) {
  return { id: u.id, name: u.name, email: u.email, avatar: u.avatar, demo: !!u.demo, owner: !!u.owner, providers: u.identities.map((i) => i.provider) };
}
