import { describe, expect, it } from "vitest";
import { handle } from "../worker/index";
import { keys, verify, type LicenceClaims } from "../worker/licence";
import { testEnv } from "./kv";

const exec = { waitUntil: () => {}, passThroughOnException: () => {} } as unknown as ExecutionContext;

function client(env: never) {
  let cookie = "";
  return async (path: string, init: RequestInit & { json?: unknown } = {}) => {
    const headers = new Headers(init.headers);
    if (cookie) headers.set("cookie", cookie);
    if (init.json !== undefined) headers.set("content-type", "application/json");
    const req = new Request(`https://djmanzo.test${path}`, { ...init, headers, body: init.json !== undefined ? JSON.stringify(init.json) : init.body });
    const res = await handle(req, env, exec);
    const set = res.headers.get("set-cookie");
    if (set) cookie = set.split(";")[0];
    const text = await res.text();
    return { status: res.status, data: text ? JSON.parse(text) : null };
  };
}

describe("a visitor buys a licence and djmanzo picks it up", () => {
  it("demo sign-in, a regional quote, a proven purchase, a device link and an activation", async () => {
    const env = testEnv();
    const web = client(env);
    expect((await web("/api/me")).data).toMatchObject({ user: null, demo: true });

    const signin = await web("/api/auth/demo", { method: "POST", json: { name: "Yolanda" } });
    expect(signin.status).toBe(200);

    const q = await web("/api/quote", { method: "POST", json: { edition: "karaoke", plan: "yearly", country: "DO" } });
    expect(q.data.band).toBe(3);
    expect(q.data.total).toBeLessThan(q.data.base);

    // The card says the US: the Dominican price is not backed, and the buyer is asked again.
    const refused = await web("/api/checkout/demo", { method: "POST", json: { edition: "karaoke", plan: "yearly", country: "DO", instrument: "US", billing: "DO" } });
    expect(refused.status).toBe(409);
    expect(refused.data.needsConfirm).toBe(true);
    expect(refused.data.total).toBe(q.data.base);

    const bought = await web("/api/checkout/demo", { method: "POST", json: { edition: "karaoke", plan: "yearly", country: "DO", instrument: "DO", billing: "DO" } });
    expect(bought.status).toBe(200);
    expect(bought.data.order.total).toBe(q.data.total);
    expect(bought.data.licence).toMatchObject({ edition: "karaoke", plan: "yearly", demo: true });

    // djmanzo asks to be linked; the person approves in the browser; djmanzo receives its token.
    const app = client(env);
    const start = await app("/api/device/start", { method: "POST", json: { machine: "Booth laptop" } });
    expect(start.data.user_code).toMatch(/^[A-Z0-9]{4}-[A-Z0-9]{4}$/);
    expect((await app("/api/device/token", { method: "POST", json: { device_code: start.data.device_code } })).data.error).toBe("authorization_pending");
    expect((await web(`/api/device/lookup?code=${start.data.user_code}`)).data.machine).toBe("Booth laptop");
    await web("/api/device/approve", { method: "POST", json: { code: start.data.user_code, approve: true } });
    const tok = await app("/api/device/token", { method: "POST", json: { device_code: start.data.device_code } });
    expect(tok.data.access_token).toMatch(/^djm_/);

    const bearer = { authorization: `Bearer ${tok.data.access_token}` };
    const me = await app("/api/app/me", { headers: bearer });
    expect(me.data.user.name).toBe("Yolanda");
    expect(me.data.licences).toHaveLength(1);
    const { publicRaw } = await keys(env);
    const claims = await verify<LicenceClaims>(me.data.licences[0].token, publicRaw, "DJM1");
    expect(claims?.edition).toBe("karaoke");

    const lid = me.data.licences[0].id;
    for (let i = 0; i < 3; i++) {
      const a = await app("/api/app/activate", { method: "POST", headers: bearer, json: { licence: lid, machine: `machine-${i}-0123456789abcdef`, name: `Laptop ${i}` } });
      expect(a.status).toBe(200);
    }
    // Karaoke has three seats; a fourth machine is refused, the first again is a renewal.
    expect((await app("/api/app/activate", { method: "POST", headers: bearer, json: { licence: lid, machine: "machine-9-0123456789abcdef" } })).status).toBe(409);
    expect((await app("/api/app/activate", { method: "POST", headers: bearer, json: { licence: lid, machine: "machine-0-0123456789abcdef" } })).status).toBe(200);

    // Signing out on the machine revokes its token.
    await app("/api/app/signout", { method: "POST", headers: bearer });
    expect((await app("/api/app/me", { headers: bearer })).status).toBe(401);
  });

  it("a promo code and a voucher", async () => {
    const env = testEnv({ ADMIN_TOKEN: "crew-secret" });
    const admin = client(env);
    const crew = { authorization: "Bearer crew-secret" };
    const made = await admin("/api/admin/promos", { method: "POST", headers: crew, json: { kind: "percent", value: 50, code: "MERENGUE50", maxUses: 10 } });
    expect(made.data.codes[0].code).toBe("MERENGUE50");
    const v = await admin("/api/admin/promos", { method: "POST", headers: crew, json: { kind: "voucher", grants: { edition: "pro", plan: "monthly", months: 3 }, count: 2, prefix: "GIFT" } });
    expect(v.data.made).toBe(2);

    const web = client(env);
    await web("/api/auth/demo", { method: "POST", json: { name: "Rafa" } });
    const q = await web("/api/quote", { method: "POST", json: { edition: "pro", plan: "perpetual", promo: "merengue50", country: "US" } });
    expect(q.data.promo.ok).toBe(true);
    expect(q.data.total).toBe(q.data.regional - Math.round(q.data.regional / 2));

    const r = await web("/api/redeem", { method: "POST", json: { code: v.data.codes[0].code } });
    expect(r.status).toBe(200);
    expect(r.data.licence.edition).toBe("pro");
    const months = (r.data.licence.expires - Date.now()) / 86400_000;
    expect(months).toBeGreaterThan(88);
    expect(months).toBeLessThan(96);
    expect((await web("/api/redeem", { method: "POST", json: { code: v.data.codes[0].code } })).status).toBe(400);
  });

  it("Backstage is closed to visitors and a cross-site post is refused", async () => {
    const env = testEnv({ ADMIN_TOKEN: "crew-secret" });
    const web = client(env);
    expect((await web("/api/admin/overview")).status).toBe(403);
    expect((await web("/api/admin/overview", { headers: { authorization: "Bearer wrong" } })).status).toBe(403);
    expect((await web("/api/wishes", { method: "POST", headers: { origin: "https://evil.example" }, json: { title: "x" } })).status).toBe(403);
  });

  it("wishes and votes", async () => {
    const env = testEnv();
    const web = client(env);
    const w = await web("/api/wishes", { method: "POST", json: { title: "Serato DVS control vinyl support", body: "please" } });
    expect(w.data.votes).toBe(1);
    const again = await web(`/api/wishes/${w.data.id}/vote`, { method: "POST" });
    // The author's own vote was counted at creation; voting again takes it back.
    expect(again.data).toMatchObject({ votes: 0, voted: false });
    const list = await web("/api/wishes");
    expect(list.data.find((x: { id: string }) => x.id === w.data.id).status).toBe("idea");
  });
});
