/**
 * What djmanzo sells, and why it costs what it does.
 *
 * Shared by the Worker (which charges) and the island (which shows). A price
 * is never computed in the browser and trusted: the browser shows the same
 * arithmetic the Worker will do, and the Worker does it again.
 *
 * The owner's rule: find what comparable software typically costs, and
 * charge half. `MARKET` holds the references, each with where it came from,
 * and the site shows them beside our price so the halving can be checked.
 */

export type Edition = "hobbyist" | "pro" | "karaoke" | "full";
export type Plan = "monthly" | "yearly" | "perpetual";
export type Currency = "USD" | "EUR";

export const EDITIONS: Edition[] = ["hobbyist", "pro", "karaoke", "full"];
export const PLANS: Plan[] = ["monthly", "yearly", "perpetual"];

/** A competitor's published price, in US dollars, with its source. */
export interface Reference {
  product: string;
  monthly?: number;
  yearly?: number;
  perpetual?: number;
  note?: string;
  source: string;
}

/**
 * Comparable products' prices as published in October 2026. Where a vendor
 * prices in pounds the dollar figure is an approximation and says so.
 */
export const MARKET: Record<Edition, Reference[]> = {
  hobbyist: [
    { product: "VirtualDJ Home", monthly: 4, yearly: 44, note: "home use only, entry-level controllers", source: "https://www.virtualdj.com/buy/" },
    { product: "djay Pro", monthly: 6.99, yearly: 49.99, source: "https://dj.studio/blog/dj-software-subscriptions-buyers-guide" },
    { product: "rekordbox Core", monthly: 15, yearly: 150, note: "≈ £12 / £120, converted", source: "https://rekordbox.com/plan/" },
    { product: "Traktor Pro 4", perpetual: 149, source: "https://www.native-instruments.com/en/products/traktor/dj-software/traktor-pro-4/pricing/" },
  ],
  pro: [
    { product: "VirtualDJ Pro", monthly: 19, yearly: 199, perpetual: 299, note: "perpetual is the Infinity licence", source: "https://www.virtualdj.com/buy/" },
    { product: "Serato DJ Pro", monthly: 11.99, perpetual: 299, source: "https://dj.studio/blog/professional-dj-software-price-guide" },
    { product: "rekordbox Creative", monthly: 22, yearly: 225, note: "≈ £18 / £180, converted", source: "https://rekordbox.com/plan/" },
  ],
  karaoke: [
    { product: "VirtualDJ Pro (karaoke built in)", monthly: 19, yearly: 199, perpetual: 299, note: "song catalogue is $19.99/mo extra", source: "https://www.virtualdj.com/buy/" },
    { product: "KaraFun Pro", monthly: 49, note: "includes a song catalogue and a commercial licence; not software-only", source: "https://www.karafun.com/subscribe.html" },
  ],
  full: [
    { product: "Serato DJ Suite", monthly: 14.99, perpetual: 499, source: "https://dj.studio/blog/professional-dj-software-price-guide" },
    { product: "rekordbox Professional", monthly: 36, yearly: 360, source: "https://rekordbox.com/plan/" },
    { product: "VirtualDJ Pro + karaoke catalogue", monthly: 38.99, yearly: 438, source: "https://www.virtualdj.com/buy/" },
  ],
};

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
 * Our base price in cents (USD; the same figures in EUR inside the euro area,
 * as most software is priced): half the typical reference, made a readable
 * price. One correction: a year costs at most about ten months, because a
 * yearly plan that saves nothing is not a plan.
 *
 * Karaoke is compared with *software-only* karaoke (VirtualDJ Pro), because
 * djmanzo comes with no song catalogue; KaraFun's price includes one and is
 * shown for honesty, not used for the halving.
 */
export function basePrice(edition: Edition, plan: Plan): number {
  const half = (p: Plan) => psychological(Math.round(((typical(edition, p) ?? 0) * 100) / 2));
  if (plan !== "yearly") return half(plan);
  return Math.min(half("yearly"), psychological(half("monthly") * 10));
}

export const BASE: Record<Edition, Record<Plan, number>> = Object.fromEntries(
  EDITIONS.map((e) => [e, Object.fromEntries(PLANS.map((p) => [p, basePrice(e, p)]))]),
) as Record<Edition, Record<Plan, number>>;

