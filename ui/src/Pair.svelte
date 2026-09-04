<script lang="ts">
  /**
   * Two records side by side, and the seam between them.
   *
   * §20's fourth view. The browser answers *what is in the collection*, the
   * rail answers *what could come next*, Set Flow answers *what shape the
   * night has* — and none of them answers the question a DJ asks in the ninety
   * seconds before a mix: **what happens when these two meet.**
   *
   * # It draws an object, it does not compute one
   *
   * Everything here comes from `dj_app::transition::Transition`, §68's
   * transition object, over one command. Nothing on this side works out a mix
   * point, a key relation or a tempo delta — which is the only way a panel and
   * djmanzo cannot disagree about where the mix is. When the DJ moves it, the
   * move goes to Rust and the answer that comes back is what is drawn,
   * including reasons re-derived over the new geometry: a transition moved off
   * its phrase boundary stops claiming to land on one.
   *
   * # Armed, not applied
   *
   * Arming holds the transition; it moves nothing. That is the same line the
   * planner has always drawn — asking for an opinion is not asking for a mix —
   * and it is why this panel is safe to open mid-set. What holding buys is
   * that the adjustment survives closing the panel, and that other surfaces
   * can read the same object rather than each planning their own.
   *
   * # What §20 asks for here and is not drawn
   *
   * **Stems and vocal conflict.** djmanzo separates stems live rather than
   * storing per-track availability, and nothing yet knows *where* the vocals
   * are in either record. A "vocal clash" drawn from two tracks both having a
   * vocal somewhere would be a guess with a confident face on it.
   *
   * **Candidate techniques**, beyond the five styles offered below. The
   * planner picks one and the DJ can take another; ranking all five per pair
   * needs a scorer that does not exist yet.
   */
  import { untrack } from "svelte";
  import IconButton from "./controls/IconButton.svelte";
  import Waveform from "./Waveform.svelte";
  import {
    formatTime,
    planTransition,
    transitionAdjust,
    transitionArm,
    transitionClear,
    transitionCurrent,
    transitionDrag,
    transitionReplan,
    TRANSITION_STYLES,
    TRANSITION_HELP,
    type AutomixState,
    type DeckState,
    type PairSide,
    type Transition,
  } from "./api";

  let {
    enabled,
    deckCount = 2,
    decks = [],
    automix,
  }: {
    enabled: boolean;
    deckCount?: number;
    /** Live deck state, so the pair can follow whatever is playing. */
    decks?: DeckState[];
    /**
     * The automix, because whether anything *performs* a set-up transition
     * depends on it. Setting one up holds it; it does not hand the mix over.
     */
    automix?: AutomixState;
  } = $props();

  const deckNumbers = $derived(Array.from({ length: deckCount }, (_, i) => i + 1));

  let pair = $state<Transition | null>(null);
  let working = $state(false);
  let error = $state<string | null>(null);

  /**
   * Which two decks, once the DJ has said.
   *
   * `null` until then, and the default follows what is playing — the deck
   * being mixed *out of* is the one that decides, and asking a DJ to pick it
   * every time is asking them to tell djmanzo something it can see. The same
   * rule the Next rail follows, and the same reason it stops guessing the
   * moment it is overridden.
   */
  let chosenFrom = $state<number | null>(null);
  let chosenTo = $state<number | null>(null);
  const playing = $derived(decks.find((d) => d.playing)?.number ?? null);
  const from = $derived(chosenFrom ?? playing ?? 1);
  const to = $derived(
    chosenTo ?? deckNumbers.find((n) => n !== from) ?? (from === 1 ? 2 : 1),
  );

  /** The deck rows, for the waveform lanes. */
  const deckOf = (number: number) => decks.find((d) => d.number === number) ?? null;

  /**
   * How far out to zoom the outgoing lane, in frames per pixel.
   *
   * A lane is centred on the playhead, so a mix planned two minutes ahead is
   * two minutes off the right-hand edge — the marks are drawn correctly and
   * nobody can see them. This zooms out far enough to put the end of the
   * transition on screen, and pulls back in as the mix approaches.
   *
   * **Powers of two, not a continuous ratio**, and for the same reason the
   * density bands are bands: the playhead moves sixty times a second, and a
   * zoom derived straight from it would re-render every tile every frame and
   * never settle. Quantised, it changes a handful of times per record.
   */
  function laneZoom(side: PairSide, index: number, mix: Transition): number {
    if (index !== 0) return 1024;
    const ahead = mix.end_frame - (deckOf(side.deck)?.position_frames ?? 0);
    // Half a lane is what is visible ahead of the playhead, and 190 px is
    // half of the narrowest column this surface docks into. Measured rather
    // than guessed: the first attempt divided by 160, which with the rounding
    // below zoomed out far enough that a two-and-a-half-minute record was a
    // hundred-pixel sliver in the middle of the lane. It showed the marks and
    // showed nothing else.
    const needed = Math.max(256, ahead / 190);
    return 2 ** Math.ceil(Math.log2(needed));
  }

  /**
   * What is on the decks, so a reload can be noticed.
   *
   * Rust drops a held transition whose records have been replaced, but only
   * when asked. Without this the panel would keep drawing the old pair until
   * something else happened to press a button.
   */
  const loaded = $derived(decks.map((d) => `${d.number}:${d.title ?? ""}`).join("|"));

  async function ask(fn: () => Promise<Transition | null>) {
    working = true;
    try {
      pair = await fn();
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      working = false;
    }
  }

  const look = () => ask(() => planTransition(from, to));
  const arm = () => ask(() => transitionArm(from, to));
  const replan = () => ask(() => transitionReplan());
  const move = (beats: number) => ask(() => transitionAdjust({ moveBeats: beats }));
  const lengthen = (beats: number) => ask(() => transitionAdjust({ lengthBeats: beats }));
  const restyle = (style: string) => ask(() => transitionAdjust({ style }));

  /**
   * A mark was dragged along the outgoing record.
   *
   * The labels are the contract between this and the lane, which knows about
   * frames and nothing about transitions. Only a held transition can be
   * dragged — the same rule the buttons follow, for the same reason: an
   * opinion is not something djmanzo is keeping.
   */
  function dragged(label: string, frame: number) {
    if (!pair?.armed) return;
    void ask(() => transitionDrag(label === "out" ? "end" : "start", frame));
  }

  async function forget() {
    await transitionClear().catch(() => {});
    pair = null;
  }

  /**
   * Show what djmanzo is already holding, before proposing anything.
   *
   * A flag rather than a test of `pair`, for the reason the rail records: an
   * empty answer is a legitimate answer, so asking whenever the answer is
   * empty is a loop that re-asks forever.
   */
  let asked = $state(false);
  $effect(() => {
    if (!enabled || asked) return;
    asked = true;
    void ask(() => transitionCurrent());
  });

  /**
   * A deck was reloaded: ask again rather than draw a pair that has gone.
   *
   * Everything but `loaded` is read inside `untrack`, and that is not
   * tidiness. Reading `pair` in the tracked scope makes this effect depend on
   * the thing it writes: the answer arrives, `pair` changes, the effect
   * re-runs, asks again — a loop that pins a core and freezes the interface.
   * The rail records the same bug in a different shape, found the same way: a
   * Playwright click that never returned. A type-check cannot see either.
   */
  $effect(() => {
    void loaded;
    untrack(() => {
      if (!asked || !pair?.armed) return;
      void ask(() => transitionCurrent());
    });
  });

  /** `+3 BPM`, `-2 BPM`, or `same tempo`. */
  function tempoDelta(delta: number): string {
    if (Math.abs(delta) < 0.05) return "same tempo";
    return `${delta > 0 ? "+" : "−"}${Math.abs(delta).toFixed(1)} BPM`;
  }

  /** `8A → 9A · neighbour`, or what is missing. */
  function keyLine(t: Transition): string {
    const out = t.outgoing.track.key;
    const into = t.incoming.track.key;
    if (!out || !into) return "key unknown";
    return `${out} → ${into} · ${t.key_relation ?? "unknown"}`;
  }

  function phraseOf(side: PairSide): string {
    return side.phrase_beats ? `${side.phrase_beats}-beat phrases` : "no phrase structure";
  }

  /** The lengths offered. Whole phrases, as the planner's own are. */
  const LENGTHS = [8, 16, 32] as const;
