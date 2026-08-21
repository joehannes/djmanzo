<script lang="ts">
  /**
   * The sampler's own panel: what the pads cannot say.
   *
   * The pads fire samples — that is the Sampler page in the pad zone, and it is
   * where a DJ actually plays from. This panel is the other half: loading a
   * file into a slot, choosing how its pad behaves, sending it to the
   * headphones, picking a bank. Setting-up work rather than playing work, which
   * is why it is a panel you open rather than something taking room on a deck.
   */
  import { loadSample, TRIGGER_MODES, type SamplerState } from "./api";
  import { open } from "@tauri-apps/plugin-dialog";
    import IconButton from "./controls/IconButton.svelte";

  let {
    sampler,
    enabled,
    send,
  }: {
    sampler: SamplerState;
    enabled: boolean;
    send: (action: string) => void;
  } = $props();

  let error = $state<string | null>(null);
  let busy = $state<number | null>(null);

  /**
   * Which slot the next recording lands in, and where it comes from.
   *
   * Chosen before the take rather than after it, because a recording is
   * something you line up: you decide "the drop, into pad 5" and then wait for
   * the drop. Asking afterwards means holding a nameless buffer while the DJ
   * decides, at exactly the moment they are busiest.
   */
  let recordSlot = $state(1);
  let recordSource = $state("master");

  /** The deck taps, plus the master. Four decks, whatever is loaded. */
  const SOURCES = ["master", "deck 1", "deck 2", "deck 3", "deck 4"];

  function toggleRecord() {
    if (sampler.record.recording) {
      send("sampler record stop");
    } else {
      send(`sampler record ${recordSlot} ${recordSource}`);
    }
  }

  async function pick(slot: number) {
    const chosen = await open({
      multiple: false,
      filters: [{ name: "Audio", extensions: ["wav", "flac", "mp3", "m4a", "aiff", "ogg"] }],
    });
    if (typeof chosen !== "string") return;
    busy = slot;
    try {
      // The bank is captured *now* rather than read when the load returns: a
      // file can take a moment to decode, and a DJ who switches banks in the
      // meantime should not find their sample in the bank they moved to.
      const bank = sampler.bank;
      await loadSample(bank, slot, chosen);
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  function clear(slot: number) {
    // The name is forgotten by the backend, which owns it — the same place the
    // deck's title is forgotten when a deck is ejected.
    send(`sampler ${slot} clear`);
  }
</script>

<div class="sampler">
  <div class="head">
    <span class="label">Sampler</span>

    <!--
      Banks are a view, not a mute: a loop keeps running when the DJ switches
      away from its bank, which is the point of having banks at all.
    -->
    <div class="banks">
      {#each [1, 2, 3, 4] as bank (bank)}
        <button
          class:active={sampler.bank === bank}
          disabled={!enabled}
          onclick={() => send(`sampler bank ${bank}`)}
          title="Bank {bank}"
        >
          {bank}
        </button>
      {/each}
    </div>

    <label class="level">
      <span>level</span>
      <input
        type="range"
        min="0"
        max="1"
        step="0.01"
        disabled={!enabled}
        value={sampler.volume}
        oninput={(event) => send(`sampler volume ${event.currentTarget.value}`)}
      />
    </label>

    <!--
      Eight loops running and no way to stop them in one gesture is a sampler
      that will one day be the loudest thing in the room. It stops every bank,
      not just the one showing, because the ones you cannot see are exactly the
      ones you will have forgotten about.
    -->
    <IconButton icon="fa-solid fa-stop" title="Stop all samples" onClick={() => send("sampler stop_all")} disabled={!enabled} />

  <!--
    Recording. A row of its own rather than a control per slot: there is one
    recorder, and eight record buttons would suggest eight.
  -->
  <div class="record" class:live={sampler.record.recording}>
    <IconButton
      class:live={sampler.record.recording}
      icon={sampler.record.recording ? 'fa-solid fa-stop' : 'fa-solid fa-microphone'}
      onClick={toggleRecord}
      disabled={!enabled || (!sampler.record.ready && !sampler.record.recording)}
      title={sampler.record.ready || sampler.record.recording
        ? "Capture into a slot"
        : "The last recording is still being made into a sample"}
    />

    {#if sampler.record.recording}
      <!--
        What is being recorded and for how long, in the words the backend used.
        The elapsed bar runs against the ceiling rather than counting up on its
        own, because "18.4" means nothing without knowing where it stops.
      -->
      <span class="what">
        {sampler.record.source} → pad {sampler.record.slot}
      </span>
      <div class="elapsed">
        <div
          class="elapsed-fill"
          style:width="{Math.min(1, sampler.record.seconds / sampler.record.max_seconds) * 100}%"
        ></div>
      </div>
      <span class="mono">{sampler.record.seconds.toFixed(1)}s</span>
      <IconButton icon="fa-solid fa-xmark" title="Cancel recording" onClick={() => send("sampler record cancel")} disabled={!enabled} />
    {:else}
      <label class="pick">
        <span>from</span>
        <select bind:value={recordSource} disabled={!enabled} aria-label="What to record">
          {#each SOURCES as source (source)}
            <option value={source}>{source}</option>
          {/each}
        </select>
      </label>
      <label class="pick">
        <span>into</span>
        <select
          bind:value={recordSlot}
          disabled={!enabled}
          aria-label="Which pad to record into"
        >
          {#each sampler.slots as slot (slot.slot)}
            <option value={slot.slot}>
              pad {slot.slot}{slot.loaded ? " — replaces" : ""}
            </option>
          {/each}
        </select>
      </label>
      {#if !sampler.record.ready}
        <span class="waiting">making the last one into a sample…</span>
      {/if}
    {/if}
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <div class="slots">
    {#each sampler.slots as slot (slot.slot)}
      <div class="slot" class:playing={slot.playing}>
        <span class="number">{slot.slot}</span>

        <button
          class="name"
          disabled={!enabled || busy === slot.slot}
          onclick={() => pick(slot.slot)}
          title={slot.loaded ? "Replace what is in this slot" : "Load a sample here"}
        >
          {#if busy === slot.slot}
            loading…
          {:else if slot.loaded}
            {slot.name ?? "sample"}
          {:else}
            — empty —
          {/if}
        </button>

        {#if slot.loaded}
          <!-- A progress bar rather than a number: mid-set this is read at a
               glance, and "0.62" is not something read at a glance. -->
          <div class="through">
            <div class="through-fill" style:width="{slot.progress * 100}%"></div>
          </div>

          <select
            disabled={!enabled}
            value={slot.mode}
            onchange={(event) => send(`sampler ${slot.slot} ${event.currentTarget.value}`)}
            aria-label="How pad {slot.slot} behaves"
          >
            {#each TRIGGER_MODES as mode (mode)}
              <option value={mode}>{mode.replace("_", " ")}</option>
            {/each}
          </select>

          <input
            class="volume"
            type="range"
            min="0"
            max="1"
            step="0.01"
            disabled={!enabled}
            value={slot.volume}
            oninput={(event) =>
              send(`sampler ${slot.slot} volume ${event.currentTarget.value}`)}
            aria-label="Level of sample {slot.slot}"
          />

          <IconButton
            class:on={slot.cue}
            icon="fa-solid fa-headphones"
            onClick={() => send(`sampler ${slot.slot} ${slot.cue ? "master" : "cue"}`)}
            disabled={!enabled}
            title={slot.cue
              ? "In the headphones only — click to send it to the mix"
              : "In the mix — click to audition it in the headphones instead"}
          />

          <!--
            Hidden for a sample with no tempo of its own. Stretching one to a
            tempo is not a thing that can be done, and a switch that cannot do
            what it says is worse than no switch.
          -->
          {#if slot.bpm != null}
            <IconButton
              class:on={slot.synced}
              icon="fa-solid fa-sync"
              onClick={() => send(`sampler ${slot.slot} ${slot.synced ? "sync_off" : "sync"}`)}
              disabled={!enabled}
              title="{slot.bpm.toFixed(1)} BPM — {slot.synced
                ? 'stretching to the room'
                : 'playing at its own speed'}"
            />
          {/if}

          <IconButton icon="fa-solid fa-xmark" title="Empty this slot" onClick={() => clear(slot.slot)} disabled={!enabled} />
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .sampler {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.6rem;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
  }

  .label {
    font-weight: 600;
    letter-spacing: 0.06em;
  }

  .banks {
    display: flex;
    gap: 0.2rem;
  }

  .banks button {
    padding: 0.15rem 0.5rem;
    font-size: 0.85em;
  }

  .banks button.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }

  .level {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.8em;
    color: var(--text-dim);
    min-width: 10rem;
  }

  .level input {
    flex: 1;
  }

  .panic {
    margin-left: auto;
    font-size: 0.8em;
  }

  .record {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .arm {
    font-weight: 600;
    letter-spacing: 0.08em;
    min-width: 3.5rem;
  }

  /* A running recorder is the one thing in this panel that has to be visible
     from across a booth. */
  .arm.live {
    background: var(--danger, #e06c75);
    border-color: var(--danger, #e06c75);
    color: var(--on-accent);
  }

  .what {
    color: var(--text);
  }

  .elapsed {
    flex: 1;
    min-width: 3rem;
    height: 4px;
    background: var(--scrim);
    border-radius: 2px;
    overflow: hidden;
  }

  .elapsed-fill {
    height: 100%;
    background: var(--danger, #e06c75);
  }

  .mono {
    font-variant-numeric: tabular-nums;
  }

  .pick {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .waiting {
    font-style: italic;
  }

  .error {
    margin: 0;
    color: var(--danger, #e06c75);
    font-size: 0.85em;
  }

  .slots {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .slot {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85em;
  }

  /* A slot that is sounding is visibly sounding: mid-set this is read by
     colour, not by watching a progress bar. */
  .slot.playing .number {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }

  .number {
    width: 1.5rem;
    text-align: center;
    border: 1px solid var(--border);
    border-radius: 3px;
    font-weight: 600;
  }

  .name {
    flex: 1 1 8rem;
    min-width: 6rem;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.95em;
  }

  .through {
    width: 4rem;
    height: 3px;
    background: var(--scrim);
    border-radius: 2px;
    overflow: hidden;
  }

  .through-fill {
    height: 100%;
    background: var(--accent-2);
  }

  .volume {
    width: 5rem;
  }

  .route {
    padding: 0.1rem 0.3rem;
    font-size: 0.75em;
    letter-spacing: 0.05em;
    color: var(--text-dim);
  }

  .route.on {
    background: var(--accent-2);
    border-color: var(--accent-2);
    color: var(--on-accent);
  }

  .drop {
    padding: 0.05rem 0.35rem;
    color: var(--text-dim);
  }
</style>
