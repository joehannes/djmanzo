<script lang="ts">
  /**
   * What the assistant has prepared, before any of it happens.
   *
   * §44's Accept / Modify / Reject, as a strip rather than as a panel. The
   * audit is explicit that the AI is never the largest thing on screen and
   * that its normal output is "a marker, a ghost, a one-line reason, a staged
   * transaction" — so this lives in the notice band under the top bar, at the
   * height of two lines, and is not there at all when nothing is staged.
   *
   * # Modify is a checkbox, not a form
   *
   * The realistic modification is *not that last bit*: load it and cue it, but
   * I will bring it in myself. That is one click on one row. Editing the
   * numbers inside a move belongs to the panel that owns the number — the mix
   * point is the pair view's, the gain is the deck's — and duplicating them
   * here would be a second place to set the same thing.
   *
   * # A refused move is shown, not hidden
   *
   * A move the posture will not allow arrives greyed with its row named. The
   * point is that the obvious fix — turn the assistant up one notch — is
   * invisible to somebody who was never told what it was going to do.
   */
  import {
    stagedAccept,
    stagedChoose,
    stagedCurrent,
    stagedReject,
    type Staged,
    type StagedOutcome,
  } from "./api";
  import { onMount } from "svelte";

  interface Props {
    /** False before an audio device is open, when nothing can be carried out. */
    enabled: boolean;
  }

  let { enabled }: Props = $props();

  /**
   * How often the plan is re-read.
   *
   * Two seconds. It is not a live reading — a plan changes when somebody asks
   * for one — but it does go stale on its own when the record it is about
   * leaves the deck, and this is what notices.
   */
  const EVERY_MS = 2000;

  let plan = $state<Staged | null>(null);
  let outcome = $state<StagedOutcome | null>(null);
  let problem = $state("");
  let busy = $state(false);

  async function refresh() {
    try {
      plan = await stagedCurrent();
      // A plan that has gone stale takes its outcome with it: an old "done"
      // list hanging over a new set is a claim about the wrong night.
      if (!plan && !busy) outcome = null;
    } catch (error) {
      problem = String(error);
    }
  }

  onMount(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), EVERY_MS);
    return () => clearInterval(timer);
  });

  async function choose(index: number, chosen: boolean) {
    try {
      plan = await stagedChoose(index, chosen);
      problem = "";
    } catch (error) {
      problem = String(error);
    }
  }

  async function accept() {
    busy = true;
    try {
      outcome = await stagedAccept();
      plan = null;
      problem = "";
    } catch (error) {
      problem = String(error);
    } finally {
      busy = false;
    }
  }

  async function reject() {
    await stagedReject();
    plan = null;
    outcome = null;
  }

  const willRun = $derived(
    plan?.moves.filter((move) => move.chosen && move.allowance !== "no").length ??
      0,
  );
</script>

{#if plan}
  <section class="staged" aria-label={plan.headline}>
    <div class="head">
      <strong>{plan.headline}</strong>
      <span class="because">{plan.because}</span>
      <span class="spacer"></span>
      <button class="accept" disabled={!enabled || busy || willRun === 0} onclick={accept}>
        Accept{willRun > 0 ? ` ${willRun}` : ""}
      </button>
      <button class="reject" disabled={busy} onclick={reject}>Reject</button>
    </div>
    <ul class="moves">
      {#each plan.moves as move, index (move.about)}
        <li class:refused={move.allowance === "no"}>
          <label>
            <input
              type="checkbox"
              checked={move.chosen}
              disabled={move.allowance === "no" || busy}
              onchange={(e) => choose(index, e.currentTarget.checked)}
            />
            <span class="what">{move.about}</span>
          </label>
          {#if move.allowance === "no"}
            <span class="chip" title="Your posture does not allow this row of the matrix">
              {move.capability.replace(/_/g, " ")} — not at this level
            </span>
          {:else if move.allowance === "limited"}
            <span class="chip limited" title="Small, reversible moves only">limited</span>
          {/if}
        </li>
      {/each}
    </ul>
    {#if problem}
      <p class="problem" role="alert">{problem}</p>
    {/if}
  </section>
{:else if outcome}
  <section class="staged done" aria-label="What was carried out">
    <div class="head">
      <strong>{outcome.stopped ? "Stopped partway" : "Done"}</strong>
      <span class="because">{outcome.done.join(" · ") || "nothing to do"}</span>
      <span class="spacer"></span>
      <button class="reject" onclick={() => (outcome = null)}>Dismiss</button>
    </div>
    {#if outcome.stopped}
      <!--
        Partial success, said plainly. Reporting "accepted" and leaving a DJ to
        find out that only half of it happened is the failure this exists to
        prevent.
      -->
      <p class="problem" role="alert">
        {outcome.stopped.about} — {outcome.stopped.because}
      </p>
    {/if}
  </section>
{/if}

<style>
  .staged {
    margin: 0 0 0.35rem;
    padding: 0.4rem 0.6rem;
    border: 1px solid var(--accent);
    border-radius: 6px;
    background: var(--panel-raised);
    font-size: 0.85em;
  }

  .staged.done {
    border-color: var(--border-strong);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .spacer {
    flex: 1;
  }

  .because {
    color: var(--text-dim);
    font-size: 0.92em;
  }

  .moves {
    list-style: none;
    margin: 0.35rem 0 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.2rem 1rem;
  }

  .moves li {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .moves label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  /* Greyed, and still readable: the point is that it can be read. */
  .moves li.refused .what {
    color: var(--text-dim);
    text-decoration: line-through;
  }

  .chip {
    font-size: 0.82em;
    color: var(--text-dim);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0 0.4rem;
  }

  .chip.limited {
    color: var(--warn);
    border-color: var(--warn);
  }

  .accept {
    border-color: var(--accent);
    color: var(--accent);
  }

  .problem {
    margin: 0.35rem 0 0;
    color: var(--danger);
  }
</style>
