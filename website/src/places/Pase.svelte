<!--
  Your pass: who you are on the island, your licences and the machines they
  are on, the code that links djmanzo to your account, the passport of
  places you have visited — and what the island remembers about you, with
  the means to make it forget.
-->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { api, post } from "../lib/api";
  import { session } from "../lib/session.svelte";
  import { profile } from "../lib/profile.svelte";
  import { PLACES, type PlaceId } from "../lib/places";
  import { GENRE_BY_ID } from "../lib/genres";
  import { EDITION_INFO, MODULE_TITLE, type Edition, type Module } from "../../shared/catalog";
  import SignIn from "../shop/SignIn.svelte";
  import { loopbackFrom } from "../lib/link";

  let { go, linkCode = null }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();

  interface Lic {
    id: string;
    edition: string;
    modules?: string[];
    plan: string;
    status: string;
    expires: number | null;
    inForce: boolean;
    demo: boolean;
    seats: number;
    activations: { name: string; at: number; seen: number }[];
  }
  let licences = $state<Lic[]>([]);
  let code = $state(untrack(() => linkCode ?? ""));
  let machine = $state<string | null>(null);
  let linkMsg = $state<string | null>(null);
  let linked = $state(false);
  let prizeMsg = $state<string | null>(null);
  // djmanzo waiting on a port of its own for the browser to come back (the loopback way).
  const loopback = location.pathname === "/link" ? loopbackFrom(location.search) : null;
  let loopbackMsg = $state<string | null>(null);
  let loopbackGone = $state(false);

  async function answerLoopback(approve: boolean) {
    if (!loopback) return;
    if (!approve) {
      loopbackGone = true;
      loopbackMsg = "Refused. That computer got nothing — djmanzo stops waiting after ten minutes.";
      return;
    }
    try {
      const r = await post("/api/device/loopback", { ...loopback });
      loopbackGone = true;
      loopbackMsg = `Linked. "${loopback.machine}" now has your licences — your browser is handing the answer back to djmanzo.`;
      location.href = r.redirect;
    } catch (e) {
      loopbackMsg = (e as Error).message;
    }
  }

  const stampable = PLACES.filter((p) => p.stamp);
  const NEED = 7;

  async function loadLicences() {
    if (!session.user) return;
    try {
      licences = await api("/api/me/licences");
    } catch {
      licences = [];
    }
  }

  async function lookup() {
    linkMsg = null;
    machine = null;
    try {
      machine = (await api(`/api/device/lookup?code=${encodeURIComponent(code.trim())}`)).machine;
    } catch (e) {
      linkMsg = (e as Error).message;
    }
  }

  async function decide(approve: boolean) {
    try {
      await post("/api/device/approve", { code: code.trim(), approve });
      linked = approve;
      linkMsg = approve ? `Linked. "${machine}" now has your licences — you can close this and go back to djmanzo.` : "Refused. That machine got nothing.";
      machine = null;
    } catch (e) {
      linkMsg = (e as Error).message;
    }
  }

  async function claim() {
    prizeMsg = null;
    try {
      const r = await post("/api/journey/reward", { stamps: profile.p.stamps });
      profile.set("prize", r.code);
    } catch (e) {
      prizeMsg = (e as Error).message;
    }
  }

  onMount(async () => {
    if (!session.loaded) await session.refresh();
    await loadLicences();
    if (code && session.user) await lookup();
  });

  $effect(() => {
    if (session.user) void loadLicences();
  });

  const date = (t: number) => new Date(t).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
</script>

