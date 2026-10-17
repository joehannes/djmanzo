/**
 * The radio tower: a blog in many languages, and the desk where the crew
 * drafts it with help.
 *
 * A draft is written once per language by the model, natively rather than
 * translated, from a topic and an angle the crew gives. It is saved
 * unpublished: nothing a model wrote goes out until a person has read it and
 * pressed publish, and the post says how it was written.
 *
 * The model is told what djmanzo actually does (`shared/catalog.ts`'s
 * feature list, with each feature's status) and nothing else, so a post
 * cannot promise a feature the application does not have.
 */

import Anthropic from "@anthropic-ai/sdk";
import { betaZodOutputFormat } from "@anthropic-ai/sdk/helpers/beta/zod";
import { z } from "zod";
import { EDITION_INFO, FEATURES, type Edition } from "../shared/catalog";
import type { AppEnv } from "./env";
import { fail, text } from "./http";
import { get, put, type Post, type PostSummary } from "./store";

export const LANGUAGES: Record<string, string> = {
  en: "English",
  es: "Spanish (as spoken across Latin America and the Caribbean)",
  pt: "Portuguese (Brazilian)",
  fr: "French",
  de: "German",
  it: "Italian",
  nl: "Dutch",
  pl: "Polish",
  ht: "Haitian Creole",
  ja: "Japanese",
  ko: "Korean",
  zh: "Chinese (Simplified)",
};

const Draft = z.object({
  title: z.string(),
  excerpt: z.string(),
  body: z.string(),
  tags: z.array(z.string()),
});
type Draft = z.infer<typeof Draft>;

export function slugify(s: string): string {
  return (
    s
      .normalize("NFKD")
      .replace(/[̀-ͯ]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 60) || `post-${Date.now()}`
  );
}

function facts(): string {
  const lines = FEATURES.map((f) => `- ${f.name} [${f.status}]: ${f.about}`);
  const eds = Object.values(EDITION_INFO).map((e) => `- ${e.name}: ${e.tagline} ${e.forWho}`);
  return `What djmanzo does today (status in brackets; "beta" means built but not yet through a real gig, "coming" means NOT in the app yet):\n${lines.join("\n")}\n\nThe four editions:\n${eds.join("\n")}`;
}

const SYSTEM = `You write the blog for djmanzo, a DJ and karaoke application for macOS and Linux that treats Dominican and Caribbean music as first-class alongside club music.

Readers are DJs and karaoke hosts, from bedroom hobbyists to working professionals. Write like a working DJ who knows the culture: concrete, warm, specific about music, gear, rooms and crowds. Short paragraphs. Markdown body with ## headings, lists where they help, no tables, no images, no HTML.

Honesty is the house rule:
- Only describe djmanzo features from the list you are given, at the status given. Never present a "coming" feature as available; say "beta" where it is.
- Do not invent quotes, testimonials, statistics, customers, prices or release dates.
- Name other DJ software only fairly and factually.

Write natively in the requested language — idiomatic, with that scene's own words for things — not as a translation of English.

${facts()}`;

async function draftWithClaude(env: AppEnv, brief: string, language: string): Promise<Draft> {
  const client = new Anthropic({ apiKey: env.ANTHROPIC_API_KEY });
  const response = await client.beta.messages.parse({
    model: "claude-opus-5-5",
    max_tokens: 16000,
    betas: ["server-side-fallback-2026-07-01"],
    fallbacks: "default",
    output_config: { effort: "medium", format: betaZodOutputFormat(Draft) },
    system: SYSTEM,
    messages: [{ role: "user", content: `Write one blog post in ${language}.\n\n${brief}\n\nReturn the title, a one-sentence excerpt, the markdown body (600–1000 words), and 3–6 short tags in the same language.` }],
  });
  if (response.stop_reason === "refusal") fail(422, "The model declined to write that post.");
  if (!response.parsed_output) fail(502, "The draft did not come back in shape.");
  return response.parsed_output;
}

