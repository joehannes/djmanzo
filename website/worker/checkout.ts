/**
 * The box office: a quote, a payment, a licence.
 *
 * Every path ends in `issue`, and every path prices through `quote`, so a
 * card, PayPal, a bank transfer, a voucher and the demo cannot disagree about
 * what djmanzo costs or what a buyer gets for it.
 *
 * The regional price is *claimed* when the quote is made (the buyer picks
 * where they are; the island guesses from the IP) and *proven* when the
 * payment is made (`shared/regions.ts`). When the proof does not back the
 * claim the buyer is told the price it does back and asked again — nobody is
 * charged a price they did not see.
 */

import { EDITIONS, PLANS, PLAN_INFO, EDITION_INFO, type Currency, type Edition, type Plan, money } from "../shared/catalog";
import { band, currencyFor, proven, regional, type Band, type Proof, EMBARGOED } from "../shared/regions";
import type { AppEnv } from "./env";
import { demoMode } from "./auth";
import { fail, randomId } from "./http";
import { inForce, licenceToken, termEnd } from "./licence";
import { consume, judge, lookup, usedBy, type Judgement } from "./promo";
import { stripe, verifyWebhook } from "./stripe";
import { paypal } from "./paypal";
import { get, put, saveLicence, type Licence, type Order, type User } from "./store";

export interface QuoteInput {
  edition: Edition;
  plan: Plan;
  promo?: string | null;
  /** The country the buyer says they are in. */
  country?: string | null;
}

export interface Quote {
  edition: Edition;
  plan: Plan;
  currency: Currency;
  country: string | null;
  band: Band;
  base: number;
  regional: number;
  discount: number;
  total: number;
  promo: (Judgement & { code: string }) | null;
  renews: boolean;
}

export function checkInput(input: Partial<QuoteInput>): QuoteInput {
  if (!EDITIONS.includes(input.edition as Edition)) fail(400, "Pick an edition: hobbyist, pro, karaoke or full.");
  if (!PLANS.includes(input.plan as Plan)) fail(400, "Pick a plan: monthly, yearly or buy once.");
  const country = input.country ? String(input.country).toUpperCase().slice(0, 2) : null;
  if (country && EMBARGOED.has(country)) fail(451, "We cannot take payment for this country.");
  return { edition: input.edition as Edition, plan: input.plan as Plan, promo: input.promo ? String(input.promo) : null, country };
}

export async function quote(env: AppEnv, user: User | null, input: QuoteInput, atBand?: Band): Promise<Quote> {
  const b = atBand ?? band(input.country);
  const regionalPrice = regional(input.edition, input.plan, b);
  let promo: Quote["promo"] = null;
  let discount = 0;
  if (input.promo) {
    const p = await lookup(env, input.promo);
    const used = await usedBy(env, input.promo, user);
    const j = judge(p, { edition: input.edition, plan: input.plan, amount: regionalPrice }, { email: user?.email ?? null, uid: user?.id ?? null }, used);
    promo = { ...j, code: p?.code ?? input.promo.toUpperCase() };
    if (j.ok) discount = j.discount;
  }
  return {
    edition: input.edition,
    plan: input.plan,
    currency: currencyFor(input.country),
    country: input.country ?? null,
    band: b,
    base: regional(input.edition, input.plan, 1),
    regional: regionalPrice,
    discount,
    total: regionalPrice - discount,
    promo,
    renews: input.plan !== "perpetual",
  };
}

function describe(q: Quote): string {
  return `djmanzo ${EDITION_INFO[q.edition].name} — ${PLAN_INFO[q.plan].name}`;
}

async function newOrder(env: AppEnv, user: User, q: Quote, claimed: Band, proof: Order["proof"], provider: Order["provider"], renews: boolean): Promise<Order> {
  const order: Order = {
    id: `o_${randomId(12)}`,
    uid: user.id,
    edition: q.edition,
    plan: q.plan,
    currency: q.currency,
    base: q.base,
    regional: q.regional,
    discount: q.discount,
    total: q.total,
    band: q.band,
    claimedBand: claimed,
    proof,
    promo: q.promo?.ok ? q.promo.code : null,
    provider,
    status: "pending",
    renews,
    createdAt: Date.now(),
    email: user.email,
    name: user.name,
  };
  await put(env.KV, `order:${order.id}`, order, undefined, { u: user.id, s: order.status, t: order.total, c: order.currency, e: order.edition, p: order.plan, v: provider, at: order.createdAt });
  return order;
}

