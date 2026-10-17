/**
 * PayPal Orders v2 over REST. Orders are *authorised* first and captured only
 * once the payer's country has been read, so a regional price that the
 * PayPal account cannot back is voided rather than taken.
 */

import type { AppEnv } from "./env";
import { fail } from "./http";

function base(env: AppEnv): string {
  return env.PAYPAL_ENV === "live" ? "https://api-m.paypal.com" : "https://api-m.sandbox.paypal.com";
}

async function token(env: AppEnv): Promise<string> {
  if (!env.PAYPAL_CLIENT_ID || !env.PAYPAL_CLIENT_SECRET) fail(503, "PayPal is not set up yet.");
  const r = await fetch(`${base(env)}/v1/oauth2/token`, {
    method: "POST",
    headers: {
      authorization: `Basic ${btoa(`${env.PAYPAL_CLIENT_ID}:${env.PAYPAL_CLIENT_SECRET}`)}`,
      "content-type": "application/x-www-form-urlencoded",
    },
    body: "grant_type=client_credentials",
  });
  const data = (await r.json()) as { access_token?: string };
  if (!data.access_token) fail(502, "PayPal did not let us in.");
  return data.access_token;
}

export async function paypal<T = Record<string, any>>(env: AppEnv, method: "GET" | "POST", path: string, payload?: unknown, requestId?: string): Promise<T> {
  const r = await fetch(`${base(env)}${path}`, {
    method,
    headers: {
      authorization: `Bearer ${await token(env)}`,
      "content-type": "application/json",
      ...(requestId ? { "paypal-request-id": requestId } : {}),
    },
    body: payload ? JSON.stringify(payload) : undefined,
  });
  const text = await r.text();
  const data = (text ? JSON.parse(text) : {}) as T & { message?: string };
  if (!r.ok) fail(402, data.message || `PayPal answered ${r.status}.`);
  return data;
}