async function draftWithWorkersAi(env: AppEnv, brief: string, language: string): Promise<Draft> {
  const result = (await env.AI.run("@cf/meta/llama-3.3-70b-instruct-fp8-fast" as keyof AiModels, {
    messages: [
      { role: "system", content: SYSTEM },
      {
        role: "user",
        content: `Write one blog post in ${language}.\n\n${brief}\n\nAnswer with JSON only: {"title": string, "excerpt": string, "body": markdown string of 500-800 words, "tags": [3-6 strings]}.`,
      },
    ],
    max_tokens: 3000,
  } as never)) as { response?: string | object };
  const raw = typeof result.response === "string" ? result.response : JSON.stringify(result.response ?? {});
  const json = raw.slice(raw.indexOf("{"), raw.lastIndexOf("}") + 1);
  const parsed = Draft.safeParse(JSON.parse(json || "{}"));
  if (!parsed.success) fail(502, "The draft did not come back in shape.");
  return parsed.data;
}

export interface Brief {
  topic: string;
  angle?: string;
  genre?: string;
  edition?: Edition;
  audience?: string;
  languages: string[];
}

export function briefText(b: Brief): string {
  return [
    `Topic: ${b.topic}`,
    b.angle && `Angle: ${b.angle}`,
    b.genre && `Musical focus: ${b.genre}`,
    b.edition && `Edition to feature: ${EDITION_INFO[b.edition].name}`,
    b.audience && `Reader: ${b.audience}`,
  ]
    .filter(Boolean)
    .join("\n");
}

/** Draft a post in each requested language, in parallel, and save every draft unpublished. */
export async function generate(env: AppEnv, b: Brief, author: string) {
  const langs = (b.languages || []).filter((l) => LANGUAGES[l]).slice(0, 8);
  if (!langs.length) fail(400, "Pick at least one language.");
  if (text(b.topic, 300).length < 4) fail(400, "Give the post a topic.");
  const brief = briefText({ ...b, topic: text(b.topic, 300), angle: b.angle ? text(b.angle, 600) : undefined });
  const useClaude = !!env.ANTHROPIC_API_KEY;
  const slug = slugify(b.topic);
  const results = await Promise.allSettled(
    langs.map(async (lang) => {
      const d = useClaude ? await draftWithClaude(env, brief, LANGUAGES[lang]) : await draftWithWorkersAi(env, brief, LANGUAGES[lang]);
      const post: Post = {
        slug,
        lang,
        title: text(d.title, 140),
        excerpt: text(d.excerpt, 300),
        body: String(d.body).slice(0, 20000),
        tags: d.tags.slice(0, 6).map((t) => text(t, 30)),
        genre: b.genre ?? null,
        edition: b.edition ?? null,
        cover: null,
        published: false,
        createdAt: Date.now(),
        updatedAt: Date.now(),
        author,
        source: useClaude ? "ai:claude" : "ai:workers-ai",
      };
      await savePost(env, post);
      return post;
    }),
  );
  return results.map((r, i) =>
    r.status === "fulfilled" ? { lang: langs[i], ok: true, title: r.value.title } : { lang: langs[i], ok: false, error: String((r.reason as Error)?.message ?? r.reason) },
  );
}

function summary(p: Post): PostSummary {
  return { slug: p.slug, lang: p.lang, title: p.title, excerpt: p.excerpt, tags: p.tags, genre: p.genre, cover: p.cover, published: p.published, updatedAt: p.updatedAt, source: p.source };
}

/**
 * The index is one document, written only from Backstage, so it is never
 * written by two people at once in practice; it is rebuilt from the posts
 * themselves if it is ever lost.
 */
async function index(env: AppEnv): Promise<PostSummary[]> {
  return (await get<PostSummary[]>(env.KV, "index:posts")) ?? [];
}

