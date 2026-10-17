/**
 * Licences, signed so djmanzo can trust one without asking.
 *
 * A licence token is `DJM1.<payload>.<signature>`: the payload is base64url
 * JSON, the signature is Ed25519 over the ASCII bytes `DJM1.<payload>`. The
 * application carries the public key and verifies offline, so a night in a
 * basement with no signal is a night the licence still works.
 *
 * An activation is the same shape with `DJA1.`: one licence on one machine,
 * valid for thirty days and renewed whenever djmanzo is online. The thirty
 * days are the grace a DJ gets without a connection, and the only thing a
 * cancelled subscription keeps.
 */

import { SEATS, type Edition, type Plan } from "../shared/catalog";
import { b64url, unb64url } from "./http";
import type { AppEnv } from "./env";
import type { Licence, User } from "./store";

export const ACTIVATION_DAYS = 30;

export interface LicenceClaims {
  v: 1;
  lid: string;
  uid: string;
  name: string;
  email: string | null;
  edition: Edition;
  plan: Plan;
  major: number;
  seats: number;
  /** Seconds since the epoch. */
  iat: number;
  /** Seconds; null for a perpetual licence. */
  exp: number | null;
  demo?: boolean;
}

export interface ActivationClaims {
  v: 1;
  lid: string;
  machine: string;
  edition: Edition;
  plan: Plan;
  major: number;
  iat: number;
  /** When djmanzo must have checked in again, in seconds. */
  exp: number;
  demo?: boolean;
}

type Keys = { privateKey: CryptoKey; publicKey: CryptoKey; publicRaw: string };

/**
 * The signing key: the `LICENCE_PRIVATE_KEY` secret in production. Without
 * it (local development, the tests) a key is generated once and kept in KV,
 * marked as a development key, so tokens stay verifiable across restarts.
 */
export async function keys(env: AppEnv): Promise<Keys> {
  let pkcs8 = env.LICENCE_PRIVATE_KEY;
  if (!pkcs8) {
    pkcs8 = (await env.KV.get("config:dev-licence-key")) ?? undefined;
    if (!pkcs8) {
      const pair = (await crypto.subtle.generateKey({ name: "Ed25519" }, true, ["sign", "verify"])) as CryptoKeyPair;
      pkcs8 = btoa(String.fromCharCode(...new Uint8Array((await crypto.subtle.exportKey("pkcs8", pair.privateKey)) as ArrayBuffer)));
      await env.KV.put("config:dev-licence-key", pkcs8);
    }
  }
  const der = Uint8Array.from(atob(pkcs8), (c) => c.charCodeAt(0));
  const privateKey = await crypto.subtle.importKey("pkcs8", der, { name: "Ed25519" }, true, ["sign"]);
  const jwk = (await crypto.subtle.exportKey("jwk", privateKey)) as JsonWebKey;
  const publicKey = await crypto.subtle.importKey("jwk", { kty: "OKP", crv: "Ed25519", x: jwk.x }, { name: "Ed25519" }, true, ["verify"]);
  return { privateKey, publicKey, publicRaw: jwk.x! };
}

async function sign(env: AppEnv, prefix: "DJM1" | "DJA1", claims: object): Promise<string> {
  const { privateKey } = await keys(env);
  const payload = b64url(new TextEncoder().encode(JSON.stringify(claims)));
  const signed = `${prefix}.${payload}`;
  const sig = await crypto.subtle.sign({ name: "Ed25519" }, privateKey, new TextEncoder().encode(signed));
  return `${signed}.${b64url(sig)}`;
}

/** Verify a token against a public key given as base64url raw bytes. Returns its claims, or null. */
export async function verify<T>(token: string, publicRaw: string, prefix: "DJM1" | "DJA1"): Promise<T | null> {
  const parts = token.split(".");
  if (parts.length !== 3 || parts[0] !== prefix) return null;
  const key = await crypto.subtle.importKey("raw", unb64url(publicRaw), { name: "Ed25519" }, false, ["verify"]);
  const ok = await crypto.subtle.verify(
    { name: "Ed25519" },
    key,
    unb64url(parts[2]),
    new TextEncoder().encode(`${parts[0]}.${parts[1]}`),
  );
  if (!ok) return null;
  return JSON.parse(new TextDecoder().decode(unb64url(parts[1]))) as T;
}

export function claims(licence: Licence, user: User): LicenceClaims {
  return {
    v: 1,
    lid: licence.id,
    uid: user.id,
    name: user.name,
    email: user.email,
    edition: licence.edition,
    plan: licence.plan,
    major: licence.major,
    seats: SEATS[licence.edition],
    iat: Math.floor(Date.now() / 1000),
    exp: licence.expires === null ? null : Math.floor(licence.expires / 1000),
    ...(licence.demo ? { demo: true } : {}),
  };
}

export function licenceToken(env: AppEnv, licence: Licence, user: User): Promise<string> {
  return sign(env, "DJM1", claims(licence, user));
}

export function activationToken(env: AppEnv, licence: Licence, machine: string): Promise<string> {
  const now = Math.floor(Date.now() / 1000);
  const until = now + ACTIVATION_DAYS * 86400;
  const ends = licence.expires === null ? until : Math.min(until, Math.floor(licence.expires / 1000));
  const c: ActivationClaims = {
    v: 1,
    lid: licence.id,
    machine,
    edition: licence.edition,
    plan: licence.plan,
    major: licence.major,
    iat: now,
    exp: ends,
    ...(licence.demo ? { demo: true } : {}),
  };
  return sign(env, "DJA1", c);
}

/** A licence is in force when it is active and not past its end. */
export function inForce(licence: Licence, now = Date.now()): boolean {
  return licence.status === "active" && (licence.expires === null || licence.expires > now);
}

/** When a term that starts now (or extends `from`) ends. */
export function termEnd(plan: Plan, from: number, months?: number): number | null {
  if (plan === "perpetual") return null;
  const d = new Date(from);
  d.setUTCMonth(d.getUTCMonth() + (months ?? (plan === "yearly" ? 12 : 1)));
  // Three days' grace, so a renewal that lands a day late does not lock a DJ out mid-set.
  return d.getTime() + 3 * 86400_000;
}
