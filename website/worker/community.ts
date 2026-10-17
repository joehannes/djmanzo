/**
 * What visitors leave behind: wishes, votes, comments, testimonials, samples,
 * and what they told the island about their music.
 *
 * Moderation follows one rule: a signed-in person's comment appears at once;
 * an anonymous one, every testimonial and every sample link wait for the
 * crew. Testimonials are the only thing a visitor might mistake for the
 * site's own claim, so nobody's words are presented as praise until a
 * person has read them.
 */

import type { AppEnv } from "./env";
import { fail, randomId, text, sha256, clientIp, brake } from "./http";
import { fit, get, list, put, type Comment, type CommentKind, type User, type Wish, type WishMeta, type WishStatus } from "./store";
import { mint } from "./promo";

export const WISH_STATUSES: WishStatus[] = ["idea", "considering", "planned", "building", "shipped", "not-planned"];

function wishMeta(w: Wish): WishMeta {
  return { t: w.title, s: w.status, v: w.votes, c: w.createdAt, a: w.author, g: w.tag ?? null, n: w.note ? w.note.slice(0, 160) : null };
}

export async function saveWish(env: AppEnv, w: Wish): Promise<void> {
  await put(env.KV, `wish:${w.id}`, w, undefined, fit(wishMeta(w), "n", "a", "t"));
}

export async function wishes(env: AppEnv) {
  const keys = await list<WishMeta>(env.KV, "wish:", 2000);
  return keys
    .filter((k) => k.metadata)
    .map((k) => ({ id: k.name.slice(5), title: k.metadata!.t, status: k.metadata!.s, votes: k.metadata!.v, createdAt: k.metadata!.c, author: k.metadata!.a, tag: k.metadata!.g ?? null, note: k.metadata!.n ?? null }))
    .sort((a, b) => b.votes - a.votes || b.createdAt - a.createdAt);
}

export async function wish(env: AppEnv, id: string) {
  const w = await get<Wish>(env.KV, `wish:${id}`);
  if (!w) fail(404, "No such wish.");
  return w;
}

export async function addWish(env: AppEnv, req: Request, user: User | null, input: { title?: string; body?: string; author?: string; tag?: string }) {
  await brake(env.KV, `wish:${clientIp(req)}`, 5, 3600);
  const title = text(input.title, 100);
  if (title.length < 4) fail(400, "Give the wish a few words of title.");
  const w: Wish = {
    id: randomId(9),
    title,
    body: text(input.body, 1200),
    author: user?.name ?? (text(input.author, 40) || "A visitor"),
    uid: user?.id ?? null,
    votes: 1,
    status: "idea",
    tag: input.tag ? text(input.tag, 24) : null,
    createdAt: Date.now(),
  };
  await saveWish(env, w);
  await env.KV.put(`vote:${w.id}:${user?.id ?? (await sha256(clientIp(req)))}`, "1");
  return w;
}

/** One vote per person (or per address, for a visitor not signed in). Voting again takes it back. */
export async function vote(env: AppEnv, req: Request, user: User | null, id: string) {
  await brake(env.KV, `vote:${clientIp(req)}`, 60, 3600);
  const w = await wish(env, id);
  const voter = user?.id ?? (await sha256(clientIp(req)));
  const key = `vote:${id}:${voter}`;
  const had = await env.KV.get(key);
  if (had) {
    await env.KV.delete(key);
    w.votes = Math.max(0, w.votes - 1);
  } else {
    await env.KV.put(key, "1");
    w.votes += 1;
  }
  await saveWish(env, w);
  return { votes: w.votes, voted: !had };
}

export async function setWishStatus(env: AppEnv, id: string, status: WishStatus, note?: string | null) {
  if (!WISH_STATUSES.includes(status)) fail(400, "Unknown status.");
  const w = await wish(env, id);
  w.status = status;
  w.note = note ? text(note, 400) : w.note;
  await saveWish(env, w);
  return w;
}

/**
 * The wishlist starts with what is honestly on the roadmap and what has
 * already shipped (docs/FEATURES.md, docs/ROADMAP.md), so a visitor can see
 * at once that wishes do become features — and which ones already have.
 */
