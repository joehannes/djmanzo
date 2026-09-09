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
  import {
    playbackFramesPerSecond,
    tileUrl,
    waveformInfo,
    type DeckState,
    type MixOutInfo,
  } from "./api";
  import { theme } from "./theme.svelte";

  let { deck, height = 34 }: { deck: DeckState; height?: number } = $props();

  let box = $state<HTMLDivElement | null>(null);
  let playhead = $state<HTMLDivElement | null>(null);
  let width = $state(600);
  let totalFrames = $state(0);
  let epoch = $state(0);
  let ready = $state(false);
  let mixOut = $state<MixOutInfo | null>(null);

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
    void waveformInfo(deck.number)
      .then((info) => {
        ready = info.ready;
        totalFrames = info.total_frames;
        epoch = info.epoch;
        mixOut = info.mix_out ?? null;
      })
      // `ready` stays false, which is the "no tiles yet" state this component
      // already draws and already explains. Deliberately quiet: this re-runs
      // on every load and every analysis, and a deck that failed once is
      // asked again a moment later.
      .catch(() => {});
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

  /**
   * §25's mix-out layer: where this record could be left.
   *
   * Here as well as in the scrolling lane, and this is the view it is really
   * for. The lane runs at a few hundred frames per pixel — two seconds of
   * record across a deck — so a band twenty beats from the end is off screen
   * until you are already inside it, which answers "what is about to happen"
   * far too late to be worth anything. Over the whole track it is visible from
   * the moment the record loads, which is the question this view exists to
   * answer.
   */
  const mixOutBand = $derived.by(() => {
    if (!mixOut || totalFrames <= 0) return null;
    const left = fraction(mixOut.opens_frame) * 100;
    const right = fraction(mixOut.closes_frame) * 100;
    // Floored for the same reason the loop band is: on a ten-minute record the
    // window is a couple of percent, and a faithful width there is a hairline
    // nobody can aim at. This view is for finding your place, not for
    // measuring — the lane draws it to scale.
    return right > left
      ? { left, width: Math.max(right - left, 0.8), onPhrase: mixOut.on_phrase }
      : null;
  });

  onMount(() => {
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) width = entry.contentRect.width;
    });
    if (box) observer.observe(box);

    let frame = 0;
    /*
      The last value written, so an unchanged one is not written again.

      This matters more than it looks. Measured in the running application, a
      *paused* deck costs about 1.5 ms a frame and a playing one about 26 ms —
      and the difference is not that the loops stop, because they do not. It is
      that a paused deck computes the same transform every frame, the browser
      sees no change, and nothing repaints. The cost is per element whose
      transform actually moves.

      An overview playhead crosses a whole track in minutes, so it moves a
      fraction of a pixel per frame. Rounded to the pixel it can actually be
      drawn at, the great majority of frames write nothing at all — and the
      remaining ones land on exactly the pixel a sub-pixel value would have been
      rasterised to anyway.
    */
    let written = "";
    const tick = () => {
      if (playhead && totalFrames > 0) {
        const elapsed = (performance.now() - anchorTime) / 1000;
        const frameNow = anchorFrame + framesPerSecond * elapsed;
        // translate3d for the same reason the lane uses it: the compositor can
        // move this without a layout or a paint.
        const next = `translate3d(${Math.round(fraction(frameNow) * width)}px, 0, 0)`;
        if (next !== written) {
          playhead.style.transform = next;
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

<div class="overview" bind:this={box} style:height="{height}px">
  {#if url}
    <!--
      Stamped with the layer each one is, like the lane's. Without it the
      browser test that reads every `data-layer` off a rendered page could not
      see this view at all — and the loop and the cues drawn here are the same
      two layers of §25's twenty, drawn in a second place.
    -->
    {#if mixOutBand}
      <div
        class="mix-out"
        class:on-phrase={mixOutBand.onPhrase}
        data-layer="mix-out"
        style:left="{mixOutBand.left}%"
        style:width="{mixOutBand.width}%"
        title={mixOutBand.onPhrase
          ? "Mix out anywhere in here and any transition djmanzo would propose still fits. It opens on a phrase."
          : "Mix out anywhere in here and any transition djmanzo would propose still fits. No phrase structure, so it opens on a beat."}
      ></div>
    {/if}
    {#if loopBand}
      <div
        class="loop-band"
        data-layer="loop"
        style:left="{loopBand.left}%"
        style:width="{loopBand.width}%"
      ></div>
    {/if}
    <img class="whole" src={url} alt="" width={tileWidth} {height} draggable="false" />
    {#each markers as marker (marker.slot)}
      <div class="cue" data-layer="cues" style:left="{marker.left}%"></div>
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

  /*
    Where the record can be left, over the whole of it. The same two edges as
    the lane's — solid where the window opens, dashed where it closes — but no
    label: this strip is thirty-four pixels tall and a word in it would cover
    the record rather than describe it.
  */
  .mix-out {
    position: absolute;
    top: 0;
    bottom: 0;
    background: color-mix(in srgb, var(--ok, #6a9955) 22%, transparent);
    border-left: 2px solid color-mix(in srgb, var(--ok, #6a9955) 75%, transparent);
    border-right: 2px dashed color-mix(in srgb, var(--ok, #6a9955) 55%, transparent);
    pointer-events: none;
  }

  .mix-out:not(.on-phrase) {
    border-left-style: dashed;
    opacity: 0.6;
  }
</style>