</script>

<div class="pair">
  {#snippet record(side: PairSide, index: number, mix: Transition)}
    <section class="side" aria-label={index === 0 ? "Going out" : "Coming in"}>
      <header>
        <span class="role">{index === 0 ? "out of" : "into"} deck {side.deck}</span>
        <h3 title={side.track.path}>{side.track.title}</h3>
        <p class="artist">{side.track.artist}</p>
      </header>
      <dl>
        <div><dt>BPM</dt><dd>{side.track.bpm ? side.track.bpm.toFixed(1) : "—"}</dd></div>
        <div>
          <dt>Key</dt>
          <dd>{side.track.key ?? "—"}{side.key_standard ? ` (${side.key_standard})` : ""}</dd>
        </div>
        <div><dt>Length</dt><dd>{formatTime(side.track.duration_seconds)}</dd></div>
        <div>
          <dt>Energy</dt>
          <dd>{side.track.loudness_lufs != null
            ? `${side.track.loudness_lufs.toFixed(1)} LUFS`
            : "—"}</dd>
        </div>
        <div><dt>Phrase</dt><dd>{phraseOf(side)}</dd></div>
      </dl>
      {#if side.functions.length > 0}
        <ul class="functions">
          {#each side.functions as slug (slug)}
            <li>{slug.replace("-", " ")}</li>
          {/each}
        </ul>
      {/if}
      {#if deckOf(side.deck)}
        <!--
          The record as it actually is, not as a row describes it. The mix
          point is drawn on the outgoing lane, which is why §20 asks for
          waveforms in this view rather than for two more tables.
        -->
        <div class="lane">
          <Waveform
            deck={deckOf(side.deck)!}
            height={48}
            framesPerPixel={laneZoom(side, index, mix)}
            marks={index === 0
              ? [
                  { frame: mix.start_frame, label: "mix in" },
                  { frame: mix.end_frame, label: "out" },
                ]
              : []}
            onMarkMoved={index === 0 && mix.armed ? dragged : undefined}
          />
        </div>
      {/if}
    </section>
  {/snippet}

  <div class="controls">
    <label>
      Out of
      <select
        value={from}
        disabled={!enabled}
        onchange={(e) => {
          chosenFrom = Number((e.currentTarget as HTMLSelectElement).value);
          void look();
        }}
      >
        {#each deckNumbers as deck (deck)}
          <option value={deck}>deck {deck}</option>
        {/each}
      </select>
    </label>
    <label>
      into
      <select
        value={to}
        disabled={!enabled}
        onchange={(e) => {
          chosenTo = Number((e.currentTarget as HTMLSelectElement).value);
          void look();
        }}
      >
        {#each deckNumbers as deck (deck)}
          <option value={deck}>deck {deck}</option>
        {/each}
      </select>
    </label>
    <button class="act" onclick={look} disabled={!enabled || working}>Compare</button>
    <button
      class="act primary"
      onclick={arm}
      disabled={!enabled || working}
      title="Hold this mix, so it survives closing this panel and can be adjusted"
    >Set up</button>
    {#if pair?.armed}
      <IconButton
        icon="fa-solid fa-xmark"
        title="Stop holding it"
        aria-label="Forget the transition"
        onClick={forget}
      />
    {/if}
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if !pair}
    <p class="empty">
      {working
        ? "Working it out…"
        : "Nothing to compare yet. Load both decks, analyse them, then press Compare."}
    </p>
  {:else}
    <!--
      The seam sits *between* the two records, which is where it is.

      It was underneath them for one build, and driving the application
      showed what that costs: the two cards plus their waveforms filled
      the docked panel, and the line a DJ opens this surface to read —
      where the mix goes and how confident djmanzo is about it — was below
      the fold with the adjustments under that. This project has shipped
      that exact bug twice in other panels. Three columns where there is
      room, stacked in the order out → seam → in where there is not.
    -->
    <div class="sides">
      {@render record(pair.outgoing, 0, pair)}
      <section class="seam" aria-label="The seam between them">
        <div class="headline">
          <span class="deltas">{tempoDelta(pair.bpm_delta)} &middot; {keyLine(pair)}</span>
          <span
            class="confidence"
            title="{Math.round(pair.confidence * 100)}% — how well these two go together, on the rail's scale"
            aria-label="Confidence {Math.round(pair.confidence * 100)} percent"
          >
            <span class="fill" style:scale="{pair.confidence.toFixed(3)} 1"></span>
          </span>
          {#if pair.edited}
            <button class="edited" onclick={replan} title="Throw the adjustments away and ask the planner again">
              adjusted &middot; replan
            </button>
          {/if}
        </div>

        <p class="when">
          Mix in at <strong>{formatTime(pair.start_seconds)}</strong>
          (beat {pair.start_beat}), out by <strong>{formatTime(pair.end_seconds)}</strong>,
          over {pair.length_beats} beats.
        </p>

        <ul class="why">
          {#each pair.reasons as reason (reason)}
            <li>{reason}</li>
          {/each}
        </ul>

        <div class="adjust" class:disabled={!pair.armed}>
          <div class="group" role="group" aria-label="Move the mix point">
            <span class="label">Move</span>
            <button onclick={() => move(-16)} disabled={!enabled || !pair.armed}>−16</button>
            <button onclick={() => move(-4)} disabled={!enabled || !pair.armed}>−4</button>
            <button onclick={() => move(4)} disabled={!enabled || !pair.armed}>+4</button>
            <button onclick={() => move(16)} disabled={!enabled || !pair.armed}>+16</button>
          </div>
          <div class="group" role="group" aria-label="How long the mix runs">
            <span class="label">Beats</span>
            {#each LENGTHS as beats (beats)}
              <button
                class:on={pair.length_beats === beats}
                onclick={() => lengthen(beats)}
                disabled={!enabled || !pair.armed}
              >{beats}</button>
            {/each}
          </div>
          <div class="group" role="group" aria-label="How it is done">
            <span class="label">Style</span>
            {#each TRANSITION_STYLES as style (style)}
              <button
                class:on={pair.style === style}
                onclick={() => restyle(style)}
                disabled={!enabled || !pair.armed}
                title={TRANSITION_HELP[style]}
              >{style}</button>
            {/each}
          </div>
        </div>
        <!--
          What happens next, said plainly.

          Setting a transition up holds it; it does not hand the mix over. If
          the automix is off, nothing will perform it and this has to say so —
          an interface that let "set up" imply "will happen" would be lying at
          the one moment a DJ is deciding whether to keep their hands free.
        -->
        {#if !pair.armed}
          <p class="hint">Set it up to adjust it. Until then this is an opinion, and nothing is held.</p>
        {:else if automix?.enabled}
          <p class="hint on">
            Automix will run this at {formatTime(pair.start_seconds)} — {pair.style} over
            {pair.length_beats} beats.
          </p>
        {:else}
          <p class="hint">
            Held, and nothing will run it: automix is off. Yours to perform, or
            hand the mix over in the booth.
          </p>
        {/if}
      </section>
      {@render record(pair.incoming, 1, pair)}
    </div>
  {/if}
</div>

<style>
  .pair {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-height: 0;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .controls label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.75rem;
    color: var(--muted);
  }

  .act {
    background: var(--control);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--text);
    padding: 0.2rem 0.5rem;
    font-size: 0.72rem;
    cursor: pointer;
  }

  .act.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }

  .act:disabled {
    opacity: 0.45;
    cursor: default;
  }

  /* Three columns where there is room and one where there is not: the record
     going out, the seam, the record coming in. This surface docks along the
     bottom at 820 px and beside the decks at 420, and a pair view that only
     worked wide would be a pair view nobody could keep open. */
  .sides {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 0.5rem;
    align-items: start;
    min-height: 0;
  }

  .side {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.4rem 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    min-width: 0;
  }

  .side header {
    min-width: 0;
  }

  .role {
    font-size: 0.65rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }

  .side h3 {
    margin: 0;
    font-size: 0.85rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .artist {
    margin: 0;
    font-size: 0.72rem;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  dl {
    margin: 0;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(96px, 1fr));
    gap: 0.15rem 0.5rem;
    font-size: 0.7rem;
  }

  dl div {
    display: flex;
    gap: 0.3rem;
    min-width: 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .functions {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.2rem;
  }

  .functions li {
    font-size: 0.65rem;
    color: var(--muted);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 0 0.25rem;
  }

  .lane {
    border-radius: 4px;
    overflow: hidden;
  }

  .seam {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.4rem 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .headline {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .deltas {
    flex: 1;
    min-width: 0;
    font-size: 0.78rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .confidence {
    flex: none;
    width: 46px;
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

  .edited {
    background: none;
    border: 1px solid var(--warn);
    border-radius: 3px;
    color: var(--warn);
    font-size: 0.65rem;
    padding: 0 0.3rem;
    cursor: pointer;
  }

  .when {
    margin: 0;
    font-size: 0.75rem;
  }

  .why {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }

  .why li {
    font-size: 0.68rem;
    color: var(--muted);
    background: var(--control);
    border-radius: 3px;
    padding: 0 0.3rem;
  }

  .adjust {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .group {
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }

  .group .label {
    font-size: 0.65rem;
    color: var(--muted);
  }

  .group button {
    background: var(--control);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    min-width: 1.6rem;
    padding: 0.1rem 0.3rem;
    font-size: 0.68rem;
    cursor: pointer;
  }

  .group button.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }

  .group button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  /* The one line that says what djmanzo is about to do on its own. */
  .hint.on {
    color: var(--accent);
  }

  .empty,
  .hint,
  .error {
    font-size: 0.72rem;
    color: var(--muted);
    margin: 0;
  }

  .error {
    color: var(--warn);
  }
</style>
