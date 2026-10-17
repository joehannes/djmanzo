// The licence a DJ's sign-in fetches: what their record says, signed.
//
// The format is the application's (`crates/dj-app/src/licence.rs`):
//
//   djmanzo-licence-1.<payload JSON, base64url>.<Ed25519 signature over the payload bytes, base64url>
//
// and a Rust test reads `test/vector.txt`, written by this file's own test, so
// the two halves cannot drift apart without one of them failing.

import editions from "./editions.json" with { type: "json" };

export const PREFIX = "djmanzo-licence-1.";
export const DAY = 24 * 60 * 60;
/** How long a licence goes before djmanzo should fetch a fresh one. */
export const RENEW_AFTER = 30 * DAY;
/** How long a trial runs. */
export const TRIAL_DAYS = 7;

const encoder = new TextEncoder();

export function base64url(bytes) {
  let text = "";
  for (const byte of bytes) text += String.fromCharCode(byte);
  return btoa(text).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
}

function fromBase64(text) {
  const raw = atob(text);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return bytes;
}

/** The store's signing key, from its PKCS#8 form in base64 (a Worker secret). */
export async function signingKey(pkcs8Base64) {
  return crypto.subtle.importKey("pkcs8", fromBase64(pkcs8Base64), { name: "Ed25519" }, false, ["sign"]);
}

/** Sign `payload` into a licence line. */
export async function sign(payload, key) {
  const bytes = encoder.encode(JSON.stringify(payload));
  const signature = new Uint8Array(await crypto.subtle.sign({ name: "Ed25519" }, key, bytes));
  return `${PREFIX}${base64url(bytes)}.${base64url(signature)}`;
}

/** Whether `slug` is an edition djmanzo knows. */
export function isEdition(slug) {
  return Object.hasOwn(editions.editions, slug);
}

/** Whether `slug` is a module djmanzo knows. */
export function isModule(slug) {
  return editions.modules.includes(slug);
}

/**
 * The edition a set of modules is: the edition whose modules are exactly
 * these, else Custom with them listed. Several purchases (an upgrade, a
 * module added to a Custom) add up this way.
 */
export function editionOf(modules) {
  const want = [...new Set(modules)].filter(isModule).sort();
  for (const [slug, has] of Object.entries(editions.editions)) {
    if (slug === "custom") continue;
    const sorted = [...has].sort();
    if (sorted.length === want.length && sorted.every((m, i) => m === want[i])) {
      return { edition: slug };
    }
  }
  // In the order djmanzo lists modules, so the same purchases always sign
  // the same line.
  return { edition: "custom", modules: editions.modules.filter((m) => want.includes(m)).join(",") };
}

/** The modules an edition (or a Custom purchase) carries. */
export function modulesOf(edition, custom) {
  if (edition === "custom") return (custom ?? "").split(",").filter(isModule);
  return editions.editions[edition] ?? [];
}

/**
 * What a DJ's record entitles them to at `now`, as a licence payload, or
 * `null` when it entitles them to nothing (no purchase, no running trial).
 *
 * A record is `{ purchases: [{ edition, modules?, refunded? }], trial?: { edition, ends } }`.
 * A purchase always outranks a trial: a DJ who buys during their trial is
 * licensed from that moment.
 */
export function entitlement(account, record, now) {
  const bought = (record?.purchases ?? [])
    .filter((p) => !p.refunded && isEdition(p.edition))
    .flatMap((p) => modulesOf(p.edition, p.modules));
  const purchased = (record?.purchases ?? []).some((p) => !p.refunded && isEdition(p.edition));
  const base = { v: 1, account, issued: now, renew_by: now + RENEW_AFTER };
  if (purchased) {
    return { ...base, ...editionOf(bought) };
  }
  const trial = record?.trial;
  if (trial && isEdition(trial.edition) && now < trial.ends) {
    const named = trial.edition === "custom" ? editionOf(modulesOf("custom", trial.modules)) : { edition: trial.edition };
    // A trial's licence is renewed no later than its end, so the end is
    // noticed even by a djmanzo that renews on time.
    return { ...base, ...named, trial: true, ends: trial.ends, renew_by: Math.min(base.renew_by, trial.ends) };
  }
  return null;
}

/**
 * Start a trial of `edition` on `record`, once per account. Returns the new
 * record, or a sentence saying why not.
 */
export function startTrial(record, edition, now, modules) {
  if (!isEdition(edition)) return { refused: "there is no such edition" };
  if (record?.trial) return { refused: "this account has had its trial" };
  if ((record?.purchases ?? []).some((p) => !p.refunded)) return { refused: "this account has already bought djmanzo" };
  const trial = { edition, started: now, ends: now + TRIAL_DAYS * DAY };
  if (edition === "custom") trial.modules = modulesOf("custom", modules).join(",");
  return { record: { ...(record ?? {}), purchases: record?.purchases ?? [], trial } };
}
