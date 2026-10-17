/**
 * Regional and student prices, and what a buyer has to show to get them.
 *
 * The owner's rule (DIRECTIVE §130, `docs/EDITIONS.md` "Discounts"): prices
 * by purchasing power in the World Bank's four income groups — high income
 * at the list price, upper-middle at 60 %, lower-middle at 40 %, low at
 * 30 % — and half price for students and teachers. **Discounts do not
 * stack: the larger one applies.**
 *
 * A regional price is given only where the buyer shows they are there
 * (DIRECTIVE §132): the country their card was issued in **and** the
 * country of their billing address must both be in the group. An IP address
 * is only a hint; it decides what the site *shows*, never what anyone pays.
 * Where the two disagree, the buyer pays the higher of the two prices. That
 * is the whole rule, and `proven` is the only place it lives.
 *
 * The groups are the World Bank's (data CC BY 4.0, `api.worldbank.org`,
 * fetched 17 October 2026). A country the World Bank does not group pays
 * the list price.
 */

import { listPrice, psychological, type Currency, type Edition, type Plan } from "./catalog";

/** 1 high income, 2 upper-middle, 3 lower-middle, 4 low. */
export type Band = 1 | 2 | 3 | 4;

/** What a band pays, as a share of the list price. */
export const BAND_SHARE: Record<Band, number> = { 1: 1, 2: 0.6, 3: 0.4, 4: 0.3 };

export const BAND_NAME: Record<Band, string> = {
  1: "List price",
  2: "−40 %",
  3: "−60 %",
  4: "−70 %",
};

/** What a student or a teacher pays, as a share of the list price. */
export const STUDENT_SHARE = 0.5;

const UPPER_MIDDLE =
  "AL AM AR AZ BA BR BW BY BZ CN CO CU CV DM DO DZ EC FJ FM GA GD GE GQ GT ID IQ IR JM JO KZ LC LK LY MD ME MH MK MN MU MV MX MY PE PH PY RS SR SV TH TM TO TR TV UA VC VN WS XK ZA".split(" ");
const LOWER_MIDDLE =
  "AO BD BJ BO BT CG CI CM DJ EG GH GN HN HT IN KE KG KH KI KM LA LB LS MA MM MR NA NG NI NP PG PK PS SB SN ST SZ TG TJ TL TN TZ UZ VE VU ZM ZW".split(" ");
const LOW = "AF BF BI CD CF ER ET GM GW KP LR MG ML MW MZ NE RW SD SL SO SS SY TD UG YE".split(" ");

/** Where djmanzo does not sell, because payment providers cannot take payment there. */
export const EMBARGOED = new Set("CU IR KP SY RU BY".split(" "));

export const EURO_AREA = new Set("AT BE HR CY EE FI FR DE GR IE IT LV LT LU MT NL PT SK SI ES".split(" "));

export function band(country: string | null | undefined): Band {
  const c = (country || "").toUpperCase();
  if (LOW.includes(c)) return 4;
  if (LOWER_MIDDLE.includes(c)) return 3;
  if (UPPER_MIDDLE.includes(c)) return 2;
  return 1;
}

export function currencyFor(country: string | null | undefined): Currency {
  return EURO_AREA.has((country || "").toUpperCase()) ? "EUR" : "USD";
}

function share(cents: number, s: number): number {
  return s === 1 ? cents : psychological(Math.round(cents * s));
}

/** The price in a band, made a readable price. */
export function regional(edition: Edition, plan: Exclude<Plan, "trial">, b: Band, chosen: readonly string[] = []): number {
  return share(listPrice(edition, plan, chosen), BAND_SHARE[b]);
}

/** The student and teacher price. */
export function studentPrice(edition: Edition, plan: Exclude<Plan, "trial">, chosen: readonly string[] = []): number {
  return share(listPrice(edition, plan, chosen), STUDENT_SHARE);
}

/**
 * The address of a school, as far as an e-mail domain can say it: `.edu`,
 * `.edu.xx`, `.ac.xx`, `.k12.…`, or a domain the crew has added. A verified
 * address there is the proof; anyone else asks the crew, who can mark the
 * account by hand.
 */
export function academic(email: string | null | undefined, extra: readonly string[] = []): boolean {
  const domain = (email || "").toLowerCase().split("@")[1] ?? "";
  if (!domain) return false;
  if (/(^|\.)edu(\.[a-z]{2})?$/.test(domain)) return true;
  if (/(^|\.)ac\.[a-z]{2}$/.test(domain)) return true;
  if (/(^|\.)k12\.[a-z]{2}\.[a-z]{2}$/.test(domain)) return true;
  return extra.some((d) => domain === d || domain.endsWith(`.${d}`));
}

export interface Proof {
  /** The country the payment card was issued in (Stripe's `card.country`), or a bank account's country. */
  instrument?: string | null;
  /** The billing address's country. */
  billing?: string | null;
}

export interface ProvenBand {
  band: Band;
  /** Why this band and not a lower one, in a sentence for the buyer. */
  reason: string;
  /** True when the band the buyer asked for was granted. */
  granted: boolean;
  /** True when no sale can be made at all. */
  refused?: boolean;
}

/**
 * The band a payment proves. Both countries must be present; the band paid
 * is the *higher-priced* (lower-numbered) of the two, so neither a foreign
 * card nor a borrowed address on its own lowers a price.
 */
export function proven(proof: Proof, claimed: Band): ProvenBand {
  const instrument = (proof.instrument || "").toUpperCase();
  const billing = (proof.billing || "").toUpperCase();
  if (!instrument || !billing) {
    return {
      band: 1,
      granted: claimed === 1,
      reason: "A regional price needs both the card's country and the billing address; one was missing.",
    };
  }
  if (EMBARGOED.has(instrument) || EMBARGOED.has(billing)) {
    return { band: 1, granted: false, refused: true, reason: "We cannot take payment for this country." };
  }
  const b = Math.min(band(instrument), band(billing)) as Band;
  if (b >= claimed) {
    return { band: claimed, granted: true, reason: `Card from ${instrument} and billing address in ${billing}: the regional price stands.` };
  }
  return {
    band: b,
    granted: false,
    reason:
      instrument === billing
        ? `Your card and address are in ${instrument}, which is priced at ${BAND_NAME[b]}.`
        : `Card from ${instrument}, billing address in ${billing}: the higher of the two prices applies.`,
  };
}
