/**
 * Promo codes and vouchers.
 *
 * Three kinds. A *percent* or *fixed* code takes money off a purchase. A
 * *voucher* is a product in a code — "Pro for 12 months", "Karaoke, bought
 * once" — redeemed with no payment at all. Any of them may be *general*
 * (anyone, up to a number of uses) or *individual* (bound to one e-mail
 * address or account, for a friend, a competition winner, a refund made good).
 *
 * The check lives in `judge`, a pure function, so the shop's quote, the
 * checkout and the tests all ask the same question the same way.
 */

import type { Edition, Plan } from "../shared/catalog";
import { EDITIONS, PLANS } from "../shared/catalog";
import type { AppEnv } from "./env";
import { fail, text } from "./http";
import { get, put, type Promo, type User } from "./store";

/** No 0/O, no 1/I/L: a code read aloud over a phone call survives the trip. */
export const ALPHABET = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";

export function normalise(code: string): string {
  return String(code || "").toUpperCase().replace(/[^A-Z0-9-]/g, "").slice(0, 40);
}

export function makeCode(prefix = "MANZO", groups = 2, size = 4): string {
  const bytes = crypto.getRandomValues(new Uint8Array(groups * size));
  const chars = [...bytes].map((b) => ALPHABET[b % ALPHABET.length]);
  const parts: string[] = [];
  for (let g = 0; g < groups; g++) parts.push(chars.slice(g * size, (g + 1) * size).join(""));
  const p = normalise(prefix).replace(/-+$/, "");
  return [p, ...parts].filter(Boolean).join("-");
}

export interface Purchase {
  edition: Edition;
  plan: Plan;
  /** Cents, after the regional price and before the promo. */
  amount: number;
}

export interface Judgement {
  ok: boolean;
  /** Cents off. Never more than the amount. */
  discount: number;
  reason: string;
}

/** Whether `who` may use `promo` on `purchase` now, and for how much. Pure: `used` is how often `who` already has. */
export function judge(promo: Promo | null, purchase: Purchase, who: { email: string | null; uid: string | null }, used: number, now = Date.now()): Judgement {
  const no = (reason: string): Judgement => ({ ok: false, discount: 0, reason });
  if (!promo) return no("That code is not one we know.");
  if (!promo.active) return no("That code has been switched off.");
  if (promo.starts && now < promo.starts) return no("That code is not valid yet.");
  if (promo.expires && now > promo.expires) return no("That code has expired.");
  if (promo.maxUses != null && promo.uses >= promo.maxUses) return no("That code has been used up.");
  if (promo.boundTo) {
    const bound = promo.boundTo.toLowerCase();
    if (bound !== (who.email || "").toLowerCase() && bound !== who.uid) {
      return no("That code was made for somebody else — sign in with the account it was sent to.");
    }
  }
  if (used >= (promo.perPerson ?? 1)) return no("You have already used that code.");
  if (promo.editions?.length && !promo.editions.includes(purchase.edition)) {
    return no(`That code is for ${promo.editions.join(", ")}.`);
  }
  if (promo.plans?.length && !promo.plans.includes(purchase.plan)) {
    return no(`That code is for ${promo.plans.join(", ")} plans.`);
  }
  if (promo.kind === "voucher") {
    const g = promo.grants;
    if (!g || g.edition !== purchase.edition || g.plan !== purchase.plan) {
      return no(g ? `That voucher is for ${g.edition}, ${g.plan}. Redeem it as it is.` : "That voucher grants nothing.");
    }
    return { ok: true, discount: purchase.amount, reason: "A voucher: this one is on the house." };
  }
  if (promo.kind === "percent") {
    const pct = Math.max(0, Math.min(100, promo.value));
    const discount = Math.min(purchase.amount, Math.round((purchase.amount * pct) / 100));
    const first = purchase.plan === "perpetual" ? "" : " on the first payment";
    return { ok: true, discount, reason: `${pct} % off${first}.` };
  }
  const discount = Math.min(purchase.amount, Math.max(0, Math.round(promo.value)));
  return { ok: true, discount, reason: `${(discount / 100).toFixed(2)} off${purchase.plan === "perpetual" ? "" : " the first payment"}.` };
}

export async function lookup(env: AppEnv, code: string): Promise<Promo | null> {
  const c = normalise(code);
  return c ? get<Promo>(env.KV, `promo:${c}`) : null;
}

function whoKey(user: User): string {
  return user.email || user.id;
}

