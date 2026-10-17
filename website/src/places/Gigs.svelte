<!--
  The gig board: flyers for kinds of nights. Pick one and walk through it
  in djmanzo's own phases of a night — setup, warm-up, build, peak,
  release, closing — with what djmanzo does at each, and which edition the
  night needs.
-->
<script lang="ts">
  import type { Edition } from "../../shared/catalog";
  import { EDITION_INFO } from "../../shared/catalog";
  import type { PlaceId } from "../lib/places";
  import { profile, type Venue } from "../lib/profile.svelte";

  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();

  interface Night {
    id: string;
    title: string;
    emoji: string;
    venues: Venue[];
    edition: Edition;
    why: string;
    phases: [string, string][];
  }

  const NIGHTS: Night[] = [
    {
      id: "bedroom",
      title: "Practising at home",
      emoji: "🛏️",
      venues: ["bedroom"],
      edition: "hobbyist",
      why: "Everything you need to learn and practise, at the lowest price; a laptop alone is enough.",
      phases: [
        ["Setup", "No controller? The keyboard is one. Import the folder of music you already have; djmanzo analyses tempo, key and phrases."],
        ["Practice", "Loop a transition and do it again. The coach names what your hands just did and suggests the next thing to try."],
        ["Review", "Your set is data: replay it, render it to a file, compare tonight's take with last week's."],
      ],
    },
    {
      id: "family",
      title: "Family party & cookout",
      emoji: "🍖",
      venues: ["parties"],
      edition: "hobbyist",
      why: "Mixing for people you love, no money changing hands — Hobbyist covers it. Add Karaoke if the uncles will sing.",
      phases: [
        ["Warm-up", "Merengue and bachata classics while the food cooks; automix keeps going when you're at the grill."],
        ["Build", "The Next rail suggests what follows, by key, tempo and loudness — so the dance floor grows by itself."],
        ["Peak", "Requests from phones: guests scan a QR code and type what they want to hear. No app to install."],
        ["Closing", "Slow it down for the last dance; save the night's tracklist and send it to the family chat."],
      ],
    },
    {
      id: "wedding",
      title: "Weddings & events",
      emoji: "💍",
      venues: ["weddings"],
      edition: "pro",
      why: "A paid night that must not stop: Pro's commercial licence, recording and the full controller support.",
      phases: [
        ["Setup", "The Wedding theme: a screen calm enough for a room that is not there for the DJ. Set the first-dance record aside."],
        ["Dinner", "Low, warm, unobtrusive: lounge and bolero; automix carries it while you talk to the planner."],
        ["Peak", "Merengue for the aunts, reggaeton for the cousins — the set planner keeps the arc and never repeats an artist twice in a row."],
        ["Emergency", "SAFE takes every control back at once without stopping the music. Nothing a DJ hopes to need, everything they want to have."],
      ],
    },
    {
      id: "club",
      title: "Club resident",
      emoji: "🪩",
      venues: ["club", "festival"],
      edition: "pro",
      why: "Four to six decks, stems, timecode vinyl, recording and MIDI clock for the light desk.",
      phases: [
        ["Warm-up", "Lower tempos, no anthems; djmanzo's night engine reads the room's loudness and tempo against the whole night so far."],
        ["Build", "Phrase-locked mixes: the transition planner shows where a mix can start and how long it can run, with its reasons."],
        ["Peak", "Stems: one record's vocal over another's drums, live, with no added latency."],
        ["Closing", "Record the master; the whole set is saved as data to replay or re-render at studio quality."],
      ],
    },
    {
      id: "bar",
      title: "Bar & beach bar",
      emoji: "🏖️",
      venues: ["bar"],
      edition: "pro",
      why: "A paid residency with long hours: Pro's commercial licence and automix that you can hand the night to.",
      phases: [
        ["Afternoon", "Beach Sunset theme, lounge and kompa; autopilot runs the set while you serve, and touching a control takes it back."],
        ["Golden hour", "Bachata and salsa, harmonic blends the Next rail lines up."],
        ["Night", "Dembow and reggaeton cuts; the room's requests come in from a printed QR sticker on the bar."],
      ],
    },
    {
      id: "karaoke",
      title: "Karaoke night",
      emoji: "🎤",
      venues: ["karaoke-bar"],
      edition: "karaoke",
      why: "The singers are the show: rotation, singers' screen, voice knob, a mic per singer. Add DJ sets between singers with Full.",
      phases: [
        ["Doors", "Guests put their names down from their phones; the rotation keeps it fair, one song each."],
        ["Singing", "Words lit in time on the singers' screen; the key shifted for the singer; a guide voice for a song new to them."],
        ["Between singers", "Break music fades under the next singer; a quiz keeps the room in it."],
        ["Last song", "Each performance recorded and handed to its singer."],
      ],
    },
    {
      id: "radio",
      title: "Radio & live streams",
      emoji: "📡",
      venues: ["radio"],
      edition: "pro",
      why: "Broadcast is commercial use; Pro adds recording and the stream overlay.",
      phases: [
        ["On air", "A now-playing overlay for OBS — Twitch, YouTube, TikTok, Kick — naming the record listeners actually hear."],
        ["After", "The set as a tracklist, shared to WhatsApp, X, Bluesky or Threads with the message already written."],
      ],
    },
    {
      id: "allin",
      title: "DJ and karaoke host",
      emoji: "👑",
      venues: ["karaoke-bar", "club", "weddings"],
      edition: "full",
      why: "Friday the club, Saturday the karaoke bar, Sunday a wedding — both halves in one application.",
      phases: [
        ["Early", "DJ sets with the whole Pro toolkit."],
        ["Midnight", "Switch to the Karaoke activity: the same decks, the singers' panel opens beside them."],
        ["Late", "Back to the decks for the last hour, while the singers' screen shows the room's emoji."],
      ],
    },
  ];

  const mine = (n: Night) => n.venues.some((v) => profile.p.venues.includes(v));
  const sorted = $derived([...NIGHTS].sort((a, b) => Number(mine(b)) - Number(mine(a))));
  let open = $state<string>(NIGHTS.find((n) => mine(n))?.id ?? "family");
  const night = $derived(NIGHTS.find((n) => n.id === open)!);
