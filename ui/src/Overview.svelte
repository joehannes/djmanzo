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
    gridSlug,
    tileUrl,
    waveformInfo,
    type DeckState,
    type MixInInfo,
    type MixOutInfo,
    type EnergyTrajectory,
  } from "./api";
  import { remembers, showing } from "./remembers.svelte";
  import { STEM_KEYS, STEM_LABELS } from "./stems";
  import { waveformAsks } from "./waveformAsks";
  import { theme } from "./theme.svelte";

  let {
    deck,
    height = 34,
    ghost = null,
  }: {
    deck: DeckState;
    height?: number;
    /**
     * §27's ghost: a mix that has not happened, over a record not loaded.
     *
     * Frames on *this* record. The caller has already converted the
     * candidate's beats onto this record's grid, because that conversion needs
     * both tempos and this component knows one.
     *
     * The whole-record view is where a ghost belongs and the scrolling lane is
     * not. A mix point is typically minutes ahead of the playhead, and the
     * lane runs at a few hundred frames per pixel — so a ghost drawn there
     * would be off screen until the DJ was already inside it, which is after
     * the decision §27 exists to inform.
     *
     * Drawn as `suggestion`, the layer §25 reserves for "what djmanzo would
     * do, drawn as a ghost rather than as a fact". It shares its colour
     * meaning with the mix-out window and with nothing else: both are djmanzo
     * saying *could*. Nothing here takes a pointer, which is §27's word —
     * non-destructive — expressed rather than promised.
     */
    ghost?: {
      /** Where the mix would begin and end, in frames. */
      from: number;
      to: number;
      /** Where the candidate's first full phrase would land. */
      landing?: number | null;
      /**
       * §27's *drop*: where the candidate's first one would land, in frames on
       * *this* record.
       *
       * Drawn as a mark on the ghost rather than beside it. §27 asks for the
       * drop to be part of the overlay, and a number in a line of text under
       * the rail is a number about a place the eye then has to find.
       */
      drop?: number | null;
      /**
       * §27's *where the vocal enters*: where the candidate's voice would
       * arrive, in frames on *this* record.
       *
       * The last of §27's seven, and drawn on the ghost for the same reason
       * the drop is: both are answers about a record that is not loaded, so
       * both wear the ghost's colour rather than the colour the loaded
       * record's own vocal layer wears.
       */
      vocalEntry?: number | null;
      /** What the band means, for the hover. */
      title?: string;
    } | null;
  } = $props();

  let box = $state<HTMLDivElement | null>(null);
  let playhead = $state<HTMLDivElement | null>(null);
  let width = $state(600);
  let totalFrames = $state(0);
  let epoch = $state(0);
  let ready = $state(false);
  let mixOut = $state<MixOutInfo | null>(null);
  let mixIn = $state<MixInInfo | null>(null);
  /**
   * §75's trajectory, and §25's `breakdowns`, `drops` and `energy` layers.
   *
   * The whole-record view is where a trajectory belongs and the scrolling lane
   * is not, for the same reason the ghost is drawn here: a breakdown two
   * minutes ahead is off screen in a lane running at a few hundred frames per
   * pixel, and "where does this record go" is a question asked before the
   * record gets there.
   */
  let trajectory = $state<EnergyTrajectory | null>(null);

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

  /**
   * What this component last asked Rust about. See `./waveformAsks`.
   *
   * Not `$state`: nothing renders it, and making it reactive would have the
   * effect depend on its own write.
   */
  let asked = "";

  $effect(() => {
    // **Guarded, not merely dependent.** These reads look fine-grained and are
    // not: `App.svelte` replaces the whole snapshot on every frame, so every
    // deck is a new proxy and every read through it is a new signal. This
    // effect therefore runs on every frame the pump sends — measured at forty
    // `waveform_info` calls for ten frames of ordinary playback, which is
    // about two hundred and forty IPC round trips a second for an answer that
    // changes twice a track.
    //
    // The effect still runs; the *call* does not. See `./waveformAsks` for
    // what counts as something new.
    const key = waveformAsks(deck);
    if (key === asked) return;
    asked = key;
    void waveformInfo(deck.number)
      .then((info) => {
        ready = info.ready;
        totalFrames = info.total_frames;
        epoch = info.epoch;
        mixOut = info.mix_out ?? null;
        mixIn = info.mix_in ?? null;
        trajectory = info.trajectory ?? null;
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
    return tileUrl(
      deck.number,
      tileWidth,
      height,
      0,
      framesPerPixel,
      theme.resolved,
      epoch,
      gridSlug(remembers.layers),
    );
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
  /**
   * §25's mix-in band, over the whole record.
   *
   * Here as well as on the lane, and for the reason the mix-out band is over
   * here: on the lane the window is only visible once the playhead is near it,
   * and the whole point of a window at the *start* of a record is to be read
   * while deciding whether to load the record at all. Floored to the same
   * minimum width, because a hairline nobody can see is not an answer.
   */
  const mixInBand = $derived.by(() => {
    if (!mixIn || totalFrames <= 0) return null;
    const left = fraction(mixIn.opens_frame) * 100;
    const right = fraction(mixIn.closes_frame) * 100;
    return right > left
      ? {
          left,
          width: Math.max(right - left, 0.8),
          onPhrase: mixIn.on_phrase,
          beforeADrop: mixIn.before_a_drop,
        }
      : null;
  });

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

  /**
   * §27's drop, where the candidate has one inside the stretch it would cover.
   *
   * Clamped to the record and dropped when it falls outside the mix: a mark at
   * ninety-eight per cent for a drop that lands after the outgoing record ends
   * is a promise about a moment nobody will hear.
   */
  const ghostDrop = $derived.by(() => {
    const at = ghost?.drop;
    if (at == null || !ghost || totalFrames <= 0) return null;
    if (at < ghost.from || at > ghost.to) return null;
    return { left: fraction(at) * 100 };
  });

  /** And where its voice would arrive. §27's seventh. */
  const ghostVoice = $derived.by(() => {
    const at = ghost?.vocalEntry;
    if (at == null || !ghost || totalFrames <= 0) return null;
    if (at < ghost.from || at > ghost.to) return null;
    return { left: fraction(at) * 100 };
  });

  /**
   * The trajectory as a run of columns, one per window.
   *
   * Columns rather than a line: a line implies a reading between the points
   * and there is none — each window is a measurement over eight beats, and
   * djmanzo has no opinion about the bar in the middle of one.
   */
  const shape = $derived.by(() => {
    const sections = trajectory?.sections ?? [];
    if (sections.length < 2 || totalFrames <= 0) return [];
    // The width of one window, from the gap between the first two. Measured
    // rather than computed from the tempo: this component does not know the
    // grid, and the gap is the same number by construction.
    const span = sections[1].at - sections[0].at;
    return sections.map((section) => ({
      at: section.at,
      left: fraction(section.at) * 100,
      width: Math.max((span / totalFrames) * 100, 0.4),
      height: Math.round(section.energy * 100),
    }));
  });

  /**
   * §75's *transient density*: how often something is struck.
   *
   * A thin band under the energy columns rather than columns of its own, and
   * the shape is the argument. Energy is *where the record goes* and a column
   * reads as a height on a journey; density is a texture, and a second run of
   * columns beside the first would invite reading them as two heights of one
   * thing. A band that darkens says "more is happening here" without
   * competing for the same axis.
   *
   * **Absolute, against a stated ceiling**, like the vocal strip beside it —
   * `strikes::BUSY` is the density at which it saturates, and it is a drawing
   * decision rather than a claim about audio. A window nobody measured draws
   * nothing rather than zero: absent and *nothing was struck* are different
   * answers.
   */
  const STRUCK_SATURATES_AT = 12;

  const struck = $derived.by(() => {
    const sections = trajectory?.sections ?? [];
    if (sections.length < 2 || totalFrames <= 0) return [];
    const span = sections[1].at - sections[0].at;
    return sections
      .filter((section) => section.strikes !== null && section.strikes !== undefined)
      .map((section) => ({
        at: section.at,
        left: fraction(section.at) * 100,
        width: Math.max((span / totalFrames) * 100, 0.4),
        strength: Math.min((section.strikes ?? 0) / STRUCK_SATURATES_AT, 1),
      }));
  });

  /** §75's breakdowns, as bands. */
  const thin = $derived.by(() => {
    const spans = trajectory?.breakdowns ?? [];
    if (totalFrames <= 0) return [];
    return spans
      .map((span) => ({
        from: span.from,
        left: fraction(span.from) * 100,
        width: Math.max((span.to - span.from) / totalFrames * 100, 0.8),
      }))
      .filter((band) => band.width > 0);
  });

  /**
   * The vocal's place in the one stem order — see `./stems`. Named rather
   * than written as `[0]` at each use, because a bare index into a four-array
   * is the kind of thing that ends up pointing at the drums.
   */
  const VOCAL = STEM_KEYS.indexOf("vocal");

  /**
   * Where the vocal strip reaches full strength.
   *
   * A drawing scale, **not** a threshold for a voice existing. Rust has the
   * one number that is a claim — `presence::STRONG`, which decides where §27
   * says the voice enters — and it is deliberately not imported here: this
   * says how dark to draw a share, that says whether to promise a DJ
   * something, and one constant serving both would make a drawing tweak a
   * change to what djmanzo asserts.
   */
  const SATURATES_AT = 0.25;

  /**
   * §25's `stems` layer: which of the four currents is carrying the record.
   *
   * One segment per window, in the colour of the fader that mutes that
   * current — the association worth making, and the reason this does not
   * share the `vocal` layer's colour: "which of four" drawn in one colour
   * answers nothing.
   *
   * The **dominant** current rather than all four stacked. Four segments in a
   * four-pixel band is one pixel each, which is texture rather than
   * information; the question §25 actually asks is *which one is carrying the
   * record here*, and that has one answer per window.
   */
  const currents = $derived.by(() => {
    const sections = trajectory?.sections ?? [];
    if (sections.length < 2 || totalFrames <= 0) return [];
    const span = sections[1].at - sections[0].at;
    return sections
      .filter((section) => section.parts != null)
      .map((section) => {
        const parts = section.parts as number[];
        let carrying = 0;
        for (let i = 1; i < parts.length; i += 1) {
          if (parts[i] > parts[carrying]) carrying = i;
        }
        return {
          at: section.at,
          left: fraction(section.at) * 100,
          width: Math.max((span / totalFrames) * 100, 0.4),
          key: STEM_KEYS[carrying],
          label: STEM_LABELS[carrying],
          says: STEM_LABELS.map(
            (name, i) => `${name} ${Math.round(parts[i] * 100)}%`,
          ).join(" · "),
        };
      });
  });

  /**
   * §25's `vocal` layer: where a lead is centred in the voice range.
   *
   * A strip along the top edge rather than more columns from the floor. The
   * trajectory already owns the floor, and a second set of columns would read
   * as a second opinion about the same question — where this record goes —
   * when it is an answer to a different one.
   *
   * Windows nobody measured are dropped rather than drawn at zero: absent and
   * "nobody was singing" are different answers, and a strip that filled the
   * gaps would claim the second when djmanzo only has the first.
   */
  const voices = $derived.by(() => {
    const sections = trajectory?.sections ?? [];
    if (sections.length < 2 || totalFrames <= 0) return [];
    const span = sections[1].at - sections[0].at;
    return sections
      .filter((section) => section.parts != null)
      .map((section) => ({
        at: section.at,
        left: fraction(section.at) * 100,
        width: Math.max((span / totalFrames) * 100, 0.4),
        share: (section.parts as number[])[VOCAL],
        // Where the strip reaches full strength. A drawing scale, **not** a
        // threshold for a voice existing: the share at which a DJ would say
        // "there's a vocal" can only come from real records with real voices
        // on them, and the machine this was written on has none. So the strip
        // says how much and never yes or no.
        opacity: Math.min(1, (section.parts as number[])[VOCAL] / SATURATES_AT),
      }));
  });

  /** And the drops, as marks. */
  const returns = $derived.by(() => {
    if (totalFrames <= 0) return [];
    return (trajectory?.drops ?? []).map((at) => ({
      at,
      left: fraction(at) * 100,
    }));
  });

  /**
   * §27's ghost band: the stretch a mix *would* cover.
   *
   * Floored to a locatable width for the same reason the other two bands are:
   * a 32-beat blend is about one percent of a five-minute record, and a
   * faithful width there is a hairline nobody can see. This view is for
   * understanding what would happen, not for measuring it.
   */
  const ghostBand = $derived.by(() => {
    if (!ghost || totalFrames <= 0) return null;
    const left = fraction(ghost.from) * 100;
    const right = fraction(ghost.to) * 100;
    return right > left ? { left, width: Math.max(right - left, 1.2) } : null;
  });

  /** Where the candidate's first full phrase lands, as a position. */
  const ghostLanding = $derived.by(() => {
    if (!ghost || ghost.landing == null || totalFrames <= 0) return null;
    return { left: fraction(ghost.landing) * 100 };
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
    <!--
      §25's mix-in layer: the stretch this record can be joined in. The same
      colour as the band below on purpose — one claim read from two ends, and
      `dj_render::layer` holds that decision where §57's rule lives.
    -->
    {#if mixInBand && showing("mix-in")}
      <div
        class="mix-in"
        class:on-phrase={mixInBand.onPhrase}
        class:before-a-drop={mixInBand.beforeADrop}
        data-layer="mix-in"
        style:left="{mixInBand.left}%"
        style:width="{mixInBand.width}%"
        title={mixInBand.beforeADrop
          ? "Bring this record in anywhere in here and its first drop is still ahead of you."
          : "Bring this record in anywhere in here. Nobody has found a drop in it, so this is the longest transition djmanzo would propose rather than the record's own shape."}
      ></div>
    {/if}
    {#if mixOutBand && showing("mix-out")}
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
    <!--
      §27: the ghost. A hatched band for the overlap and a dotted line where
      the candidate's first full phrase would land — a record that is not
      loaded and a mix that has not been armed, drawn so it cannot be mistaken
      for either.
    -->
    {#if ghostBand && showing("suggestion")}
      <div
        class="ghost-band"
        data-layer="suggestion"
        style:left="{ghostBand.left}%"
        style:width="{ghostBand.width}%"
        title={ghost?.title ?? "If this came in here, this is what it would cover"}
      ></div>
    {/if}
    {#if ghostLanding && showing("suggestion")}
      <div
        class="ghost-landing"
        data-layer="suggestion"
        style:left="{ghostLanding.left}%"
        title="Where the candidate's first full phrase would land"
      ></div>
    {/if}
    {#if ghostDrop && showing("suggestion")}
      <div
        class="ghost-drop"
        data-layer="suggestion"
        style:left="{ghostDrop.left}%"
        title="Where the candidate drops, if it came in here"
      ></div>
    {/if}
    {#if ghostVoice && showing("suggestion")}
      <div
        class="ghost-voice"
        data-layer="suggestion"
        style:left="{ghostVoice.left}%"
        title="Where the candidate's voice comes in, if it came in here"
      ></div>
    {/if}
    {#if loopBand && showing("loop")}
      <div
        class="loop-band"
        data-layer="loop"
        style:left="{loopBand.left}%"
        style:width="{loopBand.width}%"
      ></div>
    {/if}
    <img class="whole" src={url} alt="" width={tileWidth} {height} draggable="false" />
    <!--
      §75's trajectory and the two things worth marking on it.

      **After the image, not before it.** The first version drew these under
      the tile, reasoning that the shape of a record is context behind the
      waveform -- and the tile is opaque, so nothing of them reached the
      screen. `layers.spec.ts` records the same mistake being made once before
      with the runway, and Playwright's `toBeVisible` passed both times because
      it asks whether an element has a box rather than whether anything can be
      seen of it. Found by looking at the running application, twice.

      So they are over the tile and quiet instead: the breakdown is a wash at
      low opacity, the trajectory is columns from the floor, and only the drop
      is drawn at full strength -- one line, at one place, on a record.
    -->
    {#each showing("breakdowns") ? thin : [] as band (band.from)}
      <div
        class="thin"
        data-layer="breakdowns"
        style:left="{band.left}%"
        style:width="{band.width}%"
        title="The record thins out here"
      ></div>
    {/each}
    {#each showing("energy") ? shape : [] as column (column.at)}
      <div
        class="shape"
        data-layer="energy"
        style:left="{column.left}%"
        style:width="{column.width}%"
        style:height="{column.height}%"
      ></div>
    {/each}
    {#each showing("transients") ? struck : [] as band (band.at)}
      <div
        class="struck"
        data-layer="transients"
        style:left="{band.left}%"
        style:width="{band.width}%"
        style:opacity={band.strength}
        title="How often something is struck here"
      ></div>
    {/each}
    {#each showing("vocal") ? voices : [] as band (band.at)}
      <div
        class="voice"
        data-layer="vocal"
        style:left="{band.left}%"
        style:width="{band.width}%"
        style:opacity={band.opacity}
        title="A lead sits in the voice range here — {(band.share * 100).toFixed(
          0,
        )}% of this window is centred and sustained. Usually a singer; a centred synth lead reads the same way."
      ></div>
    {/each}
    {#each showing("stems") ? currents : [] as band (band.at)}
      <div
        class="current"
        data-layer="stems"
        data-current={band.key}
        style:left="{band.left}%"
        style:width="{band.width}%"
        title="{band.label} carry this stretch — {band.says}"
      ></div>
    {/each}
    {#each showing("drops") ? returns : [] as mark (mark.at)}
      <div
        class="drop"
        data-layer="drops"
        style:left="{mark.left}%"
        title="Where it comes back"
      ></div>
    {/each}
    {#each showing("cues") ? markers : [] as marker (marker.slot)}
      <div class="cue" data-layer="cues" style:left="{marker.left}%"></div>
    {/each}
    <div class="playhead" bind:this={playhead}></div>
  {:else}
    <div class="empty"></div>
  {/if}
</div>

<style>
  /*
    §75's three. One colour between them — `--shape` — because they are one
    reading of one curve drawn three ways: see `dj_render::layer::Role::Shape`.
    Told apart by form rather than by hue, which is what §57 asks for when a
    meaning is shared: the trajectory is columns from the floor, a breakdown is
    a washed band behind everything, and a drop is a line.
  */
  .shape {
    position: absolute;
    z-index: 1;
    bottom: 0;
    background: var(--shape, var(--text-dim));
    opacity: 0.5;
    pointer-events: none;
  }
  .thin {
    position: absolute;
    z-index: 1;
    inset-block: 0;
    background: var(--shape, var(--text-dim));
    opacity: 0.18;
    pointer-events: none;
  }
  /* Pinned to the top edge, 3px, so the form says it is not the trajectory
     even before the colour does -- §33's rule that colour is never the only
     channel, applied to a layer nobody has to read in a hurry. */
  /*
    §75's transient density, along the bottom rather than the top.

    The vocal strip is at the top and the stem band under it; this goes to the
    other edge on purpose. It is the one reading here that is about *texture*
    rather than about what the mixture contains, and stacking it with the two
    that are would invite reading three bands as three parts of one answer.

    Three pixels, in the shape colour it shares with the energy columns above
    it -- `layer.rs` argues that case: how often something is struck is a fact
    about the record's shape over time, and the columns are the same reading
    drawn the other way.
  */
  .struck {
    position: absolute;
    z-index: 1;
    bottom: 0;
    height: 3px;
    background: var(--shape, var(--accent-2));
    pointer-events: none;
  }
  .voice {
    position: absolute;
    z-index: 1;
    top: 0;
    height: 3px;
    background: var(--uncertain, var(--text-dim));
    pointer-events: none;
  }
  /* Under the vocal strip, in the colour of the fader that mutes the current
     it names. Four pixels: enough to read as a band of colour changing along
     the record, not enough to compete with the waveform it sits over. */
  .current {
    position: absolute;
    z-index: 1;
    top: 3px;
    height: 4px;
    opacity: 0.75;
    pointer-events: none;
  }
  .current[data-current="vocal"] {
    background: var(--stem-vocal);
  }
  .current[data-current="drums"] {
    background: var(--stem-drums);
  }
  .current[data-current="bass"] {
    background: var(--stem-bass);
  }
  .current[data-current="other"] {
    background: var(--stem-other);
  }
  .drop {
    position: absolute;
    z-index: 1;
    inset-block: 0;
    width: 2px;
    margin-left: -1px;
    background: var(--shape, var(--text-dim));
    opacity: 0.85;
    pointer-events: none;
  }

  /* §27's drop, on the ghost's own colour rather than the shape colour: this
     is a *proposal* about a record that is not loaded, and the layer table
     says so -- `suggestion` is "what djmanzo would do, drawn as a ghost rather
     than as a fact". The breakdown marks on the record that *is* loaded are
     the other thing and wear the other colour.

     `--assistant` rather than a `--proposed` that does not exist: §30's role
     tokens are the ones defined, the landing line beside this uses the same
     one, and `theme-tokens.test.ts` fails on a token nothing defines -- which
     is how the first version of this rule was caught, asking for a hue it
     would have fallen back from. */
  /* A tick at the top rather than a full-height line, so it is not read as a
     second drop. It sits in the band the loaded record's own vocal layer
     occupies, which is the association worth making: same question, one about
     a record on a deck and one about a record being considered. */
  .ghost-voice {
    position: absolute;
    z-index: 1;
    top: 0;
    height: 40%;
    width: 2px;
    margin-left: -1px;
    background: var(--assistant);
    opacity: 0.9;
    pointer-events: none;
  }
  .ghost-drop {
    position: absolute;
    z-index: 1;
    inset-block: 0;
    width: 2px;
    margin-left: -1px;
    background: var(--assistant);
    opacity: 0.9;
    pointer-events: none;
  }

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
    background: color-mix(in srgb, var(--assistant) 22%, transparent);
    border-left: 2px solid color-mix(in srgb, var(--assistant) 75%, transparent);
    border-right: 2px dashed color-mix(in srgb, var(--assistant) 55%, transparent);
    pointer-events: none;
  }

  /*
    The same colour, mirrored: the solid edge is the one that is a decision,
    and for coming in that is the close — what a DJ is judging is how long they
    have before the drop.
  */
  .mix-in {
    position: absolute;
    top: 0;
    bottom: 0;
    background: color-mix(in srgb, var(--assistant) 22%, transparent);
    border-left: 2px dashed color-mix(in srgb, var(--assistant) 55%, transparent);
    border-right: 2px solid color-mix(in srgb, var(--assistant) 75%, transparent);
    pointer-events: none;
  }

  .mix-in:not(.before-a-drop),
  .mix-in:not(.on-phrase) {
    opacity: 0.6;
  }

  .mix-out:not(.on-phrase) {
    border-left-style: dashed;
    opacity: 0.6;
  }

  /*
    §27's ghost. The same colour meaning as `.mix-out` -- both are djmanzo
    saying *could* -- and a different texture, because they say it about
    different things: the window is a place a record can be left, this is a
    whole mix that has not happened. Hatched rather than washed, so it reads as
    provisional at a glance against the solid band it sits over.
  */
  .ghost-band {
    position: absolute;
    top: 0;
    bottom: 0;
    /* §30's `assistant` role: this is djmanzo saying *could*, and the section
       asks for "what the machine did" to look different from "this is on".
       It was `var(--assistant)`, a token nothing defined, so every palette
       drew the same fixed green here. */
    background: repeating-linear-gradient(
      45deg,
      color-mix(in srgb, var(--assistant) 45%, transparent) 0 3px,
      transparent 3px 6px
    );
    border-left: 2px dashed color-mix(in srgb, var(--assistant) 85%, transparent);
    border-right: 2px dashed color-mix(in srgb, var(--assistant) 60%, transparent);
    pointer-events: none;
  }

  .ghost-landing {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 0;
    border-left: 2px dotted color-mix(in srgb, var(--assistant) 95%, transparent);
    pointer-events: none;
  }
</style>
