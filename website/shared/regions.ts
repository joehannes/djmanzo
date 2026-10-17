/**
 * Regional pricing, and what a buyer has to show to get it.
 *
 * A price in Santo Domingo should not be a price set for Zürich. djmanzo
 * charges by purchasing power in four bands — but only where the buyer can
 * show they are there: the country their card was issued in **and** the
 * country of their billing address must both be in the band. An IP address
 * is only a hint; it decides what the island *shows*, never what anyone pays.
 *
 * Where the two disagree, the buyer pays the higher of the two bands' prices.
 * That is the whole rule, and `proven` is the only place it lives.
 */

import { BASE, type Currency, type Edition, type Plan, psychological } from "./catalog";

export type Band = 1 | 2 | 3 | 4;

/** What a band pays, as a share of the base price. */
export const BAND_SHARE: Record<Band, number> = { 1: 1, 2: 0.8, 3: 0.6, 4: 0.4 };

export const BAND_NAME: Record<Band, string> = {
  1: "Full price",
  2: "−20 %",
  3: "−40 %",
  4: "−60 %",
};

// Band 1 is everywhere not listed below.
const BAND_2 = "ES IT PT GR CY MT SI CZ SK EE LV LT PL HR HU TW SA KW BH OM CL UY PA CR".split(" ");
const BAND_3 =
  "MX BR AR CO PE EC DO JM TT BB BS LC GD VC DM KN AG BZ GY SR ZA TR RO BG RS ME MK AL BA MD UA GE AM AZ KZ MY TH CN PY GT".split(" ");
const BAND_4 =
  "IN PK BD LK NP NG KE GH EG MA TN DZ ET TZ UG RW SN CI CM ZM ZW MZ AO ID PH VN KH LA MM BO HN NI SV HT VE UZ KG TJ MN".split(" ");

/** Where djmanzo does not sell, because payment providers cannot take payment there. */
export const EMBARGOED = new Set("CU IR KP SY RU BY".split(" "));

export const EURO_AREA = new Set(
  "AT BE HR CY EE FI FR DE GR IE IT LV LT LU MT NL PT SK SI ES".split(" "),
);

export function band(country: string | null | undefined): Band {
  const c = (country || "").toUpperCase();
  if (BAND_4.includes(c)) return 4;
  if (BAND_3.includes(c)) return 3;
  if (BAND_2.includes(c)) return 2;
  return 1;
}

export function currencyFor(country: string | null | undefined): Currency {
  return EURO_AREA.has((country || "").toUpperCase()) ? "EUR" : "USD";
}

export function regional(edition: Edition, plan: Plan, b: Band): number {
  const base = BASE[edition][plan];
  return b === 1 ? base : psychological(Math.round(base * BAND_SHARE[b]));
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
