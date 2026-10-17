/**
 * Stripe over its REST API, without the SDK: the Worker needs nine calls
 * and a webhook check, and the SDK is larger than the rest of the site.
 */

import type { AppEnv } from "./env";
import { equalBytes, fail } from "./http";

interface Params {
  [key: string]: string | number | boolean | null | undefined | Params | (string | Params)[];
}

/** Stripe's bracketed form encoding: `a[b][0][c]=1`. */
export function encode(params: Params, prefix = "", out = new URLSearchParams()): URLSearchParams {
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null) continue;
    const key = prefix ? `${prefix}[${k}]` : k;
    if (Array.isArray(v)) {
      v.forEach((item, i) => {
        if (typeof item === "object") encode(item, `${key}[${i}]`, out);
        else out.append(`${key}[${i}]`, String(item));
      });
    } else if (typeof v === "object") {
      encode(v, key, out);
    } else {
      out.append(key, String(v));
    }
  }
  return out;
}

export async function stripe<T = Record<string, any>>(env: AppEnv, method: "GET" | "POST" | "DELETE", path: string, params?: Params, idempotency?: string): Promise<T> {
  if (!env.STRIPE_SECRET_KEY) fail(503, "Card payments are not set up yet.");
  const url = new URL(`https://api.stripe.com/v1/${path}`);
  const init: RequestInit = {
    method,
    headers: {
      authorization: `Bearer ${env.STRIPE_SECRET_KEY}`,
      "content-type": "application/x-www-form-urlencoded",
      "stripe-version": "2024-06-20",
      ...(idempotency ? { "idempotency-key": idempotency } : {}),
    },
  };
  if (params && method === "GET") url.search = encode(params).toString();
  else if (params) init.body = encode(params);
  const r = await fetch(url, init);
  const data = (await r.json()) as T & { error?: { message?: string } };
  if (!r.ok) fail(402, data.error?.message || `Stripe answered ${r.status}.`);
  return data;
}

/**
 * Check a webhook's `Stripe-Signature` header: HMAC-SHA256 of
 * `<timestamp>.<body>` with the endpoint's secret, within five minutes.
 */
export async function verifyWebhook(secret: string, header: string | null, payload: string, now = Date.now()): Promise<boolean> {
  if (!header) return false;
  const parts = Object.fromEntries(
    header.split(",").map((p) => {
      const i = p.indexOf("=");
      return [p.slice(0, i), p.slice(i + 1)];
    }),
  );
  const t = Number(parts.t);
  const v1 = header
    .split(",")
    .filter((p) => p.startsWith("v1="))
    .map((p) => p.slice(3));
  if (!t || !v1.length || Math.abs(now / 1000 - t) > 300) return false;
  const key = await crypto.subtle.importKey("raw", new TextEncoder().encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const mac = new Uint8Array(await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(`${t}.${payload}`)));
  const hex = [...mac].map((b) => b.toString(16).padStart(2, "0")).join("");
  const enc = new TextEncoder();
  return v1.some((sig) => equalBytes(enc.encode(sig), enc.encode(hex)));
}
