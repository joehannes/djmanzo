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
    sidelist,
    type AutomixState,
  } from "./api";
  import IconButton from "./controls/IconButton.svelte";

  let {
    automix,
    enabled,
    send,
  }: {
    automix: AutomixState;
    enabled: boolean;
    send: (action: string) => void;
  } = $props();

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
    <IconButton
      class="switch"
      active={automix.enabled}
      disabled={!enabled}
      onClick={() => send(automix.enabled ? "automix off" : "automix on")}
      aria-pressed={automix.enabled}
      title={automix.enabled ? "Automix on" : "Automix off"}
    >
      {automix.enabled ? "On" : "Off"}
    </IconButton>
  </header>

  <div class="styles">
    {#each TRANSITION_STYLES as style (style)}
      <IconButton
        active={automix.style === style}
        disabled={!enabled}
        title={TRANSITION_HELP[style]}
        onClick={() => send(`automix style ${style}`)}
      >
        {style}
      </IconButton>
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

  {#if automix.enabled}
    <div class="status">
      {#if automix.mixing}
        <span class="mixing">Mixing…</span>
      {:else if queued > 0}
        <span>{queued} queued</span>
      {:else}
        <span class="warn">Sidelist empty — nothing to mix into.</span>
      {/if}
      <IconButton
        disabled={!enabled || automix.mixing}
        onClick={() => send("automix now")}
        title="Start the next transition now"
      >
        Now
      </IconButton>
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
</style>
