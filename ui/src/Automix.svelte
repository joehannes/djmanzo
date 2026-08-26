<script lang="ts">
  /**
   * Handing the mix over.
   *
   * Two controls that matter and two that do not. Whether it is on, and what a
   * transition sounds like, are decisions a DJ makes once and then watches;
   * the length is a number they set and forget. So the switch is large, the
   * style is a row of named buttons rather than a dropdown — a DJ choosing
   * between "blend" and "cut" mid-set should not have to open a menu — and the
   * length sits underneath.
   *
   * The status line is the important part while it is running. An automix that
   * gives no sign of what it is about to do is one a DJ cannot take back at
   * the right moment.
   */
  import {
    TRANSITION_STYLES,
    TRANSITION_HELP,
    formatTime,
    planTransition,
    sidelist,
    type AutomixState,
    type TransitionPlan,
  } from "./api";

  let {
    automix,
    enabled,
    send,
    deckCount = 2,
  }: {
    automix: AutomixState;
    enabled: boolean;
    send: (action: string) => void;
    deckCount?: number;
  } = $props();

  /**
   * What the planner would do, if asked.
   *
   * An opinion, not an instruction: asking for it moves nothing. That is the
   * point of the planner living beside the automix rather than inside it — a
   * DJ wants to see the proposal before deciding whether to take it, and a
   * panel that acted on being looked at would be unusable.
   *
   * Fetched on demand rather than polled: it depends on the playhead, and a
   * proposal that rewrote itself every frame could not be read.
   */
  let plan = $state<TransitionPlan | null>(null);
  let planning = $state(false);
  let planFrom = $state(1);
  let planTo = $state(2);

  async function askThePlanner() {
    planning = true;
    try {
      plan = await planTransition(planFrom, planTo);
    } catch {
      plan = null;
    } finally {
      planning = false;
    }
  }

  /**
   * How many tracks are waiting.
   *
   * Fetched rather than passed down: the Sidelist lives three components away
   * in the browser, and threading a count through all of them to show one
   * number here would tie the mixer to the library panel. Re-read when a
   * transition starts or ends, because that is when the count changes —
   * polling would be a database query sixty times a second for a number that
   * changes twice a track.
   */
  let queued = $state(0);
  $effect(() => {
    // Named so the effect re-runs on them.
    void automix.enabled;
    void automix.mixing;
    sidelist()
      .then((entries) => (queued = entries.length))
      .catch(() => (queued = 0));
  });
</script>

<section class="automix" class:running={automix.enabled}>
  <header>
    <h3>Automix</h3>
    <button
      class="switch"
      class:active={automix.enabled}
      disabled={!enabled}
      onclick={() => send(automix.enabled ? "automix off" : "automix on")}
    >
      {automix.enabled ? "On" : "Off"}
    </button>
  </header>

  <div class="styles">
    {#each TRANSITION_STYLES as style (style)}
      <button
        class:active={automix.style === style}
        disabled={!enabled}
        title={TRANSITION_HELP[style]}
        onclick={() => send(`automix style ${style}`)}
      >
        {style}
      </button>
    {/each}
  </div>

  <label class="control">
    <span>Over <em class="mono">{automix.beats.toFixed(0)} beats</em></span>
    <input
      type="range"
      min="1"
      max="64"
      step="1"
      value={automix.beats}
      disabled={!enabled}
      oninput={(e) => send(`automix beats ${e.currentTarget.value}`)}
    />
  </label>

  <!--
    The planner's opinion. Deliberately below the manual controls: a DJ who has
    chosen a style is not asking, and one who is asking has not chosen.
  -->
  <div class="planner">
    <div class="planner-controls">
      <label>
        Mix
        <select bind:value={planFrom} onchange={askThePlanner}>
          {#each Array.from({ length: deckCount }, (_, i) => i + 1) as deck (deck)}
            <option value={deck}>{deck}</option>
          {/each}
        </select>
      </label>
      <label>
        into
        <select bind:value={planTo} onchange={askThePlanner}>
          {#each Array.from({ length: deckCount }, (_, i) => i + 1) as deck (deck)}
            <option value={deck}>{deck}</option>
          {/each}
        </select>
      </label>
      <button disabled={!enabled || planning || planFrom === planTo} onclick={askThePlanner}>
        {planning ? "…" : "Suggest"}
      </button>
    </div>

    {#if plan}
      <p class="proposal">
        <strong>{plan.style}</strong> over {plan.length_beats} beats, starting at
        <span class="mono">{formatTime(plan.start_seconds)}</span>
      </p>
      <span class="reasons">
        {#each plan.reasons as reason (reason)}
          <span class="chip">{reason}</span>
        {/each}
      </span>
      <!--
        Taking the proposal sets the two things the automix actually reads. It
        does not start the mix: "Now" is still a separate press, because a
        transition that began because a plan was accepted would be a plan that
        acted on being agreed with.
      -->
      <button
        class="take"
        disabled={!enabled}
        onclick={() => {
          send(`automix style ${plan?.style}`);
          send(`automix beats ${plan?.length_beats}`);
        }}
      >
        Use this
      </button>
    {/if}
  </div>

  {#if automix.enabled}
    <div class="status">
      {#if automix.mixing}
        <span class="mixing">Mixing…</span>
      {:else if queued > 0}
        <span>{queued} queued</span>
      {:else}
        <span class="warn">Sidelist empty — nothing to mix into.</span>
      {/if}
      <button
        disabled={!enabled || automix.mixing}
        onclick={() => send("automix now")}
        title="Start the next transition now"
      >
        Now
      </button>
    </div>
  {/if}
</section>

<style>
  .automix {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.6rem;
    border: 1px solid var(--edge);
    border-radius: var(--radius);
    background: var(--panel);
  }

  .automix.running {
    border-color: var(--accent, #4a90a4);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  h3 {
    margin: 0;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }

  .switch {
    min-width: 4rem;
    font-weight: 600;
  }

  .switch.active {
    background: var(--accent, #4a90a4);
    color: var(--panel);
  }

  .styles {
    display: flex;
    gap: 0.25rem;
  }

  .styles button {
    flex: 1;
    font-size: 0.68rem;
    text-transform: capitalize;
  }

  .control {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    font-size: 0.7rem;
  }

  .control .mono {
    color: var(--muted);
    font-style: normal;
  }

  .status {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.4rem;
    font-size: 0.68rem;
    color: var(--muted);
  }

  .status .mixing {
    color: var(--accent, #4a90a4);
  }

  .status .warn {
    color: var(--warn, #d4756b);
  }

  .status button {
    font-size: 0.68rem;
  }

  .planner {
    border-top: 1px solid var(--edge, rgba(128, 128, 128, 0.25));
    margin-top: 0.5rem;
    padding-top: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .planner-controls {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.78rem;
    opacity: 0.85;
  }

  .planner-controls label {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .proposal {
    margin: 0;
    font-size: 0.85rem;
  }

  .reasons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }

  /* Chips, matching the browser's. Facts a DJ scans, not a sentence. */
  .chip {
    font-size: 0.68rem;
    line-height: 1.4;
    padding: 0 0.35rem;
    border-radius: 0.25rem;
    background: var(--chip, rgba(128, 128, 128, 0.18));
    white-space: nowrap;
  }

  .take {
    align-self: flex-start;
  }
</style>
