<script lang="ts">
  /**
   * The next-track rail: what could come next, and why, in one line each.
   *
   * # Why this is a surface and not a tab
   *
   * It was a tab inside Prepare, which meant it could only be looked at
   * instead of the sidelist rather than beside it — and a rail whose whole
   * value is being glanced at mid-transition is not a thing to go and find.
   * The directive's §22 asks for a *rail*: three to eight candidates, compact
   * enough to sit beside the decks and stay there.
   *
   * The same move Prepare itself made one commit earlier, for the same reason,
   * and with the same rule about not leaving a copy behind: the tab is gone
   * rather than duplicated. Two places that suggest the next record are two
   * places that will disagree.
   *
   * # One line, not a pile of chips
   *
   * `+3 BPM · 8A→9A · +1 dB` comes from Rust, where the reasons are typed and
   * the ranking can be argued with. Deltas rather than values: 131 BPM means
   * nothing without remembering what is playing. The full reasons are still
   * there as the row's tooltip, for when the line is not enough.
   *
   * # Reject and pin are about this minute, not forever
   *
   * Neither is written down. "Not that one" while a record is playing is not
   * the same statement as "never suggest this again", and a rail that
   * quietly learned the first as the second would slowly hide a collection
   * from its owner. They last as long as the session does.
   */
  import IconButton from "./controls/IconButton.svelte";
  import {
    loadTrack,
    sidelistAdd,
    similarTo,
    suggestNext,
    type DeckState,
    type Suggestion,
    type Trajectory,
  } from "./api";

  let {
    enabled,
    deckCount = 2,
    decks = [],
  }: {
    enabled: boolean;
    deckCount?: number;
    /** Live deck state, so the rail can follow whatever is actually playing. */
    decks?: DeckState[];
  } = $props();

  const deckNumbers = $derived(Array.from({ length: deckCount }, (_, i) => i + 1));

  let candidates = $state<Suggestion[]>([]);
  let trajectory = $state<Trajectory>("hold");

  /**
   * Which deck the rail follows, once the DJ has said.
   *
   * `null` until then, and that is the useful state: with four decks up, the
   * one being mixed *out of* is the one that decides what comes next, and
   * asking the DJ to pick it every time is asking them to tell djmanzo
   * something it can see. So the default follows whatever is playing, and
   * stops following the moment it is overridden -- an automatic choice that
   * kept overriding a deliberate one would be worse than never guessing.
   */
  let chosen = $state<number | null>(null);
  const playing = $derived(decks.find((d) => d.playing)?.number ?? null);
  const from = $derived(chosen ?? playing ?? 1);
  let working = $state(false);
  let error = $state<string | null>(null);
  let busy = $state<string | null>(null);

  /** Seed for "more like this", or null when the rail is answering the deck. */
  let like = $state<Suggestion | null>(null);

  /** Set aside for this session: not shown, not written down. */
  let rejected = $state<string[]>([]);
  /** Kept at the top, however the ranking moves under them. */
  let pinned = $state<string[]>([]);

  /**
   * How many rows the rail shows.
   *
   * §22 says three to eight. Eight, because the ranking's honest failure mode
   * is having the right record at position six, and a rail that stopped at
   * three would hide exactly the case a DJ opens it for.
   */
  const ROWS = 8;

  const shown = $derived.by(() => {
    const live = candidates.filter((c) => !rejected.includes(c.track.id));
    const up = live.filter((c) => pinned.includes(c.track.id));
    const rest = live.filter((c) => !pinned.includes(c.track.id));
    return [...up, ...rest].slice(0, ROWS);
  });

  async function refresh() {
    working = true;
    try {
      candidates = like
        ? await similarTo(like.track.id, ROWS * 2)
        : await suggestNext(from, trajectory, ROWS * 2);
      error = null;
    } catch (e) {
      error = String(e);
      candidates = [];
    } finally {
      working = false;
    }
  }

  /**
   * Asked for once, then only when something is pressed.
   *
   * Not live. The ranking depends on what is *playing*, and a rail that
   * reshuffled under the cursor every time a deck moved would be unreadable —
   * the DJ is reading it precisely while the thing it depends on is changing.
   *
   * **A flag, not a test of emptiness.** The first version of this asked
   * whenever `candidates` was empty, which reads as "fetch if we have nothing"
   * and is an infinite loop: an empty answer is a legitimate answer, so it set
   * the condition that triggered it. Pressing "more like this" on a record
   * with no neighbours froze the interface — the effect re-ran, fetched
   * nothing, and re-ran. Found by a Playwright click that never returned; a
   * type-check cannot see it, and nor can a reader who does not already know
   * to look.
   */
  let asked = $state(false);
  $effect(() => {
    if (enabled && !asked) {
      asked = true;
      void refresh();
    }
  });

  async function onto(candidate: Suggestion, deck: number) {
    busy = candidate.track.id;
    try {
      await loadTrack(deck, candidate.track.path);
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function stage(candidate: Suggestion) {
    busy = candidate.track.id;
    try {
      await sidelistAdd(candidate.track.id);
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  function moreLikeThis(candidate: Suggestion) {
    like = candidate;
    void refresh();
  }

  function backToTheDeck() {
    like = null;
    void refresh();
  }

  function reject(candidate: Suggestion) {
    rejected = [...rejected, candidate.track.id];
  }

  function togglePin(candidate: Suggestion) {
    pinned = pinned.includes(candidate.track.id)
      ? pinned.filter((id) => id !== candidate.track.id)
      : [...pinned, candidate.track.id];
  }
</script>

<div class="rail">
  <div class="rail-controls">
    {#if like}
      <!--
        The rail is answering a different question now, and says so. Without
        this the same eight rows would appear to be following the deck and be
        following a record the DJ picked four presses ago.
      -->
      <button
        class="seed"
        onclick={backToTheDeck}
        title="Back to what follows the deck"
        aria-label="Back to what follows the deck"
      >
        like <strong>{like.track.title}</strong> &times;
      </button>
    {:else}
      <label>
        After
        <select value={from} onchange={(e) => {
          chosen = Number((e.currentTarget as HTMLSelectElement).value);
          void refresh();
        }} disabled={!enabled}>
          {#each deckNumbers as deck (deck)}
            <option value={deck}>deck {deck}</option>
          {/each}
        </select>
      </label>
      <!--
        Lift, hold, ease: the one thing the ranking cannot infer, because the
        same two records are the right and the wrong answer depending on where
        the night is going.
      -->
      <div class="trajectory" role="radiogroup" aria-label="Where to take the room">
        {#each [["lift", "Lift"], ["hold", "Hold"], ["ease", "Ease"]] as [id, label] (id)}
          <button
            role="radio"
            aria-checked={trajectory === id}
            class:active={trajectory === id}
            onclick={() => {
              trajectory = id as Trajectory;
              void refresh();
            }}>{label}</button
          >
        {/each}
      </div>
    {/if}
    <IconButton
      icon="fa-solid fa-rotate-left"
      title="Work them out again"
      aria-label="Refresh the rail"
      disabled={!enabled || working}
      onClick={refresh}
    />
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if shown.length === 0}
    <p class="empty">
      {working
        ? "Working them out…"
        : rejected.length > 0
          ? "Nothing left that you have not passed on."
          : "Nothing to suggest yet. Analyse some records, then ask again."}
    </p>
  {:else}
    <ul class="candidates">
      {#each shown as candidate (candidate.track.id)}
        <li class:pinned={pinned.includes(candidate.track.id)}>
          <div class="line">
            <span class="name" title={candidate.track.path}>{candidate.track.title}</span>
            <!--
              Confidence as a bar rather than a number. A DJ scanning eight
              rows is comparing them with each other, which is what a bar is
              for and what "0.62" is not.
            -->
            <span
              class="confidence"
              title="{Math.round(candidate.confidence * 100)}% of what this ranking can be sure of"
              aria-label="Confidence {Math.round(candidate.confidence * 100)} percent"
            >
              <span class="fill" style:scale="{candidate.confidence.toFixed(3)} 1"></span>
            </span>
          </div>
          <div class="why" title={candidate.reasons.join(" · ")}>{candidate.summary}</div>
          <div class="acts">
            {#each deckNumbers as deck (deck)}
              <button
                class="deck"
                onclick={() => onto(candidate, deck)}
                disabled={!enabled || busy === candidate.track.id}
                title="Load onto deck {deck}"
                aria-label="Load {candidate.track.title} onto deck {deck}"
              >{busy === candidate.track.id ? "…" : deck}</button>
            {/each}
            <button
              onclick={() => stage(candidate)}
              disabled={!enabled || busy === candidate.track.id}
              title="Set aside in Prepare"
              aria-label="Set {candidate.track.title} aside"
            >&rarr;</button>
            <button
              onclick={() => moreLikeThis(candidate)}
              disabled={!enabled}
              title="More like this one"
              aria-label="More like {candidate.track.title}"
            >&asymp;</button>
            <button
              class:on={pinned.includes(candidate.track.id)}
              onclick={() => togglePin(candidate)}
              title="Keep this one at the top"
              aria-label="Pin {candidate.track.title}"
            >●</button>
            <button
              onclick={() => reject(candidate)}
              title="Not this one, this time"
              aria-label="Pass on {candidate.track.title}"
            >&times;</button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-height: 0;
  }

  .rail-controls {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .rail-controls label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.75rem;
    color: var(--muted);
  }

  .trajectory {
    display: flex;
    gap: 0.2rem;
  }

  .trajectory button,
  .seed {
    background: var(--control);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--muted);
    padding: 0.2rem 0.45rem;
    font-size: 0.72rem;
    cursor: pointer;
  }

  .trajectory button.active {
    background: var(--accent);
    color: var(--on-accent);
    border-color: var(--accent);
  }

  .candidates {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    min-height: 0;
    overflow-y: auto;
  }

  .candidates li {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.35rem 0.45rem;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  /* A pinned row keeps its place while the ranking moves under it, so it says
     so — otherwise the top of the rail looks like a ranking that stopped
     working. */
  .candidates li.pinned {
    border-color: var(--accent);
  }

  .line {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.8rem;
  }

  .confidence {
    flex: none;
    width: 34px;
    height: 4px;
    border-radius: 2px;
    background: var(--control);
    overflow: hidden;
  }

  .confidence .fill {
    display: block;
    width: 100%;
    height: 100%;
    background: var(--accent);
    transform-origin: left center;
  }

  .why {
    font-size: 0.7rem;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .acts {
    display: flex;
    gap: 0.2rem;
    flex-wrap: wrap;
  }

  .acts button {
    background: var(--control);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    min-width: 1.4rem;
    padding: 0.1rem 0.3rem;
    font-size: 0.7rem;
    cursor: pointer;
  }

  .acts button.on {
    background: var(--accent);
    color: var(--on-accent);
    border-color: var(--accent);
  }

  .acts button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .empty,
  .error {
    font-size: 0.75rem;
    color: var(--muted);
    margin: 0;
  }

  .error {
    color: var(--warn);
  }
</style>
