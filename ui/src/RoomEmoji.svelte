<script lang="ts">
  /**
   * Karaoke beta B2: emoji from the room, on the singers' screen — held back.
   *
   * The owner's word: show the room's cheering without distracting the
   * singer. So: nothing while the host has them off; small, in the bottom
   * corner away from the words; a few at once at most; each rises a little
   * and fades within three seconds (only fades, with reduced motion). The
   * rates and what is kept are Rust's (`dj_net::reactions`).
   */
  import { onDestroy } from "svelte";
  import { roomReactions, type RoomReaction } from "./roomReactions";

  /** How often the screen asks. */
  const EVERY_MS = 1000;
  /** The most drawn at once; past it the oldest goes first. */
  const AT_ONCE = 5;
  /** How long one is drawn, as the animation below. */
  const LASTS_MS = 3200;

  let shown = $state<(RoomReaction & { x: number })[]>([]);
  let since = 0;
  let stopped = false;

  async function ask() {
    try {
      const answer = await roomReactions(since);
      if (stopped) return;
      if (!answer.on) {
        shown = [];
        return;
      }
      for (const reaction of answer.reactions) {
        since = Math.max(since, reaction.id);
        // A few horizontal places in the corner, so two are never one on top
        // of the other; chosen by the id, not at random, so it is the same
        // every time it is drawn.
        shown = [...shown, { ...reaction, x: (reaction.id % 4) * 2.2 }].slice(-AT_ONCE);
        const id = reaction.id;
        setTimeout(() => (shown = shown.filter((one) => one.id !== id)), LASTS_MS);
      }
    } catch {
      // Nothing to draw rather than an error on the singer's screen.
    }
  }

  const timer = setInterval(() => void ask(), EVERY_MS);
  void ask();
  onDestroy(() => {
    stopped = true;
    clearInterval(timer);
  });
</script>

<div class="room-emoji" aria-hidden="true">
  {#each shown as one (one.id)}
    <span class="one" data-reaction={one.emoji} style:right="{one.x}vw">{one.emoji}</span>
  {/each}
</div>

<style>
  .room-emoji {
    position: absolute;
    right: 2vw;
    bottom: 3vh;
    width: 12vw;
    height: 22vh;
    pointer-events: none;
    overflow: hidden;
    z-index: 3;
  }

  .one {
    position: absolute;
    bottom: 0;
    font-size: 3.2vh;
    line-height: 1;
    opacity: 0;
    animation: rise 3.2s ease-out forwards;
  }

  @keyframes rise {
    0% {
      opacity: 0;
      transform: translateY(0) scale(0.8);
    }
    15% {
      opacity: 0.85;
      transform: translateY(-2vh) scale(1);
    }
    70% {
      opacity: 0.7;
    }
    100% {
      transform: translateY(-16vh) scale(1);
      opacity: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .one {
      animation: fade 3.2s ease-out forwards;
    }

    @keyframes fade {
      0%,
      100% {
        opacity: 0;
      }
      20%,
      70% {
        opacity: 0.8;
      }
    }
  }
</style>
