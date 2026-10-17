import { describe, expect, it } from "vitest";
import { judge, makeCode, ALPHABET, mint, lookup } from "../worker/promo";
import type { Promo } from "../worker/store";
import { testEnv } from "./kv";

const base: Promo = { code: "X", kind: "percent", value: 20, uses: 0, active: true, createdAt: 0, createdBy: "t" };
const buy = { edition: "pro" as const, plan: "perpetual" as const, amount: 10000 };
const anon = { email: null, uid: null };

describe("judging a code", () => {
  it("takes a percentage off", () => expect(judge(base, buy, anon, 0)).toMatchObject({ ok: true, discount: 2000 }));
  it("takes a fixed amount off, never more than the price", () => {
    expect(judge({ ...base, kind: "fixed", value: 50000 }, buy, anon, 0).discount).toBe(10000);
  });
  it("refuses an expired code", () => expect(judge({ ...base, expires: 1 }, buy, anon, 0, 2).ok).toBe(false));
  it("refuses a used-up code", () => expect(judge({ ...base, maxUses: 3, uses: 3 }, buy, anon, 0).ok).toBe(false));
  it("refuses a second use by the same person", () => expect(judge(base, buy, anon, 1).ok).toBe(false));
  it("refuses a code meant for another edition", () => expect(judge({ ...base, editions: ["karaoke"] }, buy, anon, 0).ok).toBe(false));
  it("an individual code works only for its person", () => {
    const p = { ...base, boundTo: "ana@example.com" };
    expect(judge(p, buy, { email: "bob@example.com", uid: "u1" }, 0).ok).toBe(false);
    expect(judge(p, buy, { email: "Ana@Example.com", uid: "u2" }, 0).ok).toBe(true);
  });
  it("a voucher pays for exactly what it grants", () => {
    const v: Promo = { ...base, kind: "voucher", grants: { edition: "pro", plan: "perpetual" } };
    expect(judge(v, buy, anon, 0)).toMatchObject({ ok: true, discount: 10000 });
    expect(judge(v, { ...buy, edition: "full" }, anon, 0).ok).toBe(false);
  });
  it("a switched-off code is refused", () => expect(judge({ ...base, active: false }, buy, anon, 0).ok).toBe(false));
});

describe("minting codes", () => {
  it("makes readable codes with no look-alike characters", () => {
    const c = makeCode("BACHATA");
    expect(c).toMatch(/^BACHATA-[A-Z0-9]{4}-[A-Z0-9]{4}$/);
    for (const ch of c.replace(/^BACHATA-/, "").replace(/-/g, "")) expect(ALPHABET).toContain(ch);
    expect(c).not.toMatch(/[01OIL]/.source.replace("O", "O")); // look-alikes never appear
  });

  it("an individual batch binds one single-use code to each address", async () => {
    const env = testEnv();
    const made = await mint(env, { kind: "percent", value: 30 }, { boundTo: ["a@x.com", "b@x.com"], by: "t" });
    expect(made).toHaveLength(2);
    expect(made.map((m) => m.boundTo)).toEqual(["a@x.com", "b@x.com"]);
    expect(made.every((m) => m.maxUses === 1)).toBe(true);
    expect(await lookup(env, made[0].code.toLowerCase())).not.toBeNull();
  });

  it("refuses an impossible percentage", async () => {
    await expect(mint(testEnv(), { kind: "percent", value: 1000 }, { by: "t" })).rejects.toThrow(/between 1 and 100/);
  });
});
