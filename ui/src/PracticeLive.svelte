<script lang="ts">
  /**
   * §69, live: a record in the headphones to loop, strip to its stems and put
   * an effect on, while the set goes on — the practice deck
   * (`dj_app::practice_live`), which the room never hears.
   *
   * Every press is said in the deck vocabulary a controller uses (`loop 4`,
   * `stem_mute drums`, `fx 1 echo`), so practising a move here is practising
   * the move a deck does. What the room hears is not touched: this panel has
   * no way to reach a deck, the crossfader or the master.
   */
  import { onDestroy } from "svelte";
  import {
    formatTime,
    librarySearch,
    practiceLive,
    practiceLiveDo,
    practiceLiveEject,
    practiceLiveLoad,
    type LibraryTrack,
    type PracticeLive,
  } from "./api";

  let {
    enabled,
    records = [],
  }: {
    enabled: boolean;
    /** Records to offer at once — the held pair's two, when there is one. */
    records?: { id: string; title: string }[];
  } = $props();

  let live = $state<PracticeLive | null>(null);
  let error = $state("");
  let query = $state("");
  let found = $state<LibraryTrack[]>([]);
  /** Stems the DJ has muted here: the engine does not publish the practice deck's. */
  let muted = $state<Record<string, boolean>>({});
  let fxName = $state("none");
  /** Bands killed here, as the deck's EQ words do it: 0 is the band out, 1 as recorded. */
  let killed = $state<Record<string, boolean>>({});
  const BANDS: { id: string; label: string }[] = [
    { id: "low", label: "Low" },
    { id: "mid", label: "Mid" },
    { id: "high", label: "High" },
  ];

  async function toggleBand(band: string) {
    await say(`eq_${band} ${killed[band] ? 1 : 0}`);
    if (!error) killed = { ...killed, [band]: !killed[band] };
  }
  let effectOn = $state(false);

  const LOOPS = [1, 2, 4, 8];
  const STEMS: { id: string; label: string }[] = [
    { id: "vocal", label: "Vocals" },
    { id: "drums", label: "Drums" },
    { id: "bass", label: "Bass" },
    { id: "other", label: "Other" },
  ];
  const EFFECTS = ["none", "echo", "delay", "reverb", "filter", "flanger", "phaser", "gate", "crush"];

  async function ask(run: () => Promise<PracticeLive>) {
    try {
      live = await run();
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  void ask(practiceLive);

  /** Read again while a record is on it, for the playhead and the loop. */
  const timer = setInterval(() => {
    if (enabled && live?.track) void ask(practiceLive);
  }, 500);
  onDestroy(() => clearInterval(timer));

  async function load(id: string) {
    muted = {};
    killed = {};
    fxName = "none";
    effectOn = false;
    found = [];
    query = "";
    await ask(() => practiceLiveLoad(id));
  }

  const say = (action: string) => ask(() => practiceLiveDo(action));

  async function toggleStem(stem: string) {
    await say(`stem_mute ${stem}`);
    if (!error) muted = { ...muted, [stem]: !muted[stem] };
  }

  async function chooseEffect(name: string) {
    fxName = name;
    await say(`fx 1 ${name}`);
  }

  async function toggleEffect() {
    await say(effectOn ? "fx 1 off" : "fx 1 on");
    if (!error) effectOn = !effectOn;
  }

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const text = query.trim();
    clearTimeout(searchTimer);
    if (text.length < 2) {
      found = [];
      return;
    }
    searchTimer = setTimeout(() => {
      void librarySearch(text)
        .then((tracks) => (found = (tracks ?? []).slice(0, 5)))
        .catch(() => (found = []));
    }, 250);
    return () => clearTimeout(searchTimer);
  });

  const name = (t: LibraryTrack) => (t.artist ? `${t.artist} – ${t.title}` : t.title);
</script>

