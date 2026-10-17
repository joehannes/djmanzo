import { describe, expect, it } from "vitest";
import { activationToken, keys, licenceToken, verify, termEnd, type LicenceClaims, type ActivationClaims } from "../worker/licence";
import { encode, verifyWebhook } from "../worker/stripe";
import type { Licence, User } from "../worker/store";
import { testEnv } from "./kv";

const user: User = { id: "u1", email: "dj@example.com", name: "DJ Test", avatar: null, identities: [], createdAt: 0 };
const licence: Licence = { id: "l1", uid: "u1", edition: "pro", plan: "perpetual", major: 1, status: "active", expires: null, order: "o1", activations: [], createdAt: 0 };

describe("licences are signed", () => {
  it("a licence verifies against the published key and carries its edition", async () => {
    const env = testEnv();
    const token = await licenceToken(env, licence, user);
    const { publicRaw } = await keys(env);
    const c = await verify<LicenceClaims>(token, publicRaw, "DJM1");
    expect(c).toMatchObject({ lid: "l1", edition: "pro", plan: "perpetual", exp: null, seats: 3 });
  });

  it("one changed character in the payload breaks the signature", async () => {
    const env = testEnv();
    const token = await licenceToken(env, licence, user);
    const { publicRaw } = await keys(env);
    const [p, payload, sig] = token.split(".");
    const forged = JSON.parse(atob(payload.replace(/-/g, "+").replace(/_/g, "/")));
    forged.edition = "full";
    const b64 = btoa(JSON.stringify(forged)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    expect(await verify(`${p}.${b64}.${sig}`, publicRaw, "DJM1")).toBeNull();
  });

  it("a token from another key does not verify", async () => {
    const token = await licenceToken(testEnv(), licence, user);
    const { publicRaw } = await keys(testEnv());
    expect(await verify(token, publicRaw, "DJM1")).toBeNull();
  });

  it("an activation lasts thirty days, or until a subscription ends if sooner", async () => {
    const env = testEnv();
    const { publicRaw } = await keys(env);
    const a = await verify<ActivationClaims>(await activationToken(env, licence, "machine-0123456789"), publicRaw, "DJA1");
    expect(a!.exp - a!.iat).toBe(30 * 86400);
    const soon = Date.now() + 5 * 86400_000;
    const b = await verify<ActivationClaims>(await activationToken(env, { ...licence, plan: "monthly", expires: soon }, "machine-0123456789"), publicRaw, "DJA1");
    expect(b!.exp).toBe(Math.floor(soon / 1000));
  });

  it("a month's term is a calendar month plus three days' grace", () => {
    const from = Date.UTC(2026, 0, 31);
    expect(termEnd("perpetual", from)).toBeNull();
    expect(termEnd("yearly", from)).toBe(Date.UTC(2027, 0, 31) + 3 * 86400_000);
  });
});

describe("Stripe plumbing", () => {
  it("encodes nested parameters the way Stripe reads them", () => {
    const s = encode({ items: [{ price_data: { unit_amount: 899, recurring: { interval: "month" } } }], expand: ["latest_invoice.payment_intent"] }).toString();
    expect(decodeURIComponent(s)).toBe("items[0][price_data][unit_amount]=899&items[0][price_data][recurring][interval]=month&expand[0]=latest_invoice.payment_intent");
  });

  it("accepts a correctly signed webhook and refuses a tampered one", async () => {
    const secret = "whsec_test";
    const payload = '{"type":"invoice.paid"}';
    const t = Math.floor(Date.now() / 1000);
    const key = await crypto.subtle.importKey("raw", new TextEncoder().encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
    const mac = [...new Uint8Array(await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(`${t}.${payload}`)))].map((b) => b.toString(16).padStart(2, "0")).join("");
    expect(await verifyWebhook(secret, `t=${t},v1=${mac}`, payload)).toBe(true);
    expect(await verifyWebhook(secret, `t=${t},v1=${mac}`, payload.replace("paid", "void"))).toBe(false);
    expect(await verifyWebhook(secret, `t=${t - 1000},v1=${mac}`, payload)).toBe(false);
  });
});