/** How many machines one licence may be activated on at once. */
export const SEATS: Record<Edition, number> = { hobbyist: 2, pro: 3, karaoke: 3, full: 5 };

export interface EditionInfo {
  id: Edition;
  name: string;
  tagline: string;
  /** Who it is for, in a sentence a person would say. */
  forWho: string;
  colour: string;
  emoji: string;
  /** Feature ids from FEATURES below that this edition includes. */
  features: string[];
  commercial: boolean;
}

/**
 * What djmanzo does, as shipped in the application today (docs/FEATURES.md).
 * `status` is honest: "beta" means built and tested but not yet through a
 * real gig; "coming" means on the roadmap and not in the app.
 */
export interface Feature {
  id: string;
  name: string;
  group: "decks" | "mix" | "fx" | "library" | "hardware" | "karaoke" | "assist" | "live" | "visual";
  about: string;
  status: "shipped" | "beta" | "coming";
}

export const FEATURES: Feature[] = [
  { id: "decks4", name: "2–4 decks", group: "decks", about: "Play, cue, sync, keylock, pitch ranges up to ±100 %, hot cues, loops, beat jump.", status: "shipped" },
  { id: "decks6", name: "6 decks", group: "decks", about: "Up to six decks for layered sets and stem performance.", status: "shipped" },
  { id: "mixer", name: "Full mixer", group: "mix", about: "Isolator EQ with kills, filter, gain from loudness analysis, headphone cue, booth out, limiter.", status: "shipped" },
  { id: "fx", name: "FX rack", group: "fx", about: "Echo, delay, reverb, gate, crush, flanger, phaser, filter — beat-synced, three slots per deck and master.", status: "shipped" },
  { id: "sampler", name: "Sampler", group: "fx", about: "Banks of pads, one-shot / loop / hold / stutter, synced to the master tempo, record from a deck.", status: "shipped" },
  { id: "stems", name: "Neural stems", group: "fx", about: "Vocals, drums, bass and the rest, separated ahead of the playhead — no added latency. Per-stem EQ, swap stems across decks.", status: "shipped" },
  { id: "clap", name: "CLAP plugins", group: "fx", about: "Host CLAP effect plugins on the master.", status: "shipped" },
  { id: "library", name: "Library & importers", group: "library", about: "SQLite collection, smart folders, imports from rekordbox, Serato, Traktor and iTunes with cues and grids.", status: "shipped" },
  { id: "analysis", name: "Analysis", group: "library", about: "BPM, key (Camelot), loudness, phrases — and 31 genre families from bachata to techno.", status: "shipped" },
  { id: "find", name: "Find it from memory", group: "library", about: "Search by a half-remembered line, a description, or a hum.", status: "shipped" },
  { id: "keyboard", name: "Keyboard as a controller", group: "hardware", about: "A laptop with nothing plugged in is a playable instrument.", status: "shipped" },
  { id: "midi", name: "MIDI controllers", group: "hardware", about: "Class-compliant MIDI, a mapping editor that learns, LED feedback, Lua in mappings.", status: "shipped" },
  { id: "hid", name: "HID & motorised platters", group: "hardware", about: "HID controllers and motorised turntable controllers with absolute platter position.", status: "shipped" },
  { id: "dvs", name: "Timecode vinyl (DVS)", group: "hardware", about: "Real turntables drive a deck, relative or absolute, with live calibration.", status: "beta" },
  { id: "clock", name: "MIDI clock, OSC, network API", group: "hardware", about: "Clock in and out, OSC for TouchOSC and QLab, a JSON control API for scripts and Stream Decks.", status: "shipped" },
  { id: "prolink", name: "Pro DJ Link & StagelinQ", group: "hardware", about: "Join a booth of CDJs or Denon Prime players as a peer.", status: "coming" },
  { id: "assistant", name: "The assistant", group: "assist", about: "Next-track rail, transition planner, set planning, automix — it proposes, you decide. It speaks only in actions.", status: "shipped" },
  { id: "autopilot", name: "Autopilot & postures", group: "assist", about: "Hand the night over in six postures, take any control back with one touch.", status: "beta" },
  { id: "coach", name: "Technique coach", group: "assist", about: "Twenty-eight DJ moves, what each needs and when it works; the coach names what your hands just did.", status: "shipped" },
  { id: "record", name: "Record & replay", group: "live", about: "Record the master; a set is data — replay it, re-render it, diff two takes.", status: "shipped" },
  { id: "requests", name: "Audience requests", group: "live", about: "The room requests songs from a QR code — no app to install.", status: "shipped" },
  { id: "stream", name: "Live-stream overlays", group: "live", about: "Now-playing overlay for OBS on Twitch, YouTube, TikTok, Kick.", status: "shipped" },
  { id: "visuals", name: "Visuals", group: "visual", about: "Milkdrop-style visuals, video on a deck, a second screen for the room.", status: "beta" },
  { id: "themes", name: "29 themes & skins", group: "visual", about: "From Pro Dark to Bachata Rosa, Malecón Night, Carnaval and Industrial Techno.", status: "shipped" },
  { id: "karaoke", name: "Karaoke rotation", group: "karaoke", about: "Singers' queue, a singers' screen with words timed by speech recognition, break music between singers.", status: "beta" },
  { id: "voice", name: "The voice knob", group: "karaoke", about: "Take the singer out of any record (stems), shift the key for the singer, guide voice for a song new to them.", status: "beta" },
  { id: "mics", name: "Microphones per singer", group: "karaoke", about: "A mic and an effect chain per singer, four vocal presets, pitch correction, a monitor wedge each.", status: "beta" },
  { id: "guests", name: "Guest book & room emoji", group: "karaoke", about: "Singers from their phones, emoji from the room on the singers' screen, a guest's own song made with Suno.", status: "beta" },
  { id: "recorded", name: "Recorded performances", group: "karaoke", about: "Each singer's performance kept and handed to them.", status: "beta" },
];

