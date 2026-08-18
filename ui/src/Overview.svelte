<script lang="ts">
  /**
   * The whole track at a glance.
   *
   * A DJ needs two views of a waveform at once and they answer different
   * questions. The scrolling lane answers "what is about to happen" at a few
   * hundred frames per pixel; this answers "where am I in the track, and where
   * is the breakdown" at tens of thousands. Neither substitutes for the other.
   *
   * It costs almost nothing to build, because it is the *same* renderer at a
   * different zoom: one tile spanning the entire track. No new drawing code, no
   * second code path to keep in step with the first — and it inherits the beat
   * grid, the spectral colouring and the theme for free.
   */
  import { onMount } from "svelte";
  import { playbackFramesPerSecond, tileUrl, waveformInfo, type DeckState } from "./api";
  import { theme } from "./theme.svelte";

  let { deck, height = 34 }: { deck: DeckState; height?: number } = $props();

  let box = $state<HTMLDivElement | null>(null);
  let playhead = $state<HTMLDivElement | null>(null);
  let width = $state(600);
  let totalFrames = $state(0);
  let epoch = $state(0);
  let ready = $state(false);

  // Interpolation state, as in the scrolling lane: snapshots arrive at 60 Hz
  // but a frame landing between two of them must still move.
  let anchorFrame = 0;
  let anchorTime = 0;
  let framesPerSecond = 0;

  /**
   * Width rounded to a step, so dragging the window does not mint a new tile
   * per pixel.
   *
   * Tiles are cached by URL, and an overview tile spans the whole track, so a
   * continuous resize would otherwise render and cache several hundred of the
   * most expensive tiles in the application.
   */
  const QUANTUM = 32;
  const tileWidth = $derived(Math.max(QUANTUM, Math.round(width / QUANTUM) * QUANTUM));

  $effect(() => {
    // Re-run whenever the deck's content changes: a new track changes the
    // length, and analysis finishing changes the grid drawn into the tile.
    deck.length_frames;
    deck.analysis;
    void waveformInfo(deck.number).then((info) => {
      ready = info.ready;
      totalFrames = info.total_frames;
      epoch = info.epoch;
    });
  });

  $effect(() => {
    anchorFrame = deck.position_frames;
    anchorTime = performance.now();
    framesPerSecond = playbackFramesPerSecond(deck);
  });

  const url = $derived.by(() => {
    if (!ready || totalFrames === 0) return null;
    // One tile for the entire track: the zoom *is* the track length.
    const framesPerPixel = totalFrames / tileWidth;
    if (!(framesPerPixel > 0)) return null;
    return tileUrl(deck.number, tileWidth, height, 0, framesPerPixel, theme.resolved, epoch);
  });

  /** Fraction of the track a frame sits at, clamped so nothing escapes the box. */
  function fraction(frame: number): number {
    if (totalFrames <= 0) return 0;
    return Math.min(1, Math.max(0, frame / totalFrames));
  }

  const markers = $derived(
    deck.hot_cues
      .map((frame, index) => ({ slot: index + 1, frame }))
      .filter((c): c is { slot: number; frame: number } => c.frame != null)
      .map((c) => ({ slot: c.slot, left: fraction(c.frame) * 100 })),
  );

  const loopBand = $derived.by(() => {
    const region = deck.active_loop;
    if (!region || totalFrames <= 0) return null;
    const left = fraction(region.start_frames) * 100;
    const right = fraction(region.end_frames) * 100;
    // A four-beat loop is a fraction of a percent of a five-minute track, so a
    // faithful width would be invisible. Floored to something locatable —
    // the overview is for finding your place, not for measuring.
    return { left, width: Math.max(right - left, 0.4) };
  });

  onMount(() => {
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) width = entry.contentRect.width;
    });
    if (box) observer.observe(box);

    let frame = 0;
    const tick = () => {
      if (playhead && totalFrames > 0) {
        const elapsed = (performance.now() - anchorTime) / 1000;
        const frameNow = anchorFrame + framesPerSecond * elapsed;
        // translate3d for the same reason the lane uses it: the compositor can
        // move this without a layout or a paint.
        playhead.style.transform = `translate3d(${fraction(frameNow) * width}px, 0, 0)`;
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

<div class="overview" bind:this={box} style:height="{height}px">
  {#if url}
    {#if loopBand}
      <div
        class="loop-band"
        style:left="{loopBand.left}%"
        style:width="{loopBand.width}%"
      ></div>
    {/if}
    <img class="whole" src={url} alt="" width={tileWidth} {height} draggable="false" />
    {#each markers as marker (marker.slot)}
      <div class="cue" style:left="{marker.left}%"></div>
    {/each}
    <div class="playhead" bind:this={playhead}></div>
  {:else}
    <div class="empty"></div>
  {/if}
</div>

<style>
  .overview {
    position: relative;
    overflow: hidden;
    border-radius: 4px;
    background: var(--panel-raised);
  }

  .whole {
    display: block;
    width: 100%;
    height: 100%;
    /* The tile is rendered at a quantised width and stretched to the box, so a
       resize between quanta costs a scale rather than a re-render. */
    object-fit: fill;
  }

  .empty {
    width: 100%;
    height: 100%;
  }

  .playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 2px;
    background: var(--text);
    will-change: transform;
    pointer-events: none;
  }

  .cue {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--accent);
    pointer-events: none;
  }

  .loop-band {
    position: absolute;
    top: 0;
    bottom: 0;
    background: var(--accent-2);
    opacity: 0.25;
    pointer-events: none;
  }
</style>
