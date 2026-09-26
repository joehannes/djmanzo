<script lang="ts">
  /**
   * What the DJ's other sources have for a search, below the collection's own.
   *
   * §124: *"there shall be no extra view for external sources, but the regular
   * search shall have a tickable toggle or something to include external
   * sources"*. This was the second tab of the browser, with a search box of its
   * own; now the collection's search asks it, when the tick says so.
   *
   * The rule it keeps from that tab: **a result never offers a button that
   * cannot work.** A track the DJ owns loads; a Spotify result they do not own
   * says why it cannot, in the same place the button would have been.
   */
  import {
    loadTrack,
    resolveSourceTrack,
    searchSources,
    type SearchResults,
    type SourceTrack,
  } from "./api";

  let {
    query,
    enabled,
    deckCount = 2,
  }: { query: string; enabled: boolean; deckCount?: number } = $props();

  let results = $state<SearchResults[]>([]);
  let searching = $state(false);
  let error = $state<string | null>(null);
  /** Track id currently being fetched, so its row can say so. */
  let loading = $state<string | null>(null);
  /** The query the results on screen answer, so an old answer is not shown for a new question. */
  let answered = $state("");

  /**
   * Ask, a moment after the typing stops.
   *
   * Longer than the collection's own debounce: this goes over the network to
   * every source set up, and a request per keystroke is a rate limit reached
   * by the end of a word.
   */
  const WAIT_MS = 600;
  $effect(() => {
    const asked = query.trim();
    if (!asked) {
      results = [];
      answered = "";
      return;
    }
    const timer = setTimeout(() => void ask(asked), WAIT_MS);
    return () => clearTimeout(timer);
  });

  async function ask(asked: string) {
    searching = true;
    error = null;
    try {
      const found = await searchSources(asked);
      // Only the latest question's answer is drawn.
      if (asked !== query.trim()) return;
      results = found ?? [];
      answered = asked;
    } catch (e) {
      error = String(e);
    } finally {
      searching = false;
    }
  }

  async function toDeck(track: SourceTrack, deck: number) {
    loading = track.id;
    error = null;
    try {
      // Resolve first: a streamed track is fetched to disk before it reaches a
      // deck, so a dropped connection mid-set cannot stall playback.
      const path = await resolveSourceTrack(track);
      await loadTrack(deck, path);
    } catch (e) {
      error = String(e);
    } finally {
      loading = null;
    }
  }

  const duration = (seconds: number | null) => {
    if (seconds === null || !Number.isFinite(seconds)) return "";
    const total = Math.floor(seconds);
    return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
  };

  const total = $derived(results.reduce((sum, r) => sum + r.tracks.length, 0));
</script>

<section class="sources" aria-label="From your other sources">
  <h3>
    From your other sources
    {#if searching}<span class="count">searching…</span>
    {:else if answered}<span class="count">{total}</span>{/if}
  </h3>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if !query.trim()}
    <p class="empty">Type to search the sources you have set up as well.</p>
  {:else if answered && total === 0 && !searching}
    <p class="empty">
      Nothing there either. Only sources you have set up are searched — add a
      music folder or a key in Settings to widen the net.
    </p>
  {/if}

  {#each results as group (group.provider)}
    {#if group.tracks.length > 0 || group.error}
      <div class="group">
        <h4>
          {group.label}
          <span class="count">{group.tracks.length}</span>
          {#if group.matched_locally > 0}
            <!-- The whole point of a metadata-only source: these became
                 loadable because the DJ already owns them. -->
            <span class="matched">{group.matched_locally} in your library</span>
          {/if}
        </h4>

        {#if group.error}
          <p class="group-error">{group.error}</p>
        {/if}

        {#each group.tracks as track (track.provider + track.id)}
          <div class="track">
            <div class="meta">
              <span class="title" title={track.title}>{track.title}</span>
              <span class="artist" title={track.artist}>
                {track.artist}{track.album ? ` · ${track.album}` : ""}
              </span>
            </div>
            <span class="mono time">{duration(track.duration_seconds)}</span>
            {#if track.bpm}
              <span class="mono bpm">{Math.round(track.bpm)}</span>
            {/if}

            {#if track.playable}
              <div class="load">
                {#each Array.from({ length: deckCount }, (_, i) => i + 1) as deck (deck)}
                  <button
                    disabled={!enabled || loading === track.id}
                    onclick={() => toDeck(track, deck)}
                    title="Load to deck {deck}"
                  >
                    {loading === track.id ? "…" : deck}
                  </button>
                {/each}
              </div>
            {:else}
              <!-- No dead button. The reason goes where the button would be. -->
              <span class="unplayable" title="Search results only">
                not mixable
              </span>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {/each}
</section>

<style>
  .sources {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding-top: 0.6rem;
    border-top: 1px solid var(--border);
  }

  h3 {
    margin: 0;
    font-size: 0.8rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-dim);
    display: flex;
    gap: 0.5rem;
    align-items: baseline;
  }

  .group {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.7rem 0.9rem;
  }

  h4 {
    margin: 0 0 0.4rem;
    font-size: 0.85rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .count {
    font-size: 0.75em;
    color: var(--text-dim);
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
  }

  .matched {
    font-size: 0.7em;
    font-weight: 400;
    color: var(--accent-2);
  }

  .group-error {
    margin: 0 0 0.4rem;
    font-size: 0.78em;
    color: var(--warn);
    line-height: 1.5;
  }

  .track {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.3rem 0;
    border-top: 1px solid var(--border);
  }

  .meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .title,
  .artist {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .artist {
    font-size: 0.78em;
    color: var(--text-dim);
  }

  .time,
  .bpm {
    font-size: 0.78em;
    color: var(--text-dim);
    flex: none;
  }

  .load {
    display: flex;
    gap: 0.2rem;
    flex: none;
  }

  .load button {
    padding: 0.2rem 0.5rem;
    font-size: 0.8em;
  }

  .unplayable {
    font-size: 0.7em;
    color: var(--text-dim);
    font-style: italic;
    flex: none;
  }

  .empty {
    margin: 0;
    color: var(--text-dim);
    font-size: 0.85em;
    line-height: 1.5;
  }

  .error {
    margin: 0;
    padding: 0.6rem 0.9rem;
    background: color-mix(in srgb, var(--danger) 12%, var(--panel));
    border: 1px solid var(--danger);
    border-radius: 8px;
    color: var(--danger);
    font-size: 0.85em;
  }
</style>
