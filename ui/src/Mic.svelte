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
   *
   * K3: with the singers' microphones open and nothing attached here, this is
   * Mic 1 of theirs (`mic.strip`) — the switch, the fader, talkover and the
   * headphones work that strip, as the keys and a controller's buttons for
   * the microphone do. Talkover's depth and timing are this strip's own and do
   * not reach a singer's, so they are not offered then; attaching a
   * microphone here gives the DJ their own again.
   */
  import { listInputs, openMic, closeMic, type Device, type MicState } from "./api";
  import SvgMeter from "./SvgMeter.svelte";
  import IconButton from "./controls/IconButton.svelte";

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

  /**
   * Whether the input is falling behind now. The count only says so while it
   * climbs: an input whose first block arrived after the output's first has
   * missed a few hundred frames once and is keeping up, and a warning that
   * stayed for that would be one every night — every karaoke night once this
   * panel is Mic 1. Read from the count's first sight on, so opening the
   * panel late does not warn about the start.
   */
  const SLIPPING_MS = 2000;
  let seen: number | null = null;
  let climbedAt = -Infinity;
  let slipping = $state(false);
  $effect(() => {
    const now = performance.now();
    const missed = mic.starved_frames;
    if (seen !== null && missed > seen) climbedAt = now;
    seen = missed;
    slipping = now - climbedAt < SLIPPING_MS;
  });
</script>

<section class="mic" class:live={mic.open && mic.present}>
  <header>
    <h3>Microphone</h3>
    {#if mic.present}
      <IconButton
        class="switch"
        active={mic.open}
        disabled={!enabled}
        onClick={() => send(mic.open ? "mic off" : "mic on")}
        title={mic.open ? "Close the channel" : "Open the channel"}
        aria-pressed={mic.open}
      >
        {mic.open ? "On air" : "Off"}
      </IconButton>
    {/if}
  </header>

  {#if mic.strip}
    <p class="note" data-strip>
      Mic 1 of the singers' microphones — this switch and fader, and the keys and controller buttons for the
      microphone, work it. Attach a microphone of your own and they work that instead.
    </p>
  {/if}

  {#if !mic.present || mic.strip}
    <div class="attach">
      <select
        aria-label="Microphone input"
        bind:value={chosen}
        disabled={!enabled || busy}
        onfocus={refresh}
      >
        <option value={null}>System default input</option>
        {#each devices as device (device.id)}
          <option value={device.id}>{device.name}</option>
        {/each}
      </select>
      <IconButton icon="fa-solid fa-paperclip" title={busy ? "Opening…" : "Attach"} disabled={!enabled || busy} onClick={attach} />
    </div>
    {#if !mic.present}
      <p class="note">
        Nothing is attached. A microphone through a computer is late by the input
        buffer plus the output buffer — if you can hear yourself in the monitors,
        that delay is real and no setting removes it.
      </p>
    {/if}
  {/if}

  {#if mic.present}
    <div class="meters">
      <SvgMeter label="Microphone level" value={mic.level} tone="var(--accent-warm, #d08a3a)" />
      {#if !mic.strip}
        <SvgMeter label="How far the music is stepping back" value={duckFill} tone="var(--muted)" />
      {/if}
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
      <IconButton icon="fa-solid fa-microphone" title="Talkover" active={mic.talkover} disabled={!enabled} onClick={() => send(mic.talkover ? "mic talkover_off" : "mic talkover_on")} />
      <IconButton icon="fa-solid fa-headphones" title="Headphones" active={mic.cue} disabled={!enabled} onClick={() => send(mic.cue ? "mic cue_off" : "mic cue_on")} />
      {#if !mic.strip}
        <IconButton icon="fa-solid fa-gear" title="Talkover depth and timing" active={showSettings} onClick={() => (showSettings = !showSettings)} />
      {/if}
    </div>

    {#if showSettings && !mic.strip}
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
        <IconButton icon="fa-solid fa-unlink" title="Detach device" class="detach" disabled={busy} onClick={detach} />
      </div>
    {/if}

    {#if slipping}
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

  .toggles {
    display: flex;
    gap: 0.3rem;
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
</style>
