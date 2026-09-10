<script lang="ts">
  /**
   * §74's contextual control rail: the four to eight controls that matter now.
   *
   * "This is the core idea of adaptive UI." What makes it adaptive rather than
   * arbitrary is that the judgement is one function in Rust over the same
   * snapshot everything else draws from — `dj_app::at_hand` — so the rail cannot
   * be showing a deck the rest of the application is not, and the reason it
   * changed is a sentence rather than a guess.
   *
   * # It says which deck and why
   *
   * A row of controls that silently became a different row is a row a DJ stops
   * trusting, and the whole of §74 rests on being trusted enough to reach for
   * without looking. So the deck and the reason are on screen, always, in the
   * space a heading would take anyway.
   *
   * # Every press is an ordinary action
   *
   * Each control carries the exact text the parser accepts, so pressing one is
   * the same event as typing it, mapping a controller to it, or the assistant
   * asking for it — one execution path (ADR-0003). Nothing here decides what a
   * control means.
   */
  import { onMount } from "svelte";
  import { atHand, type AtHand } from "./api";

  interface Props {
    /** False before an audio device is open, when there is nothing to control. */
    enabled: boolean;
    send: (action: string) => void;
  }

  let { enabled, send }: Props = $props();

  /**
   * How often it re-reads.
   *
   * Twice a second. Faster than any other panel here and deliberately so: this
   * is the one that has to keep up with a hand landing on a platter, and a
   * rail that took two seconds to notice would be a rail that offered the
   * wrong five controls for the length of a scratch. It is also the cheapest
   * question in the application — a match over one snapshot — so the cost is a
   * round trip, not work.
   */
  const EVERY_MS = 500;

  let hand = $state<AtHand | null>(null);
  let error = $state("");
  let timer: ReturnType<typeof setInterval> | undefined;

  async function refresh() {
    try {
      hand = await atHand();
      error = "";
    } catch (problem) {
      error = String(problem);
    }
  }

  onMount(() => {
    void refresh();
    return () => clearInterval(timer);
  });

  $effect(() => {
    clearInterval(timer);
    if (!enabled) return;
    timer = setInterval(() => void refresh(), EVERY_MS);
  });
</script>

<section class="at-hand">
  {#if error}
    <p class="problem">{error}</p>
  {:else if !hand}
    <p class="waiting">Waiting for a deck.</p>
  {:else}
    <header>
      <span class="deck">Deck {hand.deck}</span>
      <!--
        The reason, not the state's name. "scratching" is a word djmanzo uses
        about itself; "your hand is on the platter" is a thing the DJ can check.
      -->
      <span class="because">{hand.because}</span>
    </header>
    <div class="controls">
      {#each hand.controls as control (control.action)}
        <button
          class:on={control.on}
          disabled={!enabled}
          onclick={() => send(control.action)}
          title={control.action}
          aria-pressed={control.on}
        >{control.label}</button>
      {/each}
    </div>
  {/if}
</section>

<style>
  .at-hand {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
    font-size: 0.68rem;
    color: var(--muted);
    min-width: 0;
  }

  .deck {
    font-weight: 700;
    color: var(--text);
    flex: none;
  }

  .because {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /*
    Wrapping rather than scrolling. A rail is reached for without looking, and
    a control that has scrolled out of the row is one a hand cannot find — so
    the row grows downward instead, which the panel has room for and a scroll
    position does not.
  */
  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }

  .controls button {
    flex: 1 1 auto;
    /* Big enough to hit in a dark booth without reading it first. */
    min-width: 3.4rem;
    padding: 0.25rem 0.4rem;
    font-size: 0.72rem;
  }

  .controls button.on {
    background: var(--accent);
    color: var(--on-accent);
  }

  .waiting,
  .problem {
    margin: 0;
    font-size: 0.75rem;
    color: var(--text-dim);
  }

  .problem {
    color: var(--warn);
  }
</style>
