<script lang="ts">
  /**
   * Finding music, across every configured source at once.
   *
   * The rule this panel exists to honour: **a result never offers a button that
   * cannot work.** A track the user owns loads; a Spotify result they do not own
   * says why it cannot, in the same place the button would have been. Greying
   * something out and leaving the DJ to guess is the failure mode here.
   */
  import Library from "./Library.svelte";
  import IconButton from "./controls/IconButton.svelte";
  import {
    loadTrack,
    resolveSourceTrack,
    searchSources,
    type DeckState,
    type SearchResults,
    type SourceTrack,
  } from "./api";

  let {
    enabled,
    deckCount = 2,
    decks = [],
  }: { enabled: boolean; deckCount?: number; decks?: DeckState[] } = $props();

  let text = $state("");
  let results = $state<SearchResults[]>([]);
  let searching = $state(false);
  let error = $state<string | null>(null);
  /** Track id currently being fetched, so its row can say so. */
  let loading = $state<string | null>(null);
  let searched = $state(false);

  async function run() {
    if (!text.trim()) return;
    searching = true;
    error = null;
    try {
      results = await searchSources(text);
      searched = true;
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

  /**
   * Which half of the browser is showing.
   *
   * The collection first, because that is where a DJ starts. A streaming search
   * is what you do when your own crate does not have it — the exception, not
   * the default, and putting it first would put a network round trip between a
   * DJ and music they already own.
   */
  let tab = $state<"library" | "sources">("library");
</script>

<section class="browse">
  <div class="tabs" role="tablist">
    <IconButton role="tab" aria-selected={tab === "library"} active={tab === "library"} icon="fa-solid fa-book" title="My collection" onClick={() => (tab = "library")} />
    <IconButton role="tab" aria-selected={tab === "sources"} active={tab === "sources"} icon="fa-solid fa-cloud" title="Sources" onClick={() => (tab = "sources")} />
  </div>

{#if tab === "library"}
  <Library {enabled} {deckCount} {decks} />
{:else}
  <div class="search">
    <input
      type="search"
      placeholder="Search every source you have set up…"
      bind:value={text}
      onkeydown={(e) => e.key === "Enter" && run()}
    />
    <IconButton icon="fa-solid fa-magnifying-glass" title={searching ? "Searching…" : "Search"} onClick={run} disabled={searching || !text.trim()} />
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if searched && total === 0 && !searching}
    <p class="empty">
      Nothing found. Only sources you have set up are searched — add a music
      folder or a key in Settings to widen the net.
    </p>
  {/if}

  <div class="results">
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
                    <IconButton
                      disabled={!enabled || loading === track.id}
                      onClick={() => toDeck(track, deck)}
                      title={`Load to deck ${deck}`}
                    >
                      {loading === track.id ? "…" : deck}
                    </IconButton>
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
  </div>
{/if}
</section>

<style>
  .tabs {
    display: flex;
    gap: 0.3rem;
  }

  .tabs button {
    font-size: 0.85em;
    padding: 0.2rem 0.7rem;
  }

  .tabs button.active {
    background: var(--accent-2);
    color: var(--on-accent);
    border-color: var(--accent-2);
  }

  .browse {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    flex: 1;
    min-height: 0;
  }

  .search {
    display: flex;
    gap: 0.4rem;
  }

  .search input {
    flex: 1;
    min-width: 0;
  }

  .results {
    overflow: auto;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
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
