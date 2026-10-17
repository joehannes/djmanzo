/**
 * Linking djmanzo on a machine to an account on the site.
 *
 * Two ways, both ending in the same token — revocable from the site, and
 * good for nothing but this API. Neither puts anybody's Google password
 * anywhere near djmanzo.
 *
 * **The loopback redirect** (RFC 8252, with PKCE; ADR-0011's choice).
 * djmanzo listens on a port of `127.0.0.1` only it holds, opens the browser
 * at `/link?port=…&challenge=…&state=…`, the DJ signs in and says yes, and
 * the browser is sent back to that port with a one-time code. djmanzo trades
 * the code and the verifier only it knows for the token.
 *
 * **The device flow** (RFC 8628), for when the loopback cannot work — a port
 * that will not open, a browser on another machine. djmanzo asks for a pair
 * of codes, shows the short one and opens the browser at `/link`; the DJ
 * signs in, sees the machine's name and the same short code, says yes, and
 * djmanzo, polling, receives the token.
 */

import type { AppEnv } from "./env";
import { b64url, fail, randomId, sha256, text } from "./http";
import { ACTIVATION_DAYS, activationToken, inForce, licenceToken } from "./licence";
import { ALPHABET } from "./promo";
import { get, put, saveLicence, type Licence, type User } from "./store";
import { userLicences } from "./checkout";
import { SEATS, modulesOf } from "../shared/catalog";

const MINUTES = 15;

interface Pending {
  userCode: string;
  machine: string;
  status: "pending" | "approved" | "denied";
  uid?: string;
  createdAt: number;
}

function userCode(): string {
  const b = crypto.getRandomValues(new Uint8Array(8));
  const c = [...b].map((x) => ALPHABET[x % ALPHABET.length]).join("");
  return `${c.slice(0, 4)}-${c.slice(4)}`;
}

export async function startLink(env: AppEnv, origin: string, machine: string) {
  const device = randomId(32);
  const code = userCode();
  const rec: Pending = { userCode: code, machine: text(machine, 60) || "A computer", status: "pending", createdAt: Date.now() };
  const h = await sha256(device);
  await put(env.KV, `device:${h}`, rec, MINUTES * 60);
  await env.KV.put(`usercode:${code}`, h, { expirationTtl: MINUTES * 60 });
  return {
    device_code: device,
    user_code: code,
    verification_uri: `${origin}/link`,
    verification_uri_complete: `${origin}/link?code=${code}`,
    expires_in: MINUTES * 60,
    interval: 5,
  };
}

export async function lookupLink(env: AppEnv, code: string) {
  const h = await env.KV.get(`usercode:${String(code).toUpperCase().trim()}`);
  const rec = h ? await get<Pending>(env.KV, `device:${h}`) : null;
  if (!rec || rec.status !== "pending") fail(404, "That code has expired or was already used. Ask djmanzo for a new one.");
  return { machine: rec.machine, createdAt: rec.createdAt };
}

export async function decideLink(env: AppEnv, user: User, code: string, approve: boolean) {
  const key = `usercode:${String(code).toUpperCase().trim()}`;
  const h = await env.KV.get(key);
  const rec = h ? await get<Pending>(env.KV, `device:${h}`) : null;
  if (!h || !rec || rec.status !== "pending") fail(404, "That code has expired or was already used.");
  rec.status = approve ? "approved" : "denied";
  rec.uid = user.id;
  await put(env.KV, `device:${h}`, rec, MINUTES * 60);
  await env.KV.delete(key);
  return { ok: true, machine: rec.machine };
}

async function appToken(env: AppEnv, uid: string, machine: string) {
  const token = `djm_${randomId(32)}`;
  await put(env.KV, `apptoken:${await sha256(token)}`, { uid, machine, createdAt: Date.now() }, 365 * 86400);
  const user = await get<User>(env.KV, `user:${uid}`);
  return { access_token: token, token_type: "Bearer", user: user ? { name: user.name, email: user.email } : null };
}

const LOOPBACK_SECONDS = 300;

/**
 * The DJ, signed in on `/link`, said yes to the djmanzo waiting on `port`.
 * Returns where to send the browser: back to that port with a one-time code
 * and the state djmanzo chose, so djmanzo can tell its own answer from
 * anything else that knocks.
 */
export async function approveLoopback(env: AppEnv, user: User, input: { port?: unknown; challenge?: unknown; state?: unknown; machine?: unknown }) {
  const port = Number(input.port);
  if (!Number.isInteger(port) || port < 1024 || port > 65535) fail(400, "That is not a port djmanzo would listen on.");
  const challenge = String(input.challenge || "");
  if (!/^[A-Za-z0-9_-]{43}$/.test(challenge)) fail(400, "The link from djmanzo is incomplete; start again from djmanzo.");
  const state = String(input.state || "");
  if (!/^[A-Za-z0-9_-]{8,64}$/.test(state)) fail(400, "The link from djmanzo is incomplete; start again from djmanzo.");
  const code = randomId(24);
  await put(env.KV, `loopcode:${await sha256(code)}`, { uid: user.id, challenge, machine: text(input.machine, 60) || "A computer" }, LOOPBACK_SECONDS);
  return { redirect: `http://127.0.0.1:${port}/callback?code=${encodeURIComponent(code)}&state=${encodeURIComponent(state)}` };
}

