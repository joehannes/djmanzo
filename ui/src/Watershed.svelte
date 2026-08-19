<script lang="ts">
  /**
   * The whole world, on one surface.
   *
   * One canvas rather than one per lane, for the reason the rendering benchmark
   * found: on a machine without acceleration the cost is document
   * invalidation, and one self-repainting surface beats N animating ones by a
   * wide margin. It is also the only shape a GL renderer could take — a WebGL
   * context per lane would be absurd.
   *
   * # Canvas paints, DOM listens
   *
   * Everything drawn here is foliage: it reports state and nothing else, so
   * there is no hit target to get wrong. The readings sit in real elements
   * above it, which is what a screen reader finds and what stays legible when
   * the canvas is not drawn at all.
   *
   * See docs/adr/0009-the-living-interface.md.
   */
  import { onMount } from "svelte";
  import * as canvas2d from "./render/canvas2d";
  import * as webgl from "./render/webgl";
  import { build, type Lane, type Scene } from "./render/scene";
  import type { Tier, World } from "./world";

  let {
    world,
    decks,
    laneHeight = 78,
    mixerHeight = 72,
    latencyMs = 0,
    tier = "living",
    accelerate = true,
    ondriver,
  }: {
    world: World;
    /** Which decks are on screen, in order. */
    decks: number[];
    laneHeight?: number;
    mixerHeight?: number;
    latencyMs?: number;
    tier?: Tier;
    /** Whether to try WebGL at all. Off falls back to Canvas 2D. */
    accelerate?: boolean;
    /** Reports which backend and driver ended up being used. */
    ondriver?: (what: string) => void;
  } = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  let host = $state<HTMLDivElement | null>(null);
  let width = $state(0);
  let readAt = performance.now();

  $effect(() => {
    void world;
    readAt = performance.now();
  });

  const still = $derived(tier === "still");

  /**
   * The mixer sits above the decks: it is where they *arrive*, and a DJ reads
   * the mix first and the sources second.
   */
  const lanes = $derived.by((): Lane[] => {
    const out: Lane[] = [{ of: "mixer", y: 0, height: mixerHeight }];
    let y = mixerHeight + 4;
    for (const deck of decks) {
      out.push({ of: `deck.${deck}`, y, height: laneHeight });
      y += laneHeight + 4;
    }
    return out;
  });

  const totalHeight = $derived(
    mixerHeight + 4 + decks.length * (laneHeight + 4),
  );

  const animating = $derived(
    !still &&
      world.entities.some(
        (e) => e.name === "deck.river" && e.vitality.depth > 0.001,
      ),
  );

  /** Labels, positioned by the scene and rendered as real elements. */
  let labels = $state<Scene["labels"]>([]);

  let gl: webgl.GlRenderer | null = null;

  onMount(() => {
    const observer = new ResizeObserver(() => {
      width = host?.clientWidth ?? 0;
    });
    if (host) {
      observer.observe(host);
      width = host.clientWidth;
    }
    return () => observer.disconnect();
  });

  // Choosing a backend is a side effect with a lifetime, so it lives in its own
  // effect rather than inside the draw loop — a context created per frame would
  // exhaust the browser's limit within seconds.
  $effect(() => {
    const el = canvas;
    if (!el) return;
    if (!accelerate) {
      gl = null;
      ondriver?.("Canvas 2D");
      return;
    }
    gl = webgl.create(el);
    // The driver string is reported and never *trusted* — the benchmark caught
    // WebKitGTK claiming "Apple GPU" on a box with no GPU. It is a label for a
    // log, not an input to a decision.
    ondriver?.(gl ? `WebGL · ${gl.driver}` : "Canvas 2D (no GL context)");
    return () => {
      gl?.dispose();
      gl = null;
    };
  });

  $effect(() => {
    if (!canvas || width === 0) return;
    let frame = 0;
    let stop = false;

    const paint = () => {
      const el = canvas;
      if (!el) return;
      const ratio = window.devicePixelRatio || 1;
      const w = Math.round(width * ratio);
      const h = Math.round(totalHeight * ratio);
      if (el.width !== w || el.height !== h) {
        el.width = w;
        el.height = h;
      }
      const seconds = still ? 0 : (performance.now() - readAt) / 1000;
      const scene = build(world, lanes, width, seconds, latencyMs, still);
      labels = scene.labels;

      if (gl) {
        gl.draw(scene);
        return;
      }
      const ctx = el.getContext("2d");
      if (!ctx) return;
      ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
      canvas2d.draw(ctx, scene);
    };

    const loop = () => {
      if (stop) return;
      paint();
      frame = requestAnimationFrame(loop);
    };

    // Stillness is a rule about what the machine does as much as what a DJ
    // sees: nothing moving means nothing to repaint.
    if (animating) loop();
    else paint();

    return () => {
      stop = true;
      cancelAnimationFrame(frame);
    };
  });
</script>

<div class="watershed" bind:this={host} style:height="{totalHeight}px">
  <canvas bind:this={canvas} style:height="{totalHeight}px" aria-hidden="true"></canvas>
  <!--
    Text stays in the document, never in the canvas: canvas text ignores the
    system's font rendering and the user's size preference, and a DJ who set
    their font to 20px meant it.
  -->
  {#each labels as label (label.of)}
    <p class="reading mono" style:left="{label.x}px" style:top="{label.y}px">
      {label.text}
    </p>
  {/each}
</div>

<style>
  .watershed {
    position: relative;
    width: 100%;
  }

  canvas {
    display: block;
    width: 100%;
  }

  .reading {
    position: absolute;
    margin: 0;
    font-size: 0.75em;
    color: var(--text);
    text-shadow: 0 1px 3px rgb(0 0 0 / 0.75);
    pointer-events: none;
    white-space: nowrap;
  }
</style>
