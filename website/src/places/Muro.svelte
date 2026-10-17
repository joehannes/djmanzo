<!--
  The wall in town where people paste flyers: here, the wishes for djmanzo.
  Paste one, pin the ones you want too, and see each one's stamp — an idea,
  planned, being built, shipped, or not planned and why. The wall starts
  with what is honestly on the roadmap, so it is plain from the first look
  that wishes do turn into features.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { api, post } from "../lib/api";
  import { session } from "../lib/session.svelte";
  import { profile } from "../lib/profile.svelte";
  import type { PlaceId } from "../lib/places";

  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();

  interface WishRow {
    id: string;
    title: string;
    status: string;
    votes: number;
    createdAt: number;
    author: string;
    tag: string | null;
    note: string | null;
  }
  let wishes = $state<WishRow[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let filter = $state<string>("all");
  let open = $state<string | null>(null);
  let detail = $state<{ body: string } | null>(null);
  let title = $state("");
  let body = $state("");
  let author = $state(profile.p.name);
  let posted = $state<string | null>(null);
  let voted = $state<Set<string>>(new Set());

  const STAMP: Record<string, { label: string; colour: string }> = {
    idea: { label: "Idea", colour: "#7a7a7a" },
    considering: { label: "Considering", colour: "#c98b00" },
    planned: { label: "Planned", colour: "#1f6fd1" },
    building: { label: "Being built", colour: "#8a3ffc" },
    shipped: { label: "Shipped", colour: "#1f9d55" },
    "not-planned": { label: "Not planned", colour: "#c0263b" },
  };
  const PAPER = ["#fff4c2", "#ffd6e7", "#d6f5ff", "#e3ffd6", "#ffe3c7", "#efe2ff"];

  async function load() {
    loading = true;
    try {
      wishes = await api("/api/wishes");
      // The wall is seeded on its first visit; ask once more if it came back empty.
      if (!wishes.length) {
        await new Promise((r) => setTimeout(r, 1200));
        wishes = await api("/api/wishes");
      }
      error = null;
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
  }
  onMount(load);

  const shown = $derived(filter === "all" ? wishes : wishes.filter((w) => w.status === filter));

  async function vote(w: WishRow) {
    try {
      const r = await post(`/api/wishes/${w.id}/vote`);
      w.votes = r.votes;
      const v = new Set(voted);
      if (r.voted) v.add(w.id);
      else v.delete(w.id);
      voted = v;
    } catch (e) {
      error = (e as Error).message;
    }
  }

  async function expand(w: WishRow) {
    if (open === w.id) {
      open = null;
      return;
    }
    open = w.id;
    detail = null;
    try {
      detail = await api(`/api/wishes/${w.id}`);
    } catch {
      detail = { body: "" };
    }
  }

  async function paste(e: SubmitEvent) {
    e.preventDefault();
    posted = null;
    try {
      const w = await post("/api/wishes", { title, body, author, tag: profile.genre?.id });
      wishes = [{ ...w, note: null }, ...wishes];
      voted = new Set([...voted, w.id]);
      title = "";
      body = "";
      posted = "Pasted. Others can pin it now; the crew reads every one.";
    } catch (e2) {
      posted = (e2 as Error).message;
    }
  }
</script>

<p>What should djmanzo do next? Paste a wish, pin the ones you want too. Each flyer carries its stamp, so you can see what is planned and what already shipped.</p>

<div class="filters" role="radiogroup" aria-label="Show wishes">
  {#each ["all", "shipped", "building", "planned", "considering", "idea", "not-planned"] as f (f)}
    <button class="chip" role="radio" aria-checked={filter === f} onclick={() => (filter = f)}>
      {f === "all" ? "All" : STAMP[f].label} <span class="n">{f === "all" ? wishes.length : wishes.filter((w) => w.status === f).length}</span>
    </button>
  {/each}
</div>

{#if loading}
  <p class="muted">Reading the wall…</p>
{:else if error}
  <p class="notice">{error}</p>
{/if}

<div class="wall">
  {#each shown as w, i (w.id)}
    <article class="flyer" style="--paper:{PAPER[i % PAPER.length]};--tilt:{((i * 37) % 7) - 3}deg">
      <span class="tape" aria-hidden="true"></span>
      <button class="title" onclick={() => expand(w)} aria-expanded={open === w.id}>{w.title}</button>
      <span class="stamp" style="--s:{STAMP[w.status]?.colour ?? '#777'}">{STAMP[w.status]?.label ?? w.status}</span>
      {#if w.note}<p class="note">{w.note}</p>{/if}
      {#if open === w.id && detail?.body}<p class="body">{detail.body}</p>{/if}
      <footer>
        <span class="by">— {w.author}</span>
        <button class="pin" class:on={voted.has(w.id)} onclick={() => vote(w)} aria-pressed={voted.has(w.id)} aria-label="Pin this wish ({w.votes} pins)">📌 {w.votes}</button>
      </footer>
    </article>
  {/each}
</div>

<h2>Paste a wish</h2>
<form class="paste" onsubmit={paste}>
  <label class="field"><span>What should djmanzo do?</span><input type="text" bind:value={title} maxlength="100" required placeholder="e.g. Pioneer DDJ-FLX4 mapping" /></label>
  <label class="field"><span>Tell us more (optional)</span><textarea bind:value={body} maxlength="1200" placeholder="What would it let you do at your gigs?"></textarea></label>
  {#if !session.user}<label class="field"><span>Your name (optional)</span><input type="text" bind:value={author} maxlength="40" /></label>{/if}
  <button class="btn primary">Paste it on the wall</button>
  {#if posted}<p class="small" role="status">{posted}</p>{/if}
</form>
<p class="muted small">Wishes are public. Please keep them about djmanzo; the crew removes anything else. <button class="link" onclick={() => go("plaza")}>Comments and stories go to the plaza.</button></p>

<style>
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 14px;
  }
  .n {
    font-size: 12px;
    opacity: 0.7;
  }
  .wall {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 18px 14px;
    padding: 18px 12px;
    border-radius: 10px;
    background:
      linear-gradient(90deg, rgba(0, 0, 0, 0.08) 1px, transparent 1px) 0 0 / 60px 30px,
      linear-gradient(rgba(0, 0, 0, 0.08) 1px, transparent 1px) 0 0 / 60px 30px,
      #8f5b3e;
  }
  .flyer {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 18px 14px 10px;
    background: var(--paper);
    color: #1d1d1d;
    rotate: var(--tilt);
    box-shadow: 0 8px 16px rgba(0, 0, 0, 0.3);
    transition: rotate 0.2s, transform 0.2s;
  }
  .flyer:hover {
    rotate: 0deg;
    transform: scale(1.03);
  }
  .tape {
    position: absolute;
    top: -9px;
    left: 50%;
    width: 64px;
    height: 18px;
    translate: -50% 0;
    background: rgba(255, 255, 255, 0.55);
    rotate: -4deg;
  }
  .title {
    text-align: left;
    background: none;
    border: 0;
    padding: 0;
    color: inherit;
    font: 800 1.02rem/1.25 var(--body);
    cursor: pointer;
  }
  .stamp {
    align-self: flex-start;
    padding: 2px 8px;
    border: 2px solid var(--s);
    color: var(--s);
    border-radius: 4px;
    font: 800 12px var(--body);
    rotate: -4deg;
    letter-spacing: 0.02em;
  }
  .note,
  .body {
    font-size: 13px;
    margin: 0;
  }
  footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: auto;
    font-size: 12px;
  }
  .pin {
    border: 1px solid rgba(0, 0, 0, 0.25);
    background: rgba(255, 255, 255, 0.6);
    border-radius: 999px;
    padding: 4px 10px;
    cursor: pointer;
    color: #1d1d1d;
    font-weight: 700;
  }
  .pin.on {
    background: #1d1d1d;
    color: #fff;
  }
  .paste {
    max-width: 560px;
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
    margin-top: 22px;
  }
</style>
