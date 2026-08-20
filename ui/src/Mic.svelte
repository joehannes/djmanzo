<script lang="ts">
  /**
   * The microphone / line input strip.
   *
   * Two halves that are deliberately separate. The **cable** — which device is
   * attached — is set-up work done once at the start of the night, and lives
   * behind a picker. The **switch** is the thing a DJ hits dozens of times an
   * evening, and it is the biggest control here.
   *
   * Talkover's settings sit under a disclosure rather than on the face of it.
   * A DJ sets the depth once and then never touches it again; the two things
   * they need to see at a glance are whether the channel is live and how far
   * the music is currently stepping back.
   */
  import { listInputs, openMic, closeMic, type Device, type MicState } from "./api";
  import { fill } from "./meter";

  let {
    mic,
    enabled,
    send,
  }: {
    mic: MicState;
    enabled: boolean;
    send: (action: string) => void;
  } = $props();

  let devices = $state<Device[]>([]);
  let chosen = $state<string | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let showSettings = $state(false);

  async function refresh() {
    error = null;
    try {
      devices = await listInputs();
    } catch (e) {
      error = String(e);
    }
  }

  async function attach() {
    busy = true;
    error = null;
    try {
      await openMic(chosen);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function detach() {
    busy = true;
    error = null;
    try {
      // Close the channel before dropping the cable, so nothing is left
      // half-open: an armed strip with no device is the state the `present`
      // flag exists to make visible, and there is no reason to leave a DJ in it.
      if (mic.open) send("mic off");
      await closeMic();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  /**
   * The ducking meter runs the other way from a level meter: it grows as the
   * music steps back. Scaled against the configured depth rather than a fixed
   * range, so "full" always means "as far as this DJ asked it to go".
   */
  const duckFill = $derived(
    mic.duck_db > 0 ? Math.min(1, mic.ducking_db / mic.duck_db) : 0,
  );
</script>

<section class="mic" class:live={mic.open && mic.present}>
  <header>
    <h3>Microphone</h3>
    {#if mic.present}
      <button
        class="switch"
        class:active={mic.open}
        disabled={!enabled}
        onclick={() => send(mic.open ? "mic off" : "mic on")}
        title={mic.open ? "Close the channel" : "Open the channel"}
      >
        {mic.open ? "On air" : "Off"}
      </button>
    {/if}
  </header>

  {#if !mic.present}
    <div class="attach">
      <select bind:value={chosen} disabled={!enabled || busy} onfocus={refresh}>
        <option value={null}>System default input</option>
        {#each devices as device (device.id)}
          <option value={device.id}>{device.name}</option>
        {/each}
      </select>
      <button disabled={!enabled || busy} onclick={attach}>
        {busy ? "Opening…" : "Attach"}
      </button>
    </div>
    <p class="note">
      Nothing is attached. A microphone through a computer is late by the input
      buffer plus the output buffer — if you can hear yourself in the monitors,
      that delay is real and no setting removes it.
    </p>
  {:else}
    <div class="meters">
      <div class="meter" title="Microphone level">
        <span class="fill" style="transform: scaleX({fill(mic.level)})"></span>
      </div>
      <div class="meter duck" title="How far the music is stepping back">
        <span class="fill" style="transform: scaleX({fill(duckFill)})"></span>
      </div>
    </div>

    <label class="control">
      <span>Gain <em class="mono">{mic.gain_db.toFixed(1)} dB</em></span>
      <input
        type="range"
        min="-60"
        max="12"
        step="0.5"
        value={mic.gain_db}
        disabled={!enabled}
        oninput={(e) => send(`mic gain ${e.currentTarget.value}`)}
      />
    </label>

    <div class="toggles">
      <button
        class:active={mic.talkover}
        disabled={!enabled}
        onclick={() => send(mic.talkover ? "mic talkover_off" : "mic talkover_on")}
        title="Step the music back while somebody is speaking. Switch it off for a line input."
      >
        Talkover
      </button>
      <button
        class:active={mic.cue}
        disabled={!enabled}
        onclick={() => send(mic.cue ? "mic cue_off" : "mic cue_on")}
        title="Hear yourself in the headphones"
      >
        Headphones
      </button>
      <button
        class:active={showSettings}
        onclick={() => (showSettings = !showSettings)}
        title="Talkover depth and timing"
      >
        Settings
      </button>
    </div>

    {#if showSettings}
      <div class="settings">
        <label class="control">
          <span>Duck by <em class="mono">{mic.duck_db.toFixed(0)} dB</em></span>
          <input
            type="range"
            min="0"
            max="40"
            step="1"
            value={mic.duck_db}
            disabled={!enabled}
            oninput={(e) => send(`mic duck ${e.currentTarget.value}`)}
          />
        </label>
        <label class="control">
          <span>Threshold <em class="mono">{mic.threshold_db.toFixed(0)} dB</em></span>
          <input
            type="range"
            min="-60"
            max="0"
            step="1"
            value={mic.threshold_db}
            disabled={!enabled}
            oninput={(e) => send(`mic threshold ${e.currentTarget.value}`)}
          />
        </label>
        <label class="control">
          <span>Attack <em class="mono">{mic.attack_ms.toFixed(0)} ms</em></span>
          <input
            type="range"
            min="1"
            max="200"
            step="1"
            value={mic.attack_ms}
            disabled={!enabled}
            oninput={(e) => send(`mic attack ${e.currentTarget.value}`)}
          />
        </label>
        <label class="control">
          <span>Release <em class="mono">{mic.release_ms.toFixed(0)} ms</em></span>
          <input
            type="range"
            min="50"
            max="2000"
            step="10"
            value={mic.release_ms}
            disabled={!enabled}
            oninput={(e) => send(`mic release ${e.currentTarget.value}`)}
          />
        </label>
        <button class="detach" disabled={busy} onclick={detach}>
          Detach device
        </button>
      </div>
    {/if}

    {#if mic.starved_frames > 0}
      <p class="warn">
        The input is not keeping up — {Math.round(mic.starved_frames)} frames
        missed. A larger audio buffer usually fixes it.
      </p>
    {/if}
  {/if}

  {#if error}
    <p class="warn">{error}</p>
  {/if}
</section>

<style>
  .mic {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.6rem;
    border: 1px solid var(--edge);
    border-radius: var(--radius);
    background: var(--panel);
  }

  /* On air is a state a DJ must be able to read from across a booth. */
  .mic.live {
    border-color: var(--accent-warm, #d08a3a);
    box-shadow: 0 0 0 1px var(--accent-warm, #d08a3a) inset;
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
    min-width: 5rem;
    font-weight: 600;
  }

  .switch.active {
    background: var(--accent-warm, #d08a3a);
    color: var(--panel);
  }

  .attach {
    display: flex;
    gap: 0.4rem;
  }

  .attach select {
    flex: 1;
    min-width: 0;
  }

  .meters {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .meter {
    position: relative;
    height: 0.4rem;
    border-radius: 0.2rem;
    background: var(--sunken, rgba(0, 0, 0, 0.3));
    overflow: hidden;
  }

  .meter .fill {
    position: absolute;
    inset: 0;
    transform-origin: left center;
    background: var(--accent-warm, #d08a3a);
    /* Whole-pixel transforms, and only when the value actually changed —
       see `meter.ts`. */
    will-change: transform;
  }

  .meter.duck .fill {
    background: var(--muted);
  }

  .toggles {
    display: flex;
    gap: 0.3rem;
  }

  .toggles button {
    flex: 1;
    font-size: 0.7rem;
  }

  .settings {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding-top: 0.4rem;
    border-top: 1px solid var(--edge);
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

  .note,
  .warn {
    margin: 0;
    font-size: 0.68rem;
    line-height: 1.35;
    color: var(--muted);
  }

  .warn {
    color: var(--warn, #d4756b);
  }

  .detach {
    font-size: 0.7rem;
  }
</style>