</script>

<p>Every night asks for something different. Pick yours — {profile.p.venues.length ? "the ones you told the MC about come first" : "or tell the MC where you play and they'll come first"}.</p>

<div class="board">
  {#each sorted as n, i (n.id)}
    <button class="flyer" class:on={open === n.id} class:mine={mine(n)} style="--tilt:{((i * 29) % 5) - 2}deg" onclick={() => (open = n.id)} aria-pressed={open === n.id}>
      <span class="e" aria-hidden="true">{n.emoji}</span>
      <b>{n.title}</b>
      <span class="ed" style="--c:{EDITION_INFO[n.edition].colour}">{EDITION_INFO[n.edition].name}</span>
    </button>
  {/each}
</div>

<section class="night" aria-live="polite">
  <h2>{night.emoji} {night.title}</h2>
  <p><b>The edition:</b> {EDITION_INFO[night.edition].name}. {night.why}</p>
  <ol class="arc">
    {#each night.phases as [phase, what] (phase)}
      <li><span class="phase">{phase}</span><span>{what}</span></li>
    {/each}
  </ol>
  <div class="row">
    <button class="btn primary" onclick={() => go("taquilla")}>🎟️ {EDITION_INFO[night.edition].name} at the box office</button>
    {#if night.edition === "karaoke" || night.edition === "full"}<button class="btn" onclick={() => go("tarima")}>🎤 Try the karaoke stage</button>{/if}
  </div>
</section>

<style>
  .board {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 12px;
    padding: 14px;
    border-radius: 10px;
    background: #b98a5c radial-gradient(circle at 20% 30%, rgba(0, 0, 0, 0.06) 0 2px, transparent 3px) 0 0 / 22px 22px;
  }
  .flyer {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 12px;
    border: 0;
    background: #fffaf0;
    color: #1d1d1d;
    text-align: left;
    rotate: var(--tilt);
    box-shadow: 0 6px 12px rgba(0, 0, 0, 0.25);
    cursor: pointer;
    position: relative;
  }
  .flyer::before {
    content: "";
    position: absolute;
    top: -6px;
    left: 50%;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #c0263b;
    box-shadow: 0 2px 3px rgba(0, 0, 0, 0.4);
  }
  .flyer.on {
    outline: 4px solid var(--primary);
    rotate: 0deg;
  }
  .flyer.mine {
    background: #fff1b8;
  }
  .e {
    font-size: 26px;
  }
  .ed {
    font-size: 12px;
    font-weight: 800;
    padding: 1px 8px;
    border-radius: 999px;
    background: var(--c);
  }
  .night {
    margin-top: 18px;
  }
  .arc {
    list-style: none;
    padding: 0;
    position: relative;
  }
  .arc li {
    display: grid;
    grid-template-columns: 120px 1fr;
    gap: 12px;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .phase {
    font-family: var(--display);
    color: var(--primary);
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  @media (max-width: 560px) {
    .arc li {
      grid-template-columns: 1fr;
      gap: 2px;
    }
  }
</style>
