<script lang="ts">
  /**
   * The plugin insert on the master.
   *
   * # Why the controls look like this and not like the plugin
   *
   * A CLAP plugin's own interface is a native child window — an X11 window on
   * Linux, an `NSView` on macOS — and there is nowhere to put one inside a
   * webview. So djmanzo asks the plugin what controls it has and draws them
   * itself: every plugin gets the same sliders and none of them get their own
   * look.
   *
   * That is a real loss for a plugin whose panel *is* the product. It is also
   * what makes these controls part of djmanzo — a controller can be mapped to
   * one, a preset can save one, the assistant can move one. A plugin's own
   * window can do none of those things, and saying so is better than
   * pretending the window is coming.
   */
  import {
    listPlugins,
    loadPlugin,
    clearPlugin,
    pluginState,
    type ClapState,
    type PluginFile,
    type PluginParam,
    type PluginState,
  } from "./api";
  import IconButton from "./controls/IconButton.svelte";

  let {
    clap,
    enabled,
    send,
  }: {
    clap: ClapState;
    enabled: boolean;
    send: (action: string) => void;
  } = $props();

  let available = $state<PluginFile[]>([]);
  let chosen = $state<string | null>(null);
  let detail = $state<PluginState | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let scanned = $state(false);

  /**
   * The name and the control list are fetched, not streamed. They change when
   * a plugin is loaded or unloaded, which is what this watches — not sixty
   * times a second.
   */
  $effect(() => {
    void clap.loaded;
    pluginState()
      .then((state) => (detail = state.loaded ? state : null))
      .catch(() => (detail = null));
  });

  async function scan() {
    error = null;
    try {
      available = await listPlugins();
      scanned = true;
    } catch (e) {
      error = String(e);
    }
  }

  async function load() {
    if (!chosen) return;
    busy = true;
    error = null;
    try {
      detail = await loadPlugin(chosen);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function clear() {
    busy = true;
    error = null;
    try {
      await clearPlugin();
      detail = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  /**
   * Plain values, not fractions.
   *
   * A plugin's parameters carry their own units — a cutoff really is in hertz —
   * so the slider works in them directly. The number shown is the number the
   * plugin uses, and a preset saved at 440 reopens at 440.
   */
  function step(param: PluginParam): number {
    if (param.stepped) return 1;
    const span = param.max - param.min;
    return span > 0 ? span / 1000 : 0.001;
  }

  function label(param: PluginParam): string {
    const digits = param.stepped ? 0 : 2;
    return param.value.toFixed(digits);
  }
</script>

<section class="plugin" class:active={clap.loaded && !clap.bypassed}>
  <header>
    <h3>Plugin</h3>
    {#if clap.loaded}
      <IconButton
        icon={clap.bypassed ? 'fa-solid fa-toggle-off' : 'fa-solid fa-toggle-on'}
        title="Take it in and out of the signal path without unloading it"
        active={!clap.bypassed}
        disabled={!enabled}
        onClick={() => send(clap.bypassed ? "clap on" : "clap off")}
      />
    {/if}
  </header>

  {#if !clap.loaded}
    <div class="pick">
      <select
        bind:value={chosen}
        disabled={!enabled || busy}
        onfocus={() => !scanned && scan()}
      >
        <option value={null}>
          {scanned && available.length === 0 ? "No plugins found" : "Choose a plugin…"}
        </option>
        {#each available as file (file.path)}
          <option value={file.path}>{file.name}</option>
        {/each}
      </select>
      <IconButton icon="fa-solid fa-file-import" title={busy ? "Loading…" : "Load"} onClick={load} disabled={!enabled || busy || !chosen} />
    </div>
    {#if scanned && available.length === 0}
      <p class="note">
        Nothing in the CLAP folders. On Linux that is <code>~/.clap</code> and
        <code>/usr/lib/clap</code>; on macOS,
        <code>~/Library/Audio/Plug-Ins/CLAP</code>.
      </p>
    {:else}
      <p class="note">
        A plugin runs in this process. Load ones you trust — the same rule as
        any host.
      </p>
    {/if}
  {:else if detail}
    <div class="named">
      <strong>{detail.name}</strong>
      {#if detail.vendor}<span class="vendor">{detail.vendor}</span>{/if}
    </div>

    {#if detail.params.length === 0}
      <p class="note">This plugin has no controls a host can reach.</p>
    {:else}
      <div class="params">
        {#each detail.params as param (param.id)}
          <label class="control" class:read-only={param.readOnly}>
            <span>
              {param.name}
              <em class="mono">{label(param)}</em>
            </span>
            <input
              type="range"
              min={param.min}
              max={param.max}
              step={step(param)}
              value={param.value}
              disabled={!enabled || param.readOnly}
              oninput={(e) => send(`clap param ${param.id} ${e.currentTarget.value}`)}
              title={param.module || param.name}
            />
          </label>
        {/each}
      </div>
    {/if}

    <IconButton icon="fa-solid fa-trash" title="Unload" onClick={clear} disabled={busy} />
    <p class="note">
      Its own window is not shown — a plugin's interface is a native child
      window, and there is nowhere to put one here. These are its controls,
      drawn by djmanzo, and they can be mapped and saved like any other.
    </p>
  {/if}

  {#if error}
    <p class="warn">{error}</p>
  {/if}
</section>

<style>
  .plugin {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.6rem;
    border: 1px solid var(--edge);
    border-radius: var(--radius);
    background: var(--panel);
  }

  .plugin.active {
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
    min-width: 5rem;
    font-weight: 600;
  }

  .switch.active {
    background: var(--accent, #4a90a4);
    color: var(--panel);
  }

  .pick {
    display: flex;
    gap: 0.4rem;
  }

  .pick select {
    flex: 1;
    min-width: 0;
  }

  .named {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
    font-size: 0.78rem;
  }

  .vendor {
    color: var(--muted);
    font-size: 0.68rem;
  }

  /* A plugin may have a great many controls. They scroll rather than pushing
     the rest of the mixer off the screen. */
  .params {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    max-height: 16rem;
    overflow-y: auto;
  }

  .control {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    font-size: 0.7rem;
  }

  .control.read-only {
    opacity: 0.6;
  }

  .control span {
    display: flex;
    justify-content: space-between;
    gap: 0.4rem;
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

  .clear {
    font-size: 0.7rem;
  }

  code {
    font-size: 0.95em;
  }
</style>
