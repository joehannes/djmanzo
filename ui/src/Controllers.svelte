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
    closeHidController,
    controlStatus,
    openController,
    openHidController,
    setKeyboardEnabled,
    controllerLights,
    type AudioRouting,
    type ControlStatus,
    type ControllerLights,
    type MappingInfo,
  } from "./api";

  let { mappings = [] }: { mappings?: MappingInfo[] } = $props();

  /**
   * §53: what the mapping now open actually puts under the hands.
   *
   * Read from the mapping rather than from the device, because a MIDI
   * controller announces a name and nothing else. Said out loud on the panel
   * because §53 opens with *"the UI should know"* — and a DJ who has just
   * plugged something in is entitled to see what djmanzo thinks it can do
   * before finding out mid-set that it disagrees.
   */
  const reach = $derived(
    mappings.find((m) => m.name === status?.open_mapping)?.hands ?? null,
  );

  /**
   * What the open controller reaches, in the order §53 lists it.
   *
   * Zero is drawn, not hidden: "no stem controls" is the reading §53's own
   * worked example turns on, and a row that vanished when the answer was none
   * would hide exactly the fact that matters.
   */
  const reachRows = $derived(
    reach
      ? [
          ["Decks", `${reach.decks}`],
          ["Jogs", `${reach.jogs}`],
          ["Knobs and faders", `${reach.knobs}`],
          ["Mixer channels", `${reach.channels}`],
          ["Pads", `${reach.pads}`],
          ["Stem controls", reach.stems ? "yes" : "none"],
        ]
      : [],
  );

  let status = $state<ControlStatus | null>(null);
  let chosenPort = $state<string | null>(null);
  let chosenMapping = $state<string | null>(null);
  let chosenHid = $state<string | null>(null);
  let chosenHidMapping = $state<string | null>(null);
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

  /**
   * §53's other direction: what djmanzo is lighting, and why it is not.
   *
   * Asked of Rust rather than worked out from the mapping's own count, because
   * the two are different questions and the difference is the whole point: a
   * mapping that describes sixty lights and a controller whose output is held
   * by another application look identical from the mapping alone. That was the
   * state this panel was in — it said djmanzo "does not send them yet", which
   * stopped being true and would have gone on saying it.
   */
  let lights = $state<ControllerLights | null>(null);

  async function refresh() {
    try {
      status = await controlStatus();
      lights = await controllerLights();
      error = null;
      // Keep the choice pointing at something that still exists.
      if (chosenPort && !status.inputs.includes(chosenPort)) chosenPort = null;
      if (!chosenPort) chosenPort = status.open_port ?? status.inputs[0] ?? null;
      if (chosenHid && !status.hid_inputs.some((d) => d.path === chosenHid))
        chosenHid = null;
      if (!chosenHid) chosenHid = status.hid_inputs[0]?.path ?? null;
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

  async function connectHid() {
    if (!chosenHid || !chosenHidMapping) return;
    busy = true;
    error = null;
    try {
      await openHidController(chosenHid, chosenHidMapping);
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function disconnectHid() {
    busy = true;
    error = null;
    try {
      await closeHidController();
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
      <select aria-label="MIDI input" bind:value={chosenPort} disabled={busy}>
        {#each status.inputs as port (port)}
          <option value={port}>{port}</option>
        {/each}
      </select>
      <select
        aria-label="Mapping for this MIDI controller"
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

      <!--
        §53: *the UI should know*. What the open mapping reaches, read off the
        mapping rather than off the device — a MIDI controller announces a name
        and nothing else, and its bindings are what a hand will actually find.
      -->
      {#if reach}
        <dl class="reach" data-reach>
          {#each reachRows as [label, value] (label)}
            <dt>{label}</dt>
            <dd>{value}</dd>
          {/each}
        </dl>
        <!--
          The screens are the one thing §53 asks for that djmanzo still cannot
          answer from a mapping, and it is named rather than counted.

          The lights now say what is actually happening, which is three
          different answers and not one: driven, described but not driven with
          the reason, or not described at all. A count on its own would imply a
          controller that lights up, which is exactly what this line used to do
          in the other direction.
        -->
        <p class="note" data-lights>
          Screens: djmanzo cannot see them — they are driven by the
          controller's own firmware, not by a mapping.
          {#if lights && lights.lit > 0}
            Lights: djmanzo is driving {lights.lit} of them, out to
            <strong>{lights.port}</strong>.
          {:else if lights?.unlit}
            Lights: this mapping describes {reach.leds}, and djmanzo cannot
            send them — {lights.unlit}.
          {:else if reach.leds > 0}
            Lights: this mapping describes {reach.leds}.
          {/if}
        </p>
      {/if}

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

  {#if status}
    <div class="hid">
      <h4>HID</h4>
      {#if status.open_hid}
        <div class="open">
          <strong>{status.open_hid}</strong>
          <span class="mapping">{status.open_hid_mapping ?? "no mapping"}</span>
          <IconButton
            icon="unlink"
            title="Disconnect"
            onClick={disconnectHid}
            disabled={busy}
          />
        </div>
      {:else if status.hid_unavailable}
        <p class="note">
          {status.hid_unavailable}. On Linux a controller's device node usually
          belongs to root until a udev rule says otherwise.
        </p>
      {:else if status.hid_inputs.length === 0}
        <p class="note">No HID devices.</p>
      {:else}
        <div class="pick">
          <select aria-label="HID device" bind:value={chosenHid} disabled={busy}>
            {#each status.hid_inputs as device (device.path)}
              <option value={device.path}>{device.name} · {device.id}</option>
            {/each}
          </select>
          <select aria-label="Mapping for this HID device" bind:value={chosenHidMapping} disabled={busy}>
            <option value={null}>Choose a mapping…</option>
            {#each mappings as mapping (mapping.name)}
              <option value={mapping.name}>{mapping.name}</option>
            {/each}
          </select>
          <IconButton
            icon="check"
            title={chosenHidMapping
              ? "Connect"
              : "A HID mapping names byte offsets, so it has to be chosen"}
            onClick={connectHid}
            disabled={busy || !chosenHid || !chosenHidMapping}
          />
        </div>
        <p class="note">
          HID is for controllers whose jog wheels need more than seven bits.
          There is no “whichever fits” here: a HID mapping names byte offsets
          into this device's reports, and another device's offsets would bind
          the wrong controls rather than fail.
        </p>
      {/if}
    </div>
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
    border-color: var(--active);
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

  .hid {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    padding-top: 0.4rem;
    border-top: 1px solid var(--edge);
  }

  h4 {
    margin: 0;
    font-size: 0.65rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
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

  .reach {
    display: grid;
    grid-template-columns: auto auto;
    gap: 0.1rem 0.6rem;
    margin: 0.5rem 0 0;
    font-size: 0.8em;
  }

  .reach dt {
    color: var(--muted);
  }

  .reach dd {
    margin: 0;
    font-variant-numeric: tabular-nums;
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
