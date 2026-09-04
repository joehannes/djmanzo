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
    onMarkMoved,
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
    marks?: { frame: number; label: string }[];
    /**
     * Called when one of those marks is dragged somewhere else, with the frame
     * it was dropped on.
     *
     * Its presence is what makes the marks draggable: a lane with no listener
     * draws them and leaves them alone. §26 asks for the mix point to be
     * *grabbed* rather than typed into a panel, and the frame under a pointer
     * is the whole of what this side can honestly report — which beat that is
     * belongs to djmanzo, which has the grid.
     */
    onMarkMoved?: (label: string, frame: number) => void;
  } = $props();

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
  /**
   * Where the drums are out, and where they come back — §25's breakdown and
   * drop layers, in frames, straight from Rust.
   *
   * Held here rather than derived from the deck's snapshot because they belong
   * to the *record*, not to what the deck is doing: they change when a track
   * loads and when its analysis lands, and never in between. The same effect
   * that fetches the length fetches them, for the same reason.
   */
  let breakdowns = $state<{ start_frame: number; end_frame: number }[]>([]);
  let drops = $state<number[]>([]);

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
        breakdowns = info.breakdowns;
        drops = info.drops;
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

  /**
   * The mark being dragged, and where it has got to.
   *
   * Held here rather than pushed to Rust on every pointer move: a drag is
   * sixty events a second and the transition is re-scored on each change, so
   * the line follows the finger locally and djmanzo hears about it once, on
   * release. What is drawn in between is the DJ's own hand, which is the one
   * thing that does not need confirming.
   */
  let dragging = $state<{ label: string; frame: number } | null>(null);

  /**
   * Which frame of the record is under a screen position.
   *
   * From the strip's own left edge rather than from the playhead, so the mark
   * stays under the finger while the record keeps playing beneath it. A DJ
   * dragging a mix point is pointing at a place in the music, not at a place
   * on the screen.
   */
  function frameAt(clientX: number): number {
    if (!strip) return 0;
    const left = strip.getBoundingClientRect().left;
    return Math.max(0, (clientX - left) * framesPerPixel);
  }

  /**
   * Start a drag.
   *
   * The move and release listeners are added to the element rather than
   * declared on it, which is the pattern the crossfader and the jog wheel
   * already use in this interface. One way of doing a drag, not two.
   */
  function grab(event: PointerEvent, label: string, frame: number) {
    if (!onMarkMoved) return;
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    dragging = { label, frame };
    // The lane sits in a scrolling dock; a drag that also scrolled it would
    // move the thing being dragged.
    event.preventDefault();
    event.stopPropagation();

    const move = (next: PointerEvent) => {
      dragging = { label, frame: frameAt(next.clientX) };
    };
    const done = (next: PointerEvent) => {
      handle.releasePointerCapture(next.pointerId);
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", done);
      handle.removeEventListener("pointercancel", cancel);
      dragging = null;
      onMarkMoved?.(label, frameAt(next.clientX));
    };
    const cancel = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", done);
      handle.removeEventListener("pointercancel", cancel);
      dragging = null;
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", done);
    handle.addEventListener("pointercancel", cancel);
  }

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

  /**
   * The breakdowns and drops, as pixels inside the strip.
   *
   * A band along the top edge rather than a wash over the waveform. A wash
   * would tint the bands underneath it, and the bands *are* the colour that
   * says what is playing — §57 forbids exactly that, and `dj_render`'s palette
   * test exists because it was broken once already. A band at the edge cannot
   * recolour anything.
   */
  const breakdownBands = $derived(
    breakdowns.map((section) => ({
      key: section.start_frame,
      left: section.start_frame / framesPerPixel,
      // Never thinner than a hairline, for the same reason the loop band is
      // not: a short breakdown fully zoomed out is under a pixel wide.
      width: Math.max((section.end_frame - section.start_frame) / framesPerPixel, 2),
    })),
  );

  const dropMarks = $derived(drops.map((frame) => ({ frame, left: frame / framesPerPixel })));

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
        Under the cues and the mix point, over nothing: these say what the
        record does, and everything else on this lane says what the DJ or
        djmanzo is doing about it.
      -->
      {#each breakdownBands as band (band.key)}
        <div
          class="breakdown"
          style:left="{band.left}px"
          style:width="{band.width}px"
          title="The drums are out here"
        ></div>
      {/each}
      {#each dropMarks as drop (drop.frame)}
        <div class="drop" style:left="{drop.left}px" title="The drums come back here"></div>
      {/each}
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
        <!--
          A separator rather than a slider, and deliberately not focusable.
          The keyboard path to this is the pair view's own move and length
          controls, which is the right one: a mix point is a beat, and beats
          are what those buttons step through. A slider role here would
          promise arrow-key nudging of a pixel position, which is not the
          same thing and not what §26 asks for.
        -->
        <div
          class="mark"
          class:grabbable={!!onMarkMoved}
          class:held={dragging?.label === mark.label}
          role="separator"
          aria-label={mark.label}
          style:left="{(dragging?.label === mark.label ? dragging.frame : mark.frame) /
            framesPerPixel}px"
          title={onMarkMoved ? `${mark.label} — drag to move it` : mark.label}
          onpointerdown={(e) => grab(e, mark.label, mark.frame)}
        >
          <span class="mark-flag">{mark.label}</span>
        </div>
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

  /*
    §25's breakdown layer. A band along the top edge, never a wash over the
    waveform: the waveform's colour *is* the spectral balance, and tinting it
    would be the overload §57 forbids -- `dj_render`'s palette test exists
    because that rule was broken once already.

    Neutral on purpose. This lane already spends indigo, teal and amber on the
    three bands and pink on the phrase markers; a sixth hue would be a colour
    nobody could learn. Grey saying "less" is what a breakdown is.
  */
  .breakdown {
    position: absolute;
    top: 0;
    height: 3px;
    background: var(--text-dim);
    opacity: 0.75;
    pointer-events: none;
  }

  /*
    The drop, drawn as the far end of the band it belongs to rather than as a
    line of its own. A dim band ending in a bright tick reads as "quiet until
    *here*", and the pairing carries the meaning without either half claiming a
    hue. Deliberately not full height: every full-height line on this lane is
    already something else -- a beat, a bar, a phrase, a cue, a mix point.
  */
  .drop {
    position: absolute;
    top: 0;
    height: 38%;
    width: 2px;
    background: var(--text);
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

  /* Only a lane that can do something about it invites the grab. */
  .mark.grabbable {
    pointer-events: auto;
    cursor: ew-resize;
    /* A two-pixel line is a hard thing to hit with a mouse and an impossible
       one on a trackpad in a dark booth. The line stays two pixels; what
       widens is the part that answers to a pointer. */
    padding-inline: 6px;
    margin-inline: -6px;
    touch-action: none;
  }

  .mark.held {
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
