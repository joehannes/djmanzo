<script lang="ts">
  /**
   * §107: the singer rotation, as a karaoke host works it.
   *
   * The rules — first come, first served; one song a turn; not ready is the
   * bottom of the list; a singer's key remembered with their song — are
   * `dj_app::karaoke`'s, and so is every change: this draws the rotation and
   * reports a press. What is here rather than there is the one thing only the
   * interface can do: put the up-next song on a deck, in the singer's key.
   */
  import {
    dispatch,
    karaokeAsk,
    karaokeClear,
    karaokeKey,
    karaokeLeave,
    karaokeMove,
    karaokeNotHere,
    karaokeRotation,
    karaokeSang,
    librarySearch,
    loadTrack,
    type LibraryTrack,
    type Rotation,
  } from "./api";

  let { enabled, deckCount = 2 }: { enabled: boolean; deckCount?: number } = $props();

  let rotation = $state<Rotation | null>(null);
  let error = $state("");

  let singer = $state("");
  let song = $state("");
  /** The record chosen from the collection for the song, if one was. */
  let picked = $state<LibraryTrack | null>(null);
  let matches = $state<LibraryTrack[]>([]);

  async function refresh() {
    try {
      rotation = await karaokeRotation();
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    if (!enabled) return;
    void refresh();
  });

  /** Every change comes back as the whole rotation, so the list is Rust's. */
  async function change(done: Promise<Rotation>) {
    error = "";
    try {
      rotation = await done;
    } catch (e) {
      error = String(e);
    }
  }

  /**
   * The collection, searched as the song is typed. A quarter of a second
   * after the last key, so a fast typist does not send a query a letter.
   */
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const text = song.trim();
    clearTimeout(searchTimer);
    if (!enabled || text.length < 2 || (picked && label(picked) === text)) {
      matches = [];
      return;
    }
    searchTimer = setTimeout(() => {
      void librarySearch(text)
        .then((found) => (matches = (found ?? []).slice(0, 5)))
        .catch(() => (matches = []));
    }, 250);
    return () => clearTimeout(searchTimer);
  });

  function label(track: LibraryTrack): string {
    return track.artist ? `${track.artist} – ${track.title}` : track.title;
  }

  function pick(track: LibraryTrack) {
    picked = track;
    song = label(track);
    matches = [];
  }

  async function add() {
    const name = singer.trim();
    const title = song.trim();
    if (!name || !title) return;
    const chosen = picked && label(picked) === title ? picked : null;
    await change(karaokeAsk(name, title, chosen?.id ?? null, chosen?.path ?? null, null));
    if (!error) {
      singer = "";
      song = "";
      picked = null;
    }
  }

  /**
   * Put the up-next song on a deck, in the singer's key.
   *
   * The load, then the key: the key belongs to the deck, and a key sent before
   * the record arrived would be applied to whatever was on it.
   */
  async function load(deck: number) {
    const next = rotation?.singers.find((s) => s.name === rotation?.up_next)?.songs[0];
    if (!next?.path) return;
    error = "";
    try {
      await loadTrack(deck, next.path);
      await dispatch(`deck ${deck} key ${next.key}`);
    } catch (e) {
      error = String(e);
    }
  }

  /**
   * One place up in calling order: in front of the waiting singer before
   * them. Found in the whole rotation, not in the list on screen — the
   * rotation also holds singers with nothing waiting, so the two positions
   * are not the same number.
   */
  async function moveUp(name: string, before: string) {
    const to = rotation?.singers.findIndex((s) => s.name === before) ?? -1;
    if (to < 0) return;
    await change(karaokeMove(name, to));
  }

  const upNext = $derived(
    rotation?.singers.find((s) => s.name === rotation?.up_next) ?? null,
  );
  const waiting = $derived((rotation?.singers ?? []).filter((s) => s.songs.length > 0));

  function signed(key: number): string {
    return key > 0 ? `+${key}` : `${key}`;
  }
</script>

