/**
 * What djmanzo sells, and why it costs what it does.
 *
 * Shared by the Worker (which charges) and the site (which shows). A price
 * is never computed in the browser and trusted: the browser shows the same
 * arithmetic the Worker will do, and the Worker does it again.
 *
 * **What an edition is** comes from the application itself: `editions.json`
 * is written from `dj_core::edition` and a Rust test fails when the two
 * drift (`crates/dj-core/tests/website_editions.rs`). Nothing here may name
 * a module the application does not have.
 *
 * **What it costs** is `docs/EDITIONS.md`'s, the owner's rule (DIRECTIVE
 * §130): half of what the professional competitors charge on average, each
 * edition below the same modules bought one by one, an upgrade the
 * difference between two editions. EDITIONS.md prices buying once; the
 * owner also asked for monthly and yearly plans (§132), which follow from
 * those prices by one rule, below, and wait for the owner's nod.
 */

import PARTS from "./editions.json";

export type Edition = "autopilot" | "hobbyist" | "apprentice" | "karaoke" | "pro" | "full" | "custom";
export type Module =
  | "four_decks"
  | "stems"
  | "autopilot"
  | "assistant"
  | "teacher"
  | "karaoke"
  | "crowd"
  | "controllers"
  | "dvs"
  | "video"
  | "streaming"
  | "plugins"
  | "sources"
  | "events"
  | "replay";
/** How a licence was had. A trial is given, never sold. */
export type Plan = "perpetual" | "monthly" | "yearly" | "trial";
export type Currency = "USD" | "EUR";

interface Parts {
  editions: { slug: string; product: string; modules: string[] }[];
  modules: { slug: string; title: string; cheapest: string }[];
}
const parts = PARTS as Parts;

/** Every edition, cheapest first, as the application lists them. */
export const EDITIONS = parts.editions.map((e) => e.slug as Edition);
/** Every module, in the application's order. */
export const MODULES = parts.modules.map((m) => m.slug as Module);
/** What can be bought. */
export const PLANS: Exclude<Plan, "trial">[] = ["perpetual", "monthly", "yearly"];
/** Every edition but Custom, whose modules are the buyer's. */
export const NAMED = EDITIONS.filter((e) => e !== "custom") as Exclude<Edition, "custom">[];

export const MODULE_TITLE = Object.fromEntries(parts.modules.map((m) => [m.slug, m.title])) as Record<Module, string>;
/** The cheapest edition other than Custom that carries a module. */
export const CHEAPEST_WITH = Object.fromEntries(parts.modules.map((m) => [m.slug, m.cheapest])) as Record<Module, Edition>;
export const PRODUCT = Object.fromEntries(parts.editions.map((e) => [e.slug, e.product])) as Record<Edition, string>;
const CARRIES = Object.fromEntries(parts.editions.map((e) => [e.slug, e.modules])) as Record<Edition, Module[]>;

export function isModule(m: unknown): m is Module {
  return typeof m === "string" && (MODULES as string[]).includes(m);
}

/** The modules a licence carries: the edition's, or for Custom the buyer's choice (in the application's order). */
export function modulesOf(edition: Edition, chosen: readonly string[] = []): Module[] {
  if (edition !== "custom") return [...CARRIES[edition]];
  return MODULES.filter((m) => chosen.includes(m));
}

/** Whether every module of `a` is also in `b`: whether `b` is an upgrade from `a`. */
export function within(a: readonly Module[], b: readonly Module[]): boolean {
  return a.every((m) => b.includes(m));
}

// --- Buying once (EDITIONS.md, "Proposed prices") ------------------------------

/** In US cents. */
export const BUY_ONCE: Record<Exclude<Edition, "custom">, number> = {
  autopilot: 4900,
  hobbyist: 3900,
  apprentice: 6900,
  karaoke: 5500,
  pro: 11900,
  full: 14900,
};

/** Custom is the Hobbyist foundation, then each module at the owner's "5–10 USD each or similar". */
export const FOUNDATION = 3900;
export const MODULE_PRICE: Record<Module, number> = {
  four_decks: 500,
  dvs: 500,
  plugins: 500,
  events: 500,
  replay: 500,
  controllers: 1000,
  sources: 1000,
  video: 1000,
  streaming: 1000,
  crowd: 1000,
  // The five that each carry an edition: at $10 they would undercut it.
  stems: 2000,
  assistant: 2000,
  teacher: 2000,
  karaoke: 2000,
  autopilot: 2000,
};