async function saveOrder(env: AppEnv, order: Order): Promise<void> {
  await put(env.KV, `order:${order.id}`, order, undefined, { u: order.uid, s: order.status, t: order.total, c: order.currency, e: order.edition, p: order.plan, v: order.provider, at: order.createdAt });
}

export async function userLicences(env: AppEnv, uid: string): Promise<Licence[]> {
  const ids = (await get<string[]>(env.KV, `licences:${uid}`)) ?? [];
  const all = await Promise.all(ids.map((id) => get<Licence>(env.KV, `licence:${id}`)));
  return all.filter((l): l is Licence => !!l);
}

/**
 * Turn a paid order into a licence. Idempotent: an order that already has
 * one returns it, so a webhook and a browser finishing the same payment
 * cannot make two.
 */
export async function issue(env: AppEnv, order: Order, user: User, opts: { expires?: number | null; subscription?: string | null; months?: number } = {}): Promise<{ licence: Licence; token: string }> {
  if (order.licence) {
    const existing = await get<Licence>(env.KV, `licence:${order.licence}`);
    if (existing) return { licence: existing, token: await licenceToken(env, existing, user) };
  }
  const licence: Licence = {
    id: `l_${randomId(12)}`,
    uid: user.id,
    edition: order.edition,
    plan: order.plan,
    major: Number(env.MAJOR_VERSION || 1),
    status: "active",
    expires: opts.expires !== undefined ? opts.expires : termEnd(order.plan, Date.now(), opts.months),
    order: order.id,
    demo: order.provider === "demo" || !!user.demo,
    activations: [],
    createdAt: Date.now(),
    subscription: opts.subscription ?? null,
  };
  await put(env.KV, `licence:${licence.id}`, licence, undefined, { u: user.id, e: licence.edition, p: licence.plan, s: licence.status, d: !!licence.demo });
  const ids = (await get<string[]>(env.KV, `licences:${user.id}`)) ?? [];
  await put(env.KV, `licences:${user.id}`, [...ids, licence.id]);
  if (licence.subscription) await env.KV.put(`stripe:sub:${licence.subscription}`, licence.id);
  order.status = "paid";
  order.paidAt = Date.now();
  order.licence = licence.id;
  await saveOrder(env, order);
  if (order.promo) await consume(env, order.promo, user);
  return { licence, token: await licenceToken(env, licence, user) };
}

function needsConfirm(q: Quote, reason: string) {
  return fail(409, reason, {
    needsConfirm: true,
    band: q.band,
    total: q.total,
    display: money(q.total, q.currency),
  });
}

function realBuyer(user: User): void {
  if (user.demo) fail(403, "A demo account cannot buy a real licence — sign in with a real account.");
}

// --- Demo -------------------------------------------------------------------

/**
 * Demo mode's checkout: the same quote, the same proof and the same licence,
 * with the card's and address's countries typed in rather than read from a
 * payment. The licence is marked demo and djmanzo says so wherever it shows.
 */
export async function demoCheckout(env: AppEnv, user: User, input: QuoteInput & Proof & { acceptHigher?: boolean }) {
  if (!demoMode(env)) fail(404, "The demo checkout is off: this shop takes real payments.");
  const claimed = band(input.country);
  const proof = proven({ instrument: input.instrument, billing: input.billing }, claimed);
  if (proof.refused) fail(451, proof.reason);
  const q = await quote(env, user, input, proof.band);
  if (!proof.granted && !input.acceptHigher) needsConfirm(q, proof.reason);
  const order = await newOrder(env, user, q, claimed, { instrument: input.instrument, billing: input.billing, reason: proof.reason }, "demo", false);
  const out = await issue(env, order, user);
  return { order, ...out, proof };
}

// --- Nothing to pay ---------------------------------------------------------

export async function freeCheckout(env: AppEnv, user: User, input: QuoteInput) {
  const q = await quote(env, user, input);
  if (q.total !== 0) fail(402, "There is still something to pay.");
  const order = await newOrder(env, user, q, q.band, { reason: "Nothing to pay." }, "free", false);
  return { order, ...(await issue(env, order, user)) };
}