const DJ_CORE = ["decks4", "mixer", "fx", "sampler", "library", "analysis", "find", "keyboard", "midi", "assistant", "coach", "themes", "requests"];
const PRO_EXTRA = ["decks6", "stems", "clap", "hid", "dvs", "clock", "prolink", "autopilot", "record", "stream", "visuals"];
const KARAOKE_EXTRA = ["stems", "karaoke", "voice", "mics", "guests", "recorded", "visuals"];

export const EDITION_INFO: Record<Edition, EditionInfo> = {
  hobbyist: {
    id: "hobbyist", name: "Hobbyist", emoji: "🎧", colour: "#38bdf8",
    tagline: "The bedroom, the living room, the first controller.",
    forWho: "You mix for yourself and your friends, and you want the real thing rather than a toy.",
    features: DJ_CORE, commercial: false,
  },
  pro: {
    id: "pro", name: "Pro", emoji: "🔥", colour: "#f97316",
    tagline: "Paid gigs, clubs, turntables, stems.",
    forWho: "You play for money, on whatever gear the booth has, and you need it not to blink.",
    features: [...DJ_CORE, ...PRO_EXTRA], commercial: true,
  },
  karaoke: {
    id: "karaoke", name: "Karaoke", emoji: "🎤", colour: "#e879f9",
    tagline: "The host's console: singers, words, the voice knob.",
    forWho: "You run karaoke nights — a bar, a party, a family Sunday — and the singers are the show.",
    features: [...DJ_CORE, ...KARAOKE_EXTRA], commercial: true,
  },
  full: {
    id: "full", name: "Full", emoji: "👑", colour: "#facc15",
    tagline: "Everything djmanzo does. DJ and KJ in one booth.",
    forWho: "You do it all — the club on Friday, the karaoke bar on Saturday, the wedding on Sunday.",
    features: Array.from(new Set([...DJ_CORE, ...PRO_EXTRA, ...KARAOKE_EXTRA])), commercial: true,
  },
};

export const PLAN_INFO: Record<Plan, { name: string; per: string; about: string }> = {
  monthly: { name: "Monthly", per: "/ month", about: "Every update while it runs. Cancel any time." },
  yearly: { name: "Yearly", per: "/ year", about: "Every update while it runs; costs less than ten months." },
  perpetual: { name: "Buy once", per: "once", about: "Version 1 for ever, with every 1.x update. Version 2 at half price." },
};

/** Typical market price for an edition and plan: the mean of the references that publish one. */
export function typical(edition: Edition, plan: Plan): number | null {
  const values = MARKET[edition]
    .filter((r) => !(edition === "karaoke" && r.product.startsWith("KaraFun")))
    .map((r) => r[plan])
    .filter((v): v is number => typeof v === "number");
  if (!values.length) return null;
  return values.reduce((a, b) => a + b, 0) / values.length;
}

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