<div class="singers" data-singers>
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  {#if upNext}
    {@const next = upNext.songs[0]}
    <!--
      The up-next singer, large: the host reads it out, and reads it from
      across the booth with a microphone in the other hand.
    -->
    <section class="up" aria-label="Up next">
      <p class="who">{upNext.name}</p>
      <p class="what">{next.title}</p>
      <div class="key" role="group" aria-label="Key for {upNext.name}">
        <button
          aria-label="Lower the key"
          onclick={() => void change(karaokeKey(upNext.name, next.key - 1))}>−</button
        >
        <span class="mono" data-key>{next.key === 0 ? "as recorded" : `key ${signed(next.key)}`}</span>
        <button
          aria-label="Raise the key"
          onclick={() => void change(karaokeKey(upNext.name, next.key + 1))}>+</button
        >
      </div>
      <div class="actions">
        {#if next.path}
          {#each Array.from({ length: deckCount }, (_, i) => i + 1) as deck (deck)}
            <button onclick={() => void load(deck)}>Load on {deck}</button>
          {/each}
        {:else}
          <span class="hint">Not found in the collection yet — search for it below.</span>
        {/if}
        <button class="done" onclick={() => void change(karaokeSang())}>Sang</button>
        <button
          title="Called and not here: to the bottom of the list, songs kept"
          onclick={() => void change(karaokeNotHere(upNext.name))}>Not here</button
        >
      </div>
    </section>
  {:else}
    <p class="empty">Nobody is queued. Add the first singer below.</p>
  {/if}

  {#if waiting.length > 1}
    <ol class="order" aria-label="Calling order">
      {#each waiting.slice(1) as person, index (person.name)}
        <li>
          <span class="pos mono">{index + 2}</span>
          <span class="name">{person.name}</span>
          <span class="song">{person.songs[0].title}</span>
          {#if person.songs[0].key !== 0}
            <span class="mono k">{signed(person.songs[0].key)}</span>
          {/if}
          {#if person.songs.length > 1}
            <span class="more" title="More songs waiting for later turns">+{person.songs.length - 1}</span>
          {/if}
          <button
            class="quiet"
            aria-label="Move {person.name} up"
            title="Move up"
            onclick={() => void moveUp(person.name, waiting[index].name)}>↑</button
          >
          <button
            class="quiet"
            aria-label="{person.name} leaves"
            title="Leaves — their keys are remembered"
            onclick={() => void change(karaokeLeave(person.name))}>×</button
          >
        </li>
      {/each}
    </ol>
  {/if}

  <form
    class="ask"
    onsubmit={(event) => {
      event.preventDefault();
      void add();
    }}
  >
    <input bind:value={singer} placeholder="Singer" aria-label="Singer's name" />
    <div class="song-box">
      <input
        bind:value={song}
        placeholder="Song — searches your collection"
        aria-label="Song"
        oninput={() => (picked = null)}
      />
      {#if matches.length > 0}
        <ul class="matches" aria-label="Songs in your collection">
          {#each matches as track (track.id)}
            <li>
              <button type="button" onclick={() => pick(track)}>{label(track)}</button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
    <button type="submit" disabled={!singer.trim() || !song.trim()}>Add</button>
  </form>

  {#if rotation && rotation.lately.length > 0}
    <details class="lately">
      <summary>Sung lately</summary>
      <ul>
        {#each rotation.lately as sung, i (i)}
          <li>
            {sung.singer} — {sung.title}{sung.key !== 0 ? ` (${signed(sung.key)})` : ""}
          </li>
        {/each}
      </ul>
    </details>
  {/if}

  <button class="quiet new-night" onclick={() => void change(karaokeClear())}
    >New night — clear the list, keep everybody's keys</button
  >
</div>

<style>
  .singers {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0.4rem 0.2rem;
  }

  .up {
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    padding: 0.5rem 0.6rem;
    background: var(--panel);
  }

  .who {
    margin: 0;
    font-size: 1.4em;
    font-weight: 700;
  }

  .what {
    margin: 0.1rem 0 0.4rem;
    color: var(--text-dim);
  }

  .key,
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem;
    margin-top: 0.3rem;
  }

  .done {
    border-color: var(--accent);
    font-weight: 600;
  }

  .order {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .order li {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
  }

  .pos {
    color: var(--text-dim);
    min-width: 1.2rem;
  }

  .name {
    font-weight: 600;
  }

  .song {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-dim);
  }

  .more,
  .k {
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .ask {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .song-box {
    position: relative;
    flex: 1;
    min-width: 10rem;
  }

  .song-box input {
    width: 100%;
  }

  .matches {
    list-style: none;
    margin: 0.2rem 0 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg);
  }

  .matches button {
    width: 100%;
    text-align: left;
    border: 0;
    background: none;
    padding: 0.3rem 0.5rem;
  }

  .matches button:hover {
    background: var(--panel-hover);
  }

  .empty,
  .hint {
    color: var(--text-dim);
    font-size: 0.9em;
  }

  .error {
    color: var(--danger);
    margin: 0;
  }

  .new-night {
    align-self: flex-start;
    font-size: 0.85em;
  }
</style>
