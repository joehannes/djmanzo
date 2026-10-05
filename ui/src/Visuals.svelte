<script lang="ts">
  /**
   * §122: a look's layers, drawn — one canvas, filling whatever holds it,
   * transparent wherever the layers are, so what is beneath shows through.
   * See `docs/VISUALS.md`.
   *
   * The look is checked by `./looks` (a layer this interface cannot draw is
   * left out and named), drawn by `./visualStack` (a picture that will not
   * compile is left out and named), and fed the mix as it is heard: the
   * master's bands, each stem weighed by its deck's share, the beat, the
   * singers' microphones — followed as a light follows sound, so peaks do
   * not strobe.
   *
   * It draws nothing — and the canvas stays clear — with no look, with no
   * WebGL, once the graphics context is lost, and when the system asks for
   * reduced motion. Nothing here can take down what it is drawn behind.
   *
   * An `artist` layer shows photographs of the heard deck's artist, kept by
   * Rust from Wikimedia Commons, each freely licensed: its credit is drawn
   * in the corner for as long as it shows, as its licence asks.
   *
   * The 60-a-second work writes to the canvas from a frame loop; nothing
   * reactive moves at that rate.
   */
  import {
    artistPicture,
    artistPictures,
    milkdropPreset,
    type AudioMetrics,
    type DeckState,
    type Look,
  } from "./api";
  import { untrack } from "svelte";
  import { current, offer, start, step, type Show } from "./artistShow";
  import { follow, heardArtist, palette, phase, signals, SILENT, validate, type Hearing, type Signals } from "./looks";
  import { createMilkdrop, type Milkdrop } from "./milkdrop";
  import { createStack, type Stack } from "./visualStack";

  let {
    look,
    decks,
    crossfader,
    audio,
    voices = [],
    shades = [],
  }: {
    look: Look | null;
    decks: DeckState[];
    crossfader: number;
    audio: AudioMetrics;
    voices?: number[];
    /** The record's colours, `singer_backdrop`'s. */
    shades?: { colour: string }[];
  } = $props();

  let canvas = $state<HTMLCanvasElement | undefined>();
  let creditLine = $state<HTMLParagraphElement | undefined>();
  /** What the canvas is doing, for whoever looks: a test, or a DJ in the inspector. */
  let status = $state<"off" | "drawing" | "no-webgl" | "lost" | "still">("off");
  let failures = $state<string[]>([]);

  /** The canvas's size against the screen's, each way. */
  const SCALE = 0.5;
  /** Frames a second at most. */
  const FPS = 30;

  const checked = $derived(look ? validate(look) : { layers: [], dropped: [] });
  const colours = $derived(palette(shades));

  /** The latest of everything, for the frame loop: plain, read without subscribing. */
  let latest: { hearing: Hearing; layers: typeof checked.layers; colours: typeof colours } = {
    hearing: { decks: [], crossfader: 0, audio: { loudness: 0, bands: [0, 0, 0, 0] }, voices: [] },
    layers: [],
    colours: palette([]),
  };
  $effect(() => {
    latest = {
      hearing: { decks: decks as Hearing["decks"], crossfader, audio, voices },
      layers: checked.layers,
      colours,
    };
  });

  $effect(() => {
    for (const note of checked.dropped) console.warn(`[visuals] left out: ${note}`);
  });

  /**
   * The artist's photographs, for the frame loop: a plain object it steps,
   * offered a new set whenever the heard artist's are loaded.
   */
  const show: Show<{ image: HTMLImageElement; credit: string }> = start();
  const wantsArtist = $derived(checked.layers.some((l) => l.kind === "artist"));
  /** Whose photographs: the heard deck and its artist, kept while nothing is heard. */
  let followed = $state<{ deck: number; artist: string } | null>(null);
  $effect(() => {
    const heard = heardArtist(decks as Parameters<typeof heardArtist>[0], crossfader);
    const was = untrack(() => followed);
    if (heard && (heard.deck !== was?.deck || heard.artist !== was?.artist)) followed = heard;
  });
  /** Where the photographs are, for whoever looks: as Rust says, then loading, showing, or failed. */
  let artistState = $state("");
  $effect(() => {
    const whose = followed;
    if (!wantsArtist || status !== "drawing" || !whose) {
      offer(show, []);
      artistState = "";
      return;
    }
    if (!whose.artist) {
      offer(show, []);
      artistState = "unknown";
      return;
    }
    let alive = true;
    let again = 0;
    const ask = async () => {
      try {
        const answer = await artistPictures(whose.deck);
        if (!alive) return;
        artistState = answer.state;
        if (answer.state === "asking" || answer.state === "waiting") {
          // Asked in the background; a failure is waited on by Rust before
          // it is asked again, so asking about it often costs nothing.
          again = window.setTimeout(ask, answer.state === "asking" ? 3000 : 60000);
          return;
        }
        if (answer.state !== "found" || !answer.key) {
          offer(show, []);
          return;
        }
        artistState = "loading";
        const loaded: { image: HTMLImageElement; credit: string }[] = [];
        for (const [n, picture] of answer.pictures.entries()) {
          // A data URL: the page's own picture, which WebGL will draw —
          // one from an application scheme is another origin's — and the
          // same on every webview, where raw bytes reach WebKitGTK's page as
          // an array of numbers and Chromium's as an ArrayBuffer.
          const image = new Image();
          image.src = await artistPicture(answer.key, n);
          await image.decode();
          if (!alive) return;
          loaded.push({ image, credit: picture.credit });
        }
        offer(show, loaded);
        artistState = "showing";
      } catch (error) {
        if (!alive) return;
        artistState = "failed";
        console.warn(`[visuals] no photographs of ${whose.artist}: ${error instanceof Error ? error.message : String(error)}`);
        // Asked again in a moment: a record restored onto a deck as the
        // application starts can be asked about before the collection has
        // it, and one failure must not leave the layer empty all night.
        again = window.setTimeout(ask, 5000);
      }
    };
    void ask();
    return () => {
      alive = false;
      window.clearTimeout(again);
    };
  });

  /** On or off, apart from which look: changing looks keeps the context. */
  const on = $derived(look !== null);

  /**
   * The canvas's size for a screen of this size: half each way, at most half
   * a 1080p projector's. A photograph is drawn whole up to the same size: a
   * soft field stretched looks as it would drawn whole, and a face does not.
   */
  function sized(target: HTMLCanvasElement, sharp = false) {
    const scale = sharp ? 1 : SCALE;
    return {
      width: Math.min(960, Math.max(1, Math.round(target.clientWidth * scale))),
      height: Math.min(540, Math.max(1, Math.round(target.clientHeight * scale))),
    };
  }

  /**
   * Butterchurn, when the look has a MilkDrop layer and the stack is drawing:
   * a plain variable the frame loop reads, made again when the preset changes.
   * If it cannot be made — no WebGL 2, WebAssembly refused, a preset it will
   * not compile — the layer is left out and the reason said, as a picture
   * that will not compile is.
   */
  const presetName = $derived(checked.layers.find((l) => l.kind === "milkdrop")?.preset ?? null);
  let milkdrop: Milkdrop | null = null;
  let milkdropFailed = $state<string | null>(null);
  /** Where Butterchurn is, for whoever looks: loading, drawing, or failed. */
  let milkdropState = $state<"" | "loading" | "drawing" | "failed">("");
  $effect(() => {
    const name = presetName;
    if (!canvas || status !== "drawing" || !name) return;
    let alive = true;
    milkdropState = "loading";
    const { width, height } = sized(canvas);
    void milkdropPreset(name)
      .then((preset) => createMilkdrop(preset, width, height))
      .then((made) => {
        if (!alive) return;
        milkdrop = made;
        milkdropState = "drawing";
      })
      .catch((error) => {
        if (!alive) return;
        milkdropState = "failed";
        milkdropFailed = `milkdrop: ${error instanceof Error ? error.message : String(error)}`;
        console.warn(`[visuals] would not draw ${milkdropFailed}`);
      });
    return () => {
      alive = false;
      milkdrop = null;
      milkdropFailed = null;
      milkdropState = "";
    };
  });

  $effect(() => {
    if (!canvas || !on) {
      status = "off";
      return;
    }
    const target = canvas;
    const still = window.matchMedia?.("(prefers-reduced-motion: reduce)");
    if (still?.matches) {
      status = "still";
      return;
    }
    const stack: Stack | null = createStack(target);
    if (!stack) {
      status = "no-webgl";
      return;
    }
    status = "drawing";

    // Half the screen's size each way, stretched to fill it, and no more than
    // half a 1080p projector's: the layers are soft fields and glows, and a
    // stretched one looks as it would drawn whole. Where the webview draws
    // WebGL with the processor — and WebKitGTK will not say whether it does —
    // that is a quarter of the work: measured under llvmpipe, five layers at
    // full size and sixty a second took a core and a sixth more than K1's
    // background alone, and a third of a core at this size and thirty.
    let sharp = false;
    const fit = () => {
      const { width, height } = sized(target, sharp);
      if (target.width !== width) target.width = width;
      if (target.height !== height) target.height = height;
    };
    fit();
    const watch = new ResizeObserver(fit);
    watch.observe(target);

    let followed: Signals = SILENT;
    let before = performance.now();
    let frame = 0;
    let reported = 0;
    const tick = (now: number) => {
      frame = requestAnimationFrame(tick);
      // Thirty a second, whatever the display runs at; a little early is
      // allowed, so a 60 Hz display does not fall to twenty.
      if (now - before < 1000 / FPS - 4) return;
      const seconds = (now - before) / 1000;
      before = now;
      followed = follow(followed, signals(latest.hearing), seconds);
      const pictures: Parameters<Stack["draw"]>[0]["pictures"] = {};
      if (milkdrop) {
        try {
          milkdrop.resize(target.width, target.height);
          milkdrop.render(latest.hearing.audio.bands, seconds);
          pictures.milkdrop = milkdrop.canvas;
        } catch (error) {
          milkdrop = null;
          milkdropState = "failed";
          milkdropFailed = `milkdrop: ${error instanceof Error ? error.message : String(error)}`;
          console.warn(`[visuals] stopped drawing ${milkdropFailed}`);
        }
      }
      const photographed = latest.layers.some((l) => l.kind === "artist");
      if (photographed !== sharp) {
        sharp = photographed;
        fit();
      }
      step(show, seconds);
      const photo = current(show);
      if (photo) pictures.artist = photo.image;
      // The photograph fades as it gives way, and its credit with it.
      const layers =
        show.fade < 1 ? latest.layers.map((l) => (l.kind === "artist" ? { ...l, amount: l.amount * show.fade } : l)) : latest.layers;
      if (creditLine) {
        const credited = photo && latest.layers.some((l) => l.kind === "artist") ? photo.credit : "";
        if (creditLine.textContent !== credited) creditLine.textContent = credited;
        // As big as the picture it credits, not the window: legible on a
        // projector, and a corner of a preview rather than a third of it.
        const size = `${Math.round(Math.min(18, Math.max(9, target.clientHeight * 0.03)))}px`;
        if (creditLine.style.fontSize !== size) creditLine.style.fontSize = size;
        creditLine.style.opacity = credited ? String(show.fade) : "0";
      }
      stack.draw({
        layers,
        signals: followed,
        phase: phase(latest.hearing),
        colours: latest.colours,
        time: now / 1000,
        pictures,
      });
      if (stack.lost()) {
        status = "lost";
        cancelAnimationFrame(frame);
        return;
      }
      const said = stack.failures();
      if (said.length !== reported) {
        for (const note of said.slice(reported)) console.warn(`[visuals] would not draw ${note}`);
        reported = said.length;
        failures = said;
      }
    };
    frame = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(frame);
      watch.disconnect();
      stack.dispose();
      failures = [];
      if (creditLine) {
        creditLine.textContent = "";
        creditLine.style.opacity = "0";
      }
    };
  });
