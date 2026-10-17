<!--
  Backstage, for the crew: how the shop is doing, codes to mint, wishes to
  stamp, words to read before they go public, orders waiting on a transfer,
  the blog desk where drafts are written in many languages, and the videos
  the plaza shows. Each is a flight case: open the one you need.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { api, post } from "../lib/api";
  import { session } from "../lib/session.svelte";
  import { EDITIONS, PLANS, money, type Edition, type Plan } from "../../shared/catalog";
  import { GENRES } from "../lib/genres";
  import type { PlaceId } from "../lib/places";

  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();

  let token = $state("");
  let error = $state<string | null>(null);
  let overview = $state<any>(null);

  async function ownerLogin(e: SubmitEvent) {
    e.preventDefault();
    error = null;
    try {
      await post("/api/auth/owner", { token });
      await session.refresh();
      await refresh();
    } catch (e2) {
      error = (e2 as Error).message;
    }
  }

  async function refresh() {
    if (!session.admin) return;
    try {
      overview = await api("/api/admin/overview");
    } catch (e) {
      error = (e as Error).message;
    }
  }
  onMount(refresh);

  // --- Codes --------------------------------------------------------------------
  let kind = $state<"percent" | "fixed" | "voucher">("percent");
  let value = $state(20);
  let edits = $state<Edition[]>([]);
  let plans = $state<Plan[]>([]);
  let grantEdition = $state<Edition>("pro");
  let grantPlan = $state<Plan>("monthly");
  let months = $state(3);
  let mode = $state<"general" | "individual">("general");
  let count = $state(10);
  let prefix = $state("MANZO");
  let exact = $state("");
  let emails = $state("");
  let maxUses = $state<number | null>(null);
  let expires = $state("");
  let campaign = $state("");
  let minted = $state<{ code: string; boundTo: string | null }[]>([]);
  let promos = $state<any[]>([]);

  async function mint(e: SubmitEvent) {
    e.preventDefault();
    error = null;
    try {
      const r = await post("/api/admin/promos", {
        kind,
        value: kind === "fixed" ? Math.round(value * 100) : value,
        grants: kind === "voucher" ? { edition: grantEdition, plan: grantPlan, months: grantPlan === "perpetual" ? undefined : months } : null,
        editions: edits,
        plans,
        maxUses: mode === "general" ? maxUses : 1,
        expires: expires ? new Date(expires).getTime() : null,
        campaign: campaign || null,
        ...(mode === "individual" ? { boundTo: emails } : exact ? { code: exact } : { count, prefix }),
      });
      minted = r.codes;
      await loadPromos();
    } catch (e2) {
      error = (e2 as Error).message;
    }
  }
  async function loadPromos() {
    promos = (await api("/api/admin/promos")).sort((a: any, b: any) => (a.code < b.code ? -1 : 1));
  }
  async function togglePromo(code: string, active: boolean) {
    await api(`/api/admin/promos/${code}`, { method: "PATCH", json: { active } });
    await loadPromos();
  }
  function csv(): string {
    return "code,bound_to\n" + minted.map((m) => `${m.code},${m.boundTo ?? ""}`).join("\n");
  }

  // --- Wishes ---------------------------------------------------------------------
  let wishes = $state<any[]>([]);
  const STATUSES = ["idea", "considering", "planned", "building", "shipped", "not-planned"];
  async function loadWishes() {
    wishes = await api("/api/wishes");
  }
  async function stampWish(id: string, status: string, note: string) {
    await api(`/api/admin/wishes/${id}`, { method: "PATCH", json: { status, note } });
    await loadWishes();
  }
  async function dropWish(id: string) {
    await api(`/api/admin/wishes/${id}`, { method: "DELETE" });
    await loadWishes();
  }

  // --- Moderation -----------------------------------------------------------------
  let pending = $state<any[]>([]);
  async function loadPending() {
    pending = await api("/api/admin/comments");
  }
  async function moderate(key: string, approve: boolean) {
    await post("/api/admin/comments/moderate", { key, approve });
    await loadPending();
  }

  // --- Orders ---------------------------------------------------------------------
  let orders = $state<any[]>([]);
  let bankCountry = $state<Record<string, string>>({});
  async function loadOrders() {
    orders = await api("/api/admin/orders");
  }
  async function markPaid(id: string) {
    try {
      const r = await post(`/api/admin/orders/${id}/paid`, { bankCountry: bankCountry[id] || null });
      alert(`Licence ${r.licence.id} issued. ${r.proof.reason}`);
      await loadOrders();
    } catch (e) {
      error = (e as Error).message;
    }
  }

  // --- Blog desk ------------------------------------------------------------------
  let langs = $state<Record<string, string>>({});
  let topic = $state("");
  let angle = $state("");
  let genre = $state("");
  let featured = $state<Edition | "">("");
  let audience = $state("");
  let chosenLangs = $state<string[]>(["en", "es"]);
  let generating = $state(false);
  let genResult = $state<any[] | null>(null);
  let drafts = $state<any[]>([]);
  let editing = $state<any>(null);

  async function loadBlog() {
    langs = await api("/api/blog-languages");
    drafts = await api("/api/admin/blog");
  }
  async function generate(e: SubmitEvent) {
    e.preventDefault();
    generating = true;
    genResult = null;
    try {
      genResult = await post("/api/admin/blog/generate", { topic, angle, genre: genre || undefined, edition: featured || undefined, audience, languages: chosenLangs });
      await loadBlog();
    } catch (e2) {
      error = (e2 as Error).message;
    } finally {
      generating = false;
    }
  }
  async function openDraft(slug: string, lang: string) {
    editing = await api(`/api/admin/blog/${slug}/${lang}`);
  }
  async function saveDraft(publish?: boolean) {
    const p = editing;
    editing = await api(`/api/admin/blog/${p.slug}/${p.lang}`, {
      method: "PATCH",
      json: { title: p.title, excerpt: p.excerpt, body: p.body, tags: p.tags, ...(publish !== undefined ? { published: publish } : {}) },
    });
    await loadBlog();
  }
  async function dropDraft(slug: string, lang: string) {
    if (!confirm(`Delete ${slug} (${lang})?`)) return;
    await api(`/api/admin/blog/${slug}/${lang}`, { method: "DELETE" });
    editing = null;
    await loadBlog();
  }

  // --- Videos ---------------------------------------------------------------------
  let videos = $state<{ id: string; title: string }[]>([]);
  let vUrl = $state("");
  let vTitle = $state("");
  async function loadVideos() {
    videos = await api("/api/videos");
  }
  async function saveVideos(next: { id: string; title: string }[]) {
    videos = await api("/api/admin/videos", { method: "PUT", json: next });
  }
  function ytId(s: string): string {
    try {
      const u = new URL(s);
      return u.hostname === "youtu.be" ? u.pathname.slice(1) : (u.searchParams.get("v") ?? s);
    } catch {
      return s.trim();
    }
  }

  function toggle<T>(list: T[], v: T): T[] {
    return list.includes(v) ? list.filter((x) => x !== v) : [...list, v];
  }