<section class="live" aria-label="Practise live, in your headphones">
  <h3>Live, in your headphones</h3>
  <p class="hint">
    Loop a record, take its stems apart and try an effect — only you hear it, and nothing here touches the decks.
  </p>
  {#if live && !live.headphones}
    <p class="problem" data-no-headphones>
      This output has no headphone pair, so the practice deck is silent: nothing private to play it on.
    </p>
  {/if}

  <div class="choose">
    {#each records as record (record.id)}
      <button type="button" disabled={!enabled} onclick={() => void load(record.id)}>Try {record.title}</button>
    {/each}
    <input
      bind:value={query}
      placeholder="Or find a record to try"
      aria-label="Find a record to practise on"
      disabled={!enabled}
    />
  </div>
  {#if found.length > 0}
    <ul class="found" aria-label="Records to practise on">
      {#each found as track (track.id)}
        <li><button type="button" onclick={() => void load(track.id)}>{name(track)}</button></li>
      {/each}
    </ul>
  {/if}

  {#if live?.track}
    <div class="deck" data-practice-live>
      <p class="now">
        <strong>{live.title}</strong>
        <span class="time">{formatTime(live.position_seconds)} / {formatTime(live.length_seconds)}</span>
        {#if live.looping}<span class="looping">looping</span>{/if}
        <span class="meter" style:--level={Math.min(1, live.level)} aria-hidden="true"></span>
      </p>
      <div class="row">
        <button type="button" onclick={() => void say("play_pause")}>{live.playing ? "Pause" : "Play"}</button>
        <button type="button" onclick={() => void ask(practiceLiveEject)}>Take it off</button>
      </div>
      <div class="row" role="group" aria-label="Loop">
        <span class="label">Loop</span>
        {#each LOOPS as beats (beats)}
          <button type="button" onclick={() => void say(`loop ${beats}`)}>{beats}</button>
        {/each}
        <button type="button" title="Half as long" onclick={() => void say("loop_halve")}>½</button>
        <button type="button" title="Twice as long" onclick={() => void say("loop_double")}>×2</button>
        <button type="button" onclick={() => void say("loop_off")}>Off</button>
      </div>
      <div class="row stems" role="group" aria-label="Stems">
        <span class="label">Stems</span>
        {#each STEMS as stem (stem.id)}
          <button type="button" aria-pressed={!muted[stem.id]} onclick={() => void toggleStem(stem.id)}>{stem.label}</button>
        {/each}
      </div>
      <div class="row stems" role="group" aria-label="EQ">
        <span class="label">EQ</span>
        {#each BANDS as band (band.id)}
          <button type="button" aria-pressed={!killed[band.id]} onclick={() => void toggleBand(band.id)}>{band.label}</button>
        {/each}
      </div>
      <div class="row" role="group" aria-label="Effect">
        <span class="label">Effect</span>
        <select
          aria-label="Practice effect"
          value={fxName}
          onchange={(e) => void chooseEffect(e.currentTarget.value)}
        >
          {#each EFFECTS as name (name)}<option value={name}>{name}</option>{/each}
        </select>
        <button type="button" aria-pressed={effectOn} disabled={fxName === "none"} onclick={() => void toggleEffect()}
          >{effectOn ? "On" : "Off"}</button
        >
      </div>
    </div>
  {/if}
  {#if error}<p class="problem">{error}</p>{/if}
</section>

<style>
  .live {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    border-top: 1px solid var(--border);
    padding-top: 0.5rem;
  }
  h3 {
    margin: 0;
    font-size: 0.95em;
  }
  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.85em;
  }
  .problem {
    margin: 0;
    color: var(--warn);
    font-size: 0.85em;
  }
  .choose,
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    align-items: center;
  }
  .choose input {
    flex: 1 1 10rem;
    min-width: 0;
  }
  .found {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .found button {
    width: 100%;
    text-align: left;
  }
  .label {
    color: var(--muted);
    font-size: 0.85em;
    min-width: 3.5rem;
  }
  .now {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
  }
  .time {
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
  .looping {
    color: var(--accent);
    font-size: 0.85em;
  }
  .meter {
    width: 4rem;
    height: 0.35rem;
    background: linear-gradient(to right, var(--accent) calc(var(--level) * 100%), var(--border) 0);
    border-radius: 2px;
  }
  /* A stem taken out reads as out; the effect's own switch does not. */
  .stems button[aria-pressed="false"] {
    opacity: 0.55;
    text-decoration: line-through;
  }
</style>
