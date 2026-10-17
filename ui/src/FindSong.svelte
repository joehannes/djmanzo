<script lang="ts">
  /**
   * §128: a song the singer cannot name, found from what they do remember.
   *
   * The questions a host can ask across a noisy booth, each with a few
   * short replies: when it is from, how fast, who sings it, in what
   * language, what kind of music, who it sounds like, where they heard it.
   * Each answer narrows the collection at once, by what its tags know
   * (`dj_app::find_song`); who sings and the language are in no tag, so
   * they go to the assistant only, and this says so. *Ask the assistant
   * with all of this* hands every answer, as one sentence, to the guess
   * the memory panel already makes — the one way to a song nobody owns.
   */
  import { findSong, type Found, type Pace, type Remembered, type Voice } from "./findSong";
  import type { LibraryTrack } from "./api";

  let {
    enabled = true,
    onAsk,
    onFind,
    onTrack,
  }: {
    enabled?: boolean;
    /** Ask the assistant with this description. */
    onAsk: (description: string) => void;
    /** Find a record of the collection in the search box. */
    onFind?: (text: string) => void;
    /** Take the record itself, where a name in a search box is not the next gesture. */
    onTrack?: (track: LibraryTrack) => void;
  } = $props();

  const DECADES = [1960, 1970, 1980, 1990, 2000, 2010, 2020];
  const PACES: { id: Pace; label: string }[] = [
    { id: "slow", label: "Slow" },
    { id: "medium", label: "In between" },
    { id: "fast", label: "Fast" },
  ];
  const VOICES: { id: Voice; label: string }[] = [
    { id: "man", label: "A man" },
    { id: "woman", label: "A woman" },
    { id: "duet", label: "A duet" },
    { id: "group", label: "A group" },
  ];

  let remembered = $state<Remembered>({});
  let found = $state<Found | null>(null);
  let error = $state("");

  /** Asked again a moment after the last change, so typing is not a call a key. */
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const asked = $state.snapshot(remembered);
    clearTimeout(timer);
    timer = setTimeout(() => {
      findSong(asked)
        .then((answer) => {
          found = answer;
          error = "";
        })
        .catch((e) => (error = String(e)));
    }, 250);
    return () => clearTimeout(timer);
  });

  /** A reply pressed again is taken back. */
  function pick<K extends "decade" | "pace" | "voice">(key: K, value: Remembered[K]) {
    remembered = { ...remembered, [key]: remembered[key] === value ? null : value };
  }

  const name = (track: LibraryTrack) => (track.artist ? `${track.artist} — ${track.title}` : track.title);
  const decade = (year: number) => `${String(year).slice(2)}s`;
  const answered = $derived(Object.values(remembered).some((v) => v !== null && v !== undefined && v !== ""));
</script>

<section class="find-song" aria-labelledby="find-song-title">
  <h3 id="find-song-title">What do they remember?</h3>
  <p class="note">Ask the singer. Every answer narrows your collection at once; none is needed.</p>

  <div class="ask" role="group" aria-label="When is it from?">
    <span class="q">When</span>
    {#each DECADES as year (year)}
      <button class="chip" aria-pressed={remembered.decade === year} disabled={!enabled} onclick={() => pick("decade", year)}
        >{decade(year)}</button
      >
    {/each}
  </div>
  <div class="ask" role="group" aria-label="How fast is it?">
    <span class="q">How fast</span>
    {#each PACES as pace (pace.id)}
      <button class="chip" aria-pressed={remembered.pace === pace.id} disabled={!enabled} onclick={() => pick("pace", pace.id)}
        >{pace.label}</button
      >
    {/each}
  </div>
  <div class="ask" role="group" aria-label="Who sings it?">
    <span class="q">Who sings</span>
    {#each VOICES as voice (voice.id)}
      <button class="chip" aria-pressed={remembered.voice === voice.id} disabled={!enabled} onclick={() => pick("voice", voice.id)}
        >{voice.label}</button
      >
    {/each}
  </div>
  <div class="fields">
    <label><span class="q">Kind of music</span><input bind:value={remembered.genre} placeholder="bachata, rock, bolero" disabled={!enabled} /></label>
    <label><span class="q">Sounds like</span><input bind:value={remembered.like} placeholder="an artist" disabled={!enabled} /></label>
    <label><span class="q">Language</span><input bind:value={remembered.language} placeholder="Spanish" disabled={!enabled} /></label>
    <label><span class="q">Heard it at</span><input bind:value={remembered.heard} placeholder="a wedding, the radio, a film" disabled={!enabled} /></label>
    <label class="wide"><span class="q">Any words</span><input bind:value={remembered.words} placeholder="even half a line" disabled={!enabled} /></label>
  </div>
  {#if remembered.voice || remembered.language}
    <p class="note" data-assistant-only>Who sings and the language are in no tag of your files — they go to the assistant.</p>
  {/if}

  {#if error}
    <p class="error" role="alert">{error}</p>
  {:else if found}
    {#if !found.checked}
      {#if answered}
        <p class="note">When it is from, how fast, the kind of music or who it sounds like narrow your collection.</p>
      {/if}
    {:else if found.near.length === 0}
      <p class="note" data-none>Nothing in your collection fits all of that — ask the assistant.</p>
    {:else}
      <ul class="hits" aria-label="In your collection, fitting what they remember">
        {#each found.near as hit (hit.track.id)}
          <li>
            <span class="what">{name(hit.track)}</span>
            <span class="because">{hit.because.join(" · ")}</span>
            <button class="find" onclick={() => (onTrack ? onTrack(hit.track) : onFind?.(name(hit.track)))}
              >Find it</button
            >
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
  <div class="row">
    <button disabled={!enabled || !found?.description} onclick={() => found && onAsk(found.description)}
      >Ask the assistant with all of this</button
    >
  </div>
</section>

<style>
  .find-song {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.6rem 0.7rem;
    border: 1px solid color-mix(in srgb, var(--accent) 40%, var(--border));
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 5%, transparent);
  }

  h3 {
    margin: 0;
    font-size: 0.8em;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--accent);
  }

  .note,
  .error {
    margin: 0;
    font-size: 0.82em;
    color: var(--text-dim);
  }

  .ask {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.3rem;
  }

  .q {
    min-width: 5.5rem;
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .chip {
    padding: 0.15rem 0.6rem;
    border-radius: 999px;
    font-size: 0.82em;
    transition:
      background var(--motion-enter) var(--ease),
      transform var(--motion-enter) var(--ease);
  }

  .chip[aria-pressed="true"] {
    background: color-mix(in srgb, var(--accent) 28%, transparent);
    border-color: var(--accent);
    transform: scale(1.04);
  }

  .fields {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(13rem, 1fr));
    gap: 0.3rem 0.6rem;
  }

  .fields label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .fields .wide {
    grid-column: 1 / -1;
  }

  .fields input {
    flex: 1;
    min-width: 0;
  }

  .hits {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .hits li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0 0.5rem;
    align-items: center;
  }

  .because {
    grid-column: 1;
    font-size: 0.78em;
    color: var(--accent);
  }

  .find {
    grid-row: 1 / span 2;
    grid-column: 2;
  }

  .row {
    display: flex;
    gap: 0.4rem;
  }
</style>