</script>

{#if !session.admin}
  <p>Backstage is for the crew. Sign in with an address on the crew list, or use the crew's token.</p>
  <form class="inline" onsubmit={ownerLogin}>
    <input type="password" bind:value={token} placeholder="Crew token" aria-label="Crew token" autocomplete="current-password" />
    <button class="btn primary">Come in</button>
  </form>
  {#if error}<p class="notice">{error}</p>{/if}
  <p class="small"><button class="link" onclick={() => go("pase")}>Or sign in with your account at Tu Pase.</button></p>
{:else}
  {#if error}<p class="notice" role="alert">{error} <button class="link" onclick={() => (error = null)}>dismiss</button></p>{/if}

  {#if overview}
    <div class="stats">
      <div><b>{overview.users}</b><span>people</span></div>
      <div><b>{overview.paid}</b><span>paid orders</span></div>
      <div><b>{Object.entries(overview.revenue).map(([c, v]) => money(v as number, c as "USD")).join(" + ") || "—"}</b><span>taken</span></div>
      <div><b>{overview.licences}</b><span>licences</span></div>
      <div><b>{overview.pendingTransfers}</b><span>transfers waiting</span></div>
      <div><b>{overview.pending}</b><span>to read</span></div>
      <div><b>{overview.wishes}</b><span>wishes</span></div>
      <div><b>{overview.promos}</b><span>codes</span></div>
    </div>
    <p class="small muted">
      Sign-in: {overview.providers.join(", ") || "none connected (demo sign-in on)"} · Payments: {Object.entries(overview.payments).filter(([, v]) => v).map(([k]) => k).join(", ")} ·
      Licences signed with: {overview.signing} · Blog drafts by: {overview.ai}
    </p>
  {/if}

  <details class="case" ontoggle={(e) => (e.currentTarget as HTMLDetailsElement).open && loadPromos()}>
    <summary>🎟️ Promo codes & vouchers</summary>
    <form onsubmit={mint} class="grid">
      <div class="chips" role="radiogroup" aria-label="Kind">
        {#each [["percent", "% off"], ["fixed", "Amount off"], ["voucher", "Voucher (free product)"]] as [k, l] (k)}
          <button type="button" class="chip" role="radio" aria-checked={kind === k} onclick={() => (kind = k as typeof kind)}>{l}</button>
        {/each}
      </div>
      {#if kind === "percent"}<label class="field"><span>Percent off</span><input type="number" min="1" max="100" bind:value={value} /></label>{/if}
      {#if kind === "fixed"}<label class="field"><span>Amount off (in dollars/euros)</span><input type="number" min="1" step="0.01" bind:value={value} /></label>{/if}
      {#if kind === "voucher"}
        <div class="two">
          <label class="field"><span>Grants</span><select bind:value={grantEdition}>{#each EDITIONS as e (e)}<option value={e}>{e}</option>{/each}</select></label>
          <label class="field"><span>Plan</span><select bind:value={grantPlan}>{#each PLANS as p (p)}<option value={p}>{p}</option>{/each}</select></label>
        </div>
        {#if grantPlan !== "perpetual"}<label class="field"><span>Months</span><input type="number" min="1" max="36" bind:value={months} /></label>{/if}
      {:else}
        <fieldset><legend>Only for editions (none = all)</legend><div class="chips">{#each EDITIONS as e (e)}<button type="button" class="chip" aria-pressed={edits.includes(e)} onclick={() => (edits = toggle(edits, e))}>{e}</button>{/each}</div></fieldset>
        <fieldset><legend>Only for plans (none = all)</legend><div class="chips">{#each PLANS as p (p)}<button type="button" class="chip" aria-pressed={plans.includes(p)} onclick={() => (plans = toggle(plans, p))}>{p}</button>{/each}</div></fieldset>
      {/if}
      <div class="chips" role="radiogroup" aria-label="General or individual">
        <button type="button" class="chip" role="radio" aria-checked={mode === "general"} onclick={() => (mode = "general")}>General — anyone may use</button>
        <button type="button" class="chip" role="radio" aria-checked={mode === "individual"} onclick={() => (mode = "individual")}>Individual — one per person</button>
      </div>
      {#if mode === "general"}
        <div class="two">
          <label class="field"><span>Exact code (optional)</span><input type="text" bind:value={exact} placeholder="BACHATA25" /></label>
          <label class="field"><span>…or how many random codes</span><input type="number" min="1" max="500" bind:value={count} disabled={!!exact} /></label>
        </div>
        <div class="two">
          <label class="field"><span>Prefix</span><input type="text" bind:value={prefix} disabled={!!exact} /></label>
          <label class="field"><span>Uses per code (empty = unlimited)</span><input type="number" min="1" bind:value={maxUses} /></label>
        </div>
      {:else}
        <label class="field"><span>E-mail addresses, one per line — each gets its own single-use code</span><textarea bind:value={emails} placeholder="ana@example.com&#10;rafa@example.com"></textarea></label>
      {/if}
      <div class="two">
        <label class="field"><span>Expires (optional)</span><input type="date" bind:value={expires} /></label>
        <label class="field"><span>Campaign (optional)</span><input type="text" bind:value={campaign} placeholder="Summer 2027" /></label>
      </div>
      <button class="btn primary">Mint</button>
    </form>
    {#if minted.length}
      <p><b>{minted.length} minted.</b> <a class="btn quiet" href={"data:text/csv;charset=utf-8," + encodeURIComponent(csv())} download="codes.csv">Download CSV</a></p>
      <ul class="codes">{#each minted as m (m.code)}<li><code>{m.code}</code>{m.boundTo ? ` → ${m.boundTo}` : ""}</li>{/each}</ul>
    {/if}
    {#if promos.length}
      <h3>All codes</h3>
      <div class="scroll">
        <table>
          <thead><tr><th>Code</th><th>Kind</th><th>Used</th><th>For</th><th>Campaign</th><th></th></tr></thead>
          <tbody>
            {#each promos as p (p.code)}
              <tr class:off={!p.a}>
                <td><code>{p.code}</code></td>
                <td>{p.k === "percent" ? `${p.v}%` : p.k === "fixed" ? money(p.v, "USD") : "voucher"}</td>
                <td>{p.u}{p.m != null ? ` / ${p.m}` : ""}</td>
                <td>{p.b ?? "anyone"}</td>
                <td>{p.c ?? ""}</td>
                <td><button class="btn quiet" onclick={() => togglePromo(p.code, !p.a)}>{p.a ? "Switch off" : "Switch on"}</button></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </details>

  <details class="case" ontoggle={(e) => (e.currentTarget as HTMLDetailsElement).open && loadWishes()}>
    <summary>🧱 Wishes</summary>
    {#each wishes as w (w.id)}
      {@const note = { v: w.note ?? "" }}
      <div class="wish">
        <b>{w.title}</b> <span class="muted small">📌 {w.votes} · {w.author}</span>
        <div class="inline">
          <select value={w.status} onchange={(e) => stampWish(w.id, (e.currentTarget as HTMLSelectElement).value, note.v)}>
            {#each STATUSES as s (s)}<option value={s}>{s}</option>{/each}
          </select>
          <input type="text" value={note.v} oninput={(e) => (note.v = (e.currentTarget as HTMLInputElement).value)} placeholder="A note visitors see" />
          <button class="btn quiet" onclick={() => stampWish(w.id, w.status, note.v)}>Save note</button>
          <button class="btn quiet" onclick={() => dropWish(w.id)}>Remove</button>
        </div>
      </div>
    {/each}
  </details>

  <details class="case" ontoggle={(e) => (e.currentTarget as HTMLDetailsElement).open && loadPending()}>
    <summary>👀 To read before it goes public ({overview?.pending ?? "…"})</summary>
    {#if !pending.length}<p class="muted">Nothing waiting.</p>{/if}
    {#each pending as c (c.key)}
      <div class="wish">
        <span class="muted small">{c.kind} · {c.author}{c.rating ? ` · ${c.rating}★` : ""}</span>
        <p>{c.body}</p>
        {#if c.link}<p><a href={c.link} target="_blank" rel="noopener nofollow">{c.link}</a></p>{/if}
        <button class="btn primary" onclick={() => moderate(c.key, true)}>Publish</button>
        <button class="btn quiet" onclick={() => moderate(c.key, false)}>Delete</button>
      </div>
    {/each}
  </details>

  <details class="case" ontoggle={(e) => (e.currentTarget as HTMLDetailsElement).open && loadOrders()}>
    <summary>🧾 Orders</summary>
    <div class="scroll">
      <table>
        <thead><tr><th>When</th><th>What</th><th>Total</th><th>How</th><th>State</th><th></th></tr></thead>
        <tbody>
          {#each orders as o (o.id)}
            <tr>
              <td>{new Date(o.at).toLocaleString()}</td>
              <td>{o.e} · {o.p}</td>
              <td>{money(o.t, o.c)}</td>
              <td>{o.v}</td>
              <td>{o.s}</td>
              <td>
                {#if o.s === "pending" && o.v === "transfer"}
                  <input class="cc" type="text" maxlength="2" placeholder="bank country" bind:value={bankCountry[o.id]} />
                  <button class="btn quiet" onclick={() => markPaid(o.id)}>Mark paid</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </details>

  <details class="case" ontoggle={(e) => (e.currentTarget as HTMLDetailsElement).open && loadBlog()}>
    <summary>📻 The blog desk</summary>
    <form onsubmit={generate}>
      <label class="field"><span>Topic</span><input type="text" bind:value={topic} required placeholder="Running a karaoke night at a beach bar" /></label>
      <label class="field"><span>Angle (optional)</span><textarea bind:value={angle} placeholder="Practical: the first hour, getting shy guests to sing, the voice knob"></textarea></label>
      <div class="two">
        <label class="field"><span>Genre (optional)</span><select bind:value={genre}><option value="">—</option>{#each GENRES as g (g.id)}<option value={g.name}>{g.name}</option>{/each}</select></label>
        <label class="field"><span>Edition to feature</span><select bind:value={featured}><option value="">—</option>{#each EDITIONS as e (e)}<option value={e}>{e}</option>{/each}</select></label>
      </div>
      <label class="field"><span>Reader (optional)</span><input type="text" bind:value={audience} placeholder="Bar owners who have never run karaoke" /></label>
      <fieldset><legend>Languages — each is written natively, not translated</legend>
        <div class="chips">{#each Object.entries(langs) as [code, name] (code)}<button type="button" class="chip" aria-pressed={chosenLangs.includes(code)} onclick={() => (chosenLangs = toggle(chosenLangs, code))}>{name.split(" (")[0]}</button>{/each}</div>
      </fieldset>
      <button class="btn primary" disabled={generating}>{generating ? "Writing… (a minute)" : `Draft in ${chosenLangs.length} language${chosenLangs.length === 1 ? "" : "s"}`}</button>
      <p class="small muted">Drafts are saved unpublished. Read each one, fix what is wrong, then publish. The model is told only what djmanzo really does.</p>
    </form>
    {#if genResult}
      <ul>{#each genResult as r (r.lang)}<li>{r.lang}: {r.ok ? `✓ ${r.title}` : `✗ ${r.error}`}</li>{/each}</ul>
    {/if}
    <h3>Posts and drafts</h3>
    <ul class="drafts">
      {#each drafts as d (d.slug + d.lang)}
        <li>
          <button class="link" onclick={() => openDraft(d.slug, d.lang)}>{d.title}</button>
          <span class="muted small">{d.lang} · {d.published ? "published" : "draft"} · {d.source}</span>
        </li>
      {/each}
    </ul>
    {#if editing}
      <div class="editor">
        <label class="field"><span>Title</span><input type="text" bind:value={editing.title} /></label>
        <label class="field"><span>Excerpt</span><input type="text" bind:value={editing.excerpt} /></label>
        <label class="field"><span>Body (markdown)</span><textarea class="body" bind:value={editing.body}></textarea></label>
        <div class="inline">
          <button class="btn" onclick={() => saveDraft()}>Save</button>
          <button class="btn primary" onclick={() => saveDraft(!editing.published)}>{editing.published ? "Unpublish" : "Publish"}</button>
          <button class="btn quiet" onclick={() => dropDraft(editing.slug, editing.lang)}>Delete</button>
          <button class="btn quiet" onclick={() => (editing = null)}>Close</button>
        </div>
      </div>
    {/if}
  </details>

  <details class="case" ontoggle={(e) => (e.currentTarget as HTMLDetailsElement).open && loadVideos()}>
    <summary>🎬 Videos in the plaza</summary>
    <ul>{#each videos as v, i (v.id)}<li>{v.title} <code>{v.id}</code> <button class="link" onclick={() => saveVideos(videos.filter((_, j) => j !== i))}>remove</button></li>{/each}</ul>
    <form class="inline" onsubmit={(e) => { e.preventDefault(); void saveVideos([...videos, { id: ytId(vUrl), title: vTitle }]); vUrl = ""; vTitle = ""; }}>
      <input type="text" bind:value={vUrl} placeholder="YouTube link or id" required />
      <input type="text" bind:value={vTitle} placeholder="Title" required />
      <button class="btn">Add</button>
    </form>
  </details>
{/if}

<style>
  .inline {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    align-items: center;
  }
  .inline input {
    flex: 1;
    min-width: 140px;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
    gap: 8px;
    margin-bottom: 8px;
  }
  .stats div {
    display: grid;
    padding: 10px;
    border-radius: 10px;
    background: var(--surface-2);
  }
  .stats b {
    font-size: 1.3rem;
  }
  .stats span {
    font-size: 12px;
    color: var(--ink-soft);
  }
  .case {
    margin: 10px 0;
    padding: 10px 14px;
    border-radius: 12px;
    border: 2px solid var(--line);
    background: repeating-linear-gradient(45deg, transparent 0 10px, rgba(255, 255, 255, 0.015) 10px 20px);
  }
  .case summary {
    cursor: pointer;
    font-weight: 800;
    font-size: 1.05rem;
  }
  .grid {
    display: grid;
    gap: 10px;
    margin-top: 10px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  fieldset {
    border: 1px solid var(--line);
    border-radius: 10px;
  }
  .codes {
    columns: 2;
    font-size: 14px;
  }
  .scroll {
    overflow-x: auto;
  }
  tr.off {
    opacity: 0.5;
  }
  .wish {
    padding: 8px 0;
    border-bottom: 1px solid var(--line);
  }
  .cc {
    width: 70px !important;
    min-width: 0 !important;
    display: inline-block;
  }
  .editor .body {
    min-height: 320px;
    font-family: ui-monospace, monospace;
    font-size: 13px;
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
  @media (max-width: 560px) {
    .two {
      grid-template-columns: 1fr;
    }
  }
</style>
