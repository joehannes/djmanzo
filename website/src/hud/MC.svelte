<!--
  The MC: the island's host, who asks a few questions and changes the island
  with every answer. Name your music and the island takes its colours, its
  type and its loop; name your rooms and your gear and the box office knows
  which edition to open on. Every question can be skipped, and every answer
  changed later.
-->
<script lang="ts">
  import { GENRES, GENRE_BY_ID } from "../lib/genres";
  import { profile, recommend, type Experience, type Gear, type Role, type Venue } from "../lib/profile.svelte";
  import { EDITION_INFO } from "../../shared/catalog";
  import type { PlaceId } from "../lib/places";
  import { islandSound } from "./sound.svelte";
  import { session } from "../lib/session.svelte";

  interface Props {
    onclose: () => void;
    ontour: () => void;
    onplace: (id: PlaceId) => void;
  }
  let { onclose, ontour, onplace }: Props = $props();

  const ROLES: { id: Role; label: string; emoji: string }[] = [
    { id: "dj", label: "I DJ", emoji: "🎧" },
    { id: "karaoke", label: "I host karaoke", emoji: "🎤" },
    { id: "both", label: "Both", emoji: "👑" },
    { id: "learning", label: "I'm learning to DJ", emoji: "🌱" },
    { id: "curious", label: "Just looking around", emoji: "👀" },
  ];
  const VENUES: { id: Venue; label: string }[] = [
    { id: "bedroom", label: "My room" },
    { id: "parties", label: "Friends' and family parties" },
    { id: "bar", label: "Bars" },
    { id: "club", label: "Clubs" },
    { id: "weddings", label: "Weddings & events" },
    { id: "karaoke-bar", label: "Karaoke nights" },
    { id: "radio", label: "Radio & streams" },
    { id: "festival", label: "Festivals" },
    { id: "church", label: "Church & community" },
  ];
  const GEAR: { id: Gear; label: string }[] = [
    { id: "laptop", label: "Just a laptop" },
    { id: "controller", label: "A controller" },
    { id: "turntables", label: "Turntables" },
    { id: "cdjs", label: "CDJs / media players" },
    { id: "mic", label: "Microphones" },
  ];
  const EXP: { id: Experience; label: string }[] = [
    { id: "new", label: "Just starting" },
    { id: "some", label: "A few years" },
    { id: "pro", label: "It's my job" },
  ];

  const STEPS = ["name", "role", "genres", "venues", "gear", "experience", "sound"] as const;
  type Step = (typeof STEPS)[number] | "done";

  let reaction = $state<string | null>(null);
  let nameDraft = $state(profile.p.name);
  const step: Step = $derived((STEPS.find((s) => !profile.p.asked.includes(s)) as Step) ?? "done");
  const n = $derived(STEPS.indexOf(step as never));
  const rec = $derived(recommend(profile.p));
  const hello = $derived(profile.p.name ? `, ${profile.p.name}` : "");

  function next(s: Step, line: string | null) {
    profile.answered(s);
    reaction = line;
    if (session.user) void session.syncProfile();
  }

  function pickGenre(id: string) {
    const first = profile.p.genres.length === 0;
    profile.toggle("genres", id);
    islandSound.follow();
    const g = GENRE_BY_ID[id];
    if (first && profile.p.genres[0] === id) {
      reaction = `${g.name} — ${g.rooms.split(",")[0].toLowerCase()}. I've painted the island in ${g.name}'s colours; in djmanzo this room is the "${g.theme}" theme.`;
    }
  }

  const roleLines: Record<Role, string> = {
    dj: "A selector. Then the booth is yours — two decks are waiting at La Cabina.",
    karaoke: "A host! The singers are the show. La Tarima, on the beach, is your stage.",
    both: "The full night, then: the booth at ten, the karaoke at midnight. You'll want both halves.",
    learning: "Everybody starts somewhere. The cays out at sea are full of moves worth learning.",
    curious: "Welcome. Drag the map anywhere — the sea goes on for ever, and so do the stories.",
  };
</script>

