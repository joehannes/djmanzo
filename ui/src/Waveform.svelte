<script lang="ts">
  /**
   * A scrolling waveform lane.
   *
   * The rule this component exists to honour (ADR-0004): **the webview never
   * draws the waveform.** Tiles arrive as PNGs from the Rust renderer via the
   * `wave://` protocol, sit in `<img>` elements laid end to end, and the whole
   * strip is moved by a single CSS transform. That is compositor work — no
   * canvas, no WebGL, no per-frame JavaScript drawing.
   *
   * Position is interpolated between engine snapshots. Snapshots arrive at
   * 60 Hz, but a frame that lands between two of them must still move, or the
   * waveform visibly stutters against audio that is perfectly smooth.
   */
  import { onMount } from "svelte";
  import {
    playbackFramesPerSecond,
    tileUrl,
    waveformInfo,
    type DeckState,
    type MixOutInfo,
  } from "./api";
  import { theme } from "./theme.svelte";

  let {
    deck,
    height = 96,
    framesPerPixel = 256,
    marks = [],
    onMoveMark,
  }: {
    deck: DeckState;
    height?: number;
    framesPerPixel?: number;
    /**
     * Places in the *file* worth drawing a line at, beyond the deck's own cues.
     *
     * What the pair view puts here is where a transition starts and ends, so
     * the mix point is on the waveform rather than only in a number beside it.
     * Frames, like everything else in this component — a caller holding
     * seconds converts before it gets here, because the sample rate is a
     * property of the record and this component knows nothing about records.
     */
    marks?: { frame: number; label: string; draggable?: boolean }[];
    /**
     * A draggable mark was let go somewhere else. Frames, in the file.
     *
     * §26: "the DJ should be able to physically grab the thing they are
     * thinking about". What this reports is a *position*, not a decision —
     * whoever owns the mark decides what moving it means, and re-derives
     * whatever it implies. The waveform works nothing out.
     */
    onMoveMark?: (label: string, frame: number) => void;
  } = $props();

  /**
   * The mark being dragged, and where it has got to.
   *
   * Held locally so the line follows the pointer at once. The real answer
   * comes back from Rust on release, which is what is then drawn — so a drag
   * that Rust refuses or snaps elsewhere snaps visibly, rather than leaving
   * the interface showing a mix djmanzo is not holding.
   */
  let dragging = $state<{ label: string; frame: number } | null>(null);

  /**
   * How much record is left, as a band at the end of the lane.
   *
   * Thirty seconds. Long enough that noticing it is still useful — a DJ who
   * sees it has time to do something — and short enough that it is not on
   * screen for half of every record, which would make it furniture rather than
   * a warning.
   */
  const RUNWAY_SECONDS = 30;

  const runway = $derived.by(() => {
    const rate = deck.length_seconds > 0 ? totalFrames / deck.length_seconds : 0;
    if (!totalFrames || rate <= 0) return null;
    const width = (RUNWAY_SECONDS * rate) / framesPerPixel;
    const left = totalFrames / framesPerPixel - width;
    return width > 1 ? { left, width } : null;
  });

  /**
   * §25's mix-out layer: where this record could be left.
   *
   * Rust's answer, not a second one worked out here — `plan::mix_out` derives
   * both edges from the same lengths and tail margin the planner uses, so the
   * band and the planner's own "rushed" warning cannot disagree on screen
   * about the same mix.
   *
   * Drawn on every lane, whether or not a transition is planned on it, because
   * it is a fact about the record rather than about a pair. Every record is
   * left somewhere eventually.
   */
  const mixOutBand = $derived.by(() => {
    if (!mixOut) return null;
    const left = mixOut.opens_frame / framesPerPixel;
    const width = (mixOut.closes_frame - mixOut.opens_frame) / framesPerPixel;
    return width > 1 ? { left, width, onPhrase: mixOut.on_phrase } : null;
  });

  /**
   * §25's uncertainty layer: that the beat grid under all this is a guess.
   *
   * The rasteriser already fades beat lines by the grid's confidence, and that
   * fade cannot be read: at overview zoom the grid is suppressed entirely for
   * density, so faint and absent look the same, and neither says whether the
   * analyser was unsure. This is the part that says so.
   *
   * `can_sync` rather than a threshold of our own — it is `grid_confidence`
   * against `Confidence::SYNC_THRESHOLD`, decided in Rust and published. A
   * second threshold here would be a lane calling a grid trustworthy while the
   * Sync button beside it refused to touch it.
   */
  const gridIsAGuess = $derived(deck.loaded && !deck.can_sync);

  /** The stretch a transition covers, when two marks describe one. */
  const seamBand = $derived.by(() => {
    if (marks.length < 2) return null;
    const frames = marks.map((mark) => mark.frame).sort((a, b) => a - b);
    const left = frames[0] / framesPerPixel;
    const width = (frames[frames.length - 1] - frames[0]) / framesPerPixel;
    return width > 1 ? { left, width } : null;
  });

  function frameAt(event: PointerEvent): number {
    const box = strip?.getBoundingClientRect();
    if (!box) return 0;
    return Math.max(0, (event.clientX - box.left) * framesPerPixel);
  }

  /**
   * Start a drag, listening on the window rather than capturing the pointer.
   *
   * `setPointerCapture` is the obvious way and it is the one dependency worth
   * not having: djmanzo runs in WebKitGTK, the browser tests run in Chromium,
   * and capture semantics are exactly the sort of thing that differs between
   * them — leaving a control that passes its test and does nothing where it
   * ships. Window listeners behave the same everywhere, and they also handle
   * the case capture is usually reached for: a pointer that leaves the lane
   * mid-drag still moves the mark.
   */
  function grab(event: PointerEvent, mark: { label: string; draggable?: boolean }) {
    if (!mark.draggable) return;
    event.preventDefault();
    dragging = { label: mark.label, frame: frameAt(event) };
    window.addEventListener("pointermove", drag);
    window.addEventListener("pointerup", drop, { once: true });
    window.addEventListener("pointercancel", cancel, { once: true });
  }

  function drag(event: PointerEvent) {
    if (!dragging) return;
    dragging = { ...dragging, frame: frameAt(event) };
  }

  function cancel() {
    dragging = null;
    window.removeEventListener("pointermove", drag);
  }

  /**
   * How far an arrow key moves a mark, in pixels of lane.
   *
   * Pixels rather than frames, so one press is the same visible distance at
   * every zoom — which is what a hand expects from a nudge. Whoever owns the
   * mark still snaps the answer to whatever grid it belongs on.
   */
  const NUDGE_PX = 6;
  const NUDGE_FAST_PX = 48;

  function nudge(event: KeyboardEvent, mark: { label: string; frame: number }) {
    const step =
      event.key === "ArrowLeft" ? -1 : event.key === "ArrowRight" ? 1 : 0;
    if (step === 0) return;
    event.preventDefault();
    const pixels = event.shiftKey ? NUDGE_FAST_PX : NUDGE_PX;
    onMoveMark?.(
      mark.label,
      Math.max(0, mark.frame + step * pixels * framesPerPixel),
    );
  }

  function drop(event: PointerEvent) {
    window.removeEventListener("pointermove", drag);
    if (!dragging) return;
    const { label, frame } = { ...dragging, frame: frameAt(event) };
    dragging = null;
    onMoveMark?.(label, frame);
  }

  /** Tile width in pixels. Wide enough that a lane needs few of them. */
  const TILE_WIDTH = 512;
  /** Tiles kept beyond each edge, so scrolling never waits on a fetch. */
  const OVERSCAN = 2;

  let lane = $state<HTMLDivElement | null>(null);
  let strip = $state<HTMLDivElement | null>(null);
  let laneWidth = $state(1200);
  let ready = $state(false);
  let totalFrames = $state(0);
  /** Which generation of this deck's content the tiles belong to. */
  let epoch = $state(0);
  let mixOut = $state<MixOutInfo | null>(null);

  // Interpolation state. Updated from snapshots, read every animation frame.
  let anchorFrame = 0;
  let anchorTime = 0;
  let framesPerSecond = 0;

  $effect(() => {
    // Touch both so this re-runs whenever the deck's content changes: a new
    // track changes the length, and analysis finishing changes the grid without
    // touching the length at all.
    deck.length_frames;
    deck.analysis;
    void waveformInfo(deck.number)
      .then((info) => {
        ready = info.ready;
        totalFrames = info.total_frames;
        epoch = info.epoch;
        // `?? null` rather than `info.mix_out`: an older answer with no such
        // field would otherwise leave the previous record's band on screen.
        mixOut = info.mix_out ?? null;
      })
      // `ready` stays false, which is the "no tiles yet" state this component
      // already draws and already explains. Deliberately quiet: this re-runs
      // on every load and every analysis, and a deck that failed once is
      // asked again a moment later.
      .catch(() => {});
  });

  $effect(() => {
    // Snapshot arrived: re-anchor the interpolation.
    anchorFrame = deck.position_frames;
    anchorTime = performance.now();
    framesPerSecond = playbackFramesPerSecond(deck);
  });

  const tileSpanFrames = $derived(TILE_WIDTH * framesPerPixel);

  /**
   * Index of the leftmost tile to keep mounted.
   *
   * Quantised to the tile grid on purpose. Position changes 60 times a second,
   * but the *set of tiles* only changes when the playhead crosses a tile
   * boundary — every few seconds. Deriving the list straight from position
   * would recompute it on every snapshot and churn the DOM at frame rate, which
   * is exactly the per-frame JavaScript work this design exists to avoid.
   */
  const firstTile = $derived(
    Math.floor((deck.position_frames - (laneWidth * framesPerPixel) / 2) / tileSpanFrames) -
      OVERSCAN,
  );

  /** Which tiles cover the visible span. Changes only on a boundary crossing. */
  const visibleTiles = $derived.by(() => {
    if (!ready || totalFrames === 0) return [];

    const count = Math.ceil(laneWidth / TILE_WIDTH) + OVERSCAN * 2 + 1;
    return Array.from({ length: count }, (_, i) => {
      const index = firstTile + i;
      const startFrame = index * tileSpanFrames;
      return {
        key: index,
        startFrame,
        url: tileUrl(
          deck.number,
          TILE_WIDTH,
          height,
          startFrame,
          framesPerPixel,
          theme.resolved,
          epoch,
        ),
      };
    }).filter((t) => t.startFrame + tileSpanFrames > 0 && t.startFrame < totalFrames);
  });

  /**
   * Cue markers and the loop band, positioned inside the scrolling strip.
   *
   * Deliberately *not* rasterised into the tiles, unlike the beat grid — and
   * the difference is not inconsistency. Beat lines number in the hundreds and
   * must align pixel-exactly with the audio under them, so they have to be
   * drawn in the same pass as the waveform. There are at most eight cues and
   * one loop; they sit inside the strip element, so the same transform that
   * scrolls the waveform carries them for free, and changing one costs no tile
   * re-render and no cache invalidation.
   */
  const markers = $derived(
    deck.hot_cues
      .map((frame, index) => ({ slot: index + 1, frame }))
      .filter((c): c is { slot: number; frame: number } => c.frame != null)
      .map((c) => ({ slot: c.slot, left: c.frame / framesPerPixel })),
  );

  const loopBand = $derived.by(() => {
    const region = deck.active_loop;
    if (!region) return null;
    const left = region.start_frames / framesPerPixel;
    const width = (region.end_frames - region.start_frames) / framesPerPixel;
    // Sub-pixel loops exist — a sixteenth of a beat zoomed out is well under
    // one — and a zero-width band is invisible rather than wrong. Floor it to a
    // hairline so the loop is still locatable.
    return { left, width: Math.max(width, 2) };
  });

  onMount(() => {
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) laneWidth = entry.contentRect.width;
    });
    if (lane) observer.observe(lane);

    let frame = 0;
    /* See `Overview.svelte`: an unchanged transform must not be written, and a
       whole-pixel offset is what a sub-pixel one would be rasterised to. */
    let written = "";
    const tick = () => {
      if (strip) {
        // Interpolate forward from the last snapshot. Without this the strip
        // only moves when a snapshot lands, which reads as judder even though
        // the audio is perfectly smooth.
        const elapsed = (performance.now() - anchorTime) / 1000;
        const frameNow = anchorFrame + framesPerSecond * elapsed;
        const offset = laneWidth / 2 - frameNow / framesPerPixel;
        // translate3d, not left/top: this is the property compositors handle
        // without a layout or paint pass.
        const next = `translate3d(${Math.round(offset)}px, 0, 0)`;
        if (next !== written) {
          strip.style.transform = next;
          written = next;
        }
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  });
</script>

