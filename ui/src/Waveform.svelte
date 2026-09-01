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
  }: { deck: DeckState; height?: number; framesPerPixel?: number } = $props();

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
