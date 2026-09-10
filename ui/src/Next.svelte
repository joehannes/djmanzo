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
  import { tick } from "svelte";
  import IconButton from "./controls/IconButton.svelte";
  import Overview from "./Overview.svelte";
  import {
    ghostPreview,
    loadTrack,
    profileTonight,
    sidelistAdd,
    similarTo,
    suggestNext,
    type DeckState,
    type Ghost,
    type Profile,
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

  /**
   * §12: what the ranking is conditioned on tonight, when it is conditioned.
   *
   * Asked for beside the candidates rather than on a timer, because it can
   * only change when the DJ names the night — and it is drawn as **one line
   * for the whole rail** rather than a note per row. Eight rows in a docked
   * column already carry a name, a confidence bar, the deltas and the
   * transition; a fifth line per row would push the rail past what can be
   * read at a glance, which is the one thing it is for.
   */
  let profile = $state<Profile | null>(null);

  async function refresh() {
    working = true;
    try {
      profile = await profileTonight().catch(() => null);
      candidates = like
        ? await similarTo(like.track.id, ROWS * 2, from)
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

  /**
   * **And ask again when the record it follows changes.**
   *
   * Not a poll — see above, and the reason still holds: a rail that
   * reshuffled every time a deck moved would be unreadable. This is the one
   * input the answer actually depends on. Without it the rail asked once, at
   * start-up, **before the decks had loaded**, and never again: djmanzo
   * answered honestly about a deck holding nothing, so every row came up with
   * no deltas and no transition and stayed that way for the rest of the night.
   * Found by driving the application and confirmed in Rust's own log —
   * `current_track` returning `None` at start-up and `Some` a refresh later.
   *
   * The deck **picker** was already covered — its `onchange` refreshes — and
   * a first draft of the test for this measured that instead, so it passed
   * with the effect below disabled. What only this covers is the record on
   * the followed deck changing underneath it, which is the start-up case and
   * also every load from the browser, a controller or the assistant.
   *
   * A `$derived` rather than reading `decks` inside the effect. The prop is a
   * fresh array sixty times a second, so an effect touching it directly runs
   * sixty times a second — §29's trap, which remounted every knob in the
   * application. A derived string only wakes the effect when the string
   * changes.
   */
  const following = $derived(decks.find((d) => d.number === from)?.title ?? null);
  let followed = $state<string | null>(null);
  $effect(() => {
    const now = following;
    if (!enabled || !asked || now === followed) return;
    followed = now;
    void refresh();
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

  /**
   * §27's ghost, for whichever candidate is being considered.
   *
   * One at a time, and held beside the rail rather than inside a row, because
   * §27 is a question about a *pair*: this record against the one playing. Two
   * ghosts open at once would be two answers to "what happens next" with
   * nothing saying which deck each belonged to.
   */
  let ghost = $state<Ghost | null>(null);
  /** Which row asked, so a second press closes it rather than re-fetching. */
  let ghosting = $state<string | null>(null);
  /** Said out loud when there is nothing honest to draw. */
  let ghostEmpty = $state(false);

  async function toggleGhost(candidate: Suggestion) {
    if (ghosting === candidate.track.id) {
      ghosting = null;
      ghost = null;
      ghostEmpty = false;
      return;
    }
    ghosting = candidate.track.id;
    ghost = null;
    ghostEmpty = false;
    try {
      const seen = await ghostPreview(from, candidate.track.id);
      // The row may have been closed or another one opened while this was in
      // flight. Dropping a stale answer is the difference between a ghost and
      // a ghost of the record before it.
      if (ghosting !== candidate.track.id) return;
      ghost = seen;
      ghostEmpty = seen === null;
      error = null;
      /*
        And bring it into view. The rail is a docked, scrolling list a hundred
        or so pixels tall, so a panel added under the eighth row opens entirely
        below the fold — which is what the running application showed: the
        overlay was drawn correctly and every word explaining it was
        unreachable without scrolling for it.

        `block: "end"` rather than `nearest`: the panel is the tallest thing
        in the list and the line that matters most is its last one — what
        djmanzo cannot see. `nearest` brought the top of it into view and left
        that line clipped, which is the same failure one scroll position along.
      */
      await tick();
      document
        .querySelector(`[data-ghost="${candidate.track.id}"]`)
        ?.scrollIntoView({ block: "end" });
    } catch (e) {
      if (ghosting !== candidate.track.id) return;
      error = String(e);
      ghosting = null;
    }
  }

  /** The deck the ghost is drawn over, when it is still loaded. */
  const ghostDeck = $derived(decks.find((d) => d.number === from) ?? null);

  /** §27's key relationship and BPM movement, on one line. */
  const ghostMovement = $derived.by(() => {
    if (!ghost) return "";
    const parts = [
      `${ghost.bpm_delta >= 0 ? "+" : ""}${ghost.bpm_delta.toFixed(1)} BPM`,
      `${ghost.pitch_percent >= 0 ? "+" : ""}${ghost.pitch_percent.toFixed(1)}% pitch`,
    ];
    if (ghost.key_relation) parts.push(ghost.key_relation);
    return parts.join(" · ");
  });

  /** What §27 asks for that djmanzo cannot see. Named, never quietly dropped. */
  const ghostUnseen = $derived(
    (ghost?.asked ?? []).filter((a) => !a.answered).map((a) => a.about),
  );

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

  <!--
    §12. A ranking that is conditioned says so, with the evidence, because a
    DJ who cannot see why the order changed cannot disagree with it. The
    sentence is Rust's — §81's `Profile::words` — so the rail cannot make a
    claim about this DJ that djmanzo would not.
  -->
  {#if profile}
    <p class="profile" title="Records you play at this kind of night are nudged up the list. It can reorder records that all work; it can never lift one that does not mix.">
      Ranked for tonight: {profile.says}
    </p>
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
          <!--
            §22's estimated transition type. A second line rather than another
            chip on the first: the deltas line is about the two *records* and
            this is about the *mix*, and a DJ scanning eight rows reads them as
            two different questions.

            Rust's wording, not this component's — §27's ghost panel draws the
            same phrase, and one mix with two spellings is two answers.
          -->
          {#if candidate.transition}
            <div class="mix" title="What djmanzo would do if you brought this in">
              {candidate.transition.says}
            </div>
          {/if}
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
              class:on={ghosting === candidate.track.id}
              onclick={() => toggleGhost(candidate)}
              disabled={!enabled}
              title="What happens if this comes in — without loading it"
              aria-label="Preview {candidate.track.title} as a ghost"
            >&deg;</button>
            <button
              onclick={() => reject(candidate)}
              title="Not this one, this time"
              aria-label="Pass on {candidate.track.title}"
            >&times;</button>
          </div>
          <!--
            §27's ghost, under the row that asked for it.
            **Non-destructive**: nothing is loaded, nothing is armed, nothing
            is written down — the panel is the whole of the action, which is
            why it is safe to open on eight records in a row mid-set.
          -->
          {#if ghosting === candidate.track.id}
            <div class="ghost" data-ghost={candidate.track.id}>
              {#if ghost && ghostDeck}
                <Overview
                  deck={ghostDeck}
                  height={34}
                  ghost={{
                    from: ghost.start_frame,
                    to: ghost.end_frame,
                    landing: ghost.landing?.frame ?? null,
                    title: `If this came in here: a ${ghost.says} — ${ghost.reasons.join(" · ")}`,
                  }}
                />
                <div class="ghost-line" title={ghost.reasons.join(" · ")}>
                  <span class="ghost-what">{ghost.says}</span>
                  <span class="ghost-move">{ghostMovement}</span>
                </div>
                <!--
                  Terse on purpose. The rail is a few hundred pixels wide and a
                  sentence wraps to three lines in it — which is how the first
                  draft of this panel put its own numbers below the fold, in a
                  surface that had to be scrolled to reach them at all.
                -->
                <div class="ghost-line">
                  {#if !ghost.landing}
                    no phrase structure — where its first strong phrase lands is
                    not something djmanzo can say
                  {:else if ghost.landing.lead_beats < 0.5}
                    opens on a phrase — no lead-in
                  {:else}
                    {Math.round(ghost.landing.lead_beats)} beat{Math.round(
                      ghost.landing.lead_beats,
                    ) === 1
                      ? ""
                      : "s"} of lead-in{ghost.landing.within_mix
                      ? ""
                      : " — after the mix ends"}
                  {/if}
                </div>
                {#if ghostUnseen.length > 0}
                  <!--
                    Named rather than left out. An overlay quietly showing five
                    of §27's seven marks reads as a record with no vocal and no
                    drop, which is a confident lie.
                  -->
                  <div class="ghost-line unseen">
                    djmanzo cannot see: {ghostUnseen.join(" · ")}
                  </div>
                {/if}
              {:else if ghostEmpty}
                <div class="ghost-line unseen">
                  Nothing to show: the deck is empty, one of the two records is
                  unanalysed, or there is no longer room for a transition.
                </div>
              {:else}
                <div class="ghost-line">Working out what would happen…</div>
              {/if}
            </div>
          {/if}
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

  /* §12: what the whole rail is conditioned on, once rather than per row. */
  .profile {
    margin: 0;
    font-size: 0.7rem;
    color: var(--muted);
    border-left: 2px solid var(--accent-2);
    padding-left: 0.4rem;
  }

  /* §22's estimated transition: about the mix, not about the two records. */
  .mix {
    font-size: 0.7rem;
    color: var(--ok, #6a9955);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .why {
    font-size: 0.7rem;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* §27's ghost panel: a whole record, and what would happen on it. */
  .ghost {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-top: 0.35rem;
    padding: 0.35rem;
    border-radius: 4px;
    background: var(--panel-raised);
    border-left: 2px dashed color-mix(in srgb, var(--ok, #6a9955) 60%, transparent);
  }

  .ghost-line {
    display: flex;
    gap: 0.5rem;
    justify-content: space-between;
    font-size: 0.7rem;
    color: var(--muted);
  }

  .ghost-what {
    color: var(--ok, #6a9955);
    white-space: nowrap;
  }

  .ghost-move {
    white-space: nowrap;
  }

  /* What djmanzo cannot see, in the colour it uses for saying so. */
  .ghost-line.unseen {
    color: var(--warn);
    white-space: normal;
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