<div class="lane" bind:this={lane} style:height="{height}px">
  {#if ready}
    <div class="strip" bind:this={strip}>
      <!--
        §25's runway layer: how much record is left.

        Its own colour meaning, not a second use of one that already means
        something — see `dj_render::layer`, where §57's rule is enforced over
        the whole set rather than remembered one layer at a time. Drawn behind
        everything else because it is context: the thing you notice without
        looking at it, which is what "how long have I got" should be.
      -->
      {#if runway}
        <div
          class="runway"
          data-layer="runway"
          style:left="{runway.left}px"
          style:width="{runway.width}px"
        ></div>
      {/if}
      <!--
        §25's uncertainty layer. Hatched, and drawn under the record it is
        about rather than over it: the point is that everything above the line
        is approximate, and a layer that obscured the waveform to say so would
        be taking away the evidence a DJ needs to fix the grid.

        It spans the record and scrolls with it, so it ends where the record
        does — the claim is about this file, not about the lane.
      -->
      {#if gridIsAGuess}
        <div
          class="unsure"
          data-layer="confidence"
          style:width="{totalFrames / framesPerPixel}px"
          title="The beat grid here is an estimate — {(deck.grid_confidence * 100).toFixed(
            0,
          )}% confidence. Every beat line, phrase marker and mix point drawn on it is approximate. Tap or nudge the grid to fix it."
        ></div>
      {/if}
      <!--
        §25's mix-out layer: the stretch this record can be left in.

        Inside the runway on purpose — that is where it belongs and the two
        say different things about the same end of the record. The runway is
        *how long have I got*; this is *where do I go*. So the runway is a
        gradient with no edges and this is a band with two, because a DJ reads
        an edge as a decision and a wash as a condition.
      -->
      {#if mixOutBand}
        <div
          class="mix-out"
          class:on-phrase={mixOutBand.onPhrase}
          data-layer="mix-out"
          style:left="{mixOutBand.left}px"
          style:width="{mixOutBand.width}px"
          title={mixOutBand.onPhrase
            ? "Mix out anywhere in here and any transition djmanzo would propose still fits. It opens on a phrase."
            : "Mix out anywhere in here and any transition djmanzo would propose still fits. No phrase structure, so it opens on a beat."}
        >
          <span class="mix-out-flag">mix out</span>
        </div>
      {/if}
      <!--
        And the seam as a *region* rather than two lines. The marks say where
        it starts and ends; this says what it covers, which is the question a
        DJ actually asks of a mix point.
      -->
      {#if seamBand}
        <div
          class="seam-band"
          data-layer="seam"
          style:left="{seamBand.left}px"
          style:width="{seamBand.width}px"
        ></div>
      {/if}
      {#if loopBand}
        <div
          class="loop-band"
          data-layer="loop"
          style:left="{loopBand.left}px"
          style:width="{loopBand.width}px"
        ></div>
      {/if}
      {#each markers as marker (marker.slot)}
        <div class="cue-marker" data-layer="cues" style:left="{marker.left}px">
          <span class="cue-flag">{marker.slot}</span>
        </div>
      {/each}
      {#each marks as mark (mark.label)}
        {#if mark.draggable}
          <!--
            A real handle: pointer events, a grab cursor, and a hit area wider
            than the two-pixel line, because a two-pixel target in a dark booth
            is a target nobody hits. It is a slider rather than a decorated
            div, so the keyboard reaches it — §26 asks for the DJ to grab the
            thing they are thinking about, and a mouse is not the only hand.
          -->
          <div
            class="mark grabbable"
            data-layer="seam"
            class:dragging={dragging?.label === mark.label}
            style:left="{(dragging?.label === mark.label
              ? dragging.frame
              : mark.frame) / framesPerPixel}px"
            title="{mark.label} — drag, or use the arrow keys"
            role="slider"
            tabindex="0"
            aria-label="{mark.label}, drag to move"
            aria-valuenow={Math.round(mark.frame)}
            aria-valuemin={0}
            aria-valuemax={Math.round(totalFrames)}
            onpointerdown={(e) => grab(e, mark)}
            onkeydown={(e) => nudge(e, mark)}
          >
            <span class="mark-flag">{mark.label}</span>
          </div>
        {:else}
          <div
            class="mark"
            data-layer="seam"
            style:left="{mark.frame / framesPerPixel}px"
            title={mark.label}
          >
            <span class="mark-flag">{mark.label}</span>
          </div>
        {/if}
      {/each}
      {#each visibleTiles as tile (tile.key)}
        <img
          class="tile"
          src={tile.url}
          alt=""
          decoding="async"
          loading="eager"
          width={TILE_WIDTH}
          {height}
          style:left="{tile.startFrame / framesPerPixel}px"
        />
      {/each}
    </div>
  {:else if deck.loaded}
    <p class="pending">analysing…</p>
  {/if}
  <div class="playhead" aria-hidden="true"></div>
</div>

<style>
  .lane {
    position: relative;
    overflow: hidden;
    background: var(--panel-raised);
    border-radius: 6px;
    /* Promote to its own layer so the strip's transform never forces the rest
       of the interface to repaint. */
    contain: strict;
  }

  .strip {
    position: absolute;
    inset: 0;
    will-change: transform;
  }

  /*
    Under the tiles, so the waveform stays readable through it. A loop band
    that covered the audio would hide exactly the part you are looping.
  */
  /*
    The layer order, in one place.

    Written out because it was wrong and nothing noticed: the runway had
    `z-index: 0` while the tiles have `z-index: auto` and come *later* in the
    DOM, so it painted underneath an opaque waveform and was invisible in the
    application. The browser test passed anyway — Playwright's `toBeVisible`
    asks whether an element has a box, not whether anything can be seen of it.

    Bottom to top: the record, then washes over it, then things at a position,
    then the playhead. Each step is something drawn *about* the one below it,
    which is the order §25's layers actually stack in.

      0  .tile                            the record
      1  .runway .mix-out .unsure         washes and bands over the whole of it
      2  .loop-band .seam-band            regions somebody chose
      3  .cue-marker .mark                a single position
      4  .playhead                        now
  */
  .tile {
    z-index: 0;
  }

  .loop-band {
    position: absolute;
    top: 0;
    bottom: 0;
    background: var(--accent-2);
    opacity: 0.16;
    pointer-events: none;
    z-index: 2;
  }

  .cue-marker {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--accent);
    pointer-events: none;
    z-index: 3;
  }

  .cue-flag {
    position: absolute;
    top: 0;
    left: 0;
    padding: 0 0.25rem;
    font-size: 0.65rem;
    font-weight: 700;
    line-height: 1.3;
    color: var(--on-accent);
    background: var(--accent);
    border-radius: 0 3px 3px 0;
  }

  /* Distinct from a cue marker on purpose: a cue is a place the DJ put, and
     these are places djmanzo is proposing. Dashed, and labelled with a word
     rather than a number. */
  /*
    The runway. A wash rather than a line, because it is an amount of time
    rather than a moment — and behind everything, because it is the thing you
    should notice without looking for it.
  */
  .runway {
    position: absolute;
    top: 0;
    bottom: 0;
    background: linear-gradient(
      to right,
      transparent,
      color-mix(in srgb, var(--danger) 22%, transparent)
    );
    pointer-events: none;
    z-index: 1;
  }

  /*
    Where the record can be left. A band with two edges, deliberately unlike
    the runway's edgeless wash underneath it: an edge reads as a decision and a
    wash as a condition, and these two are one of each about the same end of
    the same record.

    Solid on the left, dashed on the right. The left edge is where the window
    opens and is the one worth acting on; the right is the last moment, and a
    dashed line is how every other "this is the limit" mark in the interface is
    drawn.
  */
  .mix-out {
    position: absolute;
    top: 0;
    bottom: 0;
    background: color-mix(in srgb, var(--ok, #6a9955) 10%, transparent);
    border-left: 2px solid color-mix(in srgb, var(--ok, #6a9955) 55%, transparent);
    border-right: 2px dashed color-mix(in srgb, var(--ok, #6a9955) 40%, transparent);
    pointer-events: none;
    z-index: 1;
  }

  /*
    A window that opens on a phrase is a stronger claim than one that opens on
    a beat, so it is drawn as one. Dimmed rather than hidden: "we are guessing
    at the structure" is still worth more than nothing, and hiding it would
    leave the record with no answer at all.
  */
  .mix-out:not(.on-phrase) {
    border-left-style: dashed;
    opacity: 0.6;
  }

  .mix-out-flag {
    position: absolute;
    top: 0;
    left: 0;
    padding: 0 0.25rem;
    font-size: 0.6rem;
    line-height: 1.3;
    color: var(--ok, #6a9955);
    background: var(--panel);
    border-radius: 0 3px 3px 0;
    white-space: nowrap;
  }

  /*
    The grid is a guess. Hatched, because a hatch is what every drawing
    convention uses for provisional, and along the bottom edge so it never
    covers the waveform a DJ needs in order to *fix* the grid.
  */
  .unsure {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 3px;
    background: repeating-linear-gradient(
      135deg,
      color-mix(in srgb, var(--warn) 40%, transparent) 0 2px,
      transparent 2px 5px
    );
    pointer-events: none;
    z-index: 1;
  }

  /* What the mix covers, between the two marks that bound it. */
  .seam-band {
    position: absolute;
    top: 0;
    bottom: 0;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    border-top: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
    pointer-events: none;
    z-index: 2;
  }

  .mark {
    position: absolute;
    top: 0;
    bottom: 0;
    border-left: 2px dashed var(--warn);
    pointer-events: none;
    z-index: 3;
  }

  /*
    A handle rather than a line. The hit area is eleven pixels wide and hangs
    off the left of the line so the line itself stays where it says it is: a
    marker that moves to where you can grab it is a marker that lies about the
    thing it marks.
  */
  .mark.grabbable {
    pointer-events: auto;
    cursor: grab;
    width: 11px;
    margin-left: -5px;
    border-left-style: solid;
    border-left-color: var(--accent);
    touch-action: none;
  }

  .mark.grabbable .mark-flag {
    color: var(--accent);
  }

  .mark.grabbable:focus-visible {
    outline: 2px solid var(--accent);
  }

  .mark.dragging {
    cursor: grabbing;
    border-left-style: solid;
  }

  .mark-flag {
    position: absolute;
    bottom: 0;
    left: 0;
    padding: 0 0.25rem;
    font-size: 0.6rem;
    line-height: 1.3;
    color: var(--warn);
    background: var(--panel);
    border-radius: 0 3px 3px 0;
    white-space: nowrap;
  }

  .tile {
    position: absolute;
    top: 0;
    /* Never let a tile be resampled: the renderer already produced exactly the
       right pixels, and scaling would blur them and cost a paint. */
    image-rendering: pixelated;
  }

  .playhead {
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--text);
    opacity: 0.85;
    z-index: 4;
  }

  .pending {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    margin: 0;
    font-size: 0.75em;
    color: var(--text-dim);
  }
</style>
