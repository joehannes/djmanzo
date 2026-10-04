<script lang="ts">
  /**
   * §122: the picture of a video file on a deck, for a projector.
   *
   * Its sound is the deck's, like any record's. This draws only the picture,
   * with the webview's own `<video>` — djmanzo decodes no video — muted, and
   * kept to the deck: its rate is the deck's, and it is moved when it drifts.
   * What is seen is the mix: each deck's picture in its share of what is heard,
   * so pictures crossfade as the sound does (`videoMix.ts`) — and a paused
   * deck, not heard, is not seen, so a record being cued never reaches the
   * projector. Popped out, the
   * panel is *djmanzo - Video*, the window for the projector. See
   * `docs/VIDEO.md`.
   *
   * The 60-a-second work — following, crossfading — writes to the elements
   * directly from a frame loop; nothing reactive moves at that rate.
   */
  import { deckPictures, type DeckPicture, type DeckState } from "./api";
  import { follow, stack, type DeckForPicture } from "./videoMix";

  let { decks, crossfader }: { decks: DeckState[]; crossfader: number } = $props();

  const PIN_KEY = "djmanzo.video.pinned";
  const FILL_KEY = "djmanzo.video.fill";

  function remembered(key: string): string {
    try {
      return localStorage.getItem(key) ?? "";
    } catch {
      return "";
    }
  }

  function remember(key: string, value: string) {
    try {
      localStorage.setItem(key, value);
    } catch {
      // A private window: the choice lasts as long as the panel.
    }
  }

  let pictures = $state<DeckPicture[]>([]);
  let asked = $state(false);
  /** One deck's picture alone, or the mix (null). */
  let pinned = $state<number | null>(Number(remembered(PIN_KEY)) || null);
  let fill = $state(remembered(FILL_KEY) === "1");
  let clean = $state(false);
  /** Decks whose picture this webview could not play. */
  let unplayable = $state<number[]>([]);
  /** What the webview said about each, for a report. */
  let said = $state<Record<number, string>>({});
  let stage = $state<HTMLDivElement | undefined>();

  /**
   * The latest decks and crossfader, for the frame loop: plain variables it
   * reads sixty times a second without subscribing to anything.
   */
  let latest: { decks: DeckForPicture[]; crossfader: number } = { decks: [], crossfader: 0 };
  $effect(() => {
    latest = { decks: decks as DeckForPicture[], crossfader };
  });

  $effect(() => {
    const look = () =>
      void deckPictures()
        .then((now) => {
          pictures = now;
          asked = true;
          unplayable = unplayable.filter((d) => now.some((p) => p.deck === d));
        })
        .catch(() => {});
    look();
    const poll = setInterval(look, 1_000);
    return () => clearInterval(poll);
  });

  // The frame loop: each picture kept to its deck, and drawn in its share.
  $effect(() => {
    let frame = 0;
    const tick = () => {
      frame = requestAnimationFrame(tick);
      if (!stage) return;
      const shown = stack(
        latest.decks,
        pictures.map((p) => p.deck),
        latest.crossfader,
        pinned,
      );
      stage.style.setProperty("--whole", String(shown.whole));
      for (const element of stage.querySelectorAll<HTMLVideoElement>("video[data-deck]")) {
        const number = Number(element.dataset.deck);
        const at = shown.layers.findIndex((l) => l.deck === number);
        const deck = latest.decks.find((d) => d.number === number);
        if (at < 0 || !deck) {
          // Not seen: not decoded either.
          element.style.opacity = "0";
          if (!element.paused) element.pause();
          continue;
        }
        element.style.opacity = String(shown.layers[at].opacity);
        element.style.zIndex = String(at + 1);
        if (element.readyState < 1) continue;
        const to = follow(deck, element.currentTime);
        if (to.seek !== null && !element.seeking) element.currentTime = to.seek;
        if (to.play) {
          if (element.playbackRate !== to.rate) element.playbackRate = to.rate;
          if (element.paused) void element.play().catch(() => {});
        } else if (!element.paused) {
          element.pause();
        }
      }
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  });

  $effect(() => remember(PIN_KEY, pinned === null ? "" : String(pinned)));
  $effect(() => remember(FILL_KEY, fill ? "1" : "0"));

  function failed(deck: number, element: HTMLVideoElement) {
    if (!unplayable.includes(deck)) unplayable = [...unplayable, deck];
    const error = element.error;
    if (error) said = { ...said, [deck]: `${error.code}${error.message ? `: ${error.message}` : ""}` };
  }

  /** Why a picture will not play, in the DJ's terms. */
  function why(picture: DeckPicture): string {
    const linux = navigator.userAgent.includes("Linux") && !navigator.userAgent.includes("Android");
    const h264 = picture.kind === "video/mp4" || picture.kind === "video/quicktime";
    if (linux && h264) {
      return `Deck ${picture.deck}'s picture will not play here. On Linux, H.264 video needs the distribution's gstreamer1.0-libav package; its sound plays all the same.`;
    }
    return `Deck ${picture.deck}'s picture will not play on this computer; its sound plays all the same.`;
  }

  async function fullScreen() {
    try {
      await stage?.requestFullscreen();
    } catch {
      // Refused: the window can still be maximised by hand.
    }
  }
</script>

<div class="video" class:clean data-video>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="stage" class:fill bind:this={stage} onclick={() => (clean = false)} data-stage>
    <div class="layers">
      {#each pictures as picture (`${picture.deck}/${picture.record}`)}
        <video
          data-deck={picture.deck}
          src={picture.url}
          muted
          playsinline
          preload="auto"
          aria-label={`Deck ${picture.deck}'s picture`}
          onerror={(e) => failed(picture.deck, e.currentTarget as HTMLVideoElement)}
        ></video>
      {/each}
    </div>
  </div>

  {#if !clean}
    <div class="controls">
      <label>
        Show
        <select
          aria-label="Which picture"
          value={pinned === null ? "" : String(pinned)}
          onchange={(e) => (pinned = e.currentTarget.value === "" ? null : Number(e.currentTarget.value))}
        >
          <option value="">The mix</option>
          {#each pictures as picture (picture.deck)}
            <option value={String(picture.deck)}>Deck {picture.deck} alone</option>
          {/each}
        </select>
      </label>
      <label><input type="checkbox" bind:checked={fill} /> Fill, cropping the edges</label>
      <button onclick={() => (clean = true)} title="Click the picture to bring them back">Hide the controls</button>
      <button onclick={() => void fullScreen()}>Full screen</button>
    </div>
    {#if asked && pictures.length === 0}
      <p class="note" data-video-none>
        No deck has a video on it. Load an MP4, MOV, MKV or WebM onto a deck: its sound
        plays like any record, and its picture shows here. For a projector, pop this panel
        out and drag its window there.
      </p>
    {/if}
    {#each pictures.filter((p) => unplayable.includes(p.deck)) as picture (picture.deck)}
      <p class="note" role="alert" title={said[picture.deck] ? `The webview said: ${said[picture.deck]}` : undefined}>
        {why(picture)}
        {#if said[picture.deck]}<span class="said">(The webview said: {said[picture.deck]})</span>{/if}
      </p>
    {/each}
  {/if}
</div>

<style>
  .video {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    height: 100%;
    min-height: 0;
    padding: 0.4rem;
    box-sizing: border-box;
  }

  .video.clean {
    padding: 0;
  }

  /*
   * Black, not a token: what is around and behind a picture goes onto the
   * projector and the stream with it, and a theme's tint there would too.
   */
  /*
   * A projector's shape when the panel is docked and grows only as tall as
   * what is in it; filling whatever there is when it has a window of its own.
   * Without a shape the picture had no height at all: the stage is a box of
   * layers, with no size of its own.
   */
  .stage {
    position: relative;
    flex: 1 1 auto;
    aspect-ratio: 16 / 9;
    min-height: 6rem;
    background: rgb(0 0 0);
    border-radius: var(--radius-s, 4px);
    overflow: hidden;
    --whole: 0;
  }

  .clean .stage {
    border-radius: 0;
    cursor: pointer;
  }

  /* The whole stack at its share of what is heard, over the black. */
  .layers {
    position: absolute;
    inset: 0;
    opacity: var(--whole);
  }

  video {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    opacity: 0;
  }

  .fill video {
    object-fit: cover;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.6rem;
    font-size: 0.9em;
  }

  .controls label {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
  }

  .note {
    margin: 0;
    color: var(--text-dim);
    font-size: 0.9em;
  }

  .said {
    font-size: 0.9em;
  }
</style>