export const SEED: Omit<Wish, "id" | "createdAt" | "votes" | "author">[] = [
  { title: "Pro DJ Link and StagelinQ", body: "Join a booth of CDJs or Denon players as a peer.", status: "planned", tag: "hardware", note: "Milestone M7 on the roadmap.", seeded: true },
  { title: "Broadcast to Icecast / Shoutcast", body: "Stream the master straight to an internet radio server.", status: "planned", tag: "live", note: "Milestone M5; recording already ships.", seeded: true },
  { title: "Record to FLAC and MP3", body: "Not only WAV.", status: "planned", tag: "live", note: "WAV recording ships today.", seeded: true },
  { title: "Art-Net / DMX lighting", body: "Drive the lights from the beat and the structure of the music.", status: "planned", tag: "visual", note: "In the design (docs/FEATURES.md); not in the app yet.", seeded: true },
  { title: "Video mixing for VJs", body: "Mix video clips like decks, with transitions.", status: "considering", tag: "visual", note: "Milkdrop-style visuals and video on a deck exist; full VJ mixing is milestone M8.", seeded: true },
  { title: "Duet parts for karaoke", body: "Split a duet into two singers' parts.", status: "considering", tag: "karaoke", note: "Waits for a separation model whose licence the project can accept.", seeded: true },
  { title: "Phones as microphones", body: "Guests sing into their own phone.", status: "considering", tag: "karaoke", note: "Needs a secure page on the local network.", seeded: true },
  { title: "Windows build", body: "djmanzo on Windows, not only macOS and Linux.", status: "considering", tag: "platform", seeded: true },
  { title: "Streaming catalogues (Beatport, TIDAL, SoundCloud)", body: "Play from a streaming service.", status: "considering", tag: "library", note: "The slots exist; it needs partnerships with the services.", seeded: true },
  { title: "Cloud library sync between machines", body: "The same crates on the laptop and the studio machine.", status: "not-planned", tag: "library", note: "Not planned for now: it needs hosted storage. Your session record is local and portable.", seeded: true },
  { title: "Neural stems", body: "Vocals, drums, bass and the rest, live.", status: "shipped", tag: "fx", note: "Shipped: separated ahead of the playhead, no added latency.", seeded: true },
  { title: "Timecode vinyl", body: "Real turntables, real records, digital files.", status: "shipped", tag: "hardware", note: "Shipped (beta), relative and absolute mode.", seeded: true },
  { title: "Karaoke with timed words", body: "A singers' screen with the words timed to the song.", status: "shipped", tag: "karaoke", note: "Shipped (beta): word timing by speech recognition inside djmanzo.", seeded: true },
  { title: "Caribbean themes", body: "Bachata, Malecón, Carnaval…", status: "shipped", tag: "visual", note: "Shipped: Bachata Rosa, Malecón Night, Carnaval, Bioluminescent Bay.", seeded: true },
  { title: "Find a song by humming it", body: "When nobody remembers the name.", status: "shipped", tag: "library", note: "Shipped: by a hum, a half-remembered line, or a description.", seeded: true },
];

export async function seedWishes(env: AppEnv): Promise<number> {
  if (await env.KV.get("config:seeded-wishes")) return 0;
  let n = 0;
  for (const s of SEED) {
    await saveWish(env, { ...s, id: randomId(9), votes: 0, author: "djmanzo", createdAt: Date.now() - n * 1000 });
    n++;
  }
  await env.KV.put("config:seeded-wishes", String(Date.now()));
  return n;
}

// --- Comments, testimonials, samples ---------------------------------------

const KINDS: CommentKind[] = ["testimonial", "comment", "sample"];

/** Newest first in KV's lexicographic listing. */
function inverted(t: number): string {
  return String(9_999_999_999_999 - t).padStart(13, "0");
}

export function sampleLink(link: string): string | null {
  try {
    const u = new URL(link);
    const ok = ["www.youtube.com", "youtube.com", "youtu.be", "m.youtube.com", "soundcloud.com", "www.mixcloud.com", "mixcloud.com"].includes(u.hostname);
    return ok && u.protocol === "https:" ? u.toString().slice(0, 300) : null;
  } catch {
    return null;
  }
}

