<script lang="ts">
  /**
   * The panel beside the song list: what you have set aside, and what is on
   * the decks.
   *
   * # Why the Sidelist is here and not another crate
   *
   * A crate is a decision about your collection. The Sidelist is a decision
   * about the next twenty minutes — you pull four tracks aside because the
   * room is going somewhere, play two, and drop the rest. It lives beside the
   * browser because that is where you are looking when you make it, and it is
   * deliberately not in the crate tree.
   *
   * # Two tabs that are signposts
   *
   * Sampler and Automix both shipped, and neither of them lives here — the
   * sampler is on the pads, and the automix is in the mixer because it moves
   * the channel faders. The tabs stay because a DJ opening SideView looks for
   * them here, and being told where they are beats wondering whether a setting
   * was missed. They point; they do not duplicate the controls.
   */
  import IconButton from "./controls/IconButton.svelte";
  import { onMount } from "svelte";
  import {
    formatTime,
    loadTrack,
    sidelist,
    sidelistAdd,
    sidelistClear,
    sidelistRemove,
    suggestNext,
    type DeckState,
    type PlaylistEntry,
    type Suggestion,
    type Trajectory,
  } from "./api";

  let {
    enabled,
    deckCount = 2,
    decks = [],
    pending = null,
    onconsumed,
  }: {
    enabled: boolean;
    deckCount?: number;
    /** Live deck state, for Clone. */
    decks?: DeckState[];
    /** A track id the browser has asked to add. */
    pending?: string | null;
    onconsumed?: () => void;
  } = $props();

  type Tab = "sidelist" | "next" | "clone" | "sampler" | "automix";
  let tab = $state<Tab>("sidelist");
  let entries = $state<PlaylistEntry[]>([]);

  /**
   * Suggestions for what to play next, and which deck they follow from.
   *
   * Fetched on demand rather than kept live. The ranking is cheap, but it
   * depends on what is *playing*, and a list that reshuffled itself under the
   * cursor every time a deck changed would be unusable — the DJ is reading it
   * precisely while the thing it depends on is moving.
   */
  let suggestions = $state<Suggestion[]>([]);
  let suggestFrom = $state(1);
  let trajectory = $state<Trajectory>("hold");
  let suggesting = $state(false);

  async function refreshSuggestions() {
    suggesting = true;
    try {
      suggestions = await suggestNext(suggestFrom, trajectory);
      error = null;
    } catch (e) {
      error = String(e);
      suggestions = [];
    } finally {
      suggesting = false;
    }
  }
  let error = $state<string | null>(null);
  let loading = $state<string | null>(null);

  async function refresh() {
    try {
      entries = await sidelist();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function remove(position: number) {
    try {
      await sidelistRemove(position);
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function clear() {
    try {
      await sidelistClear();
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function toDeck(entry: PlaylistEntry, deck: number) {
    loading = entry.path;
    try {
      await loadTrack(deck, entry.path);
      error = null;
    } catch (e) {
      error = `Could not load ${entry.title}. ${String(e)}`;
    } finally {
      loading = null;
    }
  }

  /**
   * Take whatever the browser handed over.
   *
   * The parent clears `pending` through `onconsumed` rather than this
   * component reaching back into it, so adding the same track twice in a row
   * still works — which it must, because a DJ playing a track twice in a night
   * is normal.
   */
  $effect(() => {
    const id = pending;
    if (id == null) return;
    void (async () => {
      try {
        await sidelistAdd(id);
        await refresh();
        if (tab !== "sidelist") tab = "sidelist";
      } catch (e) {
        error = String(e);
      } finally {
        onconsumed?.();
      }
    })();
  });

  /** Artist, tempo, key and length — whichever of them the track has. */
  function describe(entry: PlaylistEntry): string {
    const parts = [entry.artist];
    if (entry.bpm != null) parts.push(entry.bpm.toFixed(1));
    if (entry.key) parts.push(entry.key);
    parts.push(formatTime(entry.duration_seconds));
    return parts.join(" · ");
  }

  const deckNumbers = $derived(Array.from({ length: deckCount }, (_, i) => i + 1));
  const loaded = $derived(decks.filter((deck) => deck.loaded));

  onMount(() => {
    void refresh();
  });
</script>

<aside class="sideview">
  <div class="tabs" role="tablist">
    {#each [["sidelist", "Sidelist"], ["next", "Next"], ["clone", "Clone"], ["sampler", "Sampler"], ["automix", "Automix"]] as [id, label] (id)}
      <button
        role="tab"
        class:active={tab === id}
        aria-selected={tab === id}
        onclick={() => (tab = id as Tab)}
      >
        {label}{#if id === "sidelist" && entries.length > 0}<span class="count"
            >{entries.length}</span
          >{/if}
      </button>
    {/each}
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if tab === "sidelist"}
    {#if entries.length === 0}
      <p class="empty">
        Nothing set aside. Use <strong>→</strong> on a row in the browser to put a track
        here while you decide.
      </p>
    {:else}
      <ul class="list">
        {#each entries as entry (entry.id + ":" + entry.position)}
          <li>
            <div class="meta">
              <span class="title" title={entry.path}>{entry.title}</span>
              <!--
                Built as one string rather than as interleaved markup: the
                separators depend on which fields a track actually has, and
                whitespace between Svelte blocks is not reliable enough to
                carry them — an unanalysed track came out as "Unknown artist·
                8A".
              -->
              <span class="sub">{describe(entry)}</span>
            </div>
            <div class="row-actions">
              {#each deckNumbers as deck (deck)}
                <button
                  onclick={() => toDeck(entry, deck)}
                  disabled={!enabled || loading === entry.path}
                  title="Load onto deck {deck}"
                >
                  {loading === entry.path ? "…" : deck}
                </button>
              {/each}
                          <IconButton
                            icon="fa-solid fa-xmark"
                            title="Take out of the Sidelist"
                            aria-label={`Remove ${entry.title}`}
                            onClick={() => remove(entry.position)}
                          />
            </div>
          </li>
        {/each}
      </ul>
                  <IconButton icon="fa-solid fa-trash" title="Clear the list" onClick={clear} />
    {/if}
  {:else if tab === "next"}
    <div class="suggest-controls">
      <label>
        After deck
        <select bind:value={suggestFrom} onchange={refreshSuggestions}>
          {#each deckNumbers as deck (deck)}
            <option value={deck}>{deck}</option>
          {/each}
        </select>
      </label>
      <!--
        Lift, hold, ease. Not a mood or a genre: it is the one thing the
        ranking cannot infer, because the same two records are the right and
        wrong answer depending on where the night is going.
      -->
      <div class="trajectory" role="radiogroup" aria-label="Where to take the room">
        {#each [["lift", "Lift"], ["hold", "Hold"], ["ease", "Ease"]] as [id, label] (id)}
          <button
            role="radio"
            aria-checked={trajectory === id}
            class:active={trajectory === id}
            onclick={() => {
              trajectory = id as Trajectory;
              void refreshSuggestions();
            }}>{label}</button
          >
        {/each}
      </div>
      <IconButton
        icon="fa-solid fa-rotate-left"
        title="Work them out again"
        aria-label="Refresh suggestions"
        disabled={!enabled || suggesting}
        onClick={refreshSuggestions}
      />
    </div>

    {#if suggestions.length === 0}
      <p class="empty">
        {suggesting
          ? "Working them out…"
          : "Nothing to suggest yet. Analyse some tracks, then ask again."}
      </p>
    {:else}
      <ul class="list">
        {#each suggestions as suggestion (suggestion.track.id)}
          <li>
            <div class="meta">
              <span class="title" title={suggestion.track.path}
                >{suggestion.track.title}</span
              >
              <span class="sub">{suggestion.track.artist}</span>
              <!--
                The reasons, as chips. Shown rather than summarised into a
                score: a number alone cannot be disagreed with, and a DJ who
                can see "key clash" is a DJ who can decide to do it anyway.
              -->
              <span class="reasons">
                {#each suggestion.reasons as reason (reason)}
                  <span class="chip">{reason}</span>
                {/each}
              </span>
            </div>
            <div class="row-actions">
              {#each deckNumbers as deck (deck)}
                <button
                  onclick={() => toDeck(
                    {
                      id: suggestion.track.id,
                      path: suggestion.track.path,
                      title: suggestion.track.title,
                    } as PlaylistEntry,
                    deck,
                  )}
                  disabled={!enabled || loading === suggestion.track.path}
                  title="Load onto deck {deck}"
                >
                  {loading === suggestion.track.path ? "…" : deck}
                </button>
              {/each}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  {:else if tab === "clone"}
    <!--
      What is on the decks right now. Useful for the thing it is named after:
      seeing at a glance what you have already played from where, without
      looking up at four deck headers.
    -->
    {#if loaded.length === 0}
      <p class="empty">Nothing on the decks.</p>
    {:else}
      <ul class="list">
        {#each loaded as deck (deck.number)}
          <li>
            <div class="meta">
              <span class="title">{deck.title ?? "—"}</span>
              <span class="sub"
                >Deck {deck.number}{#if deck.effective_bpm != null}
                  · {deck.effective_bpm.toFixed(1)} BPM{/if}{#if deck.playing} · playing{/if}</span
              >
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  {:else if tab === "sampler"}
    <p class="empty">
      The sampler lives on the <strong>Sampler</strong> pad page and in its own
      panel, where a slot can be loaded, recorded into and given a trigger mode.
    </p>
  {:else}
    <p class="empty">
      Automix plays from this Sidelist, and its controls are in the
      <strong>mixer</strong> — it moves the channel faders, so it sits with the
      rest of the hands on the mix rather than with the library.
    </p>
  {/if}
</aside>

<style>
  .sideview {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    min-width: 13rem;
    max-width: 18rem;
    min-height: 0;
    font-size: 0.85em;
  }

  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 0.2rem;
  }

  /*
    A content-sized basis, not a zero one. With `flex: 1` every tab got the
    same width whatever its label, and the Sidelist tab -- which is both the
    longest word and the only one carrying a count -- had its number clipped
    off at the edge. The tabs wrap when there is genuinely no room, which is
    honest; a truncated number is not.
  */
  .tabs button {
    flex: 1 0 auto;
    padding: 0.2rem 0.4rem;
    font-size: 0.9em;
    white-space: nowrap;
  }

  .tabs button.active {
    background: var(--accent-2);
    color: var(--on-accent);
    border-color: var(--accent-2);
  }

  .count {
    margin-left: 0.25rem;
    opacity: 0.8;
    font-variant-numeric: tabular-nums;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .list li {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.2rem 0.3rem;
    border-radius: 4px;
    background: var(--panel-raised);
  }

  .meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .title,
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    font-size: 0.85em;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }

  .row-actions {
    display: flex;
    gap: 0.1rem;
    flex: none;
  }

  .row-actions button {
    padding: 0.05rem 0.3rem;
    font-size: 0.9em;
  }
  .empty {
    margin: 0;
    color: var(--text-dim);
    line-height: 1.45;
  }

  .error {
    margin: 0;
    color: var(--danger, #dc2626);
  }
  .suggest-controls {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    padding: 0.4rem 0.2rem 0.6rem;
  }

  .suggest-controls label {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.78rem;
    opacity: 0.8;
  }

  .trajectory {
    display: flex;
    gap: 0.2rem;
  }

  .trajectory button {
    padding: 0.2rem 0.55rem;
    font-size: 0.78rem;
    border-radius: 0.3rem;
    border: 1px solid var(--edge, rgba(128, 128, 128, 0.35));
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .trajectory button.active {
    background: var(--accent-soft, rgba(128, 128, 128, 0.25));
    border-color: var(--accent, rgba(128, 128, 128, 0.6));
  }

  .reasons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin-top: 0.2rem;
  }

  /*
    Chips, not a sentence. A DJ scanning the list reads them as a row of
    facts -- "same key", "128 BPM fits", "+3 dB" -- and stops at whichever one
    they care about tonight.
  */
  .chip {
    font-size: 0.68rem;
    line-height: 1.4;
    padding: 0 0.35rem;
    border-radius: 0.25rem;
    background: var(--chip, rgba(128, 128, 128, 0.18));
    white-space: nowrap;
  }
</style>