/** A voucher, redeemed for what it grants. */
export async function redeem(env: AppEnv, user: User, code: string) {
  const p = await lookup(env, code);
  if (!p || p.kind !== "voucher" || !p.grants) fail(404, "That is not a voucher we know.");
  const q = await quote(env, user, { edition: p.grants.edition, plan: p.grants.plan, promo: p.code, country: null });
  if (!q.promo?.ok) fail(400, q.promo?.reason || "That voucher cannot be used.");
  const order = await newOrder(env, user, q, 1, { reason: "Voucher." }, "voucher", false);
  return { order, ...(await issue(env, order, user, { months: p.grants.months })) };
}

// --- Stripe -----------------------------------------------------------------

async function stripeCustomer(env: AppEnv, user: User): Promise<string> {
  const key = `stripe:customer:${user.id}`;
  const known = await env.KV.get(key);
  if (known) return known;
  const c = await stripe(env, "POST", "customers", { email: user.email ?? undefined, name: user.name, metadata: { uid: user.id } }, `cust-${user.id}`);
  await env.KV.put(key, c.id);
  return c.id;
}

async function stripeProduct(env: AppEnv, edition: Edition): Promise<string> {
  const key = `stripe:product:${edition}`;
  const known = await env.KV.get(key);
  if (known) return known;
  const p = await stripe(env, "POST", "products", { name: `djmanzo ${EDITION_INFO[edition].name}`, metadata: { edition } }, `product-${edition}`);
  await env.KV.put(key, p.id);
  return p.id;
}

export async function stripeConfirm(env: AppEnv, user: User, origin: string, input: QuoteInput & { paymentMethod: string; acceptHigher?: boolean }) {
  realBuyer(user);
  if (!/^pm_[A-Za-z0-9]+$/.test(input.paymentMethod || "")) fail(400, "The card form did not finish.");
  const pm = await stripe(env, "GET", `payment_methods/${input.paymentMethod}`);
  const instrument: string | null = pm.card?.country ?? pm.sepa_debit?.country ?? pm.link?.country ?? null;
  const billing: string | null = pm.billing_details?.address?.country ?? null;
  const claimed = band(input.country);
  const proof = proven({ instrument, billing }, claimed);
  if (proof.refused) fail(451, proof.reason);
  const q = await quote(env, user, { ...input, country: input.country ?? billing }, proof.band);
  if (!proof.granted && !input.acceptHigher) needsConfirm(q, proof.reason);
  if (q.total === 0) return freeCheckout(env, user, input);

  const customer = await stripeCustomer(env, user);
  await stripe(env, "POST", `payment_methods/${input.paymentMethod}/attach`, { customer });
  const order = await newOrder(env, user, q, claimed, { instrument, billing, reason: proof.reason }, "stripe", q.plan !== "perpetual");
  const currency = q.currency.toLowerCase();

  if (q.plan === "perpetual") {
    const pi = await stripe(
      env,
      "POST",
      "payment_intents",
      {
        amount: q.total,
        currency,
        customer,
        payment_method: input.paymentMethod,
        confirm: true,
        description: describe(q),
        return_url: `${origin}/?order=${order.id}`,
        metadata: { order: order.id, uid: user.id },
      },
      `pi-${order.id}`,
    );
    order.providerRef = pi.id;
    await saveOrder(env, order);
    if (pi.status === "succeeded") return { order, ...(await issue(env, order, user)), proof };
    if (pi.status === "requires_action") return { order, clientSecret: pi.client_secret, requiresAction: true, proof };
    order.status = "failed";
    await saveOrder(env, order);
    fail(402, "The payment did not go through.");
  }

  let coupon: string | undefined;
  if (q.discount > 0) {
    const c = await stripe(env, "POST", "coupons", { amount_off: q.discount, currency, duration: "once", name: q.promo?.code }, `coupon-${order.id}`);
    coupon = c.id;
  }
  const sub = await stripe(
    env,
    "POST",
    "subscriptions",
    {
      customer,
      default_payment_method: input.paymentMethod,
      payment_behavior: "allow_incomplete",
      items: [{ price_data: { currency, product: await stripeProduct(env, q.edition), unit_amount: q.regional, recurring: { interval: q.plan === "yearly" ? "year" : "month" } } }],
      ...(coupon ? { discounts: [{ coupon }] } : {}),
      metadata: { order: order.id, uid: user.id },
      expand: ["latest_invoice.payment_intent"],
    },
    `sub-${order.id}`,
  );
  order.providerRef = sub.id;
  await saveOrder(env, order);
  if (sub.status === "active" || sub.status === "trialing") {
    return { order, ...(await issue(env, order, user, { expires: sub.current_period_end * 1000 + 3 * 86400_000, subscription: sub.id })), proof };
  }
  const pi = sub.latest_invoice?.payment_intent;
  if (pi?.status === "requires_action") return { order, clientSecret: pi.client_secret, requiresAction: true, proof };
  order.status = "failed";
  await saveOrder(env, order);
  fail(402, "The payment did not go through.");
}

