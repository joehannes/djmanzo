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
  /**
   * Where the drums are out, and where they come back, in frames.
   *
   * This is the view that most wants them. The comment at the top of this file
   * has said since it was written that the overview answers "where is the
   * breakdown"; until now it could not, because nothing had worked one out.
   */
  let breakdowns = $state<{ start_frame: number; end_frame: number }[]>([]);
  let drops = $state<number[]>([]);
  /**
   * §25's energy trajectory: how hard the drums drive, beat by beat, with 1.0
   * the record's own normal, and where on the record it starts.
   *
   * This view rather than the scrolling lane, deliberately. At the lane's zoom
   * about ten beats are visible, over which a trajectory is a straight line;
   * the arc of a record is a thing you see whole or not at all.
   */
  let drive = $state<number[]>([]);
  let driveFrom = $state(0);
  let driveBeatFrames = $state(0);

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
        breakdowns = info.breakdowns;
        drops = info.drops;
        drive = info.drive;
        driveFrom = info.drive_from_frame;
        driveBeatFrames = info.drive_beat_frames;
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

  /**
   * The breakdowns as a band along the top edge, and the drops as ticks at
   * their far ends.
   *
   * At the edge rather than washed over the waveform: the waveform's colour is
   * the spectral balance, and tinting it would be the overload §57 forbids.
   * The two are drawn as one shape — a dim band ending in a bright tick — so
   * "quiet until *here*" is legible without either of them claiming a hue.
   */
  const breakdownBands = $derived(
    breakdowns.map((section) => ({
      key: section.start_frame,
      left: fraction(section.start_frame) * 100,
      width: (fraction(section.end_frame) - fraction(section.start_frame)) * 100,
    })),
  );

  const dropMarks = $derived(drops.map((frame) => ({ frame, left: fraction(frame) * 100 })));

  /**
   * The trajectory as a polyline over the box, in percent of width and height.
   *
   * Drawn here rather than rasterised into the tile, unlike the beat grid. The
   * grid is in the tile because a beat line a pixel off the transient it marks
   * is worse than no line; a trajectory is a smooth quantity over tens of beats
   * and a pixel means nothing to it. Every other layer this component draws —
   * the breakdown band, the drops, the loop, the cues — is already DOM for the
   * same reason.
   *
   * Clipped at twice the record's normal. A single beat far above the
   * ninetieth percentile would otherwise flatten everything else against the
   * bottom of a thirty-pixel box, and what this is for is the *shape*.
   */
  const CEILING = 2;

  const trajectory = $derived.by(() => {
    if (drive.length < 2 || totalFrames <= 0 || driveBeatFrames <= 0) return null;
    return drive
      .map((level, index) => {
        const x = fraction(driveFrom + index * driveBeatFrames) * 100;
        const y = (1 - Math.min(1, Math.max(0, level / CEILING))) * 100;
        return `${x.toFixed(3)},${y.toFixed(2)}`;
      })
      .join(" ");
  });

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
    {#each breakdownBands as band (band.key)}
      <div
        class="breakdown"
        style:left="{band.left}%"
        style:width="{band.width}%"
        title="The drums are out here"
      ></div>
    {/each}
    {#each dropMarks as drop (drop.frame)}
      <div class="drop" style:left="{drop.left}%" title="The drums come back here"></div>
    {/each}
    <!--
      §25's energy trajectory. A curve is a shape nothing else on this view
      has, which is what lets it be neutral: the bands, the drops and the
      loop each already carry a meaning in their colour, and a sixth hue is one
      nobody could learn — §57. It is the same measurement the breakdown band
      is thresholded from, so the band reads as a part of the line rather than
      as a second opinion beside it.
    -->
    {#if trajectory}
      <svg
        class="drive"
        viewBox="0 0 100 100"
        preserveAspectRatio="none"
        aria-hidden="true"
        focusable="false"
      >
        <polyline points={trajectory} vector-effect="non-scaling-stroke" />
      </svg>
    {/if}
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

  /*
    The trajectory. Stretched to the box with `preserveAspectRatio: none` so the
    curve's coordinates can be percentages — the same coordinate space every
    other mark on this view uses — and `non-scaling-stroke` so the line stays a
    line rather than being stretched with it.

    Over the waveform rather than at an edge, unlike the breakdown band, and
    that is not the inconsistency it looks like: a band over the audio is a
    *tint*, which changes the colour the waveform is using to say something
    else, and a one-pixel line is not. §57 is about a colour carrying two
    meanings, not about ink.
  */
  .drive {
    position: absolute;
    inset: 0;
    inline-size: 100%;
    block-size: 100%;
    pointer-events: none;
  }

  .drive polyline {
    fill: none;
    stroke: var(--text);
    stroke-width: 1;
    stroke-opacity: 0.5;
    stroke-linejoin: round;
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
