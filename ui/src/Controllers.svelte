<script lang="ts">
  /**
   * What is plugged in, and what it is doing.
   *
   * # Why this panel exists
   *
   * djmanzo could already read a controller and could already edit a mapping,
   * and there was still no way to see whether the thing on the table was
   * connected. A DJ pressing a pad that does nothing has three possible
   * problems — no port open, the wrong mapping, or a binding that is not
   * there — and no way to tell them apart. This says which.
   *
   * # The routing line
   *
   * Most controllers put the room on outputs 1-2 and the headphones on 3-4,
   * and djmanzo assumes exactly that. The ones that differ say so in their
   * mapping file, and this is where that arrangement becomes visible — along
   * with the case that matters most, where the mapping asks for outputs the
   * open device does not have and the assumption is being used instead.
   */
  import { onDestroy } from "svelte";
  import IconButton from "./controls/IconButton.svelte";
  import {
    closeController,
    controlStatus,
    openController,
    setKeyboardEnabled,
    type AudioRouting,
    type ControlStatus,
  } from "./api";

  let { mappings = [] }: { mappings?: { name: string }[] } = $props();

  let status = $state<ControlStatus | null>(null);
  let chosenPort = $state<string | null>(null);
  let chosenMapping = $state<string | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  /**
   * Controllers are plugged in and unplugged while the panel is open, and
   * neither event reaches the interface any other way — the operating system
   * tells the MIDI layer, not the webview. Two seconds is slow enough to cost
   * nothing and fast enough that a DJ who just plugged something in does not
   * think it failed.
   */
  const POLL_MS = 2000;

  async function refresh() {
    try {
      status = await controlStatus();
      error = null;
      // Keep the choice pointing at something that still exists.
      if (chosenPort && !status.inputs.includes(chosenPort)) chosenPort = null;
      if (!chosenPort) chosenPort = status.open_port ?? status.inputs[0] ?? null;
    } catch (e) {
      error = String(e);
    }
  }

  void refresh();
  const poll = setInterval(refresh, POLL_MS);
  onDestroy(() => clearInterval(poll));

  async function connect() {
    if (!chosenPort) return;
    busy = true;
    error = null;
    try {
      await openController(chosenPort, chosenMapping ?? undefined);
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function disconnect() {
    busy = true;
    error = null;
    try {
      await closeController();
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function toggleKeyboard() {
    if (!status) return;
    await setKeyboardEnabled(!status.keyboard);
    await refresh();
  }

  /** `3-4`, the way the sockets are labelled. */
  function pair(p: [number, number]): string {
    return p[0] === p[1] ? `${p[0]}` : `${p[0]}-${p[1]}`;
  }

  function routingLine(audio: AudioRouting): string {
    const parts = [`room on ${pair(audio.master)}`];
    if (audio.cue) parts.push(`headphones on ${pair(audio.cue)}`);
    if (audio.booth) parts.push(`booth on ${pair(audio.booth)}`);
    return parts.join(", ");
  }
</script>

<section class="controllers" class:active={!!status?.open_port}>
  <header>
    <h3>Controllers</h3>
    <IconButton
      icon="keyboard"
      title={status?.keyboard
        ? "The keyboard is playing. Turn it off while typing."
        : "The keyboard is off"}
      active={!!status?.keyboard}
      onClick={toggleKeyboard}
    />
  </header>

  {#if status?.unavailable}
    <!-- The backend sentence already begins "MIDI is not available on this
         machine", so this adds the distinction rather than restating it. -->
    <p class="warn">
      {status.unavailable}. Plugging a controller in will not change that —
      this is the MIDI service itself, not an empty list of devices.
    </p>
  {:else if status && status.inputs.length === 0}
    <p class="note">No MIDI inputs. Plug a controller in — this checks again
      every couple of seconds.</p>
  {:else if status}
    <div class="pick">
      <select bind:value={chosenPort} disabled={busy}>
        {#each status.inputs as port (port)}
          <option value={port}>{port}</option>
        {/each}
      </select>
      <select
        bind:value={chosenMapping}
        disabled={busy}
        title="Leave on “fits the port” unless yours is not recognised"
      >
        <option value={null}>Whichever fits</option>
        {#each mappings as mapping (mapping.name)}
          <option value={mapping.name}>{mapping.name}</option>
        {/each}
      </select>
      <IconButton
        icon="check"
        title={busy ? "Connecting…" : "Connect"}
        onClick={connect}
        disabled={busy || !chosenPort}
      />
    </div>

    {#if status.open_port}
      <div class="open">
        <strong>{status.open_port}</strong>
        <span class="mapping">{status.open_mapping ?? "no mapping"}</span>
        <IconButton icon="unlink" title="Disconnect" onClick={disconnect} disabled={busy} />
      </div>

      {#if status.audio}
        <p class="note" class:warn={!!status.audio.not_applied}>
          {#if status.audio.not_applied}
            {status.audio.not_applied}.
          {:else}
            This controller routes its own outputs: {routingLine(status.audio)}.
          {/if}
        </p>
      {/if}
    {:else}
      <p class="note">
        Nothing connected. Pads and faders do nothing until a port is open.
      </p>
    {/if}
  {/if}

  {#if error}
    <p class="warn">{error}</p>
  {/if}
</section>

<style>
  .controllers {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.6rem;
    border: 1px solid var(--edge);
    border-radius: var(--radius);
    background: var(--panel);
  }

  .controllers.active {
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

  .pick {
    display: flex;
    gap: 0.4rem;
  }

  .pick select {
    flex: 1;
    min-width: 0;
  }

  .open {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
    font-size: 0.78rem;
  }

  .mapping {
    color: var(--muted);
    font-size: 0.68rem;
    flex: 1;
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
