<script lang="ts">
  /**
   * One effect rack: three slots, on a deck or on the master.
   *
   * The same component both places, because it is the same rack — a DJ who has
   * learnt one has learnt the other, and two components would be two chances
   * for them to drift apart.
   *
   * Every control sends an action string and reads its state back from the
   * snapshot. Nothing here holds a value of its own, so a slot changed by a
   * controller, a script or the assistant moves this panel too
   * (`docs/adr/0003-action-bus-and-parameter-registry.md`).
   */
  import { EFFECTS, saveRackPreset, type FxSlot } from "./api";

  let {
    slots,
    enabled,
    /** `deck 2` or `master` — the target these actions are addressed to. */
    target,
    send,
  }: {
    slots: FxSlot[];
    enabled: boolean;
    target: string;
    send: (action: string) => void;
  } = $props();

  /**
   * Which deck this rack belongs to, or nothing for the master.
   *
   * Parsed back out of `target` rather than passed separately: the target
   * string is what every other control here sends, and a second prop saying the
   * same thing is a second thing that can disagree with it.
   */
  const deckNumber = $derived.by(() => {
    const match = /^deck (\d+)$/.exec(target);
    return match ? Number(match[1]) : null;
  });

  const anyLoaded = $derived(slots.some((slot) => slot.kind !== "none"));

  let naming = $state(false);
  let chainName = $state("");
  let saved = $state(false);
  let error = $state<string | null>(null);

  async function keep() {
    const name = chainName.trim();
    if (!name) return;
    try {
      await saveRackPreset(name, deckNumber);
      naming = false;
      chainName = "";
      error = null;
      saved = true;
      // Long enough to notice, short enough not to become part of the layout.
      setTimeout(() => (saved = false), 2_000);
    } catch (e) {
      error = String(e);
    }
  }

  /** Placement is meaningless on the master: there is no fader after it. */
  const showsPlacement = $derived(target !== "master");

  /**
   * "1/4" for the sub-beat lengths, "4" for the whole ones.
   *
   * The same spelling the loop controls use. Two ways of writing half a beat in
   * one interface is one way too many.
   */
  function formatBeats(beats: number): string {
    if (beats >= 1) return String(Math.round(beats * 100) / 100);
    return `1/${Math.round(1 / beats)}`;
  }

  /** The lengths offered, shortest first. */
  const LENGTHS = [0.0625, 0.125, 0.25, 0.5, 1, 2, 4];
</script>

