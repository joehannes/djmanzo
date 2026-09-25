<script lang="ts">
  /**
   * §118a: the decision a record running out asks for.
   *
   * > whenever in a stressful situation a decision becomes necessary a super
   * > easy and quick to identify and select widget can pop up (as unobtrusive
   * > or occupying as necessary and adequate)
   *
   * Three records, one per direction the night can go, each told apart
   * before it is read -- an arrow and a colour, then the title -- and one
   * press loads it on the free deck. Or buy time: the deck running out,
   * looped where it is, and the choice stays up.
   *
   * What the three are is Rust's (`commands::next_decision`: the rail's own
   * ranking going each way, no record twice), and so is how much room this
   * takes (`dj_app::decide::presence`, from the seconds left): a card over
   * the decks, then most of the screen in the last seconds. Neither blocks
   * the decks -- a DJ in trouble may reach for the loop or the fader first.
   */
  import { portal } from "./controls/portal";
  import { loadTrack, nextDecision, type DecisionChoice, type NextDecision, type Presence } from "./api";

  let {
    deck,
    decks,
    presence,
    says,
    send,
    onTaken,
    onLater,
    later = "Not now",
  }: {
    /** The deck running out. */
    deck: number;
    /** How many decks are on screen. */
    decks: number;
    presence: Presence;
    /** The whisper's sentence: which deck ends when. */
    says: string;
    send: (action: string) => void | Promise<void>;
    /** A record was loaded. */
    onTaken: () => void;
    /** Not now. */
    onLater: () => void;
    /** The words on the way out: *Not now* for a proposal, *Close* for a guide. */
    later?: string;
  } = $props();

  let decision = $state<NextDecision | null>(null);
  let error = $state<string | null>(null);
  let loading = $state<string | null>(null);

  // Asked once per deck running out, not per frame: the seconds change on
  // every snapshot and the answer does not. Through a derived, because the
  // prop is read through the whisper's proposal, which is a new object on
  // every frame, and a derived only speaks when its value changes.
  const asking = $derived(deck);
  const shown = $derived(decks);
  $effect(() => {
    const asked = asking;
    const on = shown;
    void (async () => {
      try {
        decision = await nextDecision(asked, on);
        error = null;
      } catch (e) {
        decision = null;
        error = String(e);
      }
    })();
  });

  async function choose(choice: DecisionChoice) {
    if (!decision || loading || !choice.track.path) return;
    loading = choice.track.id;
    try {
      await loadTrack(decision.into, choice.track.path);
      onTaken();
    } catch (e) {
      error = String(e);
    } finally {
      loading = null;
    }
  }

  /**
   * Just under the top bar, measured rather than assumed: its height follows
   * the density and the toolbars, and a card over REC and SAFE would hide
   * the two things a DJ in trouble may need next.
   */
  let top = $state(72);
  $effect(() => {
    const place = () => {
      const bar = document.querySelector(".topbar")?.getBoundingClientRect();
      if (bar) top = Math.round(bar.bottom + 8);
    };
    place();
    window.addEventListener("resize", place);
    return () => window.removeEventListener("resize", place);
  });

  /** The arrow each direction is drawn with: up, level, down. */
  const ARROW: Record<DecisionChoice["direction"], string> = {
    lift: "M4 18 L18 4 M9 4 H18 V13",
    hold: "M3 11 H19 M13 5 L19 11 L13 17",
    ease: "M4 4 L18 18 M18 9 V18 H9",
  };
</script>

<div
  class="decision"
  class:whole={presence === "whole"}
  use:portal
  style="top: {top}px"
  role="dialog"
  aria-label="Choose the next record"
  data-decision={deck}
  data-presence={presence}
>
  <header>
    <strong class="says">{says}</strong>
    <button type="button" class="later" onclick={onLater}>{later}</button>
  </header>

  {#if decision && decision.choices.length > 0}
    <div class="choices" style="--n: {decision.choices.length}">
      {#each decision.choices as choice (choice.track.id)}
        <button
          type="button"
          class="choice"
          data-direction={choice.direction}
          disabled={loading !== null}
          aria-busy={loading === choice.track.id}
          onclick={() => void choose(choice)}
        >
          <span class="way">
            <svg viewBox="0 0 22 22" aria-hidden="true"><path d={ARROW[choice.direction]} /></svg>
            {choice.says}
          </span>
          <span class="title">{choice.track.title}</span>
          <span class="artist">{choice.track.artist}</span>
          {#if choice.summary}<span class="summary">{choice.summary}</span>{/if}
          {#if choice.transition}<span class="mix">{choice.transition.says}</span>{/if}
        </button>
      {/each}
    </div>
    <footer>
      <span class="into">Loads on deck {decision.into}</span>
      <button type="button" class="stall" onclick={() => decision && void send(decision.stall.run)}>
        {decision.stall.says}
      </button>
    </footer>
  {:else if decision}
    <p class="none">Nothing in the library to offer. Analyse some records, or loop this one.</p>
    <footer>
      <button type="button" class="stall" onclick={() => decision && void send(decision.stall.run)}>
        {decision.stall.says}
      </button>
    </footer>
  {:else if error}
    <p class="none">{error}</p>
  {/if}
</div>

<style>
  /* A card over the decks: below the top bar, clear of the mixer. */
  .decision {
    position: fixed;
    z-index: 64;
    left: 50%;
    transform: translateX(-50%);
    width: min(50rem, calc(100vw - 2rem));
    padding: 0.6rem 0.7rem 0.7rem;
    border: 2px solid var(--warn);
    border-radius: 0.7rem;
    background: var(--panel);
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.55);
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
  }

  /* The last seconds: most of the screen, the targets large enough to hit
     without aiming. Still no scrim -- the decks stay reachable around it. */
  .decision.whole {
    width: min(72rem, calc(100vw - 2rem));
    font-size: 1.2em;
    padding: 0.9rem 1rem 1rem;
    gap: 0.8rem;
  }

  header,
  footer {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .says {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--warn);
  }

  .later {
    flex: none;
  }

  .choices {
    display: grid;
    grid-template-columns: repeat(var(--n, 3), minmax(0, 1fr));
    gap: 0.5rem;
  }

  .choice {
    --way: var(--accent);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.2rem;
    min-width: 0;
    min-height: 7.5rem;
    padding: 0.55rem 0.65rem;
    text-align: left;
    border: 2px solid var(--way);
    border-radius: 0.55rem;
  }

  .whole .choice {
    min-height: 11rem;
    padding: 0.8rem 0.9rem;
  }

  .choice[data-direction="lift"] {
    --way: var(--accent-warm);
  }

  .choice[data-direction="ease"] {
    --way: var(--accent-2);
  }

  .way {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    color: var(--way);
    font-weight: 700;
    font-size: 0.8em;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .way svg {
    width: 1.6em;
    height: 1.6em;
    fill: none;
    stroke: currentColor;
    stroke-width: 2.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .title {
    font-size: 1.15em;
    font-weight: 700;
    line-height: 1.2;
    overflow-wrap: anywhere;
  }

  .artist,
  .mix {
    color: var(--text-dim);
    font-size: 0.85em;
  }

  .summary {
    font-family: var(--mono);
    font-size: 0.8em;
  }

  .into {
    flex: 1 1 auto;
    color: var(--text-dim);
    font-size: 0.85em;
  }

  .none {
    margin: 0;
    color: var(--text-dim);
  }

  @media (max-width: 40rem) {
    .choices {
      grid-template-columns: 1fr;
    }
  }
</style>