export async function savePost(env: AppEnv, p: Post): Promise<void> {
  await put(env.KV, `post:${p.slug}:${p.lang}`, p);
  const all = (await index(env)).filter((s) => !(s.slug === p.slug && s.lang === p.lang));
  all.unshift(summary(p));
  await put(env.KV, "index:posts", all.slice(0, 2000));
}

export async function deletePost(env: AppEnv, slug: string, lang: string) {
  await env.KV.delete(`post:${slug}:${lang}`);
  await put(env.KV, "index:posts", (await index(env)).filter((s) => !(s.slug === slug && s.lang === lang)));
  return { ok: true };
}

export async function posts(env: AppEnv, opts: { lang?: string; drafts?: boolean }) {
  const all = await index(env);
  const visible = all.filter((p) => opts.drafts || p.published);
  if (!opts.lang) return visible;
  // One entry per post: the visitor's language where it exists, English otherwise, else whatever there is.
  const bySlug = new Map<string, PostSummary>();
  for (const p of visible) {
    const have = bySlug.get(p.slug);
    const rank = (x: PostSummary) => (x.lang === opts.lang ? 0 : x.lang === "en" ? 1 : 2);
    if (!have || rank(p) < rank(have)) bySlug.set(p.slug, p);
  }
  return [...bySlug.values()].sort((a, b) => b.updatedAt - a.updatedAt);
}

export async function post(env: AppEnv, slug: string, lang: string, drafts = false) {
  const tryLang = async (l: string) => get<Post>(env.KV, `post:${slug}:${l}`);
  const p = (await tryLang(lang)) ?? (await tryLang("en")) ?? (await posts(env, { drafts }).then((all) => all.find((s) => s.slug === slug)).then((s) => (s ? tryLang(s.lang) : null)));
  if (!p || (!p.published && !drafts)) fail(404, "No such post.");
  const languages = (await index(env)).filter((s) => s.slug === slug && (drafts || s.published)).map((s) => s.lang);
  return { ...p, languages };
}

export async function editPost(env: AppEnv, slug: string, lang: string, patch: Partial<Post>) {
  const p = await get<Post>(env.KV, `post:${slug}:${lang}`);
  if (!p) fail(404, "No such post.");
  const next: Post = {
    ...p,
    title: patch.title !== undefined ? text(patch.title, 140) : p.title,
    excerpt: patch.excerpt !== undefined ? text(patch.excerpt, 300) : p.excerpt,
    body: patch.body !== undefined ? String(patch.body).slice(0, 20000) : p.body,
    tags: patch.tags ? patch.tags.slice(0, 6).map((t) => text(t, 30)) : p.tags,
    cover: patch.cover !== undefined ? (patch.cover ? text(patch.cover, 300) : null) : p.cover,
    published: patch.published ?? p.published,
    // A person who edited a model's draft has made it theirs in part; say both.
    source: patch.body !== undefined && p.source.startsWith("ai:") ? `${p.source}+human` : p.source,
    updatedAt: Date.now(),
  };
  await savePost(env, next);
  return next;
}

/** A post written by hand in Backstage. */
export async function writePost(env: AppEnv, input: Partial<Post>, author: string) {
  const title = text(input.title, 140);
  if (!title) fail(400, "A post needs a title.");
  const lang = input.lang && LANGUAGES[input.lang] ? input.lang : "en";
  const p: Post = {
    slug: input.slug ? slugify(input.slug) : slugify(title),
    lang,
    title,
    excerpt: text(input.excerpt, 300),
    body: String(input.body || "").slice(0, 20000),
    tags: (input.tags || []).slice(0, 6).map((t) => text(t, 30)),
    genre: input.genre ?? null,
    edition: input.edition ?? null,
    cover: input.cover ? text(input.cover, 300) : null,
    published: !!input.published,
    createdAt: Date.now(),
    updatedAt: Date.now(),
    author,
    source: "human",
  };
  await savePost(env, p);
  return p;
}
