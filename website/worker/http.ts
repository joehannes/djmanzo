/** Small HTTP helpers shared by every route. */

export class HttpError extends Error {
  constructor(
    public status: number,
    message: string,
    public extra?: Record<string, unknown>,
  ) {
    super(message);
  }
}

export const SECURITY_HEADERS: Record<string, string> = {
  "x-content-type-options": "nosniff",
  "referrer-policy": "strict-origin-when-cross-origin",
};

export function json(data: unknown, status = 200, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store", ...SECURITY_HEADERS, ...headers },
  });
}

export function fail(status: number, message: string, extra?: Record<string, unknown>): never {
  throw new HttpError(status, message, extra);
}

/** A request body as JSON, refused past `max` bytes so nobody fills KV with one post. */
export async function body<T = Record<string, unknown>>(req: Request, max = 64 * 1024): Promise<T> {
  const length = Number(req.headers.get("content-length") || "0");
  if (length > max) fail(413, "That is more than this form takes.");
  const text = await req.text();
  if (text.length > max) fail(413, "That is more than this form takes.");
  if (!text) return {} as T;
  try {
    return JSON.parse(text) as T;
  } catch {
    fail(400, "That was not JSON.");
  }
}

export function cookies(req: Request): Record<string, string> {
  const out: Record<string, string> = {};
  for (const part of (req.headers.get("cookie") || "").split(";")) {
    const i = part.indexOf("=");
    if (i > 0) out[part.slice(0, i).trim()] = decodeURIComponent(part.slice(i + 1).trim());
  }
  return out;
}

export function cookie(name: string, value: string, maxAge: number): string {
  return `${name}=${encodeURIComponent(value)}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=${maxAge}`;
}

const B64URL = (bytes: Uint8Array) =>
  btoa(String.fromCharCode(...bytes)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");

export function b64url(bytes: Uint8Array | ArrayBuffer): string {
  return B64URL(bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes));
}

export function unb64url(s: string): Uint8Array {
  const b = atob(s.replace(/-/g, "+").replace(/_/g, "/") + "===".slice((s.length + 3) % 4));
  return Uint8Array.from(b, (c) => c.charCodeAt(0));
}

export function randomId(bytes = 18): string {
  return b64url(crypto.getRandomValues(new Uint8Array(bytes)));
}

export async function sha256(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** Constant-time comparison of two secrets, through their digests so lengths cannot leak either. */
export async function same(a: string, b: string): Promise<boolean> {
  const [x, y] = await Promise.all(
    [a, b].map((s) => crypto.subtle.digest("SHA-256", new TextEncoder().encode(s))),
  );
  return equalBytes(new Uint8Array(x), new Uint8Array(y));
}

/** Byte-for-byte equality that takes the same time wherever the first difference is. */
export function equalBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a[i] ^ b[i];
  return diff === 0;
}

/**
 * At most `limit` calls per `window` seconds for one key. KV is eventually
 * consistent, so this is a brake, not a wall — enough to keep one visitor
 * from flooding the wishlist, not a defence against a determined attacker.
 */
export async function brake(kv: KVNamespace, key: string, limit: number, window: number): Promise<void> {
  const k = `rate:${key}:${Math.floor(Date.now() / 1000 / window)}`;
  const n = Number((await kv.get(k)) || "0");
  if (n >= limit) fail(429, "Easy — that is a lot of requests. Try again in a minute.");
  await kv.put(k, String(n + 1), { expirationTtl: Math.max(60, window * 2) });
}

export function clientIp(req: Request): string {
  return req.headers.get("cf-connecting-ip") || "0.0.0.0";
}

export function country(req: Request): string | null {
  const c = (req as Request & { cf?: { country?: string } }).cf?.country;
  return c && c !== "XX" && c !== "T1" ? c : null;
}

/** Plain text, trimmed and cut, with nothing that could be markup. */
export function text(value: unknown, max: number): string {
  return String(value ?? "")
    .replace(/[<>]/g, "")
    .replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f]/g, "")
    .trim()
    .slice(0, max);
}

export function isEmail(s: string): boolean {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(s);
}