/** After 3-D Secure: ask Stripe how it ended. */
export async function stripeFinalize(env: AppEnv, user: User, orderId: string) {
  const order = await get<Order>(env.KV, `order:${orderId}`);
  if (!order || order.uid !== user.id || order.provider !== "stripe" || !order.providerRef) fail(404, "No such order.");
  if (order.plan === "perpetual") {
    const pi = await stripe(env, "GET", `payment_intents/${order.providerRef}`);
    if (pi.status !== "succeeded") fail(402, "The payment has not gone through.");
    return { order, ...(await issue(env, order, user)) };
  }
  const sub = await stripe(env, "GET", `subscriptions/${order.providerRef}`);
  if (sub.status !== "active" && sub.status !== "trialing") fail(402, "The payment has not gone through.");
  return { order, ...(await issue(env, order, user, { expires: sub.current_period_end * 1000 + 3 * 86400_000, subscription: sub.id })) };
}

/** Stripe telling us something happened: a renewal, a cancellation, a refund, a payment the browser never reported. */
export async function stripeWebhook(env: AppEnv, req: Request): Promise<Response> {
  if (!env.STRIPE_WEBHOOK_SECRET) fail(503, "No webhook secret is set.");
  const payload = await req.text();
  if (!(await verifyWebhook(env.STRIPE_WEBHOOK_SECRET, req.headers.get("stripe-signature"), payload))) fail(400, "Bad signature.");
  const event = JSON.parse(payload) as { type: string; data: { object: Record<string, any> } };
  const o = event.data.object;
  const licenceFor = async (sub: string | undefined) => {
    const lid = sub ? await env.KV.get(`stripe:sub:${sub}`) : null;
    return lid ? get<Licence>(env.KV, `licence:${lid}`) : null;
  };
  if (event.type === "invoice.paid") {
    const licence = await licenceFor(o.subscription);
    const end = o.lines?.data?.[0]?.period?.end;
    if (licence && end) {
      licence.expires = Math.max(licence.expires ?? 0, end * 1000 + 3 * 86400_000);
      licence.status = "active";
      await saveLicence(env.KV, licence);
    }
  } else if (event.type === "customer.subscription.deleted") {
    const licence = await licenceFor(o.id);
    if (licence) {
      // It runs to the end of what was paid for, then stops.
      licence.status = "cancelled";
      await saveLicence(env.KV, licence);
    }
  } else if (event.type === "payment_intent.succeeded" && o.metadata?.order) {
    const order = await get<Order>(env.KV, `order:${o.metadata.order}`);
    const user = order && (await get<User>(env.KV, `user:${order.uid}`));
    if (order && user && order.plan === "perpetual" && !order.licence) await issue(env, order, user);
  } else if (event.type === "charge.refunded" && o.payment_intent) {
    // Find the order by the payment intent through its metadata.
    const pi = await stripe(env, "GET", `payment_intents/${o.payment_intent}`).catch(() => null);
    const order = pi?.metadata?.order ? await get<Order>(env.KV, `order:${pi.metadata.order}`) : null;
    if (order) {
      order.status = "refunded";
      await saveOrder(env, order);
      const licence = order.licence ? await get<Licence>(env.KV, `licence:${order.licence}`) : null;
      if (licence) {
        licence.status = "revoked";
        await saveLicence(env.KV, licence);
      }
    }
  }
  return new Response("ok");
}

// --- PayPal -----------------------------------------------------------------

/**
 * PayPal sells every plan as a prepaid term that does not renew: a month, a
 * year, or the version for ever. Renewing through PayPal needs plans made in
 * its dashboard ahead of time; a prepaid term needs nothing and is honest
 * about what it is.
 */
