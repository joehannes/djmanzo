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
    onCueMoved,
    onLoopEdgeMoved,
    onPhraseMoved,
    onSavedLoopRecalled,
    ghost = null,
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
    /**
     * Called when one of the deck's own hot cues is dragged, with its slot and
     * the frame it was dropped on.
     *
     * A separate prop from {@link onMarkMoved} rather than one list of
     * draggable things, because they are not the same thing: a mark is
     * djmanzo's proposal and belongs to whoever put it there, and a cue is the
     * DJ's own and belongs to the deck. A lane can honestly offer one and not
     * the other — the pair view's outgoing lane draws a mix point and has no
     * business moving cues.
     *
     * Its presence is what makes the cues draggable, on the same terms.
     */
    onCueMoved?: (slot: number, frame: number) => void;
    /**
     * Called when one edge of the active loop is dragged, with which edge and
     * the frame it was dropped on.
     *
     * Its presence is what makes the edges grabbable, on the same terms as the
     * cues: a lane that cannot do anything about a loop draws it and leaves it
     * alone.
     */
    onLoopEdgeMoved?: (edge: "start" | "end", frame: number) => void;
    /**
     * Called when a phrase boundary is dragged, with the frame it was dropped
     * on.
     *
     * There is only one anchor, so every boundary is the same statement: any
     * of them moves all of them. Its presence is what makes them grabbable.
     */
    onPhraseMoved?: (frame: number) => void;
    /**
     * Called when a saved loop drawn on the lane is asked for, with its slot.
     *
     * §25's saved-loop layer and §26's answer to it in one: the regions are
     * there to be seen, and the thing to do with the one you can see is play
     * it. Its presence is what makes them clickable — a lane that cannot
     * recall a loop draws them and leaves them alone, on the same terms as
     * every other handle here.
     */
    onSavedLoopRecalled?: (slot: number) => void;
    /**
     * A second record drawn over this one, arriving at a place on this lane.
     *
     * §27's ghost: *if I bring this in here, this is what happens*. Not a
     * second lane and not a preview player — the same tiles the incoming
     * deck's own lane draws, laid over this one from the point the mix begins,
     * so a DJ can see the two records against each other before committing to
     * either.
     *
     * The caller does the music theory. This component knows nothing about
     * records, which is why the ghost's zoom arrives already beat-matched
     * rather than being worked out from two tempos here.
     */
    ghost?: {
      /** The deck whose record is coming in. */
      deck: DeckState;
      /** Where on *this* lane it begins, in this record's frames. */
      at: number;
      /** Where in its own record it begins, in its frames. */
      from: number;
      /** Its zoom, beat-matched to this lane's by whoever passed it. */
      framesPerPixel: number;
    } | null;
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
  /**
   * Where each phrase starts, from Rust.
   *
   * The lines are drawn into the tiles; these are grab targets over them, and
   * nothing here paints. Two things drawing the same line would be two answers
   * about where beat 96 is — see `phrases_in_frames`.
   */
  let phrases = $state<number[]>([]);

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
        phrases = info.phrases;
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
  function grab(
    event: PointerEvent,
    label: string,
    frame: number,
    dropped: (frame: number) => void,
  ) {
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
      dropped(frameAt(next.clientX));
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
   * The ghost's own readiness, from the incoming deck.
   *
   * A second fetch rather than a second prop, for the reason the first one is
   * a fetch: the tile generation is djmanzo's to say, and a caller passing a
   * stale one would hand the webview a cache key for tiles that no longer
   * describe the record.
   */
  let ghostReady = $state(false);
  let ghostEpoch = $state(0);
  let ghostTotal = $state(0);
  /**
   * The incoming record's own structure — §27's "where the drop occurs".
   *
   * It arrives in the same answer the tiles' generation does, so asking for it
   * costs nothing: the fetch was already being made to find out whether the
   * ghost could be drawn at all.
   */
  let ghostBreakdowns = $state<{ start_frame: number; end_frame: number }[]>([]);
  let ghostDrops = $state<number[]>([]);

  $effect(() => {
    const number = ghost?.deck.number;
    if (number === undefined) {
      ghostReady = false;
      return;
    }
    // The same two touches the outgoing side makes: a new record changes the
    // length, and analysis finishing changes the tiles without touching it.
    ghost?.deck.length_frames;
    ghost?.deck.analysis;
    void waveformInfo(number)
      .then((info) => {
        ghostReady = info.ready;
        ghostEpoch = info.epoch;
        ghostTotal = info.total_frames;
        ghostBreakdowns = info.breakdowns;
        ghostDrops = info.drops;
      })
      .catch(() => {
        ghostReady = false;
      });
  });

  /**
   * The incoming record's tiles, over this lane, from where the mix begins.
   *
   * Windowed the same way the outgoing tiles are, in the ghost's own frames:
   * the lane shows a span of *this* record, and the ghost's frame under any
   * point of it follows from the beat-matched zoom. Drawing the whole incoming
   * record and letting the browser clip it would mount hundreds of tiles for a
   * five-minute track.
   */
  const ghostTiles = $derived.by(() => {
    if (!ghost || !ghostReady || ghostTotal === 0) return [];

    const span = TILE_WIDTH * ghost.framesPerPixel;
    // How many of the ghost's frames pass for one of this lane's.
    const scale = ghost.framesPerPixel / framesPerPixel;
    const leftFrame = deck.position_frames - (laneWidth * framesPerPixel) / 2;
    const firstGhostFrame = ghost.from + (leftFrame - ghost.at) * scale;
    const first = Math.floor(firstGhostFrame / span) - OVERSCAN;

    const count = Math.ceil(laneWidth / TILE_WIDTH) + OVERSCAN * 2 + 1;
    return Array.from({ length: count }, (_, i) => {
      const index = first + i;
      const startFrame = index * span;
      return {
        key: index,
        startFrame,
        url: tileUrl(
          ghost.deck.number,
          TILE_WIDTH,
          height,
          startFrame,
          ghost.framesPerPixel,
          theme.resolved,
          ghostEpoch,
        ),
      };
    }).filter((t) => t.startFrame + span > ghost.from && t.startFrame < ghostTotal);
  });

  /**
   * Where the ghost sits on this lane, and how far it runs.
   *
   * From the mix point to the end of the incoming record: what arrives is the
   * rest of that record, and stopping the drawing at the end of the transition
   * would say the new track ends when the blend does.
   */
  const ghostBox = $derived.by(() => {
    if (!ghost || !ghostReady || ghostTotal <= ghost.from) return null;
    return {
      left: ghost.at / framesPerPixel,
      width: (ghostTotal - ghost.from) / ghost.framesPerPixel,
    };
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
      // The frame as well as the pixel: a dragged marker is drawn from the
      // hand's own position until djmanzo answers, and that is a frame.
      .map((c) => ({ slot: c.slot, frame: c.frame, left: c.frame / framesPerPixel })),
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
    // While an edge is under a hand, the band follows the hand — the one thing
    // that does not need confirming is where the finger is. djmanzo hears
    // about it once, on release, like every other drag on this lane.
    const start = dragging?.label === "loop start" ? dragging.frame : region.start_frames;
    const end = dragging?.label === "loop end" ? dragging.frame : region.end_frames;
    const left = start / framesPerPixel;
    const width = (end - start) / framesPerPixel;
    // Sub-pixel loops exist — a sixteenth of a beat zoomed out is well under
    // one — and a zero-width band is invisible rather than wrong. Floor it to a
    // hairline so the loop is still locatable.
    return { left, width: Math.max(width, 2), start, end };
  });

  /**
   * The loops kept with this record, as spans on the lane. §25's layer 8.
   *
   * From the deck's snapshot rather than from a fetch of its own: a saved loop
   * changes when one is saved, which is a thing that happens mid-set, and the
   * snapshot is the path that already carries a mid-set change.
   */
  const savedLoops = $derived(
    deck.saved_loops.map((region) => ({
      slot: region.slot,
      left: region.start_frames / framesPerPixel,
      // A hairline for a loop shorter than a pixel, the same floor the active
      // loop's band takes: invisible is wrong, and a sixteenth of a beat zoomed
      // out is well under one pixel.
      width: Math.max((region.end_frames - region.start_frames) / framesPerPixel, 2),
    })),
  );

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
      <!--
        The phrase boundaries: a grab target over each line the tiles already
        draw. Transparent on purpose — the line belongs to the renderer, which
        is the only thing that can put it on the same pixel as the audio. What
        the DOM adds is somewhere for a hand to land, and a visible line only
        while one is dragging, because a tile cannot re-render under a finger.
      -->
      {#if onPhraseMoved}
        {#each phrases as frame (frame)}
          <div
            class="phrase-grab"
            class:held={dragging?.label === `phrase ${frame}`}
            role="separator"
            aria-label="Phrase boundary"
            title="Phrase boundary — drag to say where a phrase starts"
            style:left="{(dragging?.label === `phrase ${frame}` ? dragging.frame : frame) /
              framesPerPixel}px"
            onpointerdown={(e) =>
              grab(e, `phrase ${frame}`, frame, (f) => onPhraseMoved(f))}
          ></div>
        {/each}
      {/if}
      {#each dropMarks as drop (drop.frame)}
        <div class="drop" style:left="{drop.left}px" title="The drums come back here"></div>
      {/each}
      <!--
        §25's saved-loop layer: the regions kept with the record, drawn as a
        bracket rather than as a band. The running loop is a solid bar in the
        same colour; hollow against solid is what says "marked out" against
        "playing", which is a difference of *shape* — a second colour for the
        same idea is what §57 forbids.
      -->
      {#each savedLoops as region (region.slot)}
        <div class="saved-loop" style:left="{region.left}px" style:width="{region.width}px">
          {#if onSavedLoopRecalled}
            <button
              type="button"
              class="saved-loop-flag"
              title="Saved loop {region.slot} — click to play it"
              aria-label="Recall saved loop {region.slot}"
              onclick={() => onSavedLoopRecalled(region.slot)}>{region.slot}</button
            >
          {:else}
            <span class="saved-loop-flag">{region.slot}</span>
          {/if}
        </div>
      {/each}
      {#if loopBand}
        <div
          class="loop-band"
          style:left="{loopBand.left}px"
          style:width="{loopBand.width}px"
        ></div>
        <!--
          The edges, drawn and grabbable. §26 lists "Loop — resize" beside the
          cue marker: the length was a pair of halve/double buttons, which is
          every length that is a power of two and no other.
        -->
        {#each [["start", loopBand.start], ["end", loopBand.end]] as const as [edge, frame] (edge)}
          <div
            class="loop-edge"
            class:grabbable={!!onLoopEdgeMoved}
            class:held={dragging?.label === `loop ${edge}`}
            role={onLoopEdgeMoved ? "separator" : undefined}
            aria-label={onLoopEdgeMoved ? `Loop ${edge}` : undefined}
            title={onLoopEdgeMoved ? `Loop ${edge} — drag to resize` : undefined}
            style:left="{frame / framesPerPixel}px"
            onpointerdown={(e) =>
              onLoopEdgeMoved &&
              grab(e, `loop ${edge}`, frame, (f) => onLoopEdgeMoved(edge, f))}
          ></div>
        {/each}
      {/if}
      {#each markers as marker (marker.slot)}
        <!--
          §26's first example: "Cue marker — drag to move." Grabbable only
          where a lane was given somewhere to send it; elsewhere this is the
          same read-only marker it has always been. Not focusable, for the same
          reason the mix point is not: the keyboard path to a cue is its pad,
          which sets it at the playhead, and a slider role here would promise
          arrow-key nudging of a pixel rather than of a beat.
        -->
        <div
          class="cue-marker"
          class:grabbable={!!onCueMoved}
          class:held={dragging?.label === `cue ${marker.slot}`}
          role={onCueMoved ? "separator" : undefined}
          aria-label={onCueMoved ? `Cue ${marker.slot}` : undefined}
          title={onCueMoved ? `Cue ${marker.slot} — drag to move it` : undefined}
          style:left="{(dragging?.label === `cue ${marker.slot}`
            ? dragging.frame
            : marker.frame) / framesPerPixel}px"
          onpointerdown={(e) =>
            onCueMoved &&
            grab(e, `cue ${marker.slot}`, marker.frame, (f) => onCueMoved(marker.slot, f))}
        >
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
          onpointerdown={(e) =>
            onMarkMoved && grab(e, mark.label, mark.frame, (f) => onMarkMoved(mark.label, f))}
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
      <!--
        §27's ghost: the incoming record over this one, from the point the mix
        begins. The same tiles its own lane draws, at a zoom that makes one of
        its beats one of these — laid over rather than beside, because the
        question is what the two records do *together*.
      -->
      {#if ghost && ghostBox}
        <div
          class="ghost"
          style:left="{ghostBox.left}px"
          style:width="{ghostBox.width}px"
          aria-hidden="true"
        >
          {#each ghostTiles as tile (tile.key)}
            <img
              class="tile"
              src={tile.url}
              alt=""
              decoding="async"
              loading="eager"
              width={TILE_WIDTH}
              {height}
              style:left="{(tile.startFrame - ghost.from) / ghost.framesPerPixel}px"
            />
          {/each}
          <!--
            §27's "where the drop occurs", about the record coming in. The same
            two marks the lane draws for its own record, in the ghost's
            coordinates and inside its box, so they fade with it: what the new
            record does is part of the preview, not a fact about the audio
            playing now.
          -->
          {#each ghostBreakdowns as band (band.start_frame)}
            <div
              class="breakdown"
              style:left="{(band.start_frame - ghost.from) / ghost.framesPerPixel}px"
              style:width="{Math.max(
                (band.end_frame - band.start_frame) / ghost.framesPerPixel,
                2,
              )}px"
              title="The drums are out here in the record coming in"
            ></div>
          {/each}
          {#each ghostDrops as frame (frame)}
            <div
              class="drop"
              style:left="{(frame - ghost.from) / ghost.framesPerPixel}px"
              title="The drums come back here in the record coming in"
            ></div>
          {/each}
        </div>
      {/if}
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
    The loop, as a band along the *bottom* edge with a line at each end.

    It used to be a sixteen-per-cent wash of `--accent-2` over the whole
    height, and that is the thing §57 forbids wearing a different hat: the
    waveform's colour is its spectral balance, and a tint over it makes the
    audio inside a loop read as slightly more mid-range than it is. In
    `pkg-industrial` the wash was *exactly* the mid band. The breakdown layer
    already learnt this — see `.breakdown` — so the loop reads the same way:
    a mark at an edge, over nothing.
  */
  .loop-band {
    position: absolute;
    bottom: 0;
    height: 3px;
    background: var(--accent-2);
    pointer-events: none;
  }

  /*
    The edges. Full height because a loop's ends are where a hand goes and
    where the eye checks the length, and because they sit *over* the waveform
    rather than tinting it. Two pixels of a colour is a mark; sixteen per cent
    of one across a region is a filter.
  */
  .loop-edge {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--accent-2);
    opacity: 0.7;
    pointer-events: none;
    z-index: 2;
  }

  .loop-edge.grabbable {
    pointer-events: auto;
    cursor: ew-resize;
    touch-action: none;
  }

  .loop-edge.held {
    opacity: 1;
    background: var(--text);
  }

  /*
    The ghost, §27. Over the outgoing waveform rather than beside it, because
    the question is what the two records do together — and see-through, because
    a record that has not come in yet is not a fact about the audio playing.

    No recolouring. The ghost's colour is its own spectral balance, exactly as
    the lane's is, and tinting one of them to tell them apart would make a hue
    mean two things — §57. What separates them is that one is solid and one is
    not, which is the same distinction the saved loops draw.

    Clipped to its own box, so the incoming record starts where the mix starts
    and not at the left-hand edge of the lane.
  */
  .ghost {
    position: absolute;
    inset-block: 0;
    overflow: hidden;
    opacity: 0.42;
    border-left: 1px solid var(--text-dim);
    pointer-events: none;
    z-index: 1;
  }

  /*
    §25's saved-loop layer. A three-sided bracket along the bottom edge: the
    span between two uprights, open at the top.

    The same hue as the running loop, deliberately, because it is the same
    idea — this *is* a loop, it is simply not the one playing. What separates
    them is shape: the running loop is a solid bar, a saved one is an outline.
    Inventing a second colour for the second state is exactly the overload §57
    forbids, and this lane has already spent five hues.

    Under the full-height marks rather than over them: a cue and a phrase
    boundary are single places, and a place has to stay findable through a
    region that happens to contain it.
  */
  .saved-loop {
    position: absolute;
    bottom: 0;
    height: 7px;
    /* Faded in the colour rather than with `opacity`, which would group the
       element with its label: the bracket wants to be quiet and the number is
       a control, and a control a DJ has to squint at is not one. */
    border: 1px solid color-mix(in srgb, var(--accent-2) 60%, transparent);
    border-top: none;
    pointer-events: none;
    z-index: 1;
  }

  /*
    The slot, which is what a DJ recalls the loop by -- and, where the lane can
    do something about it, the thing to press. Above the bracket rather than
    inside it: seven pixels cannot hold a digit, and the number belongs to the
    left end the way a cue's flag belongs to its line.
  */
  .saved-loop-flag {
    position: absolute;
    bottom: 100%;
    left: 0;
    padding: 0 0.25rem;
    border: none;
    background: transparent;
    color: var(--accent-2);
    font-size: 0.65rem;
    line-height: 1.2;
    font-family: inherit;
  }

  button.saved-loop-flag {
    pointer-events: auto;
    cursor: pointer;
  }

  button.saved-loop-flag:hover,
  button.saved-loop-flag:focus-visible {
    color: var(--text);
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

  .cue-marker.grabbable {
    pointer-events: auto;
    cursor: ew-resize;
    touch-action: none;
  }

  /* Brighter while it is under a hand, so the one being moved is obvious in a
     row of eight. */
  .cue-marker.held {
    background: var(--text);
  }

  /*
    The part that answers to a pointer, wider than the part that is drawn.
    A two-pixel line is a hard thing to hit with a mouse and an impossible one
    on a trackpad in a dark booth.

    **A pseudo-element, not padding and a negative margin.** That pair was what
    the mix point used, and it moved the line: padding widens the box and a
    negative margin shifts it, so a grabbable mark was *drawn* six pixels
    earlier than djmanzo said it was — the one thing this lane exists to get
    right, given up for a hit target. A pseudo-element has no effect on the
    element's own geometry at all, and pointers hit-test it as the element.
  */
  /*
    A phrase boundary's grab target. Two pixels wide and invisible: the line it
    sits on is rasterised into the tile, where it can share a pixel with the
    audio, and drawing a second one here would be a second opinion about where
    that beat is. While it is held it *does* draw, because the tiles cannot
    re-render under a moving finger and a boundary that vanished mid-drag would
    be the one moment it is needed.
  */
  .phrase-grab {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: transparent;
    cursor: ew-resize;
    touch-action: none;
    z-index: 2;
  }

  .phrase-grab.held {
    background: var(--text);
  }

  .mark.grabbable::after,
  .loop-edge.grabbable::after,
  .phrase-grab::after,
  .cue-marker.grabbable::after {
    content: "";
    position: absolute;
    inset-block: 0;
    inset-inline: -6px;
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

  /* Only a lane that can do something about it invites the grab. The wider
     hit area is below, shared with the cue markers. */
  .mark.grabbable {
    pointer-events: auto;
    cursor: ew-resize;
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
