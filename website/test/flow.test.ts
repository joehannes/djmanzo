import { describe, expect, it } from "vitest";
import { handle } from "../worker/index";
import { quote } from "../worker/checkout";
import type { User } from "../worker/store";
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
    expect(q.data.band).toBe(2);
    expect(q.data.by).toBe("region");
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

  it("a trial is seven days of any edition, once per account", async () => {
    const env = testEnv();
    const web = client(env);
    await web("/api/auth/demo", { method: "POST", json: { name: "Mabel" } });
    expect((await web("/api/trial", { method: "POST", json: { edition: "custom" } })).status).toBe(400);
    const t = await web("/api/trial", { method: "POST", json: { edition: "full" } });
    expect(t.status).toBe(200);
    expect(t.data.licence).toMatchObject({ edition: "full", plan: "trial" });
    const days = (t.data.licence.expires - Date.now()) / 86400_000;
    expect(days).toBeGreaterThan(6.99);
    expect(days).toBeLessThanOrEqual(7);
    const { publicRaw } = await keys(env);
    const claims = await verify<LicenceClaims>(t.data.token, publicRaw, "DJM1");
    expect(claims?.plan).toBe("trial");
    expect(claims?.modules).toHaveLength(15);
    const again = await web("/api/trial", { method: "POST", json: { edition: "karaoke" } });
    expect(again.status).toBe(409);
    expect((await web("/api/trial")).data.had.edition).toBe("full");
  });

  it("an upgrade costs the difference, and only towards an edition that carries everything held", async () => {
    const env = testEnv();
    const web = client(env);
    await web("/api/auth/demo", { method: "POST", json: { name: "Rosa" } });
    const hob = await web("/api/checkout/demo", { method: "POST", json: { edition: "hobbyist", plan: "perpetual", country: "US", instrument: "US", billing: "US" } });
    expect(hob.data.order.total).toBe(3900);
    const kar = await web("/api/checkout/demo", { method: "POST", json: { edition: "karaoke", plan: "perpetual", country: "US", instrument: "US", billing: "US" } });
    expect(kar.data.order.total).toBe(5500 - 3900);
    expect(kar.data.order.upgrade.credit).toBe(3900);
    // Karaoke is inside Full, not inside Pro DJ: an upgrade to Pro DJ pays in full.
    const pro = await web("/api/quote", { method: "POST", json: { edition: "pro", plan: "perpetual", country: "US" } });
    expect(pro.data.upgrade).toBeNull();
    expect(pro.data.total).toBe(11900);
    const full = await web("/api/quote", { method: "POST", json: { edition: "full", plan: "perpetual", country: "US" } });
    expect(full.data.upgrade.edition).toBe("karaoke");
    expect(full.data.total).toBe(14900 - 5500);
    // Subscriptions are not upgrades.
    expect((await web("/api/quote", { method: "POST", json: { edition: "full", plan: "monthly", country: "US" } })).data.upgrade).toBeNull();
    // The licence it was priced against is folded into the new one.
    const lics = await web("/api/me/licences");
    expect(lics.data.find((l: { edition: string }) => l.edition === "hobbyist").status).toBe("upgraded");
  });

  it("Custom is priced by its modules and points at an edition that costs no more", async () => {
    const env = testEnv();
    const web = client(env);
    expect((await web("/api/quote", { method: "POST", json: { edition: "custom", plan: "perpetual", country: "US", modules: [] } })).status).toBe(400);
    const q = await web("/api/quote", { method: "POST", json: { edition: "custom", plan: "perpetual", country: "US", modules: ["dvs", "stems", "nonsense"] } });
    expect(q.data.modules).toEqual(["stems", "dvs"]);
    expect(q.data.total).toBe(3900 + 2000 + 500);
    expect(q.data.better).toBeNull();
    const ap = await web("/api/quote", { method: "POST", json: { edition: "custom", plan: "perpetual", country: "US", modules: ["autopilot"] } });
    expect(ap.data.better).toBe("autopilot");
  });

  it("djmanzo links by the loopback redirect, and the code is good once, with its verifier", async () => {
    const env = testEnv();
    const web = client(env);
    await web("/api/auth/demo", { method: "POST", json: { name: "Lena" } });
    const verifier = "v".repeat(20) + "erifier-0123456789-abcdefghij";
    const challenge = btoa(String.fromCharCode(...new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier)))))
      .replace(/\+/g, "-")
      .replace(/\//g, "_")
      .replace(/=+$/, "");
    const approve = { port: 53682, challenge, state: "state-0123456789", machine: "Booth laptop" };
    expect((await web("/api/device/loopback", { method: "POST", headers: { origin: "https://evil.example" }, json: approve })).status).toBe(403);
    expect((await web("/api/device/loopback", { method: "POST", json: { ...approve, port: 80 } })).status).toBe(400);
    const ok = await web("/api/device/loopback", { method: "POST", json: approve });
    const back = new URL(ok.data.redirect);
    expect(back.origin).toBe("http://127.0.0.1:53682");
    expect(back.searchParams.get("state")).toBe("state-0123456789");
    const code = back.searchParams.get("code")!;
    const app = client(env);
    expect((await app("/api/device/exchange", { method: "POST", json: { code, verifier: "wrong" } })).status).toBe(400);
    // A wrong verifier spends the code: it cannot be guessed at.
    expect((await app("/api/device/exchange", { method: "POST", json: { code, verifier } })).status).toBe(400);
    const again = await web("/api/device/loopback", { method: "POST", json: approve });
    const code2 = new URL(again.data.redirect).searchParams.get("code")!;
    const tok = await app("/api/device/exchange", { method: "POST", json: { code: code2, verifier } });
    expect(tok.data.access_token).toMatch(/^djm_/);
    expect((await app("/api/app/me", { headers: { authorization: `Bearer ${tok.data.access_token}` } })).data.user.name).toBe("Lena");
  });

  it("the student price and the regional price never stack: the larger discount applies", async () => {
    const env = testEnv();
    const student: User = { id: "u_s", email: "ana@uasd.edu.do", name: "Ana", avatar: null, identities: [], createdAt: 0 };
    const plain: User = { ...student, id: "u_p", email: "ana@gmail.com" };
    const at = async (u: User, country: string) => (await quote(env as never, u, { edition: "full", plan: "perpetual", country })).total;
    expect(await at(plain, "US")).toBe(14900);
    expect(await at(student, "US")).toBe(7449);
    // The Dominican Republic pays 60 %, a student 50 %: the student price, not 30 %.
    expect(await at(plain, "DO")).toBe(8949);
    expect(await at(student, "DO")).toBe(7449);
    // Ethiopia pays 30 %, less than a student's half: the regional price.
    expect(await at(student, "ET")).toBe(await at(plain, "ET"));
    // Marked by the crew, a student needs no school address.
    expect(await at({ ...plain, student: true }, "US")).toBe(7449);
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
