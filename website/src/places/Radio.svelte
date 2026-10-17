<!--
  Radio Manzo: the blog, in the visitor's language where a post exists in
  it. Every post says how it was written — by a person, or drafted by a
  model and published by a person.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../lib/api";
  import { markdown } from "../lib/markdown";
  import type { PlaceId } from "../lib/places";

  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();

  interface Summary {
    slug: string;
    lang: string;
    title: string;
    excerpt: string;
    tags: string[];
    updatedAt: number;
    source: string;
  }
  let languages = $state<Record<string, string>>({ en: "English" });
  let lang = $state((navigator.language || "en").slice(0, 2));
  let posts = $state<Summary[]>([]);
  let reading = $state<(Summary & { body: string; languages: string[]; author: string }) | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function load() {
    loading = true;
    try {
      posts = await api(`/api/blog?lang=${lang}`);
      error = null;
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
  }

  async function read(slug: string, l = lang) {
    try {
      reading = await api(`/api/blog/${slug}?lang=${l}`);
    } catch (e) {
      error = (e as Error).message;
    }
  }

  onMount(async () => {
    try {
      languages = await api("/api/blog-languages");
    } catch {
      /* English only, then */
    }
    if (!languages[lang]) lang = "en";
    await load();
  });

  const SOURCE: Record<string, string> = {
    human: "Written by the crew",
    "ai:claude": "Drafted with Claude, checked and published by the crew",
    "ai:workers-ai": "Drafted with an AI model, checked and published by the crew",
    "ai:claude+human": "Drafted with Claude, edited and published by the crew",
    "ai:workers-ai+human": "Drafted with an AI model, edited and published by the crew",
  };
  const date = (t: number) => new Date(t).toLocaleDateString(lang, { day: "numeric", month: "long", year: "numeric" });
</script>

{#if reading}
  <button class="btn quiet" onclick={() => (reading = null)}>◀ All posts</button>
  <article class="post">
    <h2>{reading.title}</h2>
    <p class="meta small">{date(reading.updatedAt)} · {SOURCE[reading.source] ?? reading.source}</p>
    {#if reading.languages.length > 1}
      <p class="small">Also in:
        {#each reading.languages.filter((l) => l !== reading!.lang) as l (l)}
          <button class="chip" onclick={() => read(reading!.slug, l)}>{languages[l]?.split(" (")[0] ?? l}</button>
        {/each}
      </p>
    {/if}
    <div class="prose">{@html markdown(reading.body)}</div>
    {#if reading.tags.length}<p class="tags">{#each reading.tags as t (t)}<span>#{t}</span>{/each}</p>{/if}
  </article>
{:else}
  <div class="tune">
    <label class="field">
      <span>📻 Tune to</span>
      <select bind:value={lang} onchange={load}>
        {#each Object.entries(languages) as [code, name] (code)}<option value={code}>{name}</option>{/each}
      </select>
    </label>
  </div>
  {#if loading}
    <p class="muted">Tuning in…</p>
  {:else if error}
    <p class="notice">{error}</p>
  {:else if !posts.length}
    <div class="empty">
      <p><b>Nothing on air yet.</b> The first posts are being written — stories from the booth, guides for karaoke hosts, the culture behind the genres.</p>
      <p>Meanwhile, the cays out at sea each hold something worth knowing, and <button class="link" onclick={() => go("muro")}>the wall</button> takes your wishes.</p>
    </div>
  {:else}
    <ul class="list">
      {#each posts as p (p.slug)}
        <li>
          <button onclick={() => read(p.slug, p.lang)}>
            <b>{p.title}</b>
            <span>{p.excerpt}</span>
            <small>{date(p.updatedAt)}{p.lang !== lang ? ` · in ${languages[p.lang]?.split(" (")[0] ?? p.lang}` : ""}</small>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
{/if}

<style>
  .tune {
    max-width: 320px;
  }
  .list {
    list-style: none;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  .list button {
    display: grid;
    gap: 4px;
    width: 100%;
    text-align: left;
    padding: 14px;
    border-radius: 12px;
    border: 2px solid var(--line);
    background: var(--surface-2);
    cursor: pointer;
  }
  .list b {
    font-family: var(--display);
    font-weight: 400;
    font-size: 1.15rem;
  }
  .list small {
    color: var(--ink-soft);
  }
  .post {
    margin-top: 12px;
  }
  .meta {
    color: var(--ink-soft);
  }
  .prose :global(h3),
  .prose :global(h4) {
    margin-top: 1.2em;
  }
  .prose :global(blockquote) {
    border-left: 4px solid var(--primary);
    margin: 0 0 1em;
    padding-left: 12px;
    font-style: italic;
  }
  .tags span {
    margin-right: 8px;
    color: var(--ink-soft);
  }
  .empty {
    padding: 16px;
    border-radius: 12px;
    border: 2px dashed var(--line);
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--primary);
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
  }
</style>