/** How EDITIONS.md reached each price, in its words. */
export const REASON: Record<Edition, string> = {
  autopilot: "Below the same modules bought in Custom ($59).",
  hobbyist: "Half of the first year of a paid entry tier: djay Pro $50 and rekordbox Core about $120, average $85.",
  apprentice: "The Hobbyist's, with the two largest modules, below their Custom price ($79).",
  karaoke: "Below the same modules bought in Custom ($59); a karaoke host alone sells at $80–99.",
  pro: "Half of the professional average: Serato DJ Pro $299, VirtualDJ Pro Infinity $299, Traktor Pro 4 $149, DEX 4 Pro $199 → $236.50.",
  full: "Half of VirtualDJ Pro Infinity, the all-in-one that also carries karaoke and video; and Pro DJ plus $30.",
  custom: "The Hobbyist foundation at $39, then $5, $10 or $20 a module.",
};

/** A competitor's published price, with its source (EDITIONS.md, October 2026). */
export interface Reference {
  product: string;
  perpetual?: number;
  monthly?: number;
  yearly?: number;
  note?: string;
  source: string;
}

export const MARKET: Reference[] = [
  { product: "Serato DJ Pro", perpetual: 299, monthly: 11.99, source: "https://dj.studio/blog/professional-dj-software-price-guide" },
  { product: "Serato DJ Suite", perpetual: 499, monthly: 14.99, source: "https://www.sweetwater.com/store/detail/SeratoDJ--serato-dj-pro-software" },
  { product: "VirtualDJ Pro Infinity", perpetual: 299, monthly: 19, note: "lifetime; carries karaoke and video", source: "https://dj.studio/blog/professional-dj-software-price-guide" },
  { product: "Traktor Pro 4", perpetual: 149, note: "one-time only", source: "https://www.native-instruments.com/products/traktor-pro" },
  { product: "PCDJ DEX 4 Pro", perpetual: 199, source: "https://www.clubdjsoftware.com/blog/how-much-does-dj-software-cost" },
  { product: "rekordbox Creative", monthly: 15, note: "Core about $10, Professional about $30; subscription only", source: "https://dj.studio/blog/dj-software-subscriptions-buyers-guide" },
  { product: "djay Pro", yearly: 49.99, note: "carries its AI automix", source: "https://www.algoriddim.com/news" },
  { product: "PCDJ Karaoki", perpetual: 99, note: "a karaoke host", source: "https://pcdj.com/karaoke-software/karaoki/" },
  { product: "KaraFun Pro", monthly: 49, note: "public venues; carries a song catalogue", source: "https://pcdj.com/karaoke-software/" },
];

/** The professional subscriptions Pro DJ's month is set against. */
const PRO_SUBSCRIPTIONS = ["Serato DJ Pro", "VirtualDJ Pro Infinity", "rekordbox Creative"];

/**
 * A price a person reads as a price: the nearest amount ending in .49 or .99
 * (ties go down). In cents.
 */
export function psychological(cents: number): number {
  const dollars = Math.floor(cents / 100);
  const candidates = [dollars * 100 - 1, dollars * 100 + 49, dollars * 100 + 99].filter((c) => c > 0);
  let best = candidates[0];
  for (const c of candidates) if (Math.abs(c - cents) < Math.abs(best - cents)) best = c;
  return best;
}

/**
 * The monthly rule. Pro DJ's month is half the professional subscriptions'
 * average (Serato DJ Pro $11.99, VirtualDJ Pro $19, rekordbox Creative about
 * $15 → $15.33, half $7.66), and every other edition's month stands to its
 * one-time price as Pro DJ's does — so the editions keep EDITIONS.md's
 * proportions, and none costs more than half of what it is compared with.
 * A year is ten months.
 */
export const PRO_MONTH = (() => {
  const prices = MARKET.filter((r) => PRO_SUBSCRIPTIONS.includes(r.product)).map((r) => r.monthly!);
  return (prices.reduce((a, b) => a + b, 0) / prices.length / 2) * 100;
})();
const MONTH_SHARE = PRO_MONTH / BUY_ONCE.pro;

