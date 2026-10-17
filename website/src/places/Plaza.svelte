<!--
  The plaza, where people meet: what DJs and singers say about djmanzo, their
  mixes and performances, videos, and a way to tell friends. Nothing here is
  written by the site about itself: testimonials are real people's words,
  read by the crew before they appear, and the plaza says so when it is
  still empty.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { api, post } from "../lib/api";
  import { session } from "../lib/session.svelte";
  import { profile } from "../lib/profile.svelte";
  import type { PlaceId } from "../lib/places";

  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();

  interface Item {
    id: string;
    kind: string;
    body: string;
    author: string;
    rating?: number | null;
    link?: string | null;
    role?: string | null;
    genre?: string | null;
    createdAt: number;
  }
  let testimonials = $state<Item[]>([]);
  let comments = $state<Item[]>([]);
  let samples = $state<Item[]>([]);
  let videos = $state<{ id: string; title: string }[]>([]);
  let playing = $state<string | null>(null);

  let kind = $state<"comment" | "testimonial" | "sample">("comment");
  let body = $state("");
  let link = $state("");
  let rating = $state(5);
  let author = $state(profile.p.name);
  let message = $state<string | null>(null);

  onMount(async () => {
    const [t, c, s, v] = await Promise.allSettled([
      api("/api/comments?kind=testimonial"),
      api("/api/comments?kind=comment"),
      api("/api/comments?kind=sample"),
      api("/api/videos"),
    ]);
    if (t.status === "fulfilled") testimonials = t.value;
    if (c.status === "fulfilled") comments = c.value;
    if (s.status === "fulfilled") samples = s.value;
    if (v.status === "fulfilled") videos = v.value;
  });

  async function send(e: SubmitEvent) {
    e.preventDefault();
    message = null;
    try {
      const r = await post("/api/comments", { kind, body, link, rating, author, role: profile.p.role, genre: profile.genre?.name });
      body = "";
      link = "";
      if (r.approved) {
        comments = [r, ...comments];
        message = "Posted.";
      } else {
        message = kind === "comment" ? "Thanks — the crew reads comments from visitors who are not signed in before they appear." : "Thanks — the crew reads every testimonial and sample before it appears.";
      }
    } catch (e2) {
      message = (e2 as Error).message;
    }
  }

  function youtubeId(url: string): string | null {
    try {
      const u = new URL(url);
      if (u.hostname === "youtu.be") return u.pathname.slice(1, 12);
      return u.searchParams.get("v");
    } catch {
      return null;
    }
  }

  const shareText = $derived(`Found djmanzo — a DJ and karaoke app that learns how you play.${profile.genre ? ` Plays ${profile.genre.name.toLowerCase()} like it belongs.` : ""}`);
  const url = typeof location !== "undefined" ? location.origin : "";
  async function share() {
    if (navigator.share) {
      try {
        await navigator.share({ title: "djmanzo", text: shareText, url });
      } catch {
        /* the visitor closed the sheet */
      }
    }
  }
  const SHARES = $derived([
    { name: "WhatsApp", href: `https://wa.me/?text=${encodeURIComponent(`${shareText} ${url}`)}` },
    { name: "X", href: `https://x.com/intent/post?text=${encodeURIComponent(shareText)}&url=${encodeURIComponent(url)}` },
    { name: "Bluesky", href: `https://bsky.app/intent/compose?text=${encodeURIComponent(`${shareText} ${url}`)}` },
    { name: "Facebook", href: `https://www.facebook.com/sharer/sharer.php?u=${encodeURIComponent(url)}` },
    { name: "Threads", href: `https://www.threads.net/intent/post?text=${encodeURIComponent(`${shareText} ${url}`)}` },
  ]);
  interface Reel {
    id: string;
    title: string;
    by: string;
    link: string | null;
  }
  const reels: Reel[] = $derived([
    ...videos.map((v) => ({ id: v.id, title: v.title, by: "", link: null })),
    ...samples.map((s) => ({ id: youtubeId(s.link ?? "") ?? "", title: s.body, by: s.author, link: s.link ?? null })),
  ]);
  const date = (t: number) => new Date(t).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
</script>