{#if loopback}
  <section class="link-box" data-loopback>
    <h2>Link djmanzo</h2>
    {#if !session.user}
      <SignIn why={`djmanzo on "${loopback.machine}" is asking to be linked to your account. Sign in with the account you bought with, then say yes.`} />
    {:else if loopbackGone}
      <p class="notice ok">{loopbackMsg}</p>
    {:else}
      <p>djmanzo on <b>{loopback.machine}</b> is asking for your licences. Did you just press <b>Sign in</b> in djmanzo on that computer?</p>
      <div class="row">
        <button class="btn primary" onclick={() => answerLoopback(true)}>Yes — link it</button>
        <button class="btn quiet" onclick={() => answerLoopback(false)}>No, that wasn't me</button>
      </div>
      {#if loopbackMsg}<p class="notice">{loopbackMsg}</p>{/if}
    {/if}
  </section>
{:else if code || linkCode}
  <section class="link-box">
    <h2>Link djmanzo</h2>
    {#if !session.user}
      <SignIn why="djmanzo is asking to be linked to your account. Sign in with the account you bought with — then check the code and say yes." />
    {:else if linked}
      <p class="notice ok">{linkMsg}</p>
    {:else}
      <form class="code" onsubmit={(e) => { e.preventDefault(); void lookup(); }}>
        <input type="text" bind:value={code} placeholder="ABCD-EFGH" aria-label="The code djmanzo shows" autocomplete="off" />
        <button class="btn">Check the code</button>
      </form>
      {#if machine}
        <p>The code belongs to <b>{machine}</b>. Is that the computer in front of you, showing the same code?</p>
        <div class="row">
          <button class="btn primary" onclick={() => decide(true)}>Yes — link it</button>
          <button class="btn quiet" onclick={() => decide(false)}>No, that isn't mine</button>
        </div>
      {/if}
      {#if linkMsg}<p class="notice">{linkMsg}</p>{/if}
    {/if}
  </section>
{/if}

{#if !session.user}
  {#if !code && !loopback}<SignIn why="Sign in to see your licences, link djmanzo, and claim the island's prize." />{/if}
{:else}
  <section class="who">
    {#if session.user.avatar}<img src={session.user.avatar} alt="" />{/if}
    <div>
      <b>{session.user.name}</b>{#if session.user.demo}<span class="badge">demo account</span>{/if}
      <span class="muted small">{session.user.email ?? "no e-mail shared"} · signed in with {session.user.providers.join(", ") || "the crew's token"}</span>
    </div>
    <button class="btn quiet" onclick={() => session.signOut()}>Sign out</button>
  </section>

  <h2>Your licences</h2>
  {#if licences.length}
    <ul class="lics">
      {#each licences as l (l.id)}
        <li class:off={!l.inForce}>
          <b>djmanzo {EDITION_INFO[l.edition as Edition]?.name ?? l.edition}</b>{#if l.modules?.length && l.edition === "custom"} ({l.modules.map((m) => MODULE_TITLE[m as Module] ?? m).join(", ")}){/if} — {l.plan === "perpetual" ? "bought once" : l.plan === "trial" ? "a free trial" : l.plan}
          {#if l.demo}<span class="badge">demo</span>{/if}
          <span class="muted small">{l.inForce ? (l.expires ? `until ${date(l.expires)}` : "for ever (version 1)") : `ended (${l.status})`} · on {l.activations.length} of {l.seats} computers</span>
          {#if l.activations.length}
            <ul class="machines">
              {#each l.activations as a (a.at)}<li>💻 {a.name} <span class="muted small">last seen {date(a.seen)}</span></li>{/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
    <p class="small">To use a licence: in djmanzo, <b>Settings → Account → Sign in</b>. It opens this page; sign in and say yes.</p>
  {:else}
    <p class="muted">None yet. <button class="link" onclick={() => go("taquilla")}>The box office is at the harbour.</button></p>
  {/if}
{/if}


<h2>Your passport</h2>
<div class="stamps">
  {#each stampable as p (p.id)}
    {@const has = profile.p.stamps.includes(p.id)}
    <button class="stamp" class:has onclick={() => go(p.id)} aria-label="{p.name}{has ? ', visited' : ', not yet visited'}">
      <span aria-hidden="true">{p.emoji}</span>
      <small>{p.name}</small>
    </button>
  {/each}
</div>
{#if profile.p.prize}
  <p class="notice ok">Your prize: <b class="prize">{profile.p.prize}</b> — 15 % off once, for 30 days, for your account only. Use it at the box office.</p>
{:else if profile.p.stamps.length >= NEED}
  {#if session.user && !session.user.demo}
    <button class="btn primary" onclick={claim}>🎁 Claim the island's prize</button>
  {:else}
    <p class="small">You've seen enough of the island to earn its prize — sign in with a real account to claim it.</p>
  {/if}
{:else}
  <p class="small">Visit {NEED - profile.p.stamps.length} more {NEED - profile.p.stamps.length === 1 ? "place" : "places"} and the island has a prize for you.</p>
{/if}
{#if prizeMsg}<p class="notice">{prizeMsg}</p>{/if}

<h2>What the island knows about you</h2>
<ul class="known">
  <li>Name: {profile.p.name || "—"}</li>
  <li>You: {profile.p.role ?? "—"}</li>
  <li>Music: {profile.p.genres.map((g) => GENRE_BY_ID[g]?.name).join(", ") || "—"}</li>
  <li>Rooms: {profile.p.venues.join(", ") || "—"}</li>
  <li>Gear: {profile.p.gear.join(", ") || "—"}</li>
</ul>
<p class="small muted">Kept in this browser{session.user ? " and copied to your account, so another browser knows it too" : ""}. Nothing else is tracked.</p>
<button class="btn quiet" onclick={() => profile.reset()}>Forget all of it</button>

<style>
  .link-box {
    padding: 14px;
    border-radius: 14px;
    border: 3px solid var(--primary);
    margin-bottom: 18px;
  }
  .code {
    display: flex;
    gap: 8px;
    max-width: 360px;
  }
  .code input {
    text-transform: uppercase;
    font-size: 1.3rem;
    letter-spacing: 0.08em;
    font-weight: 800;
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .who {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .who img {
    width: 48px;
    height: 48px;
    border-radius: 50%;
  }
  .who div {
    display: grid;
    flex: 1;
  }
  .badge {
    margin-left: 8px;
    padding: 1px 8px;
    border-radius: 999px;
    background: var(--secondary);
    color: var(--on-secondary);
    font-size: 11px;
    font-weight: 800;
  }
  .lics {
    list-style: none;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  .lics > li {
    display: grid;
    gap: 2px;
    padding: 12px;
    border-radius: 12px;
    background: var(--surface-2);
  }
  .lics li.off {
    opacity: 0.6;
  }
  .machines {
    list-style: none;
    padding: 0;
    margin: 4px 0 0;
  }
  .stamps {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(92px, 1fr));
    gap: 10px;
  }
  .stamp {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 10px 6px;
    border-radius: 50%;
    aspect-ratio: 1;
    justify-content: center;
    border: 3px dashed var(--line);
    background: transparent;
    cursor: pointer;
    opacity: 0.55;
  }
  .stamp span {
    font-size: 26px;
  }
  .stamp small {
    font-size: 10.5px;
    font-weight: 800;
    text-align: center;
    line-height: 1.1;
  }
  .stamp.has {
    opacity: 1;
    border: 3px solid var(--secondary);
    rotate: -8deg;
    color: var(--secondary);
  }
  .prize {
    font-size: 1.2rem;
    letter-spacing: 0.06em;
  }
  .known {
    padding-left: 18px;
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