<aside class="mc" aria-label="The MC">
  <div class="head">
    <span class="mic" aria-hidden="true">🎙️</span>
    <div>
      <b>MC Manzo</b>
      {#if step !== "done"}<span class="muted small">Question {n + 1} of {STEPS.length}</span>{/if}
    </div>
    <button class="x" onclick={onclose} aria-label="Close the MC">✕</button>
  </div>

  {#if reaction}
    <p class="reaction">{reaction}</p>
  {/if}

  {#if step === "name"}
    <p class="q">¡Bienvenido a Isla Manzo! I'm the MC. What do they call you behind the decks — or at the mic?</p>
    <form
      onsubmit={(e) => {
        e.preventDefault();
        profile.set("name", nameDraft.trim().slice(0, 40));
        next("name", nameDraft.trim() ? `${nameDraft.trim()} on the decks. Nice ring to it.` : null);
      }}
    >
      <input type="text" bind:value={nameDraft} placeholder="DJ name, stage name, or just your name" aria-label="Your name" maxlength="40" />
      <div class="row">
        <button class="btn primary" type="submit">That's me</button>
        <button class="btn quiet" type="button" onclick={() => next("name", null)}>Skip</button>
      </div>
    </form>
  {:else if step === "role"}
    <p class="q">What brings you to the island{hello}?</p>
    <div class="chips">
      {#each ROLES as r (r.id)}
        <button class="chip" aria-pressed={profile.p.role === r.id} onclick={() => { profile.set("role", r.id); next("role", roleLines[r.id]); }}>{r.emoji} {r.label}</button>
      {/each}
    </div>
    <button class="skip" onclick={() => next("role", null)}>Skip</button>
  {:else if step === "genres"}
    <p class="q">What do you play — or love dancing to? Pick as many as you like; the first one becomes your island.</p>
    <div class="chips genres">
      {#each GENRES as g (g.id)}
        <button class="chip" aria-pressed={profile.p.genres.includes(g.id)} onclick={() => pickGenre(g.id)} style="--chip:{g.palette.primary}">
          {g.emoji} {g.name}{#if profile.p.genres[0] === g.id}<span class="first">your island</span>{/if}
        </button>
      {/each}
    </div>
    <div class="row">
      <button class="btn primary" disabled={!profile.p.genres.length} onclick={() => next("genres", `${profile.p.genres.length} ${profile.p.genres.length === 1 ? "genre" : "genres"} — their islets have sailed in closer. Look around the coast.`)}>Done</button>
      <button class="btn quiet" onclick={() => next("genres", null)}>Skip</button>
    </div>
  {:else if step === "venues"}
    <p class="q">Where do your nights happen?</p>
    <div class="chips">
      {#each VENUES as v (v.id)}
        <button class="chip" aria-pressed={profile.p.venues.includes(v.id)} onclick={() => profile.toggle("venues", v.id)}>{v.label}</button>
      {/each}
    </div>
    <div class="row">
      <button class="btn primary" onclick={() => next("venues", profile.p.venues.includes("weddings") ? "Weddings: a room that is not there for the DJ. djmanzo has a theme for exactly that." : "Noted. The gig board (El Tablero) shows what each of those nights needs.")}>Done</button>
      <button class="btn quiet" onclick={() => next("venues", null)}>Skip</button>
    </div>
  {:else if step === "gear"}
    <p class="q">What's on your table?</p>
    <div class="chips">
      {#each GEAR as gr (gr.id)}
        <button class="chip" aria-pressed={profile.p.gear.includes(gr.id)} onclick={() => profile.toggle("gear", gr.id)}>{gr.label}</button>
      {/each}
    </div>
    <div class="row">
      <button
        class="btn primary"
        onclick={() =>
          next(
            "gear",
            profile.p.gear.includes("turntables")
              ? "Turntables! djmanzo reads timecode vinyl, so your records drive the decks."
              : profile.p.gear.includes("laptop") && profile.p.gear.length === 1
                ? "A laptop is enough: djmanzo makes the keyboard a full controller."
                : "Good. djmanzo learns a controller's mapping when you press its buttons.",
          )}>Done</button
      >
      <button class="btn quiet" onclick={() => next("gear", null)}>Skip</button>
    </div>
  {:else if step === "experience"}
    <p class="q">How long have you been at it?</p>
    <div class="chips">
      {#each EXP as x (x.id)}
        <button class="chip" aria-pressed={profile.p.experience === x.id} onclick={() => { profile.set("experience", x.id); next("experience", x.id === "new" ? "Then djmanzo's coach is for you: it names what your hands just did." : null); }}>{x.label}</button>
      {/each}
    </div>
    <button class="skip" onclick={() => next("experience", null)}>Skip</button>
  {:else if step === "sound"}
    <p class="q">Want to hear your island? It plays {profile.genre ? profile.genre.name.toLowerCase() : "a groove"} quietly while you look around. Everything is synthesised right here — no recordings.</p>
    <div class="row">
      <button class="btn primary" onclick={async () => { await islandSound.start(); next("sound", "There it is. The waves move with the bass now."); }}>Turn the sound on</button>
      <button class="btn quiet" onclick={() => next("sound", "Quiet it is. The speaker button in the corner turns it on later.")}>Not now</button>
    </div>
  {:else}
    <p class="q">
      Here's your island{hello}{#if profile.genre}: {profile.genre.name}'s colours and type{/if}. If I were choosing for you, it would be
      <b>{EDITION_INFO[rec.edition].name}</b> — {rec.why}
    </p>
    <div class="row wrap">
      <button class="btn primary" onclick={() => { profile.set("welcomed", true); ontour(); }}>▶ Play the set (a guided tour)</button>
      <button class="btn" onclick={() => { profile.set("welcomed", true); onplace("taquilla"); }}>Go to the box office</button>
      <button class="btn quiet" onclick={() => { profile.set("welcomed", true); onclose(); }}>Explore on my own</button>
    </div>
    <button class="skip" onclick={() => { profile.p.asked = []; profile.persist(); reaction = null; }}>Answer again</button>
  {/if}
</aside>

<style>
  .mc {
    position: fixed;
    left: 14px;
    bottom: 110px;
    width: min(460px, calc(100vw - 28px));
    max-height: calc(100dvh - 200px);
    overflow: auto;
    padding: 16px 18px 18px;
    border-radius: 20px 20px 20px 4px;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    border: 2px solid var(--line);
    box-shadow: var(--shadow);
    backdrop-filter: blur(12px);
    z-index: 25;
    animation: rise 0.4s cubic-bezier(0.3, 1.4, 0.5, 1);
  }
  @keyframes rise {
    from {
      transform: translateY(20px) scale(0.96);
      opacity: 0;
    }
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }
  .head div {
    display: flex;
    flex-direction: column;
    line-height: 1.15;
  }
  .mic {
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    border-radius: 50%;
    background: var(--primary);
    font-size: 22px;
  }
  .x {
    margin-left: auto;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: 0;
    background: transparent;
    cursor: pointer;
    font-size: 16px;
  }
  .q {
    font-size: 1.08rem;
    font-weight: 600;
  }
  .reaction {
    padding: 10px 12px;
    border-radius: 12px;
    background: color-mix(in srgb, var(--primary) 16%, transparent);
    border-left: 4px solid var(--primary);
    font-style: italic;
  }
  .row {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }
  .row.wrap {
    flex-wrap: wrap;
  }
  form input {
    margin-top: 4px;
  }
  .skip {
    margin-top: 10px;
    background: none;
    border: 0;
    color: var(--ink-soft);
    text-decoration: underline;
    cursor: pointer;
    padding: 4px 0;
  }
  .genres .chip[aria-pressed="true"] {
    background: var(--chip);
    border-color: var(--chip);
    color: #111;
  }
  .first {
    font-size: 11px;
    font-weight: 800;
    padding: 1px 6px;
    margin-left: 4px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.75);
    color: #fff;
  }
  @media (max-width: 640px) {
    .mc {
      left: 8px;
      right: 8px;
      width: auto;
      bottom: 80px;
      max-height: calc(100dvh - 160px);
    }
  }
</style>