<div class="rack">
  {#each slots as slot (slot.slot)}
    {@const loaded = slot.kind !== "none"}
    <div class="slot" class:on={slot.enabled && loaded}>
      <!--
        The switch first and largest, because it is the control reached for
        mid-mix. Selecting an effect is something done once, while setting up.
      -->
      <button
        class="power"
        class:lit={slot.enabled && loaded}
        disabled={!enabled || !loaded}
        onclick={() => send(`${target} fx ${slot.slot} toggle`)}
        title={loaded
          ? slot.enabled
            ? `${slot.kind} on — click to switch it off`
            : `${slot.kind} loaded — click to switch it on`
          : "Load an effect first"}
      >
        {slot.slot}
      </button>

      <select
        class="pick"
        disabled={!enabled}
        value={slot.kind}
        onchange={(event) =>
          send(`${target} fx ${slot.slot} ${event.currentTarget.value}`)}
        aria-label="Effect in slot {slot.slot}"
      >
        {#each EFFECTS as name (name)}
          <option value={name}>{name === "none" ? "—" : name}</option>
        {/each}
      </select>

      <!--
        Wet is a slider and the amount is a slider, but the beat length is a
        row of buttons: a DJ picking a quarter-beat echo wants a quarter beat,
        not something near it, and a slider cannot promise that.
      -->
      <label class="knob">
        <span>wet</span>
        <input
          type="range"
          min="0"
          max="1"
          step="0.01"
          disabled={!enabled || !loaded}
          value={slot.wet}
          oninput={(event) =>
            send(`${target} fx ${slot.slot} wet ${event.currentTarget.value}`)}
        />
      </label>

      {#if loaded}
        <label class="knob">
          <span>{slot.amount_label}</span>
          <input
            type="range"
            min="0"
            max="1"
            step="0.01"
            disabled={!enabled}
            value={slot.amount}
            oninput={(event) =>
              send(`${target} fx ${slot.slot} amount ${event.currentTarget.value}`)}
          />
        </label>
      {/if}

      <!--
        Hidden rather than greyed out for an effect with no time in it. A
        control that is absent asks no questions; one that is greyed out asks
        "what would that have done?".
      -->
      {#if slot.timed}
        <div class="lengths">
          {#each LENGTHS as beats (beats)}
            <button
              class:active={Math.abs(slot.beats - beats) < 0.001}
              disabled={!enabled}
              onclick={() => send(`${target} fx ${slot.slot} beats ${beats}`)}
              title="{formatBeats(beats)} beat{beats === 1 ? '' : 's'}"
            >
              {formatBeats(beats)}
            </button>
          {/each}
        </div>
      {/if}

      {#if showsPlacement && loaded}
        <button
          class="place"
          disabled={!enabled}
          onclick={() =>
            send(`${target} fx ${slot.slot} ${slot.post_fader ? "pre" : "post"}`)}
          title={slot.post_fader
            ? "After the fader — pulling the fader down takes the tail with it"
            : "Before the fader — the tail survives the fader coming down"}
        >
          {slot.post_fader ? "post" : "pre"}
        </button>
      {/if}
    </div>
  {/each}

  <!--
    Saving the chain. Here rather than in the preset panel because this is
    where a DJ *is* when they have just found something worth keeping — a
    control that requires crossing the interface to reach is one that gets used
    once and then forgotten.
  -->
      <div class="keep">
    {#if naming}
      <input
        class="chain-name"
        bind:value={chainName}
        placeholder="name this chain"
        onkeydown={(event) => {
          if (event.key === "Enter") void keep();
          if (event.key === "Escape") naming = false;
        }}
      />
      <IconButton icon="fa-solid fa-floppy-disk" title="Save" onClick={() => void keep()} disabled={!enabled || !chainName.trim()} />
      <IconButton icon="fa-solid fa-xmark" title="Cancel" onClick={() => (naming = false)} />
    {:else}
      <IconButton
        icon="fa-solid fa-floppy-disk"
        title={anyLoaded ? "Keep this chain as a preset" : "Nothing in the rack to keep yet"}
        aria-label="Save chain"
        disabled={!enabled || !anyLoaded}
        onClick={() => {
          naming = true;
          error = null;
        }}
      />
      {#if saved}
        <span class="kept">saved</span>
      {/if}
    {/if}
  </div>
  {#if error}
    <p class="chain-error">{error}</p>
  {/if}
</div>

<style>
  .keep {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    margin-top: 0.2rem;
  }

  .keep-open,
  .keep button {
    font-size: 0.75em;
    letter-spacing: 0.04em;
  }

  .chain-name {
    flex: 1;
    min-width: 5rem;
    font-size: 0.8em;
  }

  .kept {
    font-size: 0.75em;
    color: var(--accent-2);
  }

  .chain-error {
    margin: 0.2rem 0 0;
    color: var(--danger, #e06c75);
    font-size: 0.75em;
  }

  .rack {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .slot {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.8em;
    /* The master column is a third the width of a deck's. Wrapping rather than
       a second component for the narrow case: the same rack has to fit both,
       and a squeezed slider is worse than a second line. */
    flex-wrap: wrap;
  }

  .power {
    width: 1.6rem;
    padding: 0.15rem 0;
    font-weight: 600;
  }

  .power.lit {
    background: var(--accent-2);
    border-color: var(--accent-2);
    color: var(--on-accent);
  }

  .pick {
    font-size: 0.9em;
    padding: 0.1rem 0.2rem;
    min-width: 5.5rem;
  }

  .knob {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    flex: 1 1 8rem;
    /* Below this a slider is a decoration rather than a control. */
    min-width: 7rem;
  }

  .knob span {
    color: var(--text-dim);
    font-size: 0.85em;
    /* Fixed so the sliders line up down the rack even though "feedback" and
       "grit" are different lengths. */
    width: 3.6rem;
    text-align: right;
  }

  .knob input {
    flex: 1;
    min-width: 2.5rem;
  }

  .lengths {
    display: flex;
    gap: 0.15rem;
  }

  .lengths button {
    padding: 0.1rem 0.25rem;
    font-size: 0.85em;
  }

  .lengths button.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }

  .place {
    padding: 0.1rem 0.3rem;
    font-size: 0.8em;
    letter-spacing: 0.03em;
    color: var(--text-dim);
  }
</style>