export async function addComment(
  env: AppEnv,
  req: Request,
  user: User | null,
  input: { kind?: CommentKind; body?: string; author?: string; station?: string; rating?: number; link?: string; role?: string; genre?: string },
) {
  await brake(env.KV, `comment:${clientIp(req)}`, 8, 3600);
  const kind = KINDS.includes(input.kind as CommentKind) ? (input.kind as CommentKind) : "comment";
  const body = text(input.body, kind === "testimonial" ? 600 : 800);
  if (body.length < 3) fail(400, "Say a little more.");
  let link: string | null = null;
  if (kind === "sample") {
    link = sampleLink(String(input.link || ""));
    if (!link) fail(400, "A sample is a YouTube, SoundCloud or Mixcloud link.");
  }
  const c: Comment = {
    id: randomId(9),
    kind,
    station: input.station ? text(input.station, 30) : null,
    body,
    author: user?.name ?? (text(input.author, 40) || "A visitor"),
    uid: user?.id ?? null,
    rating: kind === "testimonial" && input.rating ? Math.max(1, Math.min(5, Math.round(Number(input.rating)))) : null,
    link,
    role: input.role ? text(input.role, 40) : null,
    genre: input.genre ? text(input.genre, 30) : null,
    approved: kind === "comment" && !!user && !user.demo,
    createdAt: Date.now(),
  };
  await put(env.KV, `comment:${kind}:${inverted(c.createdAt)}:${c.id}`, c, undefined, fit(c, "body", "author"));
  return c;
}

export async function comments(env: AppEnv, kind: CommentKind, opts: { station?: string | null; pending?: boolean; limit?: number } = {}) {
  const keys = await list<Comment>(env.KV, `comment:${kind}:`, 600);
  return keys
    .map((k) => ({ ...(k.metadata as Comment), key: k.name }))
    .filter((c) => c && (opts.pending ? !c.approved : c.approved) && (!opts.station || c.station === opts.station))
    .slice(0, opts.limit ?? 100);
}

export async function moderate(env: AppEnv, key: string, approve: boolean) {
  if (!key.startsWith("comment:")) fail(400, "Not a comment.");
  const c = await get<Comment>(env.KV, key);
  if (!c) fail(404, "Gone already.");
  if (!approve) {
    await env.KV.delete(key);
    return { deleted: true };
  }
  c.approved = true;
  await put(env.KV, key, c, undefined, fit(c, "body", "author"));
  return { approved: true };
}

// --- The island's memory of a person ---------------------------------------

/** What a person told the island. Small, typed loosely, and capped. */
export async function saveProfile(env: AppEnv, user: User, profile: unknown) {
  const s = JSON.stringify(profile ?? {});
  if (s.length > 8000) fail(413, "That is a lot of taste. Keep it under 8 kB.");
  await env.KV.put(`profile:${user.id}`, s);
  return { ok: true };
}

/**
 * The island's prize: walk it (visit enough of its places) and a personal
 * code is minted — 15 % off, once, for 30 days, bound to the account that
 * walked it. One per account, ever.
 */
export const REWARD_STAMPS = 7;

export async function claimReward(env: AppEnv, user: User, stamps: string[]) {
  if (user.demo) fail(403, "Sign in with a real account to claim the island's prize.");
  const unique = new Set((stamps || []).map((s) => text(s, 20)));
  if (unique.size < REWARD_STAMPS) fail(400, `Visit ${REWARD_STAMPS - unique.size} more places on the island first.`);
  const key = `reward:${user.id}`;
  const existing = await env.KV.get(key);
  if (existing) return { code: existing, again: true };
  const [p] = await mint(
    env,
    { kind: "percent", value: 15, expires: Date.now() + 30 * 86400_000, campaign: "island", note: "Walked the island" },
    { prefix: "ISLA", boundTo: [user.email || user.id], by: "island" },
  );
  await env.KV.put(key, p.code);
  return { code: p.code, again: false };
}
