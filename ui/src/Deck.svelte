<script lang="ts">
  import {
    dispatch,
    formatTime,
    loadTrack,
    moveHotCue,
    moveLoopEdge,
    padPages,
    type DeckState,
    type Placed,
    type PadPageDto,
    type SamplerState,
    type StemSwap,
  } from "./api";
  import JogWheel from "./JogWheel.svelte";
  import { fill } from "./meter";
  import Fx from "./Fx.svelte";
  import Stems from "./Stems.svelte";
  import Pads from "./Pads.svelte";
  import Overview from "./Overview.svelte";
  import Waveform from "./Waveform.svelte";
  import SvgKnob from "./controls/SvgKnob.svelte";
  import SvgFader from "./controls/SvgFader.svelte";
  import SvgPad from "./controls/SvgPad.svelte";
  import IconButton from "./controls/IconButton.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { acceptFiles, isOver } from "./dragdrop.svelte";

  let {
    deck,
    /** The sampler, for the pad page whose pads are not about this deck. */
    sampler,
    enabled,
    cueAvailable = false,
    density = 1,
    zones = null,
    stemSwap = null,
    deckCount = 2,
    careful = false,
  }: {
    deck: DeckState;
    sampler: SamplerState;
    enabled: boolean;
    cueAvailable?: boolean;
    /** The one stem swap in force, from the master snapshot. */
    stemSwap?: StemSwap | null;
    /** How many decks there are, so a swap can name a real one. */
    deckCount?: number;
    /**
     * Whether a mistake right now is expensive.
     *
     * From the assistant's occasion. When it is, the controls that cannot be
     * undone by pressing them again -- ejecting a playing deck -- need a
     * deliberate hold instead of a click. At a rehearsal or alone at home they
     * stay ordinary presses, because slowing every action down to guard
     * against a few is a worse trade than the accident it prevents.
     */
    careful?: boolean;
    /**
     * What to draw and in what order, from the resolved layout tree.
     *
     * Null until the tree arrives, and null again if it cannot be read. That
     * is not "draw nothing" -- it falls back to [`FULL`] below, which is the
     * deck djmanzo has always drawn. A DJ whose layout file is unreadable
     * gets the interface, not an empty column.
     */
    /**
     * The interface scale, as a number.
     *
     * Every other size on a deck reads `--density` off the document, but the
     * waveform lane cannot: it is drawn by Rust at a pixel height, so scaling
     * it in CSS would stretch tiles rendered for a different size. It is
     * multiplied into the number asked for instead.
     */
    density?: number;
    zones?: Placed[] | null;
  } = $props();

  /**
   * The deck to draw when nothing has said otherwise.
   *
   * The same order `dj_app::widgets::from_layout` produces for the default
   * layout, and there is a Rust test asserting that list against the order
   * this component draws in. Duplicated deliberately rather than awaited: the
   * tree arrives over a command, and a deck that is blank for one frame at
   * startup -- or for the whole session if the command fails -- would be a
   * worse failure than a list that has to be kept in step with a test that
   * fails when it is not.
   */
  const FULL: Placed[] = [
    "deck.waveform",
    "deck.overview",
    "deck.progress",
    "deck.stems",
    "deck.times",
    "deck.pads",
    "deck.beat_jump",
    "deck.loops",
    "deck.fx",
    "deck.grid",
    "deck.transport",
    "deck.perform",
    "deck.jog",
    "deck.eq",
    "deck.filter",
    "deck.volume",
    "deck.pitch",
    "deck.keylock",
    "deck.cue",
    "deck.xfader",
    "deck.meter",
  ].map((widget) => ({ widget, props: {}, children: {} }));

  const placed = $derived(zones && zones.length > 0 ? zones : FULL);

  /**
   * Keylock is drawn inside the tempo block, not beside it.
   *
   * It qualifies the pitch fader -- it only means anything once the fader has
   * moved -- so it belongs in that control the way a units switch belongs on
   * the instrument it labels. The tree can therefore say *whether* keylock is
   * shown but not *where*, which is a real limitation of this phase and is
   * written down here rather than discovered later.
   */
  const keylock = $derived(placed.some((zone) => zone.widget === "deck.keylock"));

  /** A prop the tree set, if it set one and it is a number. */
  function height(props: Record<string, unknown>, fallback: number): number {
    const value = props?.height;
    return Math.round((typeof value === "number" ? value : fallback) * density);
  }

  /**
   * The two runs of widgets that are drawn as one row rather than as a stack.
   *
   * The deck is a flex column and almost every widget is a row in it. Two
   * groups are not: the channel strip puts tone, colour, level and tempo side
   * by side, and its foot puts the cue and the crossfader assignment on one
   * line. Both are the reason the crossfader is reachable at all -- stacked,
   * the strip alone took about 530 px of a column that has to end above it.
   *
   * Grouping happens here, in the renderer, rather than as nesting in the
   * layout format. CSS cannot say "wrap a run of siblings", and making the
   * strip a container widget would push a presentational detail into a file
   * format that ADR-0008 deliberately keeps free of geometry. Order and
   * presence still come entirely from the tree; only the box around a run of
   * them is decided here.
   */
  const STRIP = ["deck.jog", "deck.eq", "deck.filter", "deck.volume", "deck.pitch"];
  const FOOT = ["deck.cue", "deck.xfader"];

  type Row = {
    kind: "strip" | "foot" | "one";
    key: string;
    zones: { key: string; placed: Placed }[];
  };

  /**
   * The rows that stay put when the rest of the deck scrolls.
   *
   * The same move the master strip made, one level down. A deck at djmanzo's
   * own default window is 685 px in a stage that has 559, so something has to
   * be below the fold -- and letting it be the channel strip means the volume
   * fader and the filter, which are touched continuously, are the two controls
   * a DJ has to scroll to find. The waveform and the pads scroll instead; the
   * strip, its foot and the level meter are always where they were left.
   *
   * Presentation, not identity: nothing moved in the reading order, and a
   * control that was the fourth thing down the deck is still the fourth thing
   * down the deck.
   */
  const PINNED = ["deck.jog", "deck.eq", "deck.filter", "deck.volume", "deck.pitch", "deck.cue", "deck.xfader", "deck.meter"];

  const rows = $derived.by(() => {
    const out: Row[] = [];
    placed.forEach((zone, index) => {
      // Drawn by the tempo block above, so it is not a row of its own.
      if (zone.widget === "deck.keylock") return;
      const kind: Row["kind"] = STRIP.includes(zone.widget)
        ? "strip"
        : FOOT.includes(zone.widget)
          ? "foot"
          : "one";
      // Indexed, because a layout may place the same widget twice and a key
      // that is only the name would then collide.
      const entry = { key: `${zone.widget}-${index}`, placed: zone };
      const last = out[out.length - 1];
      if (kind !== "one" && last?.kind === kind) last.zones.push(entry);
      else out.push({ kind, key: entry.key, zones: [entry] });
    });
    return out;
  });

  /**
   * The deck's own box, measured, because pinning is only affordable above a
   * size.
   *
   * Both are inputs rather than outputs: `.decks` sizes every row
   * `minmax(0, 1fr)` and every column `1fr`, so a deck's height and width come
   * from the stage and the deck count, not from what is inside it. That is
   * what makes the decision below safe to take -- a threshold read off a box
   * that the decision itself changes would oscillate.
   */
  let deckWidth = $state(0);
  let deckHeight = $state(0);

  /**
   * What the pinned foot costs, at the two sizes it actually has.
   *
   * Measured in Chromium at 900 px tall, sweeping the window from 1100 to
   * 1920 with a surface docked, and it is a step rather than a curve: the
   * channel strip is `flex-wrap: wrap`, so it is either on one line or it is
   * not.
   *
   * | deck width | channel | foot |
   * |---|---|---|
   * | 543 px and up | 125 px | **168 px** |
   * | 515 px and down | 257 px | **300 px** |
   *
   * The step is between 515 and 543; 530 is the middle of it.
   */
  const WRAPS_BELOW = 530;
  const FOOT_ON_ONE_LINE = 168;
  const FOOT_WRAPPED = 300;

  /**
   * What is left of the deck has to be worth having.
   *
   * The waveform lane is 96 px at Relaxed and 77 at the density this ships
   * at, the overview 30 and the progress bar 8. 140 px is those three plus
   * the gaps between them: the least that still answers *what is about to
   * happen*, which is the question the deck is looked at to answer.
   */
  const BODY_FLOOR = 140;
  /** The header, and the gaps the deck's own column puts between its rows. */
  const DECK_CHROME = 61;

  /**
   * Whether this deck is tall enough to pin its channel strip.
   *
   * # Why this is a question at all
   *
   * Pinning the strip was the fix for §103 -- the volume fader and the filter
   * are touched continuously, so they are the wrong things to put below the
   * fold. That reasoning is sound and it is unconditional, which is the bug.
   * A pinned region is `flex: none`; a `flex: none` region in a column with
   * less room than it wants does not scroll, it **overflows**, and paints
   * over whatever is drawn below it.
   *
   * Measured with four decks and a surface docked at 1280x800: the deck was
   * 22 px tall, its body 0, and its 300 px foot ran 328 px past the bottom of
   * the card and across the master strip. Two decks and a dock at 1280x680
   * left the body 110 px of a 471 px deck -- the waveform, the overview, the
   * pads, the loops, the FX rack and the transport sharing less than a
   * quarter of the deck so that the strip could have three quarters.
   *
   * So pinning is a luxury, and this is the price of it. Below the price the
   * deck goes back to being one scrolling column: everything is still
   * reachable, in the same order, and nothing is drawn on top of anything.
   *
   * Zero means not yet measured -- the first render, before the binding has
   * a box -- and pins, because the default at djmanzo's own window is to pin.
   */
  const pinning = $derived.by(() => {
    if (deckHeight === 0) return true;
    const foot = deckWidth > 0 && deckWidth < WRAPS_BELOW ? FOOT_WRAPPED : FOOT_ON_ONE_LINE;
    return deckHeight >= foot + DECK_CHROME + BODY_FLOOR;
  });

  /** Everything above the strip: it scrolls when there is not room for it. */
  const scrolling = $derived(
    pinning ? rows.filter((row) => !PINNED.includes(row.zones[0].placed.widget)) : rows,
  );
  /** The strip and what travels with it: always on screen, when it fits. */
  const pinned = $derived(
    pinning ? rows.filter((row) => PINNED.includes(row.zones[0].placed.widget)) : [],
  );

  let error = $state<string | null>(null);
  let loading = $state(false);

  // Name and artist come from the snapshot, not from this component.
  //
  // They used to be set here, by this deck's own Load button — which meant a
  // track arriving from the browser, the assistant, a preset or a controller
  // played perfectly while the header still read "no track". The engine has no
  // metadata, so the application remembers it per deck and sends it down with
  // everything else.
  const title = $derived(deck.title ?? "");
  const artist = $derived(deck.artist ?? "");

  const progress = $derived(
    deck.length_frames > 0 ? deck.position_frames / deck.length_frames : 0,
  );

  const analysis = $derived(deck.analysis);

  /** "4" for whole loops, "1/4" for halved ones, which is how DJs say them. */
  function formatBeats(beats: number): string {
    if (beats >= 1) return String(Math.round(beats * 100) / 100);
    return `1/${Math.round(1 / beats)}`;
  }

  /**
   * The pad pages, fetched once.
   *
   * Once because they do not change: a page is a fixed table in Rust, and the
   * only thing that varies is which deck number the actions are addressed to.
   * Everything that *does* change — what is lit, what is held — comes from the
   * snapshot the component already has.
   */
  let padPageList = $state<PadPageDto[]>([]);
  $effect(() => {
    void padPages(deck.number)
      .then((pages) => {
        padPageList = pages;
      })
      // An empty list draws no pad zone, which is a deck missing its whole
      // performance surface with nothing saying so.
      .catch((problem) => {
        error = `could not read the pad pages: ${problem}`;
      });
  });

  /**
   * Take the analyser's rejected octave.
   *
   * Autocorrelation genuinely cannot tell 80 from 160 — a curve periodic at one
   * is periodic at the other — so the octave is a *guess*, and the analyser
   * reports the runner-up precisely so a wrong guess costs one click instead of
   * a retapped grid. This only changes what is displayed; the stored analysis
   * is untouched until M2's grid editing lands.
   */
  let octaveSwapped = $state(false);
  function swapOctave() {
    if (analysis?.bpm_alternative != null) octaveSwapped = !octaveSwapped;
  }
  // Reset when the deck gets a different track, or the swap would carry over.
  $effect(() => {
    void deck.analysis;
    octaveSwapped = false;
  });

  /**
   * The element a desktop drop is aimed at.
   *
   * The whole deck, not just its header: aiming at a strip of text while
   * dragging a file is a precision task, and the deck is the thing a DJ means
   * when they drag a track "onto deck 2".
   */
  let dropZone = $state<HTMLElement | null>(null);

  $effect(() => {
    if (!dropZone) return;
    return acceptFiles(dropZone, (paths) => {
      // One track per deck, so the first of a multi-file drag wins rather than
      // the last -- dropping five files should load the one you grabbed first,
      // not leave you on whichever the operating system happened to list last.
      void load(paths[0]);
    });
  });

  async function load(path: string) {
    loading = true;
    error = null;
    try {
      await loadTrack(deck.number, path);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function pickTrack() {
    error = null;
    const path = await open({
      multiple: false,
      filters: [
        {
          name: "Audio",
          extensions: ["mp3", "flac", "wav", "aiff", "aif", "ogg", "m4a", "aac", "opus"],
        },
      ],
    });
    if (typeof path !== "string") return;
    await load(path);
  }

  /**
   * The three positions of the assignment switch.
   *
   * `THRU` in the middle because that is where it sits on a hardware mixer, and
   * because the middle position is the one that means "the crossfader does not
   * touch me" on both.
   */
  const assignments = [
    { value: "left", text: "A", title: "Cut by the left half of the crossfader" },
    {
      value: "thru",
      text: "\u2014",
      title: "Off the crossfader — this deck plays wherever the crossfader is parked",
    },
    { value: "right", text: "B", title: "Cut by the right half of the crossfader" },
  ] as const;

  /**
   * Whether the grid editing row is showing.
   *
   * Component state rather than engine state on purpose: it is a view
   * preference, not something a controller or the assistant should be able to
   * change out from under the person looking at it.
   */
  let gridOpen = $state(false);

  const send = async (action: string) => {
    try {
      await dispatch(action);
      error = null;
    } catch (e) {
      error = String(e);
    }
  };
</script>
<section
  class="deck"
  class:playing={deck.playing}
  class:drop-target={isOver(dropZone)}
  bind:this={dropZone}
  bind:clientWidth={deckWidth}
  bind:clientHeight={deckHeight}
>
  <header>
    <span class="number">{deck.number}</span>
    <!--
      An empty deck is a **button**, not a caption.
      
      It read "— no track —  /  load a file to begin", which is an instruction
      pointing at a small folder icon three hundred pixels away at the other end
      of the header. Loading a track is the first thing anybody does and the
      thing done most often all night; the largest empty area on the deck should
      be the way to do it, rather than telling you to look elsewhere.
      
      Once something is loaded it goes back to being a caption -- clicking the
      title of a playing track to replace it is not a gesture anyone wants
      within reach.
    -->
    {#if deck.loaded}
      <div class="meta">
        <div class="title" title={title}>{title}</div>
        <div class="artist">{artist}</div>
      </div>
    {:else}
      <button
        class="meta empty"
        onclick={pickTrack}
        disabled={!enabled || loading}
        title="Load a track onto deck {deck.number}"
      >
        <span class="title">{loading ? "Loading…" : "Load a track"}</span>
        <span class="artist">or drop a file here</span>
      </button>
    {/if}

    <!--
      Tempo and key, once the analyser has them.
      
      Deliberately absent rather than zeroed while analysis runs, and deliberately
      absent again when the analyser could not tell: a plausible-looking 0.0 BPM
      or a guessed key is worse than a blank, because a DJ reads these at a
      glance and will not stop to wonder whether the number is real.
    -->
    {#if deck.loaded}
      <div class="analysis mono">
        {#if analysis?.bpm != null}
          <!--
            Confidence comes from the deck, not from `analysis`: the analyser's
            number is what it originally found, and a grid the DJ has since
            edited by hand is certain. Reading the cached number here would show
            "weak grid" beside an enabled Sync button.
          -->
          <span
            class="bpm"
            class:unsure={!deck.can_sync}
            title={deck.can_sync
              ? `Beat grid confidence ${(deck.grid_confidence * 100).toFixed(0)}%`
              : `Weak beat grid — ${(deck.grid_confidence * 100).toFixed(0)}% confidence. Sync stays disabled rather than guessing. Tap or nudge the grid to fix it.`}
          >
            <!--
              The tempo being *played*, not the tempo the file was recorded at.
              With the pitch fader moved or sync engaged those differ, and the
              one that matters when you are matching two tracks is this one.
            -->
            {(deck.effective_bpm ??
              (octaveSwapped && analysis.bpm_alternative != null
                ? analysis.bpm_alternative
                : analysis.bpm)
            ).toFixed(1)}<em>BPM</em>
          </span>
          <!--
            The octave offered as a control rather than by making the number
            itself clickable: a clickable number is not discoverable, and this
            way the alternative is visible before you commit to it.
          -->
          {#if analysis.bpm_alternative != null}
            <IconButton
              icon="fa-solid fa-circle-question"
              title={`Autocorrelation cannot tell one octave from its double. Use ${(octaveSwapped
                ? analysis.bpm
                : analysis.bpm_alternative
              ).toFixed(1)} instead.`}
              aria-label="Choose alternate octave"
              onClick={swapOctave}
            />
          {/if}
        {/if}
        {#if analysis?.key_camelot}
          <span
            class="key"
            class:unsure={(analysis.key_confidence ?? 0) < 0.5}
            title="{analysis.key_standard}{analysis.key_alternative
              ? ` — could also be ${analysis.key_alternative}`
              : ''} ({((analysis.key_confidence ?? 0) * 100).toFixed(0)}% correlation)"
          >
            {analysis.key_camelot}
          </span>
        {/if}
        {#if analysis == null}
          <span class="pending">analysing…</span>
        {/if}
        <!--
          The way in to grid editing, next to the number it corrects. Shown
          whether or not the analyser found a grid: a track it could not read at
          all is exactly the one that needs tapping in.
        -->
        <IconButton
          icon="fa-solid fa-table-cells"
          title={deck.can_sync ? "Edit the beat grid" : "Edit the beat grid — this one is too weak to sync to"}
          active={gridOpen}
          onClick={() => (gridOpen = !gridOpen)}
          disabled={!enabled}
        />
      </div>
    {/if}
    <IconButton icon="fa-solid fa-folder-open" title={loading ? "Loading…" : "Load"} onClick={pickTrack} disabled={!enabled || loading} />
  </header>

  <!--
    One snippet per named widget, then one loop that draws them in the order
    the layout tree gives. Everything below is markup that was already here;
    what changed is that its *order and presence* now come from data instead of
    from the file. See docs/adr/0008-one-widget-vocabulary.md.
  -->
  {#snippet zoneWaveform(props: Record<string, unknown>)}
  <!--
    Tiles come from the Rust renderer and are scrolled by a CSS transform;
    nothing here draws. See docs/adr/0004-waveform-rendering-strategy.md.
  -->
  <!--
    The cues are draggable here and nowhere else. This is the DJ's own deck;
    the pair view's lanes draw the same record and have no business moving its
    cues from a panel about a transition.
  -->
  <Waveform
    {deck}
    height={height(props, 96)}
    onCueMoved={(slot, frame) => void moveHotCue(deck.number, slot, frame)}
    onLoopEdgeMoved={(edge, frame) => void moveLoopEdge(deck.number, edge, frame)}
  />
  {/snippet}
  {#snippet zoneOverview()}
  <!--
    The whole track under the scrolling lane. Two views answering different
    questions: the lane says what is about to happen, this says where in the
    track you are and where the breakdown is.
  -->
  {#if deck.loaded}
    <Overview {deck} height={30} />
  {/if}
  {/snippet}
  {#snippet zoneProgress()}
  <div class="progress" role="progressbar" aria-valuenow={progress * 100}>
    <div class="fill" style:scale="{fill(progress)} 1"></div>
  </div>
  {/snippet}
  {#snippet zoneStems()}
  {#if deck.loaded}
    <!--
      Below the waveform, not above it. Mounted above, this pushed the one
      thing a DJ actually watches down by its own height the instant a track
      loaded -- and it is the biggest block on the deck. It folds now (see
      `Stems.svelte`), so it costs a row when nothing is using it.
    -->
    <Stems deckNumber={deck.number} muteState={deck.stem_mutes} volumeState={deck.stem_volumes} eqState={deck.stem_eq} filterState={deck.stem_filters} soloing={deck.stem_soloing} swap={stemSwap} deckCount={deckCount} />
  {/if}
  {/snippet}
  {#snippet zoneTimes()}
  <div class="times mono">
    <span>{formatTime(deck.position_seconds)}</span>
    <span class="remaining">
      -{formatTime(Math.max(0, deck.length_seconds - deck.position_seconds))}
    </span>
  </div>
  {/snippet}
  {#snippet zonePads()}
  <!--
    The pad zone. One grid of eight with a page selector, the way hardware
    works — and the way it has to work if a controller's pads are ever to mean
    the same thing as these. The pages come from Rust, already rendered into
    action strings; see `crates/dj-core/src/pads.rs`.

    This replaced five separate fixed rows — hot cues, auto loops, saved loops,
    roll — each of which took vertical space whether or not the DJ wanted it,
    and none of which a controller could have mapped onto without the mapping
    being written a second time.
  -->
  {#if deck.loaded}
    <Pads pages={padPageList} {deck} {sampler} {enabled} {send} />
  {/if}
  {/snippet}
  {#snippet zoneBeatJump()}
  <!--
    Beat jump and auto loops. Only shown when there is a grid to measure them
    against: without one the buttons would be present and inert, which reads as
    broken rather than as "this track has no beats yet". Manual looping still
    works — see the in/out pair below, which needs no grid at all.
  -->
  {#if analysis?.bpm != null}
    <div class="beatjump">
      <span class="label">Jump</span>
      {#each [-4, -1, 1, 4] as beats (beats)}
        <IconButton
          title={beats > 0 ? `Forward ${beats} beat${beats === 1 ? '' : 's'}` : `Back ${Math.abs(beats)} beat${Math.abs(beats) === 1 ? '' : 's'}`}
          aria-label={beats > 0 ? `Forward ${beats}` : `Back ${Math.abs(beats)}`}
          disabled={!enabled || !deck.loaded}
          onClick={() => send(`deck ${deck.number} beatjump ${beats}`)}
        >
          {beats > 0 ? `+${beats}` : beats}
        </IconButton>
      {/each}

    </div>
  {/if}
  {/snippet}
  {#snippet zoneLoops()}
  {#if deck.loaded}
    <div class="beatjump loop-row">
      <IconButton
        icon="fa-solid fa-flag"
        title="Drop the loop's in point here"
        aria-label="Loop in"
        disabled={!enabled}
        onClick={() => send(`deck ${deck.number} loop_in`)}
      />
      <IconButton
        icon="fa-solid fa-flag-checkered"
        title="Drop the out point and start looping"
        aria-label="Loop out"
        disabled={!enabled}
        onClick={() => send(`deck ${deck.number} loop_out`)}
      />
      <!--
        Loop the phrase the playhead is inside. Disabled, rather than hidden,
        when the analyser found no phrase structure: a control that vanishes
        between tracks is one a DJ stops reaching for, and "this track has no
        phrases" is worth saying.
      -->
      <IconButton
        icon="phrase"
        title={analysis?.phrase_beats != null
          ? `Loop this ${analysis.phrase_beats}-beat phrase`
          : "No phrase structure was found in this track"}
        aria-label="Loop this phrase"
        disabled={!enabled || analysis?.phrase_beats == null}
        onClick={() => send(`deck ${deck.number} loop_phrase 1`)}
      />
      <IconButton
        icon="fa-solid fa-compress"
        title="Halve the loop, keeping its start"
        aria-label="Halve loop"
        disabled={!enabled || deck.active_loop == null}
        onClick={() => send(`deck ${deck.number} loop_halve`)}
      />
      <IconButton
        icon="fa-solid fa-expand"
        title="Double the loop, keeping its start"
        aria-label="Double loop"
        disabled={!enabled || deck.active_loop == null}
        onClick={() => send(`deck ${deck.number} loop_double`)}
      />
      <IconButton
        title="Stop looping and carry on"
        active={deck.active_loop != null}
        disabled={!enabled || deck.active_loop == null}
        onClick={() => send(`deck ${deck.number} loop_off`)}
      >
        {#if deck.active_loop}
          {deck.rolling ? "Rolling" : "Looping"}{deck.active_loop.beats
            ? ` ${formatBeats(deck.active_loop.beats)}`
            : ""}
        {:else}
          No loop
        {/if}
      </IconButton>
    </div>

  {/if}
  {/snippet}
  {#snippet zoneFx()}
  <!--
    The effect rack. Below the loops because that is the order a DJ builds in:
    find the section, loop it, then colour it.
  -->
    <Fx slots={deck.fx} {enabled} target="deck {deck.number}" {send} />

  {/snippet}
  {#snippet zoneGrid()}
  <!--
    Grid editing. Hidden behind a toggle rather than always on: it is the
    control a DJ reaches for once per track at most, and never during a mix,
    while everything above it is touched constantly.

    The order is the order of use. `Here` fixes phase, which is the common
    failure and the one-button case. The nudges are for the last few
    milliseconds. `Tap` is for a track with no usable grid at all, and the
    halve/double pair is for the octave errors autocorrelation cannot resolve.
  -->
  {#if deck.loaded && gridOpen}
    <div class="beatjump grid-row">
      <span class="label">Grid</span>
      <IconButton icon="fa-solid fa-location-dot" title="Put a beat on the playhead" onClick={() => send(`deck ${deck.number} grid_here`)} disabled={!enabled} />
      {#each [-10, -1, 1, 10] as ms (ms)}
        <IconButton
          title={`Slide the whole grid ${Math.abs(ms)} ms ${ms < 0 ? 'earlier' : 'later'}`}
          disabled={!enabled}
          onClick={() => send(`deck ${deck.number} grid_nudge ${ms}`)}
        >
          {ms > 0 ? `+${ms}` : ms}
        </IconButton>
      {/each}
      <IconButton icon="fa-solid fa-hand-pointer" title="Tap along with the music" onClick={() => send(`deck ${deck.number} grid_tap`)} disabled={!enabled} />
      <IconButton title="Halve the grid tempo, keeping the beat you lined up" aria-label="Grid ÷2" disabled={!enabled} onClick={() => send(`deck ${deck.number} grid_scale 0.5`)}>÷2</IconButton>
      <IconButton title="Double the grid tempo, keeping the beat you lined up" aria-label="Grid ×2" disabled={!enabled} onClick={() => send(`deck ${deck.number} grid_scale 2`)}>×2</IconButton>
      <IconButton icon="fa-solid fa-rotate-left" title="Reset grid edits" onClick={() => send(`deck ${deck.number} grid_reset`)} disabled={!enabled} />
    </div>
  {/if}
  {/snippet}
  {#snippet zoneTransport()}
  <div class="transport">
    <SvgPad
      label="CUE"
      disabled={!enabled || !deck.loaded}
      onclick={() => send(`deck ${deck.number} cue`)}
    />
    <SvgPad
      label={deck.playing ? "PAUSE" : "PLAY"}
      active={deck.playing}
      disabled={!enabled || !deck.loaded}
      onclick={() => send(`deck ${deck.number} play_pause`)}
    />
    <SvgPad
      label="SYNC"
      active={deck.synced}
      disabled={!enabled || !deck.can_sync}
      onclick={() => send(`deck ${deck.number} ${deck.synced ? "sync_off" : "sync"}`)}
    />
    <!--
      Eject is the one control on this panel that cannot be undone by pressing
      it again: the track, its cues and its analysis all go. So it is the one
      that holds when the night is expensive.
    -->
    <SvgPad
      label="EJECT"
      hold={careful && deck.playing}
      disabled={!enabled || !deck.loaded}
      onclick={() => send(`deck ${deck.number} eject`)}
    />
  </div>
  {/snippet}
  {#snippet zonePerform()}
  <!--
    The rest of the transport.

    Slip, reverse, censor, brake and backspin are all one idea — something
    diverting the audible playhead while the track carries on underneath — and
    they are transport, not channel: none of them touches tone, colour, level
    or tempo. They spent their life in the tempo block, wrapping it onto a
    second and third row, and they were *nested inside the keylock condition*
    there, so a layout that hid keylock silently took slip, reverse, censor,
    brake and backspin with it. `showSlip` and `showKeylock` are independent
    layout flags and now behave like it.

    Its own row rather than more columns in the one above: CUE and PLAY are
    the two biggest targets on the deck and a DJ aims at them without looking.
    Nine controls sharing four columns' worth of width would have shrunk those
    two to pay for five that are pressed a hundredth as often.
  -->
    <div class="perform" role="group" aria-label="Playhead">
      <IconButton
        title={deck.slip
          ? "Slip on — loop, reverse or censor, and the track carries on underneath"
          : "Slip off — the playhead stays wherever a loop or a censor leaves it"}
        active={deck.slip}
        disabled={!enabled}
        onClick={() => send(`deck ${deck.number} slip_toggle`)}
        aria-label="Slip"
      >
        SLIP
      </IconButton>
      <IconButton
        icon="fa-solid fa-backward"
        title="Play backwards"
        active={deck.reversed}
        disabled={!enabled || !deck.loaded}
        onClick={() => send(`deck ${deck.number} reverse_toggle`)}
        aria-label="Reverse"
      />
      <!--
        Held rather than clicked, and on pointer events rather than mouse ones
        so it works from a touchscreen. `pointerleave` releases too: dragging
        off the pad mid-censor must not leave the deck stuck in reverse.
      -->
      <IconButton
        icon="fa-solid fa-hand-paper"
        title="Hold to reverse over a word, and land back on the beat"
        disabled={!enabled || !deck.loaded}
        onpointerdown={() => send(`deck ${deck.number} censor_on`)}
        onpointerup={() => send(`deck ${deck.number} censor_off`)}
        onpointerleave={() => send(`deck ${deck.number} censor_off`)}
        aria-label="Censor"
      />

      <!--
        Brake and backspin. Held rather than clicked, and momentary in an
        unusual way: the press *starts* a coast that runs on its own, and the
        release puts the motor back on wherever the record got to. Letting one
        run to the end leaves the deck stopped, which is the point.

        Only with a grid, because a coast is measured in beats — the engine
        refuses one it cannot measure, and a control that silently does nothing
        reads as broken.
      -->
      {#if analysis?.bpm != null}
        <IconButton
          title="Cut the motor and coast to a stop over two beats. Let go to put it back on."
          active={deck.spinning}
          disabled={!enabled || !deck.playing}
          onpointerdown={() => send(`deck ${deck.number} brake 2`)}
          onpointerup={() => send(`deck ${deck.number} brake_off`)}
          onpointercancel={() => send(`deck ${deck.number} brake_off`)}
          aria-label="Brake"
        >
          BRAKE
        </IconButton>
        <IconButton
          title="Throw the record backwards and let friction take it down over a beat"
          active={deck.spinning}
          disabled={!enabled || !deck.playing}
          onpointerdown={() => send(`deck ${deck.number} backspin 1`)}
          onpointerup={() => send(`deck ${deck.number} backspin_off`)}
          onpointercancel={() => send(`deck ${deck.number} backspin_off`)}
          aria-label="Spin"
        >
          SPIN
        </IconButton>
      {/if}
    </div>

  {/snippet}
  {#snippet zoneJog()}
  <!--
    The platter. Drag the middle to scratch, the rim to bend, and wind it to
    search a paused deck -- the same three things the hardware does, and the
    same actions a controller mapping sends.

    In the strip rather than on a row of its own. On a laptop it is a nudge
    target and a position display, and the waveform above already answers
    position better than a circle does; a full row for it cost about 155 px,
    which is more than the waveform got in three of the four shipped presets.
  -->
  <div class="jog-row">
    <JogWheel
      deckNumber={deck.number}
      touched={deck.jog_touched}
      mode={deck.jog_mode}
      bend={deck.jog_bend}
      enabled={enabled && deck.loaded}
    />
  </div>
  {/snippet}
  {#snippet zoneEq()}
  <!--
    Isolator EQ: each knob runs from a true kill at 0 to +12 dB. Double-click
    resets to unity, because reaching for exactly 1.00 with a mouse mid-mix is
    not a thing anyone can do.
  -->
  <div class="eq">
    {#each [{ id: "eq_high", label: "HI", value: deck.eq_high }, { id: "eq_mid", label: "MID", value: deck.eq_mid }, { id: "eq_low", label: "LOW", value: deck.eq_low }] as band (band.id)}
      <label class="band" class:killed={band.value < 0.001}>
        <SvgKnob
          value={band.value}
          min={0}
          max={4}
          step={0.01}
          label={band.label}
          readout={band.value < 0.001 ? "kill" : band.value.toFixed(2)}
          size={46}
          disabled={!enabled}
          oninput={(val) => send(`deck ${deck.number} ${band.id} ${val}`)}
          ondblclick={() => send(`deck ${deck.number} ${band.id} 1`)}
        />
        <button
          class="kill"
          class:on={band.value < 0.001}
          disabled={!enabled}
          onclick={() => send(`deck ${deck.number} ${band.id} ${band.value < 0.001 ? 1 : 0}`)}
          title="Kill {band.label}"
          aria-label="Kill {band.label}"
        ></button>
      </label>
    {/each}
  </div>

  {/snippet}
  {#snippet zoneFilter()}
  <label class="control">
    <SvgKnob
      value={deck.filter}
      min={-1}
      max={1}
      step={0.01}
      label="Filter"
      readout={Math.abs(deck.filter) <= 0.02
        ? "off"
        : deck.filter < 0
          ? `LP ${Math.round(-deck.filter * 100)}%`
          : `HP ${Math.round(deck.filter * 100)}%`}
      disabled={!enabled}
      size={56}
      oninput={(val) => send(`deck ${deck.number} filter ${val}`)}
      ondblclick={() => send(`deck ${deck.number} filter 0`)}
    />
  </label>

  {/snippet}
  {#snippet zoneVolume()}
  <label class="control fader-wrap">
    <SvgFader
      value={deck.volume}
      min={0}
      max={1}
      step={0.01}
      label="Volume"
      readout={deck.volume.toFixed(2)}
      disabled={!enabled}
      height={140}
      width={40}
      oninput={(val) => send(`deck ${deck.number} volume ${val}`)}
    />
  </label>
  {/snippet}
  {#snippet zonePitch()}
  <!--
    Tempo, and the two things that qualify it.

    Keylock only means anything once the fader has moved, and the semitone
    shift is the same idea a step further, so all three are one block. Double-
    click the fader to snap back to zero — hitting exactly 0.0% with a mouse is
    not something anyone can do mid-mix.

    A flat run of children rather than a grid. It used to be
    `grid-template-columns: 1fr auto auto` holding eight controls, which is
    three implicit rows: the pitch fader stood 140 px tall in the first one and
    everything else stacked beside and below it, so the tempo block alone was
    about 185 px and forced the whole strip onto a second line. Measured at
    djmanzo's own default window, that was most of what put the crossfader
    below the fold.
  -->
  <div class="tempo">
    <label class="control fader-wrap">
      <SvgFader
        value={deck.pitch}
        min={-0.16}
        max={0.16}
        step={0.001}
        label="Pitch"
        readout={`${(deck.pitch * 100).toFixed(1)}%`}
        disabled={!enabled}
        height={140}
        width={40}
        oninput={(val) => send(`deck ${deck.number} pitch ${val}`)}
        ondblclick={() => send(`deck ${deck.number} pitch 0`)}
      />
    </label>
    <!--
      Harmonic mixing: shift the key in semitones without touching tempo.
      Separate from keylock, and engages the shifter on its own — but hidden
      with it, because a layout that judges keylock too advanced to show is not
      one that wants semitone transposition either.
    -->
    {#if keylock}
      <div class="tempo-extras">
        <div class="keyshift">
          <IconButton
            icon="fa-solid fa-minus"
            title="Down a semitone"
            disabled={!enabled}
            onClick={() => send(`deck ${deck.number} key ${deck.key_shift - 1}`)}
            aria-label="Down a semitone"
          />
          <span class="mono" class:shifted={deck.key_shift !== 0}>
            {deck.key_shift > 0 ? `+${deck.key_shift}` : deck.key_shift}
          </span>
          <IconButton
            icon="fa-solid fa-plus"
            title="Up a semitone"
            disabled={!enabled}
            onClick={() => send(`deck ${deck.number} key ${deck.key_shift + 1}`)}
            aria-label="Up a semitone"
          />
        </div>
        <IconButton
          icon="fa-solid fa-lock"
          title={deck.keylock
            ? `Keylock on — tempo changes without changing key (adds ${deck.keylock_latency_ms.toFixed(0)} ms, compensated)`
            : "Keylock off — the pitch fader moves tempo and key together"}
          active={deck.keylock}
          disabled={!enabled}
          onClick={() => send(`deck ${deck.number} keylock_toggle`)}
          aria-label="Keylock"
        />
      </div>
    {/if}
  </div>
  {/snippet}
  {#snippet zoneCue()}
  <button
    class="cue"
    class:on={deck.cue_enabled}
    disabled={!enabled || !cueAvailable}
    onclick={() => send(`deck ${deck.number} cue_toggle`)}
    title={cueAvailable
      ? "Pre-fader listen — hear this deck in the headphones"
      : "Needs a 4-channel output device"}
  >
    <span>CUE</span>
    <span class="pfl-meter" aria-hidden="true">
      <span class="pfl-fill" style:scale="{fill(deck.pre_fader_level)} 1"></span>
    </span>
  </button>
  {/snippet}
  {#snippet zoneXfader()}
  <!--
    Crossfader assignment. A hardware mixer puts this switch on every channel,
    and once four decks are on screen it stops being optional: without it the
    crossfader can only reach decks 1 and 2, so half the mixer is outside the
    one control a DJ uses without looking.
  -->
  <div class="xfader-assign" role="group" aria-label="crossfader assignment">
    <span class="label">X</span>
    {#each assignments as option (option.value)}
      <IconButton
        active={deck.crossfader_assign === option.value}
        disabled={!enabled}
        onClick={() => send(`deck ${deck.number} xfader_${option.value}`)}
        title={option.title}
        aria-pressed={deck.crossfader_assign === option.value}
      >
        {option.text}
      </IconButton>
    {/each}
  </div>
  {/snippet}
  {#snippet zoneMeter()}
  <div class="meter" aria-label="deck level">
    <div class="meter-fill" style:scale="{fill(deck.peak)} 1"></div>
  </div>
  {/snippet}

  <!--
    Name to markup. A chain of comparisons rather than a lookup table because
    Svelte's snippets are markup constructs, not values that survive a trip
    through an object -- and because an unknown name falling through to nothing
    is the behaviour ADR-0008's third rule asks for anyway.
  -->
  {#snippet zone(placed: Placed)}
    {#if placed.widget === "deck.waveform"}{@render zoneWaveform(placed.props)}
    {:else if placed.widget === "deck.overview"}{@render zoneOverview()}
    {:else if placed.widget === "deck.progress"}{@render zoneProgress()}
    {:else if placed.widget === "deck.stems"}{@render zoneStems()}
    {:else if placed.widget === "deck.times"}{@render zoneTimes()}
    {:else if placed.widget === "deck.pads"}{@render zonePads()}
    {:else if placed.widget === "deck.beat_jump"}{@render zoneBeatJump()}
    {:else if placed.widget === "deck.loops"}{@render zoneLoops()}
    {:else if placed.widget === "deck.fx"}{@render zoneFx()}
    {:else if placed.widget === "deck.grid"}{@render zoneGrid()}
    {:else if placed.widget === "deck.transport"}{@render zoneTransport()}
    {:else if placed.widget === "deck.perform"}{@render zonePerform()}
    {:else if placed.widget === "deck.jog"}{@render zoneJog()}
    {:else if placed.widget === "deck.eq"}{@render zoneEq()}
    {:else if placed.widget === "deck.filter"}{@render zoneFilter()}
    {:else if placed.widget === "deck.volume"}{@render zoneVolume()}
    {:else if placed.widget === "deck.pitch"}{@render zonePitch()}
    {:else if placed.widget === "deck.cue"}{@render zoneCue()}
    {:else if placed.widget === "deck.xfader"}{@render zoneXfader()}
    {:else if placed.widget === "deck.meter"}{@render zoneMeter()}
    {/if}
  {/snippet}

  <!--
    One loop, rendered twice: once for what scrolls and once for what does not.

    A snippet taking the list rather than two copies of the markup, because two
    copies of a fourteen-branch dispatch is two places to forget a widget.
  -->
  {#snippet band(which: Row[])}
    {#each which as row (row.key)}
      {#if row.kind === "strip"}
    <!--
      The channel strip, side by side rather than stacked.

      Measured, at djmanzo's own default 1280x800: stacked, the EQ, filter,
      volume and pitch took about 530 px of a deck column, which put the
      crossfader roughly 1,500 px down — two screens below the waveform it is
      used against. No shipped preset fixed it, including the one whose
      description is "everything you need and nothing else". Side by side the
      same four controls take about 190 px and keep every one of them in the
      orientation a DJ's hands already know: knobs for tone, vertical faders for
      level and pitch.

      Ordered as a hardware channel is, left to right: tone, then colour, then
      level, then tempo.

      And on **one line**, which took a second pass to actually achieve. Pitch
      was nominally in the strip and in practice was not: it lived in a grid of
      its own carrying eight controls in three columns, so it wrapped the strip
      onto a second row and then a third. Measured again at 1280x800 with the
      grid flattened and the playhead controls moved up to the transport where
      they belong, the deck goes from 695 px to 539 px -- and the crossfader's
      thumb from 117 px below the fold to 758 px down a screen 800 px tall.
      Reachable at djmanzo's own default window size, for the first time.
    -->
        <div class="channel">
          {#each row.zones as entry (entry.key)}{@render zone(entry.placed)}{/each}
        </div>
      {:else if row.kind === "foot"}
    <!--
      The foot of the channel: what this deck sends where.

      Pre-fader listen and the crossfader assignment on one line, because they
      are the same question asked twice -- which outputs hear this deck -- and
      because two rows of one control each is how a deck column grows until the
      crossfader is off the screen.

      Pre-fader listen deliberately explains itself when unavailable rather than
      just sitting greyed out: a 2-channel laptop output has nowhere to send a
      cue, and "why is this dead" is a bad thing to wonder mid-set.
    -->
        <div class="channel-foot">
          {#each row.zones as entry (entry.key)}{@render zone(entry.placed)}{/each}
        </div>
      {:else}
        {@render zone(row.zones[0].placed)}
      {/if}
    {/each}
  {/snippet}

  <!--
    The part of the deck that scrolls when the window is too short for all of
    it, and the part that never does.

    A deck at djmanzo's own default window is 685 px in a stage with 559, so
    something is below the fold either way. Letting it be the channel strip
    means the volume fader and the filter -- touched continuously, not once a
    transition -- are what a DJ has to scroll to find. The waveform and the
    pads go instead. Same move the master strip made one level up, same
    reason.
  -->
  <div class="deck-body">
    {@render band(scrolling)}
  </div>
  <div class="deck-foot">
    {@render band(pinned)}
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}
</section>

<style>
  /* A drag with no feedback is a drag you cannot aim: without this, a DJ
     dropping a file onto four decks finds out which one they hit by hearing
     it. */
  .deck.drop-target {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  /* The empty deck's load target. Fills the space a title would take, so the
     affordance is where the eye already is. */
  .meta.empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    gap: 0.1rem;
    text-align: left;
    background: transparent;
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
    color: var(--text-dim);
    padding: 0.3rem 0.6rem;
    cursor: pointer;
  }

  .meta.empty:hover:not(:disabled) {
    background: var(--panel-hover);
    border-color: var(--accent);
    color: var(--text);
  }

  .meta.empty:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }

  .jog-row {
    display: flex;
    justify-content: center;
    padding: 0.4rem 0;
  }

  /*
    The channel strip. Bottom-aligned so the faders' zero ends line up with the
    knobs' baselines, which is what makes it read as one strip rather than four
    controls that happen to be adjacent.

    Allowed to wrap: at the 900 px minimum window width a deck column is about
    415 px inside its padding, and a strip that overflowed would put a control
    off the edge rather than onto a second line.
  */
  .channel {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 0.6rem 0.9rem;
  }

  /*
    The three EQ knobs stay a row of their own inside the strip, because they
    are one control in three parts and separating them would invite reading
    them as three. `justify-content` is overridden because the standalone rule
    spreads them across the whole deck; inside the strip they are as wide as
    the knobs and no wider.
  */
  .channel .eq {
    justify-content: flex-start;
    gap: 0.55rem;
  }

  /*
    The foot of the channel. The cue bar takes the room, because it carries a
    level meter and a meter needs width; the assignment is three small buttons
    and needs none.
  */
  .channel-foot {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .channel-foot .cue {
    flex: 1;
    min-width: 0;
  }

  .channel-foot .xfader-assign {
    flex: none;
  }

  /* The jog is a position display the waveform above already gives in a form
     easier to read, and a nudge target a mouse can use. It stays, at a size
     that reflects what it adds rather than what it is. */
  .jog-row {
    --jog-size: 5rem;
    padding: 0;
    align-items: flex-end;
  }

  /*
    The deck is exactly as tall as it is given, and its body scrolls inside it.

    `min-height: 0` on both, because a flex child defaults to `min-height:
    auto` and refuses to shrink below its content -- which is how a "scrolling"
    region ends up not scrolling and pushing its siblings off the bottom
    instead. That is the same default that clipped the browser panel twice.
  */
  .deck-body {
    display: flex;
    flex-direction: column;
    gap: inherit;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  /*
    Every zone keeps the height it asked for, and the body scrolls instead.

    Without this the waveform vanished. A flex child's `flex-shrink` is 1, so
    in a column with less room than its contents the browser shrinks them all
    rather than overflowing -- and the waveform lane, being a fixed pixel
    height with nothing inside holding it open, went to zero. The screenshot
    showed a deck whose first visible row was the stems fold: no waveform, no
    overview, no progress bar, and no scrollbar either, because nothing
    overflowed. It is the same class of mistake as the `min-height: auto` that
    clipped the browser panel twice, arriving from the opposite direction.

    `flex: none` makes them refuse to shrink, which is what turns the body
    into something that scrolls.
  */
  .deck-body > :global(*) {
    flex: none;
  }

  .deck-foot {
    display: flex;
    flex-direction: column;
    gap: inherit;
    flex: none;
  }

  .deck {
    /* Fills the row it is given and no more; the body inside it scrolls. */
    min-height: 0;
    /*
      And nothing inside it is ever drawn outside it.

      The pinning rule above is what keeps the foot from wanting more room
      than the deck has. This is what happens if that rule is ever wrong: a
      clipped control is a bug that is visible, an overflowing one is a bug
      that paints across the master strip and looks like a rendering glitch.
      Given the choice, fail where it can be seen.
    */
    overflow: hidden;
    background: var(--panel);
    border: 1px solid var(--border);
    /* Border colour only: a deck that resized or moved when it started playing
       would shift everything around it, mid-set. */
    transition: border-color var(--motion-enter) var(--ease);
    border-radius: 10px;
    padding: 0.9rem;
    display: flex;
    flex-direction: column;
    /*
      Tightened from 0.65rem. With seven or eight children the gap alone was
      about 70 px of a column that has to end above the crossfader, and the
      groups inside now carry their own separation -- the channel strip reads
      as one thing whether or not there is a wide gap above it.
    */
    gap: 0.45rem;
    min-width: 0;
  }

  .deck.playing {
    border-color: color-mix(in srgb, var(--accent-2) 45%, var(--border));
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    min-width: 0;
  }
  .beatjump {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.8em;
  }

  /*
    Slightly quieter than the row above it: grid editing is a repair job, not
    part of playing, and it should not compete with the loop controls for
    attention while a mix is running.
  */
  .beatjump .label {
    color: var(--text-dim);
    margin-right: 0.2rem;
  }
  .analysis {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    font-size: 0.85em;
    white-space: nowrap;
  }

  .analysis em {
    font-style: normal;
    font-size: 0.7em;
    color: var(--text-dim);
    margin-left: 0.15em;
  }

  .bpm {
    color: var(--text);
    font-weight: 600;
  }
  /*
    A number the analyser is not sure of still gets shown — it is usually right,
    and a blank would be less useful — but it is visibly dimmer, because the one
    thing worse than no BPM is a wrong one presented with confidence.
  */
  .bpm.unsure,
  .key.unsure {
    color: var(--warn);
    font-weight: 400;
  }

  .key {
    color: var(--accent-2);
    font-weight: 600;
  }

  .pending {
    color: var(--text-dim);
    font-size: 0.9em;
  }

  .number {
    font-size: 1.5rem;
    font-weight: 700;
    color: var(--accent);
    width: 1.4rem;
    text-align: center;
    flex: none;
  }

  .meta {
    flex: 1;
    min-width: 0;
  }

  .title,
  .artist {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title {
    font-weight: 600;
  }

  .artist {
    color: var(--text-dim);
    font-size: 0.85em;
  }

  .progress {
    height: 8px;
    background: var(--panel-raised);
    border-radius: 4px;
    overflow: hidden;
  }

  .fill {
    width: 100%;
    transform-origin: left center;
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
  }

  .times {
    display: flex;
    justify-content: space-between;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .remaining {
    color: var(--warn);
  }

  /*
    Four columns, not three. Sync is a transport control -- it belongs beside
    play and cue, where a DJ's hand already is -- and adding it to a
    three-column grid dropped Eject onto a row of its own, which reads as two
    unrelated groups of buttons rather than one transport.
  */
  .transport {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.4rem;
  }

  .control {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }


  /* Three knobs across, as on the mixer they stand in for. The sliders these
     replaced were stacked, which a knob cannot be without making the EQ taller
     than the transport. */
  .eq {
    display: flex;
    justify-content: space-evenly;
    align-items: flex-start;
    gap: 0.5rem;
  }

  .band {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.75em;
    color: var(--text-dim);
    letter-spacing: 0.05em;
  }

  .band.killed :global(.label) {
    color: var(--danger);
  }

  .kill {
    width: 14px;
    height: 14px;
    padding: 0;
    border-radius: 3px;
    background: var(--panel-raised);
  }

  .kill.on {
    background: var(--danger);
    border-color: var(--danger);
  }

  .keyshift {
    display: flex;
    align-items: center;
    gap: 0.15rem;
    font-size: 0.72em;
  }
  .keyshift span {
    min-width: 1.6rem;
    text-align: center;
    color: var(--text-dim);
  }

  .keyshift span.shifted {
    color: var(--accent-2);
    font-weight: 600;
  }

  /*
    Flex rather than the grid this used to be. A grid gives every child a cell,
    so eight controls in three columns became three rows and the block stood
    185 px tall; flex lets the three that are left sit on one line at the
    height of the tallest, which is the fader.
  */
  .tempo {
    display: flex;
    align-items: flex-end;
    gap: 0.5rem;
  }

  /*
    Keylock and the semitone shift, as one parcel.

    Grouped so that when the strip runs out of width they wrap *together*,
    onto a line of their own about 44 px tall, instead of one of them going
    over alone and leaving the other stranded beside the fader.
  */
  .tempo-extras {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  /*
    The playhead row. Small, uniform and dense, under the four big pads: these
    are held rather than aimed at, and a censor is a gesture rather than a
    target.
  */
  .perform {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-wrap: wrap;
    margin-top: 0.4rem;
  }
  /* Momentary, so it must not look like something that stays pressed. */
  .cue {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75em;
    letter-spacing: 0.08em;
    font-weight: 600;
  }

  .cue.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }

  .pfl-meter {
    flex: 1;
    height: 4px;
    background: var(--scrim);
    border-radius: 2px;
    overflow: hidden;
  }

  .pfl-fill {
    width: 100%;
    transform-origin: left center;
    display: block;
    height: 100%;
    background: var(--accent-2);
  }

  /*
    A compact three-way switch rather than three full-width buttons: it has to
    fit a quarter-width deck in the four-deck layout, and it is set once at the
    start of a mix rather than reached for constantly.
  */
  .xfader-assign {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .xfader-assign .label {
    font-size: 0.6rem;
    letter-spacing: 0.08em;
    color: var(--text-dim);
    margin-right: 0.3rem;
  }
  .meter {
    height: 4px;
    background: var(--panel-raised);
    border-radius: 2px;
    overflow: hidden;
  }

  .meter-fill {
    width: 100%;
    transform-origin: left center;
    height: 100%;
    background: var(--accent-2);
    /* No CSS transition: a level meter that lags is a lying level meter. */
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 0.8em;
  }
</style>
