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
  import IconButton from "./controls/IconButton.svelte";
  import SvgKnob from "./controls/SvgKnob.svelte";
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

<!--
  Folded until something is in it.

  Measured: three empty slots are three dropdowns reading "—" beside three
  sliders that do nothing, and they cost about 90 px of a deck column
  permanently, whether or not the DJ has ever used an effect. That is space
  taken from the waveform above by a control in its resting state. Unfolded the
  rack is exactly what it was; folded it is one line that says what it is and
  opens on a click.

  `open` starts true when anything is loaded, so a rack in use is never hidden
  from the DJ who set it up -- including after a restart, because the slots come
  from the engine's snapshot.
-->
<details class="rack-fold" open={anyLoaded}>
  <summary>
    Effects
    {#if anyLoaded}
      <span class="loaded">{slots.filter((s) => s.kind !== "none").length}</span>
    {/if}
  </summary>
<!--
  §120: three units side by side, like a hardware effects unit, rather than
  three full-width rows. *"fx also and design seems poor/redundant"*: each slot
  was a dropdown beside a slider the width of the deck, and a loaded one grew a
  second slider and a third row. Now each is a card -- its switch and its
  effect on top, wet and the effect's own control as the deck's knobs (a
  hundred pixels of drag for the whole range, Shift for fine, a double press
  back to where it started), the beat lengths under them. A narrow column (the
  master's) stacks the units instead of squeezing them.
-->
<div class="rack">
  {#each slots as slot (slot.slot)}
    {@const loaded = slot.kind !== "none"}
    <div class="slot" class:on={slot.enabled && loaded} data-fx-slot={slot.slot}>
      <div class="unit-head">
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
      </div>

      {#if loaded}
        <div class="unit-knobs">
          <SvgKnob
            value={slot.wet}
            min={0}
            max={1}
            step={0.01}
            label="wet"
            name="Slot {slot.slot} wet"
            readout={`${Math.round(slot.wet * 100)}%`}
            size={34}
            origin={0}
            disabled={!enabled}
            oninput={(value) => send(`${target} fx ${slot.slot} wet ${value}`)}
            ondblclick={() => send(`${target} fx ${slot.slot} wet 0`)}
          />
          <SvgKnob
            value={slot.amount}
            min={0}
            max={1}
            step={0.01}
            label={slot.amount_label}
            name="Slot {slot.slot} {slot.amount_label}"
            readout={`${Math.round(slot.amount * 100)}%`}
            size={34}
            origin={0.5}
            disabled={!enabled}
            oninput={(value) => send(`${target} fx ${slot.slot} amount ${value}`)}
            ondblclick={() => send(`${target} fx ${slot.slot} amount 0.5`)}
          />
        </div>

        <!--
          Hidden rather than greyed out for an effect with no time in it. A
          control that is absent asks no questions; one that is greyed out asks
          "what would that have done?". Buttons, not a knob: a DJ picking a
          quarter-beat echo wants a quarter beat, not something near it.
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

        {#if showsPlacement}
          <button
            class="place"
            disabled={!enabled}
            onclick={() =>
              send(`${target} fx ${slot.slot} ${slot.post_fader ? "pre" : "post"}`)}
            title={slot.post_fader
              ? "After the fader — pulling the fader down takes the tail with it"
              : "Before the fader — the tail survives the fader coming down"}
          >
            {slot.post_fader ? "post-fader" : "pre-fader"}
          </button>
        {/if}
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
        aria-label="Name for this chain"
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
</details>

<style>
  .keep {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    margin-top: 0.2rem;
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
    display: grid;
    /* Three units across a deck; the master's narrow column stacks them. */
    grid-template-columns: repeat(auto-fit, minmax(7.5rem, 1fr));
    gap: 0.35rem;
    align-items: start;
  }

  /* Saving the chain spans the rack, under the units. */
  .rack > .keep,
  .rack > .chain-error {
    grid-column: 1 / -1;
  }

  .slot {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.3rem;
    padding: 0.35rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel);
    font-size: 0.8em;
  }

  .slot.on {
    border-color: var(--active);
  }

  .unit-head {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .unit-head .pick {
    flex: 1;
    min-width: 0;
  }

  .unit-knobs {
    display: flex;
    justify-content: space-around;
    gap: 0.3rem;
  }

  .power {
    width: 1.6rem;
    padding: 0.15rem 0;
    font-weight: 600;
  }

  /* A rack that is *running*, which is not the same as one the DJ picked --
     the beat length below is the picked thing. They were both `--accent-2`
     before the roles were applied, so the two states this panel has were one
     colour. */
  .power.lit {
    background: var(--active);
    border-color: var(--active);
    color: var(--on-active);
  }

  .pick {
    font-size: 0.9em;
    padding: 0.1rem 0.2rem;
  }

  /* Wrapped inside the unit: seven lengths are wider than a third of a
     deck, and the first version ran them out past the card's edge. */
  .lengths {
    display: flex;
    flex-wrap: wrap;
    gap: 0.15rem;
  }

  .lengths button {
    padding: 0.1rem 0.25rem;
    font-size: 0.85em;
  }

  .lengths button.active {
    background: var(--selected);
    border-color: var(--selected);
    color: var(--on-selected);
  }

  .place {
    padding: 0.1rem 0.3rem;
    font-size: 0.8em;
    letter-spacing: 0.03em;
    color: var(--text-dim);
  }

  /*
    The fold. Deliberately quiet: it is a label, not a control, and it is
    reached for once while setting up rather than during a mix.
  */
  .rack-fold summary {
    font-size: 0.78em;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-dim);
    cursor: pointer;
    padding: 0.15rem 0;
  }

  .rack-fold[open] summary {
    margin-bottom: 0.35rem;
  }

  .loaded {
    margin-left: 0.35rem;
    color: var(--accent);
  }
</style>