/** djmanzo trading the one-time code, and the verifier behind the challenge, for its token. */
export async function exchangeLoopback(env: AppEnv, code: string, verifier: string) {
  const key = `loopcode:${await sha256(String(code || ""))}`;
  const rec = await get<{ uid: string; challenge: string; machine: string }>(env.KV, key);
  if (!rec) fail(400, "invalid_grant", { error: "invalid_grant" });
  await env.KV.delete(key);
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(String(verifier || "")));
  if (b64url(digest) !== rec.challenge) fail(400, "invalid_grant", { error: "invalid_grant" });
  return appToken(env, rec.uid, rec.machine);
}

/** djmanzo polling. RFC 8628's answers: pending, denied, expired, or a token. */
export async function pollLink(env: AppEnv, deviceCode: string) {
  const h = await sha256(String(deviceCode || ""));
  const rec = await get<Pending>(env.KV, `device:${h}`);
  if (!rec) fail(400, "expired_token", { error: "expired_token" });
  if (rec.status === "pending") fail(400, "authorization_pending", { error: "authorization_pending" });
  await env.KV.delete(`device:${h}`);
  if (rec.status === "denied" || !rec.uid) fail(400, "access_denied", { error: "access_denied" });
  return appToken(env, rec.uid, rec.machine);
}

export async function revokeAppToken(env: AppEnv, bearer: string) {
  if (bearer.startsWith("djm_")) await env.KV.delete(`apptoken:${await sha256(bearer)}`);
  return { ok: true };
}

/** What djmanzo needs on start: who, which licences, each signed, and the island profile. */
export async function appMe(env: AppEnv, user: User) {
  const licences = await userLicences(env, user.id);
  const signed = await Promise.all(
    licences.map(async (l) => ({
      id: l.id,
      edition: l.edition,
      modules: modulesOf(l.edition, l.modules),
      plan: l.plan,
      status: l.status,
      expires: l.expires,
      inForce: inForce(l),
      demo: !!l.demo,
      seats: SEATS[l.edition],
      activations: l.activations.map((a) => ({ name: a.name, at: a.at, seen: a.seen })),
      token: await licenceToken(env, l, user),
    })),
  );
  const profile = await get<Record<string, unknown>>(env.KV, `profile:${user.id}`);
  return { user: { name: user.name, email: user.email, avatar: user.avatar }, licences: signed, profile };
}

function machineId(m: unknown): string {
  const s = String(m || "");
  if (!/^[A-Za-z0-9_-]{16,128}$/.test(s)) fail(400, "A machine id is 16 to 128 letters, digits, dashes or underscores.");
  return s;
}

/**
 * One licence on one machine. Re-activating a machine already counted is a
 * renewal, never a second seat; a new machine past the edition's seats is
 * refused with the list of where it is in use, so the DJ can free one.
 */
export async function activate(env: AppEnv, user: User, lid: string, machine: unknown, name: unknown) {
  const id = machineId(machine);
  const licence = await get<Licence>(env.KV, `licence:${lid}`);
  if (!licence || licence.uid !== user.id) fail(404, "That licence is not yours.");
  if (!inForce(licence)) fail(402, "That licence has ended.");
  const now = Date.now();
  const existing = licence.activations.find((a) => a.machine === id);
  if (existing) {
    existing.seen = now;
    existing.name = text(name, 60) || existing.name;
  } else {
    // A machine not seen for twice the grace period has gone; its seat comes back.
    licence.activations = licence.activations.filter((a) => now - a.seen < 2 * ACTIVATION_DAYS * 86400_000);
    if (licence.activations.length >= SEATS[licence.edition]) {
      fail(409, `This licence is on ${licence.activations.length} machines already — free one first.`, {
        activations: licence.activations.map((a) => ({ machine: a.machine.slice(0, 6), name: a.name, seen: a.seen })),
      });
    }
    licence.activations.push({ machine: id, name: text(name, 60) || "A computer", at: now, seen: now });
  }
  await saveLicence(env.KV, licence);
  return { activation: await activationToken(env, licence, id), licence: await licenceToken(env, licence, user) };
}

export async function deactivate(env: AppEnv, user: User, lid: string, machine: string) {
  const licence = await get<Licence>(env.KV, `licence:${lid}`);
  if (!licence || licence.uid !== user.id) fail(404, "That licence is not yours.");
  const before = licence.activations.length;
  licence.activations = licence.activations.filter((a) => a.machine !== machine && !a.machine.startsWith(machine || "\u0000"));
  await saveLicence(env.KV, licence);
  return { freed: before - licence.activations.length };
}
