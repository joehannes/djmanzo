<script lang="ts">
  /**
   * Hearing a mix before you play it.
   *
   * §69 asks for a practice surface where "two tracks can be explored without
   * altering the live master", and the words that carry it are the last four.
   * A lab that borrows the decks is a booth: a DJ who has to stop playing in
   * order to try something has not been given a sandbox.
   *
   * # It renders; it does not play
   *
   * Every button here produces a *file*. The automix is stepped offline
   * against a simulated playhead in Rust and `replay` renders the result
   * headless — no fader moves, no deck is touched, and the record playing to
   * the room keeps playing. See `dj_app::practice`.
   *
   * That is also why the alternatives are worth having: rehearsing the same
   * pair four ways leaves four files that can be played against each other,
   * which is §69's "hear alternative transitions" as something on disk rather
   * than as a promise.
   *
   * # It rehearses the held transition, and does not change it
   *
   * Trying a vocal drop here must not restyle the mix djmanzo is about to
   * perform. Rust clones the transition and restyles the clone; this panel
   * says which style each file is so the two cannot be confused.
   *
   * # What is not here
   *
   * §69 also lists experimenting with stems, testing FX and creating loops.
   * Those are *live* gestures — they need a second engine feeding the
   * headphone output, which is a real piece of work and is not this one. What
   * ships is the half that can be done honestly offline: hearing the mix, and
   * hearing the alternatives. Comparing BPM, phrase structure and keys is the
   * pair view's, and is drawn there rather than a second time here.
   */
  import { onMount } from "svelte";
  import {
    formatTime,
    practiceRehearse,
    transitionCurrent,
    transitionStyles,
    type Rehearsal,
    type Transition,
    type TransitionStyleInfo,
  } from "./api";

  let { enabled }: { enabled: boolean } = $props();

  /**
   * The held transition, asked for rather than handed down.
   *
   * The same object the pair view draws, from the same command — so this panel
   * cannot be rehearsing a mix the pair view has since moved. Slowly, because
   * what changes it is a DJ pressing something.
   */
  const EVERY_MS = 2000;

  let pair = $state<Transition | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;

  async function refresh() {
    try {
      const held = await transitionCurrent();
      // A restyle in the pair view makes every rendered alternative a fact
      // about a mix that no longer exists, and a list of stale files reads as
      // current ones. Cleared on any change to what is held.
      if (held?.style !== pair?.style || held?.start_frame !== pair?.start_frame) {
        heard = {};
      }
      pair = held;
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

  /** The styles on offer, from Rust — see `transitionStyles`. */
  let styles: TransitionStyleInfo[] = $state([]);
  transitionStyles()
    .then((offered) => (styles = offered))
    .catch(() => (styles = []));

  /**
   * One rehearsal at a time, by style.
   *
   * Two renders at once would decode the same two records twice for no
   * benefit, and a panel that cannot say which style it is busy on is a
   * spinner with no meaning.
   */
  let working = $state<string | null>(null);
  let heard = $state<Record<string, Rehearsal>>({});
  let error = $state("");

  /**
   * The answer, brought into view.
   *
   * A result that appears below the fold of its own panel reads as nothing
   * having happened. This project has shipped that three times now, in three
   * different panels, and found it every time by driving the application
   * rather than by type-checking it.
   */
  let answer = $state<HTMLElement | null>(null);
  $effect(() => {
    if (Object.keys(heard).length) answer?.scrollIntoView({ block: "nearest" });
  });

  async function rehearse(style: string) {
    working = style;
    error = "";
    try {
      const result = await practiceRehearse(style);
      heard = { ...heard, [result.style]: result };
    } catch (problem) {
      error = String(problem);
    } finally {
      working = null;
    }
  }
</script>

<section class="practice">
  {#if !pair}
    <!--
      An empty panel reads as broken, so it says which empty this is and what
      to do about it. There is nothing to rehearse until a pair is held.
    -->
    <p class="empty">
      Nothing to rehearse yet. Set a transition up in the pair view, then hear
      it here — the decks keep playing while you do.
    </p>
  {:else}
    <p class="about">
      <strong>{pair.outgoing.track.title}</strong> into
      <strong>{pair.incoming.track.title}</strong>, over
      {pair.length_beats} beats. Rendered, not played: nothing below touches the
      decks.
    </p>

    <div class="tries" role="group" aria-label="Hear it this way">
      {#each styles as style (style.name)}
        <button
          class:on={pair.style === style.name}
          class:done={!!heard[style.name]}
          disabled={!enabled || working !== null}
          onclick={() => rehearse(style.name)}
          title={style.shape.does.join(". ")}
        >
          {style.name}{#if working === style.name}…{/if}
        </button>
      {/each}
    </div>
    <p class="hint">
      The one djmanzo is holding is marked. Hearing another does not change it.
    </p>

    {#if error}
      <p class="problem">{error}</p>
    {/if}

    {#if Object.keys(heard).length}
      <ol class="heard" bind:this={answer} data-testid="practice-heard">
        {#each Object.values(heard) as take (take.style)}
          <li>
            <span class="style">{take.style}</span>
            <span class="length">{formatTime(take.seconds)}</span>
            <!--
              The mix inside the file, so a DJ knows where to skip to rather
              than hunting for the moment the second record arrives.
            -->
            <span class="where">
              mix at {formatTime(take.mix_from)}–{formatTime(take.mix_to)}
            </span>
            <span class="actions">{take.actions} actions</span>
            <span class="path">{take.path}</span>
          </li>
        {/each}
      </ol>
    {/if}
  {/if}
</section>

<style>
  .practice {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.6rem;
    min-width: 0;
  }

  .empty,
  .hint,
  .about {
    margin: 0;
    font-size: 0.74rem;
    color: var(--muted);
    line-height: 1.45;
  }

  .about strong {
    color: var(--text);
  }

  .hint {
    font-size: 0.68rem;
  }

  .problem {
    margin: 0;
    font-size: 0.72rem;
    color: var(--warn);
  }

  .tries {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .tries button {
    font: inherit;
    font-size: 0.72rem;
    padding: 0.25rem 0.6rem;
    border-radius: 999px;
    border: 1px solid var(--line);
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
  }

  .tries button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  /* The held one, so trying an alternative cannot be mistaken for changing it. */
  .tries button.on {
    border-color: var(--accent);
    color: var(--accent);
  }

  /* Already rendered once. */
  .tries button.done {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .heard {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.7rem;
  }

  .heard li {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.2rem 0.6rem;
    padding: 0.3rem 0.4rem;
    border: 1px solid var(--line);
    border-radius: 4px;
    min-width: 0;
  }

  .style {
    font-weight: 600;
  }

  .length,
  .where,
  .actions {
    color: var(--muted);
  }

  .path {
    flex: 1 1 100%;
    color: var(--muted);
    font-family: var(--mono, monospace);
    font-size: 0.64rem;
    overflow-wrap: anywhere;
  }
</style>