</script>

<canvas
  class="visuals"
  bind:this={canvas}
  aria-hidden="true"
  data-visuals={status}
  data-look={look?.name ?? ""}
  data-layers={checked.layers.map((l) => l.kind).join(" ")}
  data-dropped={checked.dropped.length}
  data-failures={[...failures, ...(milkdropFailed ? [milkdropFailed] : [])].join(" | ")}
  data-milkdrop={presetName ?? ""}
  data-milkdrop-state={milkdropState}
  data-artist={artistState}
></canvas>
{#if wantsArtist}
  <!-- The photograph's credit, as its licence asks: who took it, under what, from where. -->
  <p class="credit" bind:this={creditLine} data-credit></p>
{/if}

<style>
  .credit {
    position: absolute;
    right: 0.8em;
    bottom: 0.6em;
    max-width: min(60%, 42em);
    margin: 0;
    padding: 0.2em 0.6em;
    border-radius: 0.4em;
    background: rgb(0 0 0 / 0.45);
    color: rgb(255 255 255 / 0.85);
    font-size: 11px;
    line-height: 1.3;
    text-align: right;
    pointer-events: none;
    opacity: 0;
    transition: none;
  }
  .credit:empty {
    display: none;
  }
  .visuals {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
    pointer-events: none;
  }
</style>
