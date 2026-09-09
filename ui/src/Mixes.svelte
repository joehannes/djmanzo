<script lang="ts">
  /**
   * The mixes tonight, as mixes rather than as a log.
   *
   * [§67](../../docs/DIRECTIVE.md) makes the session the application's central
   * object and says it *contains transitions*; [§68](../../docs/DIRECTIVE.md)
   * asks for those transitions to be explicit objects. The pair view holds the
   * one that is about to happen. This holds the ones that already did.
   *
   * # Why it is not the History panel
   *
   * History answers which *records* were played and keeps answering it across
   * nights. This answers how tonight's were joined — the decks, the length in
   * beats, and what kind of mix it was. A DJ looking back at a set wants both
   * and they are not the same question: two records can appear in any night's
   * history and have been put together in four different ways.
   *
   * # Derived, so it is never wrong about what was done
   *
   * Rust reads it back out of the action log rather than from anything
   * recorded alongside — see `dj_app::mixes`. Nothing here interprets: the
   * style, the decks and the length all arrive decided, because a second
   * opinion formed in the browser would be one the coach and the session file
   * disagree with.
   */
  import { onMount } from "svelte";
  import { formatTime, keepMix, sessionMixes, sessionRenderMix, type Mix } from "./api";

  interface Props {
    /** False before an audio device is open, when there is no set to read. */
    enabled: boolean;
  }

  let { enabled }: Props = $props();

  /**
   * How often the list is fetched.
   *
   * Eight seconds. A mix takes tens of seconds and only appears here once it
   * has finished, so anything faster asks a question whose answer cannot have
   * moved — and this walks the whole night's log to answer it.
   */
  const EVERY_MS = 8000;

  let mixes = $state<Mix[]>([]);
  let error = $state("");
  let timer: ReturnType<typeof setInterval> | undefined;

  async function refresh() {
    try {
      mixes = await sessionMixes();
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

  /**
   * What each style means, in the words a DJ would use.
   *
   * Here rather than in Rust because it is a gloss for a reader, not a
   * decision: `TransitionStyle` is the vocabulary and Rust owns it, and this
   * is a tooltip. A wrong word here is a confusing tooltip; a wrong style
   * would be a lie about the set.
   */
  const MEANS: Record<string, string> = {
    cut: "One stopped, the next started.",
    fade: "A straight crossfade, nothing else touched.",
    blend: "A crossfade with the outgoing bass pulled out.",
    echo: "An echo thrown over the record going out.",
    "vocal drop": "The outgoing vocal kept over the record coming in.",
  };

  /** Newest first: what you just did is what you want to see. */
  const recent = $derived([...mixes].reverse());

  /**
   * Hearing one back.
   *
   * §68's object driving replay. The whole set up to the mix is re-rendered
   * and thrown away — the engine's state at any moment *is* everything before
   * it — so this is minutes of work for a set that has been running a while,
   * and the button says so rather than appearing to do nothing.
   *
   * One at a time, by which mix is working: two renders of the same set at
   * once would decode the same records twice for no benefit, and a panel that
   * cannot say which one it is busy on is a panel with a spinner and no
   * meaning.
   */
  let rendering = $state<number | null>(null);
  let rendered = $state<{ at: number; said: string } | null>(null);

  /**
   * The answer, brought into view.
   *
   * A result line that appears below the fold of its own panel reads as
   * nothing having happened — this project has shipped that twice, in two
   * different panels, and found it both times by driving the application. The
   * row grows by a line when the path arrives, and in a docked panel that line
   * is exactly the one pushed out of sight.
   */
  let answer = $state<HTMLElement | null>(null);
  $effect(() => {
    if (rendered) answer?.scrollIntoView({ block: "nearest" });
  });

  /**
   * §24's "Save this transition".
   *
   * The one thing djmanzo stores about a pair. Every mix a night contained is
   * already derivable from the log, so keeping is not a record of what
   * happened — it is the DJ saying this one was worth having back, which is
   * the only part that cannot be derived.
   *
   * Refreshes rather than counting locally: the count comes back from the
   * database, and a number the interface incremented itself would be a second
   * opinion about how many times something was kept.
   */
  let keeping = $state<number | null>(null);

  async function keep(mix: Mix) {
    keeping = mix.at;
    try {
      await keepMix(mix.at);
      await refresh();
      error = "";
    } catch (problem) {
      error = String(problem);
    } finally {
      keeping = null;
    }
  }

  async function hearItAgain(mix: Mix) {
    rendering = mix.at;
    rendered = null;
    try {
      rendered = { at: mix.at, said: await sessionRenderMix(mix.at, mix.took_seconds) };
    } catch (problem) {
      rendered = { at: mix.at, said: String(problem) };
    } finally {
      rendering = null;
    }
  }
</script>

<section class="mixes">
  {#if error}
    <p class="problem">{error}</p>
  {:else if recent.length === 0}
    <!--
      An empty panel reads as broken, so it says which of the two empties this
      is. A set with one record in it has no mixes in it *yet*, and that is a
      different thing from a panel that cannot see the log.
    -->
    <p class="empty">
      No mixes yet. One appears here when the room stops hearing one record and
      starts hearing another — however you do it: the crossfader, the channel
      faders, or both.
    </p>
  {:else}
    <ol>
      {#each recent as mix, index (mix.at + ":" + index)}
        <li>
          <div class="when">
            <span class="mono">{formatTime(mix.at)}</span>
            <span class="style" title={MEANS[mix.style] ?? ""}>{mix.style}</span>
          </div>
          <div class="what">
            <span class="track">{mix.out_title ?? `deck ${mix.out_deck}`}</span>
            <span class="arrow" aria-hidden="true">→</span>
            <span class="track">{mix.in_title ?? `deck ${mix.in_deck}`}</span>
          </div>
          <div class="how">
            <span>deck {mix.out_deck} → {mix.in_deck}</span>
            <!--
              §24. Lit once kept, with the count, because keeping the same pair
              again is a stronger claim about it rather than a duplicate — and
              a DJ should be able to see they have said it before.
            -->
            <button
              class="keep"
              class:on={mix.kept > 0}
              disabled={!enabled || keeping !== null}
              onclick={() => void keep(mix)}
              title={mix.kept > 0
                ? `Kept ${mix.kept} time${mix.kept === 1 ? "" : "s"}. Keeping it again says so more strongly.`
                : "Keep this transition, so djmanzo offers it back"}
              aria-pressed={mix.kept > 0}
            >{keeping === mix.at ? "…" : mix.kept > 0 ? `kept ×${mix.kept}` : "keep"}</button>
            <button
              class="again"
              disabled={!enabled || rendering !== null}
              onclick={() => void hearItAgain(mix)}
              title="Re-render this mix to a file, with eight seconds of run-up. The set is replayed up to it, so this takes a while on a long night."
            >
              {rendering === mix.at ? "rendering…" : "hear it again"}
            </button>
            <!--
              Beats where the record's tempo is known, seconds where it is not.
              Not both: a DJ reads one number, and "21 s · 43 beats" is two
              spellings of one fact taking twice the room.
            -->
            <span class="mono">
              {#if mix.beats != null}
                {mix.beats.toFixed(0)} beats
              {:else}
                {mix.took_seconds.toFixed(0)} s
              {/if}
            </span>
          </div>
          {#if rendered?.at === mix.at}
            <p class="said" bind:this={answer}>{rendered.said}</p>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .mixes {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    font-size: 0.8rem;
  }

  ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  li {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    padding: 0.35rem 0.45rem;
    border-left: 2px solid var(--accent);
    background: var(--panel-raised);
    border-radius: 0 var(--radius) var(--radius) 0;
  }

  .when {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.4rem;
    color: var(--muted);
    font-size: 0.72rem;
  }

  .style {
    text-transform: capitalize;
    color: var(--accent);
  }

  .what {
    display: flex;
    align-items: baseline;
    gap: 0.3rem;
    min-width: 0;
  }

  /*
    The two titles sit together and shrink together.

    `flex: 1 1 0` first, which gave each half the panel and pushed two short
    names to opposite ends with the arrow stranded in the middle — a pair drawn
    as two columns. `0 1 auto` lets them take the room they need and give it
    back in proportion when a name is long, which is what a pair looks like.
  */
  .track {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .arrow {
    flex: 0 0 auto;
    color: var(--muted);
  }

  .how {
    display: flex;
    justify-content: space-between;
    gap: 0.4rem;
    color: var(--text-dim);
    font-size: 0.72rem;
  }

  /* Quiet: it is a thing you do to one row occasionally, not the reason the
     panel exists. */
  .again,
  .keep {
    font-size: 0.66rem;
    padding: 0 0.3rem;
  }

  .keep.on {
    background: var(--accent);
    color: var(--on-accent);
  }

  /* The path it wrote, which is the whole point of having pressed it. Wrapped
     rather than truncated: a file path with its middle missing is a path
     nobody can use. */
  .said {
    margin: 0.2rem 0 0;
    font-size: 0.68rem;
    line-height: 1.4;
    color: var(--text-dim);
    overflow-wrap: anywhere;
  }

  .empty,
  .problem {
    margin: 0;
    color: var(--text-dim);
    line-height: 1.5;
  }

  .problem {
    color: var(--warn);
  }
</style>