export async function usedBy(env: AppEnv, code: string, user: User | null): Promise<number> {
  if (!user) return 0;
  return Number((await env.KV.get(`promouse:${normalise(code)}:${whoKey(user)}`)) || "0");
}

/** Record one use, after a payment has gone through (or a voucher was redeemed). */
export async function consume(env: AppEnv, code: string, user: User): Promise<void> {
  const promo = await lookup(env, code);
  if (!promo) return;
  promo.uses += 1;
  await put(env.KV, `promo:${promo.code}`, promo, undefined, meta(promo));
  const k = `promouse:${promo.code}:${whoKey(user)}`;
  await env.KV.put(k, String(Number((await env.KV.get(k)) || "0") + 1));
}

export function meta(p: Promo) {
  return { k: p.kind, v: p.value, u: p.uses, m: p.maxUses ?? null, a: p.active, b: p.boundTo ?? null, c: p.campaign ?? null, x: p.expires ?? null };
}

export interface PromoSpec {
  kind: Promo["kind"];
  value?: number;
  grants?: Promo["grants"];
  editions?: Edition[];
  plans?: Plan[];
  maxUses?: number | null;
  perPerson?: number | null;
  starts?: number | null;
  expires?: number | null;
  campaign?: string | null;
  note?: string | null;
}

/** Check an admin's spec, so a typo cannot mint a 1000 % code. */
export function validSpec(spec: PromoSpec): PromoSpec {
  if (!["percent", "fixed", "voucher"].includes(spec.kind)) fail(400, "A code is percent, fixed or voucher.");
  if (spec.kind === "percent" && !(Number(spec.value) > 0 && Number(spec.value) <= 100)) fail(400, "A percentage is between 1 and 100.");
  if (spec.kind === "fixed" && !(Number(spec.value) > 0)) fail(400, "A fixed discount needs an amount in cents.");
  if (spec.kind === "voucher") {
    const g = spec.grants;
    if (!g || !EDITIONS.includes(g.edition) || !PLANS.includes(g.plan)) fail(400, "A voucher names the edition and plan it grants.");
    if (g.months != null && !(g.months >= 1 && g.months <= 36)) fail(400, "A voucher lasts 1 to 36 months.");
  }
  const editions = (spec.editions || []).filter((e) => EDITIONS.includes(e));
  const plans = (spec.plans || []).filter((p) => PLANS.includes(p));
  return {
    ...spec,
    value: spec.kind === "voucher" ? 100 : Number(spec.value),
    editions,
    plans,
    campaign: spec.campaign ? text(spec.campaign, 60) : null,
    note: spec.note ? text(spec.note, 200) : null,
  };
}

/**
 * Mint codes. `count` general codes, or one individual code for each address
 * in `boundTo`. An explicit `code` makes one memorable general code
 * ("BACHATA25") instead of random ones.
 */
export async function mint(
  env: AppEnv,
  spec: PromoSpec,
  opts: { count?: number; prefix?: string; code?: string; boundTo?: string[]; by: string },
): Promise<Promo[]> {
  const s = validSpec(spec);
  const targets: (string | null)[] = opts.boundTo?.length
    ? opts.boundTo.map((b) => b.trim().toLowerCase()).filter(Boolean).slice(0, 500)
    : Array.from({ length: Math.min(Math.max(1, opts.count ?? 1), 500) }, () => null);
  if (opts.code && targets.length !== 1) fail(400, "A chosen code is a single code.");
  const out: Promo[] = [];
  for (const boundTo of targets) {
    let code = opts.code ? normalise(opts.code) : makeCode(opts.prefix || "MANZO");
    if (!opts.code) {
      // A collision in 31^8 is unlikely, not impossible.
      for (let tries = 0; tries < 5 && (await env.KV.get(`promo:${code}`)); tries++) code = makeCode(opts.prefix || "MANZO");
    } else if (await env.KV.get(`promo:${code}`)) {
      fail(409, `${code} already exists.`);
    }
    const promo: Promo = {
      code,
      kind: s.kind,
      value: s.value ?? 0,
      grants: s.grants ?? null,
      editions: s.editions,
      plans: s.plans,
      maxUses: boundTo ? 1 : (s.maxUses ?? null),
      uses: 0,
      perPerson: s.perPerson ?? 1,
      starts: s.starts ?? null,
      expires: s.expires ?? null,
      boundTo,
      campaign: s.campaign,
      note: s.note,
      active: true,
      createdAt: Date.now(),
      createdBy: opts.by,
    };
    await put(env.KV, `promo:${code}`, promo, undefined, meta(promo));
    out.push(promo);
  }
  return out;
}
