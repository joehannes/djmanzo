<script lang="ts">
  /**
   * How much the assistant does, and taking it back.
   *
   * # Why this panel is laid out the way it is
   *
   * A booth is dark, loud, and full of people, and the DJ has one hand free.
   * Three consequences, and they are the whole design:
   *
   * 1. **The two takeover controls never move and are always present.** Not
   *    shown-when-relevant: a control that appears and disappears is one you
   *    cannot build muscle memory for, and muscle memory is what you have at
   *    01:40. "Hand back" is disabled when nothing is held rather than hidden,
   *    so its position is learnable even on the nights you never need it.
   * 2. **They are the largest things here.** Reaching for the right one under
   *    pressure should not require reading.
   * 3. **What it will do next is shown before it does it.** At every posture,
   *    including the ones that will not act — seeing what it *would* do is how
   *    you decide whether to let it. An assistant that acts and then explains
   *    is one you cannot get ahead of.
   *
   * The posture ladder is a row rather than a dropdown for the same reason the
   * transition styles are: choosing between "prepare" and "autopilot" mid-set
   * should not mean opening a menu.
   */
  import {
    assistantApplyPack,
    assistantConduct,
    assistantHandBack,
    assistantPacks,
    assistantSetOccasion,
    assistantSetPosture,
    assistantTakeOver,
    OCCASIONS,
    POSTURES,
    POSTURE_HELP,
    type AssistantPack,
    type Conduct,
  } from "./api";
  import { onMount } from "svelte";

  let { enabled }: { enabled: boolean } = $props();

  let conduct = $state<Conduct | null>(null);
  let packs = $state<AssistantPack[]>([]);
  let error = $state<string | null>(null);

  /**
   * How often the panel re-reads what the assistant would do.
   *
   * Two seconds. The answer changes on the scale of a record ending, not a
   * frame, and polling faster would put a database read and a plan on the
   * interface thread sixty times a second for a line of text that barely
   * moves.
   */
  const REFRESH_MS = 2000;

  async function refresh() {
    try {
      conduct = await assistantConduct();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    void (async () => {
      try {
        packs = await assistantPacks();
      } catch {
        packs = [];
      }
    })();
    void refresh();
    const timer = setInterval(() => void refresh(), REFRESH_MS);
    return () => clearInterval(timer);
  });

  async function choose(action: () => Promise<void>) {
    try {
      await action();
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  /** "warm_up" reads badly on a button. */
  function label(name: string): string {
    return name.replace(/_/g, " ");
  }
</script>

<section class="conduct">
  <!--
    The two controls that matter most, at the top and never moving. Everything
    below them is configuration; these two are what you reach for while
    something is happening.
  -->
  <div class="hands">
    <button
      class="take"
      disabled={!enabled}
      onclick={() => choose(assistantTakeOver)}
      title="Take every control back from the assistant, now"
    >
      I'll take it
    </button>
    <button
      class="give"
      disabled={!enabled || !conduct?.anything_held}
      onclick={() => choose(assistantHandBack)}
      title={conduct?.anything_held
        ? "Hand everything back to the assistant"
        : "Nothing is held — the assistant already has the controls"}
    >
      You take it back
    </button>
  </div>

  {#if conduct?.decks_held.length}
    <p class="held">
      You have {conduct.decks_held.length === 1 ? "deck" : "decks"}
      {conduct.decks_held.join(", ")}
    </p>
  {/if}

  <!--
    What it will do next. Present at every posture: at Suggest and below this
    is the whole value of the panel, and at Autopilot it is the warning.
  -->
  <div class="next" class:idle={conduct?.next_step === "nothing"}>
    <span class="step">{conduct?.next_step ?? "…"}</span>
    <span class="because">{conduct?.because ?? ""}</span>
  </div>

  <h3>How much it does</h3>
  <div class="ladder" role="radiogroup" aria-label="How much the assistant does">
    {#each POSTURES as posture (posture)}
      <button
        role="radio"
        aria-checked={conduct?.posture === posture}
        class:active={conduct?.posture === posture}
        class:acting={posture === "assist" || posture === "autopilot"}
        disabled={!enabled}
        title={POSTURE_HELP[posture]}
        onclick={() => choose(() => assistantSetPosture(posture))}
      >
        {posture}
      </button>
    {/each}
  </div>
  <p class="hint">{POSTURE_HELP[conduct?.posture ?? "suggest"]}</p>

  <h3>What the night is</h3>
  <select
    disabled={!enabled}
    value={conduct?.occasion ?? "open"}
    onchange={(e) => choose(() => assistantSetOccasion(e.currentTarget.value))}
  >
    {#each OCCASIONS as occasion (occasion)}
      <option value={occasion}>{label(occasion)}</option>
    {/each}
  </select>

  {#if packs.length}
    <h3>Or start from one of these</h3>
    <div class="packs">
      {#each packs as pack (pack.name)}
        <button
          disabled={!enabled}
          title={pack.summary}
          class:active={conduct?.posture === pack.posture &&
            conduct?.occasion === pack.occasion}
          onclick={() => choose(() => assistantApplyPack(pack.name))}
        >
          {pack.name}
        </button>
      {/each}
    </div>
  {/if}

  {#if error}
    <p class="error">{error}</p>
  {/if}
</section>

<style>
  .conduct {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.6rem 0;
    border-top: 1px solid var(--edge, rgba(128, 128, 128, 0.25));
  }

  /* The pair that never moves. Deliberately the largest controls here. */
  .hands {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
  }

  .hands button {
    padding: 0.7rem 0.5rem;
    font-size: 0.95rem;
    font-weight: 600;
    border-radius: 0.4rem;
    cursor: pointer;
    border: 1px solid var(--edge, rgba(128, 128, 128, 0.4));
    background: transparent;
    color: inherit;
  }

  /* Taking over is the one you reach for when something is wrong, so it reads
     as the urgent one. Handing back is calm on purpose: it should never look
     like the thing to press in a hurry. */
  .hands .take {
    border-color: var(--warn, #d97706);
  }

  .hands button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .held {
    margin: 0;
    font-size: 0.78rem;
    color: var(--warn, #d97706);
  }

  .next {
    padding: 0.45rem 0.6rem;
    border-radius: 0.35rem;
    background: var(--chip, rgba(128, 128, 128, 0.14));
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  /* When there is nothing to do, the box recedes rather than shouting
     "nothing" at the same weight as a real instruction. */
  .next.idle {
    opacity: 0.55;
  }

  .step {
    font-size: 0.9rem;
    font-weight: 600;
  }

  .because {
    font-size: 0.75rem;
    opacity: 0.8;
  }

  h3 {
    margin: 0.2rem 0 0;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    opacity: 0.65;
    font-weight: 600;
  }

  .ladder {
    display: flex;
    gap: 0.15rem;
    flex-wrap: wrap;
  }

  .ladder button,
  .packs button {
    padding: 0.3rem 0.55rem;
    font-size: 0.78rem;
    border-radius: 0.3rem;
    border: 1px solid var(--edge, rgba(128, 128, 128, 0.35));
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .ladder button.active,
  .packs button.active {
    background: var(--accent-soft, rgba(128, 128, 128, 0.28));
    border-color: var(--accent, rgba(128, 128, 128, 0.7));
  }

  /* The two levels that move a live control are marked, so choosing one is
     never accidental. */
  .ladder button.acting.active {
    border-color: var(--warn, #d97706);
  }

  .packs {
    display: flex;
    gap: 0.25rem;
    flex-wrap: wrap;
  }

  .hint {
    margin: 0;
    font-size: 0.75rem;
    opacity: 0.75;
  }

  .error {
    margin: 0;
    font-size: 0.78rem;
    color: var(--danger, #dc2626);
  }
</style>
