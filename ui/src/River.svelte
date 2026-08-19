<script lang="ts">
  /**
   * A deck as a river.
   *
   * The first piece of the living interface, and it is deliberately the whole
   * vocabulary in one component: flow, pulse, clarity and the mouth. If the
   * metaphor does not carry a deck it will not carry a mixer, and better to
   * find that out here than after four more components are built on it.
   *
   * # Canvas paints, DOM listens
   *
   * Nothing here is clickable. Everything drawn is `Foliage` — it reports state
   * and nothing else — so there is no hit target to get wrong. The reading sits
   * in a real DOM element beside it, which is what a screen reader finds and
   * what stays legible when the canvas is not drawn at all.
   *
   * See docs/adr/0009-the-living-interface.md.
   */
  import { onMount } from "svelte";
  import { css, phaseAt, type Entity, type Tier } from "./world";

  let {
    river,
    mouth = null,
    strata = [],
    shear = null,
    eddy = null,
    stones = [],
    height = 64,
    latencyMs = 0,
    alarming = false,
    tier = "living",
  }: {
    river: Entity;
    mouth?: Entity | null;
    /** The three strata of the water column, low to high. */
    strata?: Entity[];
    /** Where the filter has narrowed the channel, when it has. */
    shear?: Entity | null;
    /** Water circulating instead of passing. */
    eddy?: Entity | null;
    /** Fixed places a DJ can return to. */
    stones?: Entity[];
    height?: number;
    /**
     * What the output chain adds after the engine. The pulse is delayed by it
     * so the crest lands when the *room* hears the beat rather than when the
     * engine computed it.
     */
    latencyMs?: number;
    /** True when this river holds the peripheral channel. Only one ever does. */
    alarming?: boolean;
    /**
     * How richly to draw. Chosen by measurement upstream, not here — one
     * decision for the whole world, so the lanes cannot disagree about it.
     */
    tier?: Tier;
  } = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  /** When this river's vitality was last read, for interpolating the pulse. */
  let readAt = performance.now();

  /**
   * When this river took the peripheral channel.
   *
   * Motion is spent on the *transition* and then settles: attention is captured
   * by change, not by condition, and a thing that has been moving for five
   * minutes is wallpaper — wallpaper that hides the next event behind it. So
   * the onset runs once and the claim then rests as a static shape, which still
   * says the same thing when looked at. See VISUAL-LANGUAGE.md §5.
   */
  let alarmedAt = $state<number | null>(null);

  $effect(() => {
    // A claim that is already held does not re-fire. A *worsening* one does,
    // and arrives here as a new `alarming` after a false.
    alarmedAt = alarming ? (alarmedAt ?? performance.now()) : null;
  });

  // A new reading resets the clock the pulse is interpolated from. Without
  // this the phase would jump every time the world was re-read.
  $effect(() => {
    void river.vitality;
    readAt = performance.now();
  });

  const still = $derived(tier === "still");

  /**
   * Whether there is any water at all.
   *
   * An empty deck is a *dry bed*, not a low river — VISUAL-LANGUAGE.md §2 says
   * so, and drawing a quarter-full channel for a deck with nothing on it was
   * the interface showing water where there is none. A faded-out deck, by
   * contrast, is still a river: the track is there and running, it is just not
   * being heard, so it keeps a thin body rather than drying up.
   */
  const dry = $derived(river.reading === "empty");

  /**
   * Whether there is anything to animate.
   *
   * "Stillness is the default" is not only about what a DJ sees, it is about
   * what the machine does: a dry bed, a paused deck and a reduced-motion
   * preference all have nothing moving in them, and repainting sixty times a
   * second to draw the same pixels is exactly the waste the rule exists to
   * prevent. This component was doing it, and it cost most of the frame budget
   * on a machine without acceleration.
   */
  const animating = $derived(
    !still && !dry && (river.vitality.depth > 0.001 || alarming),
  );

  onMount(() => {
    let frame = 0;
    let stop = false;

    const loop = () => {
      if (stop) return;
      if (canvas) paint(canvas);
      frame = requestAnimationFrame(loop);
    };

    // Start and stop the loop with the world, rather than running it always.
    const control = $effect.root(() => {
      $effect(() => {
        if (animating) {
          if (!frame) loop();
        } else {
          cancelAnimationFrame(frame);
          frame = 0;
          // One last paint, so a river that has just stopped shows its resting
          // state rather than whatever the final animated frame happened to be.
          if (canvas) paint(canvas);
        }
      });
    });

    const observer = new ResizeObserver(() => {
      if (!animating && canvas) paint(canvas);
    });
    if (canvas) observer.observe(canvas);

    return () => {
      stop = true;
      cancelAnimationFrame(frame);
      observer.disconnect();
      control();
    };
  });

  // A still river repaints when its state changes, and only then.
  $effect(() => {
    void river;
    void mouth;
    if (!animating && canvas) paint(canvas);
  });

  /**
   * How much of the lane the water fills.
   *
   * Never zero for a loaded deck: closing the fader is not ejecting the track,
   * and drawing nothing would say it had been.
   */
  function bodyOf(w: number, h: number) {
    const surface = h * (0.12 + river.extent * 0.68);
    return { top: h - surface, depth: surface, width: w };
  }

  /**
   * How long the onset runs before the claim settles.
   *
   * About two seconds: long enough to be caught in peripheral vision without
   * looking, short enough that it is over before it becomes something to
   * ignore.
   */
  const ONSET_MS = 2000;

  function paint(el: HTMLCanvasElement) {
    const ratio = window.devicePixelRatio || 1;
    const w = el.clientWidth;
    const h = el.clientHeight;
    if (w === 0 || h === 0) return;
    if (el.width !== Math.round(w * ratio) || el.height !== Math.round(h * ratio)) {
      el.width = Math.round(w * ratio);
      el.height = Math.round(h * ratio);
    }
    const ctx = el.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, w, h);

    const body = bodyOf(w, h);
    const seconds = still ? 0 : (performance.now() - readAt) / 1000;
    const phase = phaseAt(river.vitality, seconds, latencyMs);

    if (dry) {
      drawDryBed(ctx, w, h);
      return;
    }

    drawWater(ctx, w, h, body, phase);
    drawStrata(ctx, w, body);
    if (shear) drawShear(ctx, w, body);
    drawCrests(ctx, w, body, phase);
    drawTurbidity(ctx, w, body);
    if (eddy) drawEddy(ctx, w, body, phase);
    for (const stone of stones) drawStone(ctx, h, body, stone);
    drawPlayhead(ctx, h, body);
    if (mouth) drawMouth(ctx, w, h, mouth);
    if (alarming) drawAlarm(ctx, w, h);
  }

  /**
   * The three strata of the water column.
   *
   * Low is the deep current at the bottom, high is the surface light at the top
   * — which is where they physically are, so a DJ swapping lows on a transition
   * watches the deep current pass from one river to the other. A killed band is
   * drought at that stratum: a hard discontinuity, because a kill is not a
   * gentle turn of a knob and must not look like one.
   */
  function drawStrata(ctx: CanvasRenderingContext2D, w: number, body: Body) {
    if (strata.length === 0) return;
    const slice = body.depth / 3;
    for (const stratum of strata) {
      // Slot 0 is low, and low is the bottom of the column.
      const top = body.top + (2 - stratum.slot) * slice;
      // Unity sits at half of `extent`'s range, so a band at unity fills its
      // slice and a boosted one overflows upward into the light.
      const fill = Math.min(1, stratum.extent * 2);
      if (fill <= 0.001) {
        // Drought: the stratum is scoured out, not merely dimmed.
        ctx.fillStyle = "hsl(0 0% 0% / 0.55)";
        ctx.fillRect(0, top, w, slice);
        continue;
      }
      ctx.fillStyle = `hsl(0 0% 100% / ${0.04 + 0.1 * (fill - 0.5)})`;
      ctx.fillRect(0, top + slice * (1 - fill), w, slice * fill);
    }
  }

  /**
   * The channel narrowed from one side.
   *
   * A low-pass shears the surface away and leaves something deep and slow; a
   * high-pass cuts the depth and leaves something thin and bright. `along`
   * below the middle is a low-pass, above it a high-pass — the world's
   * convention, so the renderer does not have to know about filter signs.
   */
  function drawShear(ctx: CanvasRenderingContext2D, w: number, body: Body) {
    if (!shear) return;
    const fromTop = shear.along < 0.5;
    const cut = body.depth * shear.extent;
    ctx.fillStyle = "hsl(220 12% 8% / 0.72)";
    ctx.fillRect(0, fromTop ? body.top : body.top + body.depth - cut, w, cut);

    // The edge where the cut happens, which is the frequency being swept.
    const edge = fromTop ? body.top + cut : body.top + body.depth - cut;
    ctx.beginPath();
    ctx.moveTo(0, edge);
    ctx.lineTo(w, edge);
    ctx.strokeStyle = "hsl(0 0% 85% / 0.5)";
    ctx.lineWidth = 1;
    ctx.stroke();
  }

  /**
   * Water circulating instead of passing.
   *
   * The clearest metaphor in the system: a loop *is* an eddy, and a DJ who has
   * never read a word of this recognises a whirl in the water. It turns at the
   * track's tempo, so a halved loop visibly turns twice as fast over the same
   * stretch of water.
   */
  function drawEddy(ctx: CanvasRenderingContext2D, w: number, body: Body, phase: number) {
    if (!eddy) return;
    const from = w * eddy.along;
    const width = Math.max(3, w * eddy.extent);
    const mid = body.top + body.depth * 0.5;

    ctx.fillStyle = "hsl(0 0% 100% / 0.12)";
    ctx.fillRect(from, body.top, width, body.depth);

    // The whirl: an arc that goes round rather than along, at the beat's rate.
    const radius = Math.min(body.depth * 0.32, width * 0.42);
    if (radius < 2) return;
    const turn = phase * Math.PI * 2;
    ctx.beginPath();
    ctx.arc(from + width / 2, mid, radius, turn, turn + Math.PI * 1.4);
    ctx.strokeStyle = "hsl(0 0% 100% / 0.7)";
    ctx.lineWidth = 1.5;
    ctx.stroke();
  }

  /**
   * A stone in the river: a fixed place, visible from upstream.
   *
   * Structural, never the key's hue — a cue is a place, not a sound, and giving
   * it musical colour would put two meanings on one channel.
   */
  function drawStone(
    ctx: CanvasRenderingContext2D,
    h: number,
    body: Body,
    stone: Entity,
  ) {
    const at = body.width * stone.along;
    ctx.beginPath();
    ctx.moveTo(at, body.top - 2);
    ctx.lineTo(at + 3, body.top + 4);
    ctx.lineTo(at - 3, body.top + 4);
    ctx.closePath();
    ctx.fillStyle = css(stone.tint, 0.9);
    ctx.fill();
    void h;
  }

  /**
   * A deck with nothing on it: bare bed, no water.
   *
   * Drawn rather than left blank, because an empty lane and a missing lane look
   * the same and mean different things — one is a deck waiting for a track, the
   * other is a deck that is not there.
   */
  function drawDryBed(ctx: CanvasRenderingContext2D, w: number, h: number) {
    ctx.strokeStyle = "hsl(30 6% 45% / 0.35)";
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 6]);
    ctx.beginPath();
    ctx.moveTo(0, h * 0.72);
    ctx.lineTo(w, h * 0.72);
    ctx.stroke();
    ctx.setLineDash([]);
  }

  /**
   * The peripheral claim.
   *
   * Luminance and area, never hue: the periphery reads brightness and motion
   * well and colour badly, so a warning carried by colour is one a DJ watching
   * the room never receives. The onset sweeps once; after it the edge stays
   * lit, which is the static form the claim settles into.
   */
  function drawAlarm(ctx: CanvasRenderingContext2D, w: number, h: number) {
    const age = alarmedAt == null ? ONSET_MS : performance.now() - alarmedAt;
    const onset = still ? 0 : Math.max(0, 1 - age / ONSET_MS);

    // The settled form: a lit edge, for as long as the claim holds.
    ctx.fillStyle = "hsl(0 0% 100% / 0.55)";
    ctx.fillRect(0, 0, w, 2);

    if (onset <= 0) return;
    // The onset: one bright sweep downstream, fading as it goes.
    const at = (1 - onset) * w;
    const glow = ctx.createLinearGradient(at - w * 0.25, 0, at, 0);
    glow.addColorStop(0, "hsl(0 0% 100% / 0)");
    glow.addColorStop(1, `hsl(0 0% 100% / ${0.3 * onset})`);
    ctx.fillStyle = glow;
    ctx.fillRect(at - w * 0.25, 0, w * 0.25, h);
  }

  type Body = ReturnType<typeof bodyOf>;

  /**
   * The water's gradient, rebuilt only when it would actually differ.
   *
   * `createLinearGradient` plus two `addColorStop`s per frame was the single
   * most expensive thing in this component — the rendering benchmark measured
   * flat-filled discs and so under-counted gradient work considerably. The
   * gradient depends on the tint and the two y coordinates, none of which
   * change on most frames.
   */
  let cachedGradient: CanvasGradient | null = null;
  let cacheKey = "";

  function waterGradient(ctx: CanvasRenderingContext2D, top: number, h: number): CanvasGradient {
    const key = `${river.tint.hue}|${river.tint.saturation}|${river.tint.lightness}|${Math.round(top)}|${Math.round(h)}`;
    if (cachedGradient && key === cacheKey) return cachedGradient;
    const gradient = ctx.createLinearGradient(0, top, 0, h);
    gradient.addColorStop(0, css(river.tint, 0.85));
    // Deeper water is darker, which is also the low stratum of the EQ column
    // the mixer will draw into later.
    gradient.addColorStop(1, css({ ...river.tint, lightness: river.tint.lightness * 0.45 }, 0.95));
    cachedGradient = gradient;
    cacheKey = key;
    return gradient;
  }

  /** The water itself: depth is level, colour is key, paleness is certainty. */
  function drawWater(
    ctx: CanvasRenderingContext2D,
    w: number,
    h: number,
    body: Body,
    phase: number,
  ) {
    // The surface breathes on the beat, within the excursion the world allows.
    const swell =
      river.vitality.depth *
      river.vitality.excursion.drift *
      Math.cos(phase * Math.PI * 2) *
      (body.depth * 0.12);

    const gradient = waterGradient(ctx, body.top, h);

    ctx.beginPath();
    ctx.moveTo(0, body.top + swell);
    // One gentle bend rather than a straight edge: a straight line reads as a
    // bar chart, and the point is that this is water.
    ctx.bezierCurveTo(w * 0.3, body.top - swell, w * 0.7, body.top + swell * 2, w, body.top - swell);
    ctx.lineTo(w, h);
    ctx.lineTo(0, h);
    ctx.closePath();
    ctx.fillStyle = gradient;
    ctx.fill();
  }

  /**
   * The crests — the beat, travelling.
   *
   * This is the channel that answers "are these two in time?": two synced decks
   * share a phase, so their crests sit at the same place across lanes, and two
   * that are not visibly slide against each other.
   */
  function drawCrests(ctx: CanvasRenderingContext2D, w: number, body: Body, phase: number) {
    if (!(river.vitality.pulse_bpm > 0) || river.vitality.depth <= 0.001) return;

    const beats = 4;
    ctx.lineWidth = 1.5;
    for (let n = 0; n < beats; n += 1) {
      // Downstream is the future, so crests travel left to right.
      const at = ((n + phase) / beats) * w;
      // The downbeat is the bright one; a bar you can count is worth more than
      // four identical pulses.
      const strength = n === 0 ? 0.55 : 0.22;
      ctx.beginPath();
      ctx.moveTo(at, body.top);
      ctx.lineTo(at, body.top + body.depth);
      ctx.strokeStyle = `hsl(0 0% 100% / ${strength * river.vitality.depth})`;
      ctx.stroke();
    }
  }

  /**
   * Murk, where the grid is not trusted.
   *
   * The engine already refuses auto-sync below a confidence threshold, and the
   * reason for that is currently a tooltip. As clarity it needs no explaining:
   * you do not navigate water you cannot see through.
   */
  function drawTurbidity(ctx: CanvasRenderingContext2D, w: number, body: Body) {
    const murk = river.vitality.turbidity;
    if (murk <= 0.001) return;
    ctx.fillStyle = `hsl(30 12% 55% / ${murk * 0.45})`;
    ctx.fillRect(0, body.top, w, body.depth);
  }

  /** Where along the river we are. Structural: this is time, not music. */
  function drawPlayhead(ctx: CanvasRenderingContext2D, h: number, body: Body) {
    const at = body.width * river.along;
    ctx.beginPath();
    ctx.moveTo(at, body.top - 4);
    ctx.lineTo(at, h);
    ctx.strokeStyle = "hsl(0 0% 90% / 0.8)";
    ctx.lineWidth = 1;
    ctx.stroke();
  }

  /**
   * The mouth: the end of the track, visible from far off.
   *
   * The one thing a DJ wants in peripheral vision while looking at something
   * else, which a number turning red cannot do.
   */
  function drawMouth(
    ctx: CanvasRenderingContext2D,
    w: number,
    h: number,
    end: Entity,
  ) {
    const nearness = end.extent;
    if (nearness <= 0.001) return;
    const reach = w * 0.35 * nearness;
    const gradient = ctx.createLinearGradient(w - reach, 0, w, 0);
    gradient.addColorStop(0, "hsl(0 0% 0% / 0)");
    gradient.addColorStop(1, css(end.tint, 0.55 * nearness));
    ctx.fillStyle = gradient;
    ctx.fillRect(w - reach, 0, reach, h);
  }
</script>

<!--
  The canvas is decoration in the strict sense: everything it says is also in
  the reading below, which is what makes it safe to hide from assistive
  technology rather than exposing an unlabelled image.
-->
<div class="river" style:height="{height}px">
  <canvas bind:this={canvas} aria-hidden="true"></canvas>
  <p class="reading mono">{river.reading}</p>
</div>

<style>
  .river {
    position: relative;
    border-radius: 6px;
    overflow: hidden;
    background: var(--panel-raised);
  }

  canvas {
    display: block;
    width: 100%;
    height: 100%;
  }

  /*
    Text stays in the document, never in the canvas: canvas text ignores the
    system's font rendering and the user's size preference, and a DJ who set
    their font to 20px meant it.
  */
  .reading {
    position: absolute;
    left: 0.5rem;
    top: 0.35rem;
    margin: 0;
    font-size: 0.75em;
    color: var(--text);
    text-shadow: 0 1px 3px rgb(0 0 0 / 0.7);
    pointer-events: none;
  }
</style>
