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
  import { playbackFramesPerSecond, tileUrl, waveformInfo, type DeckState } from "./api";
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
      {#if loopBand}
        <div
          class="loop-band"
          style:left="{loopBand.left}px"
          style:width="{loopBand.width}px"
        ></div>
      {/if}
      {#each markers as marker (marker.slot)}
        <div class="cue-marker" style:left="{marker.left}px">
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
  .loop-band {
    position: absolute;
    top: 0;
    bottom: 0;
    background: var(--accent-2);
    opacity: 0.16;
    pointer-events: none;
  }

  .cue-marker {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--accent);
    pointer-events: none;
    z-index: 2;
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
  .mark {
    position: absolute;
    top: 0;
    bottom: 0;
    border-left: 2px dashed var(--warn);
    pointer-events: none;
    z-index: 2;
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