<h2>What people say</h2>
{#if testimonials.length}
  <div class="quotes">
    {#each testimonials as t (t.id)}
      <blockquote>
        {#if t.rating}<span class="stars" aria-label="{t.rating} out of 5">{"★".repeat(t.rating)}{"☆".repeat(5 - t.rating)}</span>{/if}
        <p>{t.body}</p>
        <footer>— {t.author}{t.role ? `, ${t.role}` : ""}{t.genre ? ` · ${t.genre}` : ""}</footer>
      </blockquote>
    {/each}
  </div>
{:else}
  <p class="empty">No testimonials yet — djmanzo is in beta, and we would rather show none than invent any. If you have played a night with it, yours could be the first.</p>
{/if}

<h2>Mixes, performances and videos</h2>
{#if videos.length || samples.length}
  <div class="videos">
    {#each reels as v, i (i)}
      <figure>
        {#if v.id && playing === v.id}
          <iframe src="https://www.youtube-nocookie.com/embed/{v.id}?autoplay=1" title={v.title} allow="autoplay; encrypted-media; picture-in-picture" allowfullscreen></iframe>
        {:else if v.id}
          <button class="facade" onclick={() => (playing = v.id)} aria-label="Play {v.title} (loads YouTube)">
            <img src="https://i.ytimg.com/vi/{v.id}/hqdefault.jpg" alt="" loading="lazy" />
            <span class="play" aria-hidden="true">▶</span>
          </button>
        {:else if v.link}
          <a class="btn" href={v.link} target="_blank" rel="noopener nofollow">Listen</a>
        {/if}
        <figcaption>{v.title}{v.by ? ` — ${v.by}` : ""}</figcaption>
      </figure>
    {/each}
  </div>
  <p class="small muted">Videos load from YouTube only when you press play.</p>
{:else}
  <p class="empty">Nothing here yet. Share a mix or a karaoke performance made with djmanzo — a YouTube, SoundCloud or Mixcloud link — and it appears once the crew has seen it.</p>
{/if}

<h2>The conversation</h2>
{#if comments.length}
  <ul class="comments">
    {#each comments as c (c.id)}
      <li><b>{c.author}</b> <span class="muted small">{date(c.createdAt)}</span><p>{c.body}</p></li>
    {/each}
  </ul>
{:else}
  <p class="muted">Quiet so far. Start it.</p>
{/if}

<form class="say" onsubmit={send}>
  <div class="chips" role="radiogroup" aria-label="What are you leaving?">
    <button type="button" class="chip" role="radio" aria-checked={kind === "comment"} onclick={() => (kind = "comment")}>💬 A comment</button>
    <button type="button" class="chip" role="radio" aria-checked={kind === "testimonial"} onclick={() => (kind = "testimonial")}>⭐ A testimonial</button>
    <button type="button" class="chip" role="radio" aria-checked={kind === "sample"} onclick={() => (kind = "sample")}>🎬 A mix or video</button>
  </div>
  {#if kind === "sample"}
    <label class="field"><span>Link (YouTube, SoundCloud or Mixcloud)</span><input type="url" bind:value={link} required placeholder="https://youtu.be/…" /></label>
  {/if}
  {#if kind === "testimonial"}
    <div class="rate" role="radiogroup" aria-label="Rating">
      {#each [1, 2, 3, 4, 5] as n (n)}<button type="button" class="star" class:on={n <= rating} role="radio" aria-checked={rating === n} aria-label="{n} stars" onclick={() => (rating = n)}>★</button>{/each}
    </div>
  {/if}
  <label class="field"><span>{kind === "sample" ? "What is it?" : kind === "testimonial" ? "Your night with djmanzo" : "Your comment"}</span><textarea bind:value={body} required maxlength="800"></textarea></label>
  {#if !session.user}<label class="field"><span>Your name</span><input type="text" bind:value={author} maxlength="40" /></label>{/if}
  <button class="btn primary">Leave it in the plaza</button>
  {#if message}<p class="small" role="status">{message}</p>{/if}
</form>

<h2>Tell a friend</h2>
<div class="share">
  {#if typeof navigator !== "undefined" && "share" in navigator}<button class="btn primary" onclick={share}>Share…</button>{/if}
  {#each SHARES as s (s.name)}<a class="btn quiet" href={s.href} target="_blank" rel="noopener">{s.name}</a>{/each}
</div>
<p class="small muted">Wishes for new features go on <button class="link" onclick={() => go("muro")}>the wall</button>.</p>

<style>
  .quotes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 12px;
  }
  blockquote {
    margin: 0;
    padding: 14px;
    border-radius: 12px;
    background: var(--surface-2);
  }
  .stars {
    color: var(--primary);
  }
  .empty {
    padding: 14px;
    border-radius: 12px;
    border: 2px dashed var(--line);
  }
  .videos {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 12px;
  }
  iframe,
  .facade {
    width: 100%;
    aspect-ratio: 16 / 9;
    border: 0;
    border-radius: 10px;
  }
  .facade {
    position: relative;
    padding: 0;
    cursor: pointer;
    overflow: hidden;
    background: #000;
  }
  .facade img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-size: 40px;
    color: #fff;
    text-shadow: 0 2px 10px #000;
  }
  .comments {
    list-style: none;
    padding: 0;
  }
  .comments li {
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .comments p {
    margin: 4px 0 0;
  }
  .say {
    max-width: 560px;
    margin-top: 10px;
  }
  .say .chips {
    margin-bottom: 10px;
  }
  .rate {
    margin-bottom: 8px;
  }
  .star {
    font-size: 26px;
    background: none;
    border: 0;
    color: var(--line);
    cursor: pointer;
  }
  .star.on {
    color: var(--primary);
  }
  .share {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
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
  h2 {
    margin-top: 20px;
  }
</style>
