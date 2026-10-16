<script lang="ts">
  /**
   * §127: the playlist under a deck.
   *
   * A tab on the deck's bottom edge; a press opens a drawer over the lower
   * part of the deck — the playlist chosen for this deck, its records in
   * order, the one this deck took last and the one that comes next. A press
   * on a record loads it onto this deck; ▶ loads and plays it. *Next when
   * this ends* and *Repeat* make it a deck that goes round by itself — a
   * filler deck — and Rust does the going round (`dj_app::deck_queue`).
   *
   * Drawn over the deck rather than under it: a deck is already taller than
   * the stage at djmanzo's own window, and a drawer that pushed the deck
   * down would push the crossfader with it.
   */
  import { listPlaylists, type Playlist } from "./api";
  import { deckQueue, deckQueueLoad, deckQueueSet, type DeckQueue } from "./deckQueue";

  let {
    deck,
    title = null,
    enabled = true,
  }: {
    /** 1-based. */
    deck: number;
    /** What is on the deck now; the drawer asks again when it changes. */
    title?: string | null;
    enabled?: boolean;
  } = $props();

  let open = $state(false);
  let queue = $state<DeckQueue | null>(null);
  let lists = $state<Playlist[]>([]);
  let error = $state("");
  let busy = $state(false);

  async function refresh() {
    try {
      queue = await deckQueue(deck);
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  /**
   * Asked once at first, so the tab can name the deck's playlist, then on
   * opening and when the record on the deck changes — a key of primitives,
   * not the snapshot object, which is new sixty times a second.
   */
  let asked = "";
  $effect(() => {
    const key = `${deck}|${title ?? ""}|${open}`;
    if (key === asked) return;
    asked = key;
    void refresh();
  });

  /**
   * While open, asked again every two seconds: a deck that went on by itself
   * may have taken the same record again, which no title change would say.
   * Never per frame (docs/HANDOFF.md: a call on every snapshot is forty a
   * second of nothing).
   */
  $effect(() => {
    if (!open) return;
    const timer = setInterval(() => void refresh(), 2_000);
    return () => clearInterval(timer);
  });

  /** The queue's record is on the deck only while the deck says so. */
  const onDeck = (i: number) => queue?.at === i && queue.entries[i]?.title === title;

  $effect(() => {
    if (!open || lists.length > 0) return;
    void listPlaylists()
      .then((found) => (lists = found.filter((list) => list.kind === "list")))
      .catch((e) => (error = String(e)));
  });

  async function choose(playlist: number | null) {
    try {
      queue = await deckQueueSet(deck, playlist, queue?.advance ?? false, queue?.repeat ?? false);
    } catch (e) {
      error = String(e);
    }
  }

  async function setFlags(advance: boolean, repeat: boolean) {
    try {
      queue = await deckQueueSet(deck, queue?.playlist ?? null, advance, repeat);
    } catch (e) {
      error = String(e);
    }
  }

  async function load(index: number, play: boolean) {
    busy = true;
    try {
      queue = await deckQueueLoad(deck, index, play);
      error = "";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  const time = (seconds: number) => {
    const whole = Math.max(0, Math.round(seconds));
    return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
  };
  /** The whole list's running time. */
  const length = $derived((queue?.entries ?? []).reduce((sum, entry) => sum + (entry.duration_seconds ?? 0), 0));
</script>

<div class="deck-drawer" data-open={open}>
  {#if open}
    <div class="drawer" role="region" aria-label="Deck {deck}'s playlist">
      <div class="head">
        <select
          aria-label="Playlist for deck {deck}"
          value={queue?.playlist ?? ""}
          disabled={!enabled}
          onchange={(e) => void choose(e.currentTarget.value ? Number(e.currentTarget.value) : null)}
        >
          <option value="">No playlist</option>
          {#each lists as list (list.id)}
            <!-- Marked here as well as by the select's value: the lists can
                 arrive after the queue, and a value set before its option
                 exists is lost. -->
            <option value={list.id} selected={list.id === queue?.playlist}>{list.name} — {list.track_count}</option>
          {/each}
        </select>
        {#if queue?.playlist != null}
          <label class="flag" title="When this deck runs out, the next record is loaded and played">
            <input
              type="checkbox"
              checked={queue.advance}
              aria-label="Deck {deck}: next when this ends"
              disabled={!enabled}
              onchange={(e) => void setFlags(e.currentTarget.checked, queue?.repeat ?? false)}
            />
            <span>Next when this ends</span>
          </label>
          <label class="flag" title="After the last record, the first again">
            <input
              type="checkbox"
              checked={queue.repeat}
              aria-label="Deck {deck}: repeat"
              disabled={!enabled}
              onchange={(e) => void setFlags(queue?.advance ?? false, e.currentTarget.checked)}
            />
            <span>Repeat</span>
          </label>
        {/if}
      </div>
      {#if queue?.problem}
        <p class="problem" role="status">{queue.problem}</p>
      {:else if queue?.playlist == null}
        <p class="hint">Choose a playlist for this deck — a press on a record loads it here.</p>
      {:else if queue.entries.length === 0}
        <p class="hint">{queue.name ?? "This playlist"} is empty.</p>
      {:else}
        <p class="sum">
          {queue.entries.length} records · {time(length)}{#if queue.advance} · goes on by itself{/if}{#if queue.repeat}
            · round again{/if}
        </p>
        <ol class="entries" aria-label="Records in deck {deck}'s playlist">
          {#each queue.entries as entry, i (entry.position)}
            <li data-at={onDeck(i)} data-next={queue.next === i}>
              <button
                class="pick"
                disabled={!enabled || busy}
                title="Load {entry.title} on deck {deck}"
                onclick={() => void load(i, false)}
              >
                <span class="n mono">{i + 1}</span>
                <span class="what"
                  ><span class="title">{entry.title}</span>{#if entry.artist}<span class="artist"
                      >&nbsp;— {entry.artist}</span
                    >{/if}</span
                >
                <span class="tag"
                  >{onDeck(i) ? "on the deck" : queue.next === i ? "next" : ""}</span
                >
                <span class="len mono">{time(entry.duration_seconds ?? 0)}</span>
              </button>
              <button
                class="play"
                disabled={!enabled || busy}
                aria-label="Load and play {entry.title} on deck {deck}"
                onclick={() => void load(i, true)}>▶</button
              >
            </li>
          {/each}
        </ol>
      {/if}
      {#if error}
        <p class="problem" role="alert">{error}</p>
      {/if}
    </div>
  {/if}
  <button
    class="tab"
    aria-expanded={open}
    aria-label="Deck {deck}'s playlist"
    title={queue?.name ? `${queue.name} — deck ${deck}'s playlist` : `A playlist for deck ${deck}`}
    onclick={() => (open = !open)}
  >
    <span class="chev" aria-hidden="true">{open ? "▾" : "▴"}</span>
    <span class="label">{queue?.name ?? "Playlist"}</span>
    {#if queue?.advance}<span class="badge" title="Goes on by itself">⏭</span>{/if}
    {#if queue?.repeat}<span class="badge" title="Round again">↻</span>{/if}
  </button>
</div>

<style>
  /* Out of the deck's flow: no height of its own, so the deck is exactly as
     tall with the tab as without it. */
  .deck-drawer {
    position: absolute;
    inset: auto 0 0 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    pointer-events: none;
    z-index: 4;
  }

  .deck-drawer > * {
    pointer-events: auto;
  }

  .tab {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    max-width: 60%;
    padding: 0.1rem 0.8rem 0.15rem;
    font-size: 0.72rem;
    line-height: 1.2;
    letter-spacing: 0.04em;
    color: var(--text-dim);
    background: linear-gradient(
      to bottom,
      color-mix(in srgb, var(--accent) 22%, var(--panel)),
      var(--panel)
    );
    border: 1px solid color-mix(in srgb, var(--accent) 45%, var(--border));
    border-bottom: none;
    border-radius: 0.7rem 0.7rem 0 0;
    cursor: pointer;
    transition:
      color var(--motion-enter) var(--ease),
      border-color var(--motion-enter) var(--ease);
  }

  .tab:hover,
  .tab[aria-expanded="true"] {
    color: var(--text);
    border-color: var(--accent);
  }

  .tab .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    color: var(--accent);
  }

  .drawer {
    align-self: stretch;
    max-height: min(22rem, 60vh);
    overflow: auto;
    padding: 0.55rem 0.7rem 0.4rem;
    background: color-mix(in srgb, var(--panel) 94%, black);
    border-top: 1px solid var(--accent);
    box-shadow: 0 -0.6rem 1.4rem rgb(0 0 0 / 0.35);
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.82rem;
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.8rem;
  }

  .head select {
    min-width: 0;
    max-width: 100%;
    flex: 1 1 12rem;
  }

  .flag {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    white-space: nowrap;
  }

  .hint,
  .sum,
  .problem {
    margin: 0;
    color: var(--text-dim);
  }

  .problem {
    color: var(--warn, var(--text));
  }

  .entries {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .entries li {
    display: flex;
    align-items: stretch;
    gap: 2px;
  }

  .pick {
    flex: 1;
    min-width: 0;
    display: grid;
    grid-template-columns: 1.6rem minmax(0, 1fr) auto auto;
    gap: 0.5rem;
    align-items: center;
    text-align: left;
    padding: 0.25rem 0.4rem;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 0.3rem;
    color: inherit;
    cursor: pointer;
  }

  .pick:hover {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  li[data-at="true"] .pick {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }

  li[data-next="true"] .tag {
    color: var(--accent);
  }

  .what {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .artist,
  .n,
  .len,
  .tag {
    color: var(--text-dim);
  }

  .tag {
    font-size: 0.75em;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .play {
    width: 2rem;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    color: var(--accent);
    cursor: pointer;
  }
</style>