/** The list price, in US cents, of an edition (Custom: of its modules) on a plan. */
export function listPrice(edition: Edition, plan: Exclude<Plan, "trial">, chosen: readonly string[] = []): number {
  const once = edition === "custom" ? FOUNDATION + modulesOf("custom", chosen).reduce((sum, m) => sum + MODULE_PRICE[m], 0) : BUY_ONCE[edition];
  if (plan === "perpetual") return once;
  const month = psychological(Math.round(once * MONTH_SHARE));
  return plan === "monthly" ? month : psychological(month * 10);
}

/**
 * The edition that carries everything a Custom choice does for no more
 * money, if there is one: the checkout says so rather than sell the dearer
 * way (EDITIONS.md: "nobody is better off building an edition out of Custom").
 */
export function betterThanCustom(chosen: readonly string[]): Edition | null {
  const want = modulesOf("custom", chosen);
  const cost = listPrice("custom", "perpetual", want);
  const fits = NAMED.filter((e) => within(want, CARRIES[e]) && BUY_ONCE[e] <= cost);
  fits.sort((a, b) => BUY_ONCE[a] - BUY_ONCE[b]);
  return fits[0] ?? null;
}

/** How many computers one licence may be on at once. */
export const SEATS: Record<Edition, number> = { autopilot: 2, hobbyist: 2, apprentice: 2, karaoke: 3, pro: 3, full: 5, custom: 3 };

/** Days of the free trial, once per account (EDITIONS.md "The 7-day trial"). */
export const TRIAL_DAYS = 7;

export interface EditionInfo {
  id: Edition;
  /** As EDITIONS.md names it. */
  name: string;
  tagline: string;
  /** Who it is for, in EDITIONS.md's words. */
  forWho: string;
  colour: string;
  emoji: string;
  modules: Module[];
}

const INFO: Record<Edition, Omit<EditionInfo, "id" | "modules">> = {
  autopilot: { name: "Auto-Pilot", emoji: "🛫", colour: "#22d3ee", tagline: "Two decks the application mixes; take over whenever you like.", forWho: "Parties, bars, anyone who wants the music to run itself." },
  hobbyist: { name: "Hobbyist", emoji: "🎧", colour: "#38bdf8", tagline: "Everything basic a DJ needs, and a look at what more there is.", forWho: "A DJ playing for fun." },
  apprentice: { name: "Apprentice", emoji: "🎓", colour: "#a3e635", tagline: "The Hobbyist's, with an assistant and a teacher.", forWho: "Someone learning to DJ: the craft, the music behind it, the crowd, the first gigs and the road ahead." },
  karaoke: { name: "Karaoke", emoji: "🎤", colour: "#e879f9", tagline: "The Hobbyist's, with karaoke.", forWho: "Karaoke hosts and bars." },
  pro: { name: "Pro DJ", emoji: "🔥", colour: "#f97316", tagline: "Everything except karaoke and the teacher.", forWho: "The working DJ." },
  full: { name: "Full", emoji: "👑", colour: "#facc15", tagline: "Everything djmanzo does.", forWho: "Everyone who wants it all." },
  custom: { name: "Custom", emoji: "🧩", colour: "#94a3b8", tagline: "The Hobbyist foundation, with the modules you pick.", forWho: "A DJ who knows what they want." },
};

export const EDITION_INFO: Record<Edition, EditionInfo> = Object.fromEntries(
  EDITIONS.map((e) => [e, { id: e, ...INFO[e], modules: modulesOf(e) }]),
) as Record<Edition, EditionInfo>;

export const PLAN_INFO: Record<Plan, { name: string; per: string; about: string }> = {
  perpetual: { name: "Buy once", per: "once", about: "Version 1 for ever, with every 1.x update." },
  monthly: { name: "Monthly", per: "/ month", about: "Every update while it runs. Cancel any time." },
  yearly: { name: "Yearly", per: "/ year", about: "Every update while it runs; ten months' price." },
  trial: { name: "Trial", per: "free", about: "Seven days, once per account." },
};

export function money(cents: number, currency: Currency): string {
  const amount = cents / 100;
  const whole = Number.isInteger(amount);
  return new Intl.NumberFormat(currency === "EUR" ? "de-DE" : "en-US", {
    style: "currency",
    currency,
    minimumFractionDigits: whole ? 0 : 2,
    maximumFractionDigits: 2,
  }).format(amount);
}
