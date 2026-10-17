<!--
  The karaoke stage on the beach: a singers' screen with the words lit in
  time, a guide voice the voice knob takes away, a key knob for the singer,
  a rotation, and emoji from the room. The song is our own — four lines
  written for the island — and the voice is synthesised: it is the shape of
  djmanzo's karaoke, not the sound of it.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { getEngine, Singer, type Engine } from "../lib/audio/engine";
  import { GENRE_BY_ID, scaleNotes } from "../lib/genres";
  import { profile } from "../lib/profile.svelte";
  import type { PlaceId } from "../lib/places";
  import { islandSound } from "../hud/sound.svelte";

  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();

  // Four lines of eight syllables each, one syllable a beat, two bars a line.
  // A syllable ending in "-" runs into the next one without a space.
  const SONG = [
    { words: ["Ba-", "ja", "la", "luz", "y", "su-", "be el", "son"], en: "Lower the lights and raise the sound", melody: [0, 2, 4, 4, 5, 4, 2, 0] },
    { words: ["To-", "da", "la is-", "la", "can-", "ta", "con", "vos"], en: "The whole island sings with you", melody: [0, 2, 4, 5, 7, 5, 4, 2] },
    { words: ["Tu", "voz,", "mi", "voz,", "u-", "na", "can-", "ción"], en: "Your voice, my voice, one song", melody: [4, 4, 5, 4, 2, 2, 0, 2] },
    { words: ["Y el", "mar", "con-", "tes-", "ta", "con", "tam-", "bor"], en: "And the sea answers with a drum", melody: [0, 2, 4, 5, 4, 2, 1, 0] },
  ];
  const COUNT_IN = 4; // beats
  const SINGABLE = ["bachata", "pop", "merengue", "salsa", "cumbia", "kompa", "reggaeton", "rock", "disco", "lounge"];

  let engine: Engine | null = null;
  let singer: Singer | null = null;
  let playing = $state(false);
  let voice = $state(0.8);
  let key = $state(0);
  let at = $state(-1); // the syllable being sung, across the whole song
  let times: number[] = [];
  let endsAt = 0;
  let floaters = $state<{ id: number; e: string; x: number }[]>([]);
  let queue = $state<{ name: string; song: string }[]>([
    { name: profile.p.name || "You", song: "Baja la luz (Isla Manzo)" },
  ]);
  let newName = $state("");
  let newSong = $state("");

  const backing = $derived(SINGABLE.includes(profile.genre?.id ?? "") ? profile.genre!.id : "bachata");
  const line = $derived(at < 0 ? 0 : Math.floor(at / 8));

  function vowel(syl: string): "a" | "e" | "i" | "o" | "u" {
    const m = syl.toLowerCase().match(/[aeiouó]/g);
    const v = (m ? m[m.length - 1] : "a").replace("ó", "o");
    return v as "a" | "e" | "i" | "o" | "u";
  }

  async function start() {
    const e = getEngine();
    await e.wake();
    engine = e;
    if (islandSound.on) islandSound.stop();
    e.decks.forEach((d) => d.stop());
    const g = GENRE_BY_ID[backing];
    const bpm = Math.min(132, Math.round((g.bpm[0] + g.bpm[1]) / 2));
    e.sync = true;
    e.setTempo(bpm);
    e.transpose = key;
    e.setCrossfader(0);
    e.setVolume(0.55);
    e.decks[0].setGenre(backing);
    e.decks[0].start();
    singer ??= new Singer(e);
    singer.level(voice);
    const beat = 60 / bpm;
    const t0 = e.decks[0].nextTime + COUNT_IN * beat;
    const scale = scaleNotes(g.scale === "minor" ? "minor" : "major");
    times = [];
    SONG.forEach((l, li) =>
      l.words.forEach((syl, si) => {
        const t = t0 + (li * 8 + si) * beat;
        times.push(t);
        const deg = l.melody[si];
        const semis = scale[deg % 7] + 12 * Math.floor(deg / 7) + key;
        singer!.note(t, g.root * Math.pow(2, semis / 12), beat * 0.95, vowel(syl));
      }),
    );
    endsAt = t0 + SONG.length * 8 * beat;
    playing = true;
  }

  function stop() {
    engine?.decks.forEach((d) => d.stop());
    singer?.silence();
    playing = false;
    at = -1;
  }

  function setVoice(v: number) {
    voice = v;
    singer?.level(v);
  }

  function setKey(k: number) {
    key = Math.max(-6, Math.min(6, k));
    if (playing) {
      stop();
      void start();
    }
  }

  let nextId = 0;
  function cheer(e: string) {
    const id = nextId++;
    floaters = [...floaters, { id, e, x: 10 + Math.random() * 80 }];
    setTimeout(() => (floaters = floaters.filter((f) => f.id !== id)), 2600);
  }

  function nextSinger() {
    stop();
    queue = [...queue.slice(1), queue[0]];
  }

  onMount(() => {
    let raf = 0;
    const tick = () => {
      if (playing && engine) {
        const now = engine.ctx.currentTime;
        let i = -1;
        for (let k = 0; k < times.length; k++) if (times[k] <= now) i = k;
        at = i;
        if (now > endsAt + 0.5) {
          stop();
          cheer("👏");
          cheer("🎉");
        }
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      stop();
      if (engine) engine.transpose = 0;
    };
  });
</script>

<p>The singers' screen, the voice knob and the queue — the three things a karaoke host lives by. Press <b>Sing</b> and follow the words.</p>

<div class="screen" aria-live="off">
  <div class="up">Up now: <b>{queue[0]?.name}</b> — {queue[0]?.song}</div>
  {#each SONG as l, li (li)}
    {#if li === line || li === line + 1}
      <p class="lyric" class:current={li === line}>
        {#each l.words as w, wi (wi)}
          {@const idx = li * 8 + wi}
          <span class:sung={idx < at} class:now={idx === at}>{w.replace(/-$/, "")}</span>{w.endsWith("-") ? "" : " "}
        {/each}
        <small>{l.en}</small>
      </p>
    {/if}
  {/each}
  {#if playing && at < 0}<div class="count">…ready…</div>{/if}
  {#each floaters as f (f.id)}<span class="float" style="left:{f.x}%">{f.e}</span>{/each}
</div>

<div class="controls">
  <button class="btn primary big" onclick={() => (playing ? stop() : start())}>{playing ? "■ Stop" : "🎤 Sing"}</button>
  <label class="knob">
    <span>Voice knob — {voice < 0.05 ? "singer out (instrumental)" : voice > 0.95 ? "the original singer" : "guide voice"}</span>
    <input type="range" min="0" max="1" step="0.01" value={voice} oninput={(e) => setVoice(+(e.currentTarget as HTMLInputElement).value)} />
  </label>
  <div class="key">
    <span>Key for the singer: <b>{key > 0 ? "+" : ""}{key}</b></span>
    <button class="chip" onclick={() => setKey(key - 1)} aria-label="Lower the key">♭ lower</button>
    <button class="chip" onclick={() => setKey(key + 1)} aria-label="Raise the key">♯ higher</button>
  </div>
</div>
<p class="small muted">Backing: a {GENRE_BY_ID[backing].name.toLowerCase()} groove{profile.genre && SINGABLE.includes(profile.genre.id) ? " — your island's genre" : ""}. In djmanzo the voice knob works on any record you own: stems take the vocal out ahead of the playhead.</p>

<div class="room">
  <span>From the room:</span>
  {#each ["👏", "🔥", "❤️", "😂", "🎉", "🌹"] as e (e)}
    <button class="emoji" onclick={() => cheer(e)} aria-label="Send {e}">{e}</button>
  {/each}
</div>

<h2>The queue</h2>
<ol class="queue">
  {#each queue as q, i (i)}
    <li class:first={i === 0}><b>{q.name}</b> — {q.song}</li>
  {/each}
</ol>
<form
  class="add"
  onsubmit={(e) => {
    e.preventDefault();
    if (!newName.trim()) return;
    queue = [...queue, { name: newName.trim().slice(0, 30), song: newSong.trim().slice(0, 60) || "Their pick" }];
    newName = "";
    newSong = "";
  }}
>
  <input type="text" bind:value={newName} placeholder="Singer" aria-label="Singer's name" />
  <input type="text" bind:value={newSong} placeholder="Song" aria-label="Song" />
  <button class="btn">Add</button>
  <button class="btn quiet" type="button" onclick={nextSinger}>Next singer</button>
</form>

<h2>In djmanzo</h2>
<figure>
  <img class="shot" src="/shots/karaoke.webp" alt="djmanzo in its Karaoke activity: two decks, the library, and the Singers panel with break music, a quiz and the songbook" loading="lazy" width="1280" height="800" />
  <figcaption>The Karaoke activity in djmanzo today: the Singers panel with break music between singers, a music quiz for the room, and the songbook.</figcaption>
</figure>
<ul class="feat">
  <li><b>A rotation that is fair</b> — one song each, in turn; djmanzo says who is next and fades break music under them.</li>
  <li><b>Words timed by speech recognition</b> inside djmanzo, on records you own; a second screen for the singers.</li>
  <li><b>The voice knob</b> — as recorded, a guide voice, or the singer taken out — and the key shifted without changing the tempo.</li>
  <li><b>A microphone and an effect chain per singer</b>, four vocal presets, pitch correction, and a monitor for each singer where the sound card has the outputs.</li>
  <li><b>Guests from their phones</b>: they put their names down and send emoji to the singers' screen. No app to install.</li>
  <li><b>Recorded performances</b>: each singer's, kept.</li>
</ul>
<p class="muted small">Karaoke is in beta: built and tested, and not yet through a real night with real microphones. The Karaoke edition and Full include it.</p>
<div class="row">
  <button class="btn primary" onclick={() => go("taquilla")}>🎟️ The Karaoke edition</button>
  <button class="btn" onclick={() => go("gigs")}>📌 Karaoke nights on the gig board</button>
</div>

<style>
  .screen {
    position: relative;
    overflow: hidden;
    min-height: 240px;
    padding: 18px 20px;
    border-radius: 14px;
    background: radial-gradient(ellipse at 50% 120%, #3b0d5c, #0b0414 70%);
    color: #fff;
    text-align: center;
  }
  .up {
    font-size: 13px;
    opacity: 0.75;
    margin-bottom: 18px;
  }
  .lyric {
    font: 800 clamp(1.4rem, 3.4vw, 2.1rem) / 1.25 var(--body);
    max-width: none;
    margin: 0 0 14px;
    opacity: 0.45;
    transition: opacity 0.3s;
  }
  .lyric.current {
    opacity: 1;
  }
  .lyric small {
    display: block;
    font-size: 0.85rem;
    font-weight: 500;
    opacity: 0.7;
  }
  .lyric span {
    transition: color 0.12s;
  }
  .lyric .sung {
    color: #ff4fd8;
  }
  .lyric .now {
    color: #2ef2ff;
    text-shadow: 0 0 14px #2ef2ff;
  }
  .count {
    font-weight: 800;
    letter-spacing: 0.1em;
    opacity: 0.8;
  }
  .float {
    position: absolute;
    bottom: 0;
    font-size: 34px;
    animation: up 2.6s ease-out forwards;
  }
  @keyframes up {
    to {
      transform: translateY(-260px) scale(1.4);
      opacity: 0;
    }
  }
  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    align-items: center;
    margin: 14px 0 6px;
  }
  .big {
    min-width: 130px;
    font-size: 1.1rem;
  }
  .knob {
    display: grid;
    flex: 1;
    min-width: 220px;
    font-size: 14px;
    font-weight: 700;
  }
  .knob input {
    accent-color: var(--primary);
  }
  .key {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
    font-size: 14px;
  }
  .room {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
    margin: 10px 0;
  }
  .emoji {
    font-size: 24px;
    width: 46px;
    height: 46px;
    border-radius: 50%;
    border: 2px solid var(--line);
    background: var(--surface-2);
    cursor: pointer;
  }
  .emoji:active {
    transform: scale(0.9);
  }
  .queue li.first {
    font-weight: 700;
    color: var(--primary);
  }
  .add {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .add input {
    flex: 1;
    min-width: 140px;
  }
  .feat li {
    margin-bottom: 6px;
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  h2 {
    margin-top: 20px;
  }
</style>
