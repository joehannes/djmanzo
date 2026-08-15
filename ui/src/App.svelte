<script lang="ts">
  import Assistant from "./Assistant.svelte";
  import Browse from "./Browse.svelte";
  import Deck from "./Deck.svelte";
  import Presets from "./Presets.svelte";
  import Settings from "./Settings.svelte";
  import { watchFrameRate } from "./framerate";
  import {
    dispatch,
    getSnapshot,
    hasBrandLogo,
    listDevices,
    logoUrl,
    onSnapshot,
    openDevice,
    sessionLog,
    type ActiveDevice,
    type Device,
    type Snapshot,
  } from "./api";

  let devices = $state<Device[]>([]);
  let selectedDevice = $state<string | null>(null);
  let bufferFrames = $state(256);
  let active = $state<ActiveDevice | null>(null);
  let error = $state<string | null>(null);
  let snapshot = $state<Snapshot | null>(null);
  let log = $state<string[]>([]);
  let showLog = $state(false);
  let slowFrames = $state<number | null>(null);
  /** Which side panel is open, if any. Only one at a time: the decks matter more. */
  let panel = $state<"none" | "browse" | "assistant" | "presets" | "settings">("none");
  let logo = $state(false);
  /** Bumped when the logo changes, to defeat the webview's image cache. */
  let logoVersion = $state(0);

  // The engine only exists once a device is open, so every control that would
  // send an action stays disabled until then.
  const ready = $derived(active !== null);

  $effect(() => {
    const unlisten = onSnapshot((next) => {
      snapshot = next;
    });
    // Paint immediately rather than waiting for the engine to change something.
    // The stream only emits on change, so a quiet startup would otherwise leave
    // the interface blank.
    void getSnapshot().then((initial) => {
      snapshot ??= initial;
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  });

  $effect(() => {
    refreshDevices();
  });

  $effect(() => {
    void hasBrandLogo().then((present) => {
      logo = present;
    });
  });

  async function refreshLogo() {
    logo = await hasBrandLogo();
    logoVersion += 1;
  }

  // Watch our own frame rate. On a machine where the webview has no accelerated
  // compositing the waveform drops to ~16 fps with nothing to indicate why --
  // see the benchmark in ADR-0004. Better to say so than to look broken.
  $effect(() => watchFrameRate((health) => {
    slowFrames = health.degraded ? health.fps : null;
  }));

  async function refreshDevices() {
    try {
      devices = await listDevices();
      selectedDevice ??= devices.find((d) => d.is_default)?.id ?? devices[0]?.id ?? null;
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function connect() {
    try {
      active = await openDevice(selectedDevice, bufferFrames);
      error = null;
    } catch (e) {
      error = String(e);
      active = null;
    }
  }

  async function send(action: string) {
    try {
      await dispatch(action);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function toggleLog() {
    showLog = !showLog;
    if (showLog) log = await sessionLog();
  }

  const load = $derived(snapshot?.master.cpu_load ?? 0);
  // Read outside the template so event handlers, which run later, do not have
  // to prove `snapshot` is still non-null.
  const cueSplit = $derived(snapshot?.master.cue_split ?? false);
</script>

<main>
  <header class="topbar">
    <!--
      The DJ's own logo, if they set one. A booth screen carrying someone
      else's product name all night is a small daily insult, so the
      application steps out of the way when asked.
    -->
    <h1 class:branded={logo}>
      {#if logo}
        <img src={logoUrl(logoVersion)} alt="" />
      {:else}
        djmanzo
      {/if}
    </h1>

    <div class="device">
      <select bind:value={selectedDevice} disabled={devices.length === 0}>
        {#each devices as device (device.id)}
          <option value={device.id}>
            {device.name}{device.is_default ? " (default)" : ""}
          </option>
        {/each}
      </select>

      <select bind:value={bufferFrames}>
        {#each [64, 128, 256, 512, 1024] as frames (frames)}
          <option value={frames}>{frames} frames</option>
        {/each}
      </select>

      <button class="primary" onclick={connect}>
        {active ? "Reconnect" : "Connect"}
      </button>
    </div>

    <div class="status mono">
      {#if active}
        <span>{active.sample_rate / 1000} kHz</span>
        <span>{active.latency_ms.toFixed(1)} ms</span>
        <span class:hot={load > 0.7}>CPU {(load * 100).toFixed(0)}%</span>
        {#if snapshot && snapshot.master.xruns > 0}
          <span class="xruns">{snapshot.master.xruns} xruns</span>
        {/if}
      {:else}
        <span class="idle">no device</span>
      {/if}
      <button
        class:active={panel === "browse"}
        onclick={() => (panel = panel === "browse" ? "none" : "browse")}
      >
        Browse
      </button>
      <button
        class:active={panel === "presets"}
        onclick={() => (panel = panel === "presets" ? "none" : "presets")}
      >
        Presets
      </button>
      <button
        class:active={panel === "assistant"}
        onclick={() => (panel = panel === "assistant" ? "none" : "assistant")}
      >
        Assistant
      </button>
      <button
        class:active={panel === "settings"}
        onclick={() => (panel = panel === "settings" ? "none" : "settings")}
      >
        Settings
      </button>
      <button onclick={toggleLog}>Log</button>
    </div>
  </header>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if slowFrames !== null}
    <p class="warning">
      Interface running at {slowFrames.toFixed(0)} fps. This usually means the
      webview has no hardware acceleration — the audio engine is unaffected, but
      the waveform will not scroll smoothly.
    </p>
  {/if}

  <!--
    Decks and mixer sit in their own scrolling region so that opening the
    browser compresses them rather than being squeezed to nothing itself. This
    is the layout every DJ application converges on, for the reason it matters:
    you search for the next track while the current one is playing, so both
    have to be on screen at once.
  -->
  <div class="stage" class:shared={panel !== "none"}>
  {#if snapshot}
    <div class="decks">
      {#each snapshot.decks.slice(0, 2) as deck (deck.number)}
        <Deck {deck} enabled={ready} cueAvailable={snapshot.master.cue_available} />
      {/each}
    </div>

    <section class="mixer">
      <label class="control">
        <span>Crossfader</span>
        <input
          type="range"
          min="-1"
          max="1"
          step="0.01"
          value={snapshot.master.crossfader}
          disabled={!ready}
          oninput={(e) => send(`crossfader ${e.currentTarget.value}`)}
        />
        <div class="ends"><span>1</span><span>2</span></div>
      </label>

      <label class="control">
        <span>Master <em class="mono">{snapshot.master.gain_db.toFixed(1)} dB</em></span>
        <input
          type="range"
          min="-24"
          max="6"
          step="0.5"
          value={snapshot.master.gain_db}
          disabled={!ready}
          oninput={(e) => send(`master gain ${e.currentTarget.value}`)}
        />
      </label>

      <div class="cue-section" class:unavailable={!snapshot.master.cue_available}>
        {#if snapshot.master.cue_available}
          <label class="control">
            <span>Headphones <em class="mono">{snapshot.master.cue_split ? "split" : "blend"}</em></span>
            <input
              type="range"
              min="0"
              max="1"
              step="0.01"
              value={snapshot.master.cue_mix}
              disabled={!ready || snapshot.master.cue_split}
              oninput={(e) => send(`cue mix ${e.currentTarget.value}`)}
            />
            <div class="ends"><span>cue</span><span>master</span></div>
          </label>
          <button
            class:active={snapshot.master.cue_split}
            disabled={!ready}
            onclick={() => send(`cue ${cueSplit ? "split_off" : "split_on"}`)}
            title="Cue in one ear, master in the other"
          >
            Split
          </button>
        {:else}
          <p class="no-cue">
            {#if ready}
              No headphone cue — this device has only two output channels.
              Cueing needs a four-channel interface.
            {:else}
              Connect a device to see whether it can carry a headphone cue.
            {/if}
          </p>
        {/if}
      </div>

      <div class="master-meters">
        <div class="meter">
          <div class="meter-fill" style:width="{Math.min(snapshot.master.peak_left, 1) * 100}%"></div>
        </div>
        <div class="meter">
          <div
            class="meter-fill"
            style:width="{Math.min(snapshot.master.peak_right, 1) * 100}%"
          ></div>
        </div>
      </div>
    </section>
  {:else}
    <p class="waiting">Waiting for the engine…</p>
  {/if}
  </div>

  {#if panel !== "none"}
    <div class="panel">
      {#if panel === "browse"}
        <Browse enabled={ready} deckCount={2} />
      {:else if panel === "presets"}
        <Presets enabled={ready} deckCount={2} />
      {:else if panel === "assistant"}
        <Assistant enabled={ready} />
      {:else}
        <Settings onLogoChange={refreshLogo} />
      {/if}
    </div>
  {/if}

  {#if showLog}
    <section class="log">
      <h2>Session log</h2>
      <p class="hint">
        Every action, in order, with its timestamp. This log is what makes a set
        replayable — see ADR-0003.
      </p>
      <pre class="mono">{log.length ? log.join("\n") : "(nothing yet)"}</pre>
    </section>
  {/if}
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    padding: 0.9rem;
    height: 100vh;
    overflow: hidden;
  }

  .topbar {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.7rem 0.9rem;
  }

  h1 {
    margin: 0;
    font-size: 1.1rem;
    letter-spacing: 0.02em;
    color: var(--accent);
    display: flex;
    align-items: center;
  }

  /* A logo is given room but never allowed to push the toolbar around. */
  h1.branded img {
    height: 28px;
    max-width: 200px;
    object-fit: contain;
    display: block;
  }

  .status button.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #0e0f14;
  }

  .device {
    display: flex;
    gap: 0.4rem;
    flex: 1;
    min-width: 0;
  }

  .device select:first-child {
    flex: 1;
    min-width: 0;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .status .hot {
    color: var(--warn);
  }

  .status .xruns {
    color: var(--danger);
  }

  .status .idle {
    color: var(--warn);
  }

  .stage {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    min-height: 0;
    /* Takes everything when alone; yields to the panel when one is open, and
       scrolls rather than clipping if the window is short. */
    flex: 1;
    overflow: auto;
  }

  .stage.shared {
    flex: 1 1 55%;
  }

  .panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1 1 45%;
    /* A floor, so a short window still leaves the panel usable rather than
       collapsing it to a sliver. */
    min-height: 220px;
  }

  .decks {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.9rem;
  }

  .mixer {
    display: grid;
    grid-template-columns: 2fr 1fr 1.2fr auto;
    gap: 1.2rem;
    align-items: center;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.9rem;
  }

  .control {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .control em {
    font-style: normal;
    color: var(--text);
  }

  .ends {
    display: flex;
    justify-content: space-between;
    font-size: 0.8em;
  }

  .cue-section {
    display: flex;
    align-items: end;
    gap: 0.5rem;
  }

  .cue-section.unavailable {
    align-items: center;
  }

  .no-cue {
    margin: 0;
    font-size: 0.75em;
    line-height: 1.4;
    color: var(--text-dim);
  }

  .master-meters {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 140px;
  }

  .meter {
    height: 6px;
    background: var(--panel-raised);
    border-radius: 3px;
    overflow: hidden;
  }

  .meter-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent-2), var(--warn));
  }

  .log {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.9rem;
    overflow: auto;
    flex: 1;
    min-height: 0;
  }

  .log h2 {
    margin: 0 0 0.3rem;
    font-size: 0.95rem;
  }

  .hint {
    margin: 0 0 0.6rem;
    color: var(--text-dim);
    font-size: 0.8em;
  }

  .log pre {
    margin: 0;
    font-size: 0.8em;
    white-space: pre-wrap;
    user-select: text;
    -webkit-user-select: text;
  }

  .error {
    margin: 0;
    padding: 0.6rem 0.9rem;
    background: color-mix(in srgb, var(--danger) 12%, var(--panel));
    border: 1px solid var(--danger);
    border-radius: 8px;
    color: var(--danger);
    font-size: 0.85em;
  }

  .waiting {
    color: var(--text-dim);
  }

  .warning {
    margin: 0;
    padding: 0.6rem 0.9rem;
    background: color-mix(in srgb, var(--warn) 12%, var(--panel));
    border: 1px solid var(--warn);
    border-radius: 8px;
    color: var(--warn);
    font-size: 0.85em;
  }
</style>