export async function paypalCreate(env: AppEnv, user: User, input: QuoteInput) {
  realBuyer(user);
  const q = await quote(env, user, input);
  if (q.total === 0) fail(400, "Nothing to pay — use the free checkout.");
  const order = await newOrder(env, user, q, q.band, { reason: "Waiting for PayPal." }, "paypal", false);
  const pp = await paypal(
    env,
    "POST",
    "/v2/checkout/orders",
    {
      intent: "AUTHORIZE",
      purchase_units: [{ reference_id: order.id, description: describe(q), amount: { currency_code: q.currency, value: (q.total / 100).toFixed(2) } }],
    },
    `create-${order.id}`,
  );
  order.providerRef = pp.id;
  await saveOrder(env, order);
  return { order: order.id, paypalOrder: pp.id };
}

export async function paypalCapture(env: AppEnv, user: User, orderId: string, acceptHigher = false) {
  const order = await get<Order>(env.KV, `order:${orderId}`);
  if (!order || order.uid !== user.id || order.provider !== "paypal" || !order.providerRef) fail(404, "No such order.");
  if (order.licence) return { order, ...(await issue(env, order, user)) };
  const auth = await paypal(env, "POST", `/v2/checkout/orders/${order.providerRef}/authorize`, {}, `auth-${order.id}`);
  const authorization = auth.purchase_units?.[0]?.payments?.authorizations?.[0];
  // PayPal vouches for the account's country; the billing country is the card's or the address PayPal holds.
  const instrument: string | null = auth.payment_source?.card?.bin_details?.issuing_country_code ?? auth.payer?.address?.country_code ?? null;
  const billing: string | null =
    auth.payment_source?.card?.billing_address?.country_code ??
    auth.payment_source?.paypal?.address?.country_code ??
    auth.purchase_units?.[0]?.shipping?.address?.country_code ??
    auth.payer?.address?.country_code ??
    null;
  const proof = proven({ instrument, billing }, order.claimedBand);
  if (proof.refused || (!proof.granted && !acceptHigher)) {
    if (authorization?.id) await paypal(env, "POST", `/v2/payments/authorizations/${authorization.id}/void`).catch(() => null);
    order.status = "cancelled";
    order.proof = { instrument, billing, reason: proof.reason };
    await saveOrder(env, order);
    if (proof.refused) fail(451, proof.reason);
    const q = await quote(env, user, { edition: order.edition, plan: order.plan, promo: order.promo, country: instrument }, proof.band);
    needsConfirm(q, `${proof.reason} Nothing was taken — start again at ${money(q.total, q.currency)}.`);
  }
  const cap = await paypal(env, "POST", `/v2/payments/authorizations/${authorization.id}/capture`, {}, `cap-${order.id}`);
  if (cap.status !== "COMPLETED" && cap.status !== "PENDING") fail(402, "PayPal did not complete the payment.");
  order.proof = { instrument, billing, reason: proof.reason };
  return { order, ...(await issue(env, order, user)), proof };
}

// --- Bank transfer ----------------------------------------------------------

/**
 * For a buyer with no card PayPal will take — common across the Caribbean.
 * The order waits; the owner marks it paid in Backstage after reading the
 * transfer, whose sending bank's country is the proof.
 */
export async function transferCheckout(env: AppEnv, user: User, input: QuoteInput & { billing?: string }) {
  realBuyer(user);
  if (!env.BANK_TRANSFER_DETAILS) fail(404, "Bank transfer is not offered.");
  const q = await quote(env, user, input);
  const order = await newOrder(env, user, q, q.band, { billing: input.billing ?? null, reason: "Proven by the sending bank, checked by hand." }, "transfer", false);
  return { order, instructions: env.BANK_TRANSFER_DETAILS, reference: order.id, display: money(q.total, q.currency) };
}

export async function markPaid(env: AppEnv, orderId: string, bankCountry: string | null) {
  const order = await get<Order>(env.KV, `order:${orderId}`);
  if (!order) fail(404, "No such order.");
  const user = await get<User>(env.KV, `user:${order.uid}`);
  if (!user) fail(404, "The buyer's account is gone.");
  const proof = proven({ instrument: bankCountry, billing: order.proof.billing ?? bankCountry }, order.claimedBand);
  order.proof = { ...order.proof, instrument: bankCountry, reason: proof.reason };
  return { ...(await issue(env, order, user)), proof };
}

export { inForce };
