<script lang="ts">
  /**
   * Where two rivers meet.
   *
   * The first thing in the living interface that says something no rectangle
   * could, and the reason is the *beating* — whether two decks are locked, sat
   * at an offset, or sliding apart. Those are three different actions (nothing,
   * a nudge, the pitch fader), and every conventional interface shows them as
   * one thing: two BPM numbers a DJ has to compare in their head.
   *
   * See docs/VISUAL-LANGUAGE.md §2, "The confluence".
   */
  import { css, phaseAt, type Beating, type Entity, type Tier } from "./world";

  let {
    confluence,
    banks,
    beating,
    seam = false,
    height = 56,
    latencyMs = 0,
    tier = "living",
  }: {
    confluence: Entity;
    /** The two rivers meeting here, left bank first. Either may be absent. */
    banks: [Entity | null, Entity | null];
    beating: Beating;
    /** True when the two keys will not mix. */
    seam?: boolean;
    height?: number;
    latencyMs?: number;
    tier?: Tier;
  } = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  let readAt = performance.now();

  $effect(() => {
    void banks;
    readAt = performance.now();
  });

  const still = $derived(tier === "still");
  const animating = $derived(
    !still && banks.some((b) => b != null && b.vitality.depth > 0.001),
  );

  /**
   * What the beating says, in words.
   *
   * The precision half of "nature carries the gestalt, digits carry the
   * precision" — and here the words are doing more than confirming the picture,
   * because they name the control. A DJ who has not yet learned the shapes can
   * still read "nudge back" and know what to do.
   */
  const verdict = $derived.by(() => {
    if (beating === "Unknown") return "";
    if (beating === "Locked") return "locked";
    if ("Offset" in beating) {
      const beats = beating.Offset.beats;
      return `${beats > 0 ? "nudge back" : "nudge forward"} ${Math.abs(beats).toFixed(2)} beat`;
    }
    const difference = beating.Sliding.bpm_difference;
    return `sliding ${difference > 0 ? "+" : ""}${difference.toFixed(1)} BPM`;
  });

  onMountish();

  function onMountish() {
    $effect(() => {
      if (!canvas) return;
      let frame = 0;
      let stop = false;

      const loop = () => {
        if (stop) return;
        if (canvas) paint(canvas);
        frame = requestAnimationFrame(loop);
      };

      if (animating) {
        loop();
      } else if (canvas) {
        paint(canvas);
      }

      return () => {
        stop = true;
        cancelAnimationFrame(frame);
      };
    });
  }

  // A still confluence repaints when its state changes, and only then.
  $effect(() => {
    void confluence;
    void beating;
    void seam;
    if (!animating && canvas) paint(canvas);
  });

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

    const seconds = still ? 0 : (performance.now() - readAt) / 1000;

    drawBanks(ctx, w, h, seconds);
    if (seam) drawSeam(ctx, w, h);
    drawCrests(ctx, w, h, seconds);
    drawConstriction(ctx, w, h);
  }

  /**
   * The two tributaries converging on the meeting point.
   *
   * The crossfader is *where* they meet — `confluence.along` — so a hard-left
   * fader puts the junction at the left edge and only the left river's water
   * carries on downstream. That is the control's actual meaning, drawn.
   */
  function drawBanks(ctx: CanvasRenderingContext2D, w: number, h: number, seconds: number) {
    const meet = w * confluence.along;
    const mid = h * 0.5;

    banks.forEach((bank, index) => {
      if (!bank) return;
      const fromTop = index === 0;
      // Clear of the reading, which sits at the top. Text is trunk and must
      // stay legible; the water gives way to it, not the other way round.
      const startY = fromTop ? h * 0.24 : h * 0.9;
      // How much of this side survives the crossfader. The near side keeps its
      // width; the far side is pinched off, which is what a crossfader does.
      const survives = fromTop ? 1 - confluence.along : confluence.along;
      const width = Math.max(1, h * 0.18 * bank.extent * (0.25 + survives * 0.75));

      ctx.beginPath();
      ctx.moveTo(0, startY);
      ctx.bezierCurveTo(meet * 0.5, startY, meet * 0.7, mid, meet, mid);
      ctx.lineWidth = width;
      ctx.lineCap = "round";
      ctx.strokeStyle = css(bank.tint, 0.8);
      ctx.stroke();
      void seconds;
    });

    // Downstream of the meeting point is the *merge*, drawn once. Letting each
    // bank run to the right edge painted the same stretch twice, so whichever
    // was drawn last won — which said the crossfader had no effect at all.
    const merged = h * 0.2 * confluence.extent;
    ctx.beginPath();
    ctx.moveTo(meet, mid);
    ctx.lineTo(w, mid);
    ctx.lineWidth = Math.max(1, merged);
    ctx.lineCap = "butt";
    ctx.strokeStyle = css(confluence.tint, 0.9);
    ctx.stroke();
  }

  /**
   * Keys that will not mix.
   *
   * A seam down the middle rather than a warning colour: hue already means key,
   * and one man in twelve could not read a colour-coded warning anyway. This is
   * the redundant channel doing its job — behaviour, not colour.
   */
  function drawSeam(ctx: CanvasRenderingContext2D, w: number, h: number) {
    const meet = w * confluence.along;
    ctx.beginPath();
    ctx.moveTo(meet, h * 0.5);
    ctx.lineTo(w, h * 0.5);
    ctx.lineWidth = 1.5;
    ctx.setLineDash([3, 3]);
    ctx.strokeStyle = "hsl(40 60% 85% / 0.95)";
    ctx.stroke();
    ctx.setLineDash([]);
  }

  /**
   * The crests arriving from each side.
   *
   * This is the channel that answers *are these two in time?* — the crests are
   * drawn at each bank's own phase, so locked decks put their marks in the same
   * place and a slide is two marks visibly walking apart. Nothing decides for
   * the DJ: the interference is simply shown, which is what being out of time
   * actually looks like.
   */
  function drawCrests(ctx: CanvasRenderingContext2D, w: number, h: number, seconds: number) {
    const meet = w * confluence.along;
    const run = Math.max(8, meet);

    banks.forEach((bank, index) => {
      if (!bank || bank.vitality.depth <= 0.001 || !(bank.vitality.pulse_bpm > 0)) return;
      const phase = phaseAt(bank.vitality, seconds, latencyMs);
      const y = index === 0 ? h * 0.36 : h * 0.78;
      // One crest per side, at that side's phase along the run into the meeting
      // point. Two marks at the same x means two decks in time.
      const at = run * phase;
      ctx.beginPath();
      ctx.arc(at, y, 3, 0, Math.PI * 2);
      ctx.fillStyle = "hsl(0 0% 100% / 0.85)";
      ctx.fill();
    });
  }

  /**
   * The estuary's banks: fixed, and the water squeezed through them.
   *
   * `confluence.extent` is what the limiter has left. A DJ sees the mix being
   * crushed instead of reading a gain-reduction number — and the number is
   * still in the reading, because both.
   */
  function drawConstriction(ctx: CanvasRenderingContext2D, w: number, h: number) {
    const open = confluence.extent;
    if (open >= 0.999) return;
    const squeeze = (1 - open) * h * 0.3;
    ctx.fillStyle = "hsl(20 40% 70% / 0.5)";
    ctx.fillRect(w - 3, h * 0.5 - squeeze, 3, squeeze * 2);
  }
</script>

<div class="confluence" style:height="{height}px">
  <canvas bind:this={canvas} aria-hidden="true"></canvas>
  <!--
    The words, not a duplicate of the picture: they name the *control* to reach
    for, which the shapes cannot. A DJ who has not learned the vocabulary yet
    can still read "nudge back" and act on it.
  -->
  <p class="reading mono">
    <span class="where">{confluence.reading}</span>
    {#if verdict}
      <span class="verdict" class:locked={beating === "Locked"}>{verdict}</span>
    {/if}
    {#if seam}<span class="verdict warn">keys clash</span>{/if}
  </p>
</div>

<style>
  .confluence {
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

  .reading {
    position: absolute;
    left: 0.5rem;
    top: 0.35rem;
    display: flex;
    gap: 0.6rem;
    margin: 0;
    font-size: 0.75em;
    color: var(--text);
    text-shadow: 0 1px 3px rgb(0 0 0 / 0.7);
    pointer-events: none;
  }

  .where {
    color: var(--text-dim);
  }

  .verdict.locked {
    color: var(--accent-2, #22d3aa);
  }

  .verdict.warn {
    color: var(--warn, #d8a657);
  }
</style>
