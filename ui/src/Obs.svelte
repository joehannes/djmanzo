<script lang="ts">
  /**
   * §119: djmanzo drives OBS — the switch, where OBS is, and what it is doing.
   *
   * The link is Rust's (`dj_app::obs`): one thread holds the connection,
   * reconnects by itself when OBS comes back, and carries out what is asked
   * here. This draws what it says and sends what the DJ presses; nothing
   * here talks to OBS. The password goes one way: typed here, kept by Rust,
   * and never read back — the screen is told only whether one is kept.
   */
  import {
    obsScene,
    obsSettings,
    obsSettingsSet,
    obsStatus,
    obsStream,
    type ObsSettings,
    type ObsStatus,
  } from "./api";

  let settings = $state<ObsSettings | null>(null);
  let status = $state<ObsStatus | null>(null);
  /** The address and password as typed, until they are kept. */
  let host = $state("localhost");
  let port = $state(4455);
  let password = $state("");
  let error = $state("");
  let busy = $state(false);

  $effect(() => {
    void obsSettings()
      .then((kept) => {
        settings = kept;
        host = kept.host;
        port = kept.port;
      })
      .catch((e) => (error = String(e)));
  });

  $effect(() => {
    const look = () =>
      void obsStatus()
        .then((now) => (status = now))
        .catch(() => {});
    look();
    const poll = setInterval(look, 1_000);
    return () => clearInterval(poll);
  });

  /** Keep the switch, the address and — if one was typed — the password. */
  async function keep(on: boolean) {
    error = "";
    try {
      settings = await obsSettingsSet(on, host, Number(port), password === "" ? undefined : password);
      password = "";
    } catch (e) {
      error = String(e);
    }
  }

  async function act(work: () => Promise<void>) {
    busy = true;
    error = "";
    try {
      await work();
      status = await obsStatus();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="obs" data-obs>
  {#if settings}
    <label class="switch">
      <input type="checkbox" checked={settings.on} onchange={(event) => void keep(event.currentTarget.checked)} />
      Drive OBS from djmanzo
    </label>
    <div class="where">
      <label>
        Address
        <input class="mono" aria-label="OBS address" bind:value={host} onchange={() => void keep(settings?.on ?? false)} />
      </label>
      <label>
        Port
        <input
          class="mono port"
          type="number"
          min="1"
          max="65535"
          aria-label="OBS port"
          bind:value={port}
          onchange={() => void keep(settings?.on ?? false)}
        />
      </label>
      <label>
        Password
        <input
          type="password"
          aria-label="OBS WebSocket password"
          placeholder={settings.has_password ? "kept — type to change" : "none"}
          bind:value={password}
          onchange={() => void keep(settings?.on ?? false)}
        />
      </label>
    </div>

    {#if settings.on && status}
      {#if status.connected}
        <p class="state" data-obs-state="connected">
          Connected.
          {#if status.streaming}<strong class="streaming">Streaming</strong>{:else}Not streaming{/if}
          {#if status.scenes.current}· on air: <strong>{status.scenes.current}</strong>{/if}
        </p>
        <div class="row">
          {#if status.streaming}
            <button class="stop" disabled={busy} onclick={() => void act(() => obsStream(false))}>Stop the stream</button>
          {:else}
            <button disabled={busy} onclick={() => void act(() => obsStream(true))}>Start the stream</button>
          {/if}
        </div>
        {#if status.scenes.all.length > 0}
          <div class="scenes" role="group" aria-label="OBS scenes">
            {#each status.scenes.all as scene (scene)}
              <button
                aria-pressed={scene === status.scenes.current}
                disabled={busy}
                onclick={() => void act(() => obsScene(scene))}>{scene}</button
              >
            {/each}
          </div>
        {/if}
      {:else}
        <p class="state" data-obs-state="waiting">
          {status.error ?? "Reaching OBS…"}
          <span class="dim">— djmanzo tries again by itself every few seconds.</span>
        </p>
      {/if}
    {/if}
    <p class="how">
      OBS 28 or later: <em>Tools → WebSocket Server Settings</em>, tick
      <em>Enable WebSocket server</em>, and copy the port and password here. The
      scenes, sources and stream key stay OBS's; djmanzo only starts and stops
      the stream and puts a scene on air.
    </p>
  {/if}
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
</div>

<style>
  .obs {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .switch {
    display: flex;
    gap: 0.4rem;
    align-items: baseline;
  }

  .where {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .where label {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    color: var(--text-dim);
    font-size: 0.85em;
  }

  .where input {
    font: inherit;
    font-size: 1.1em;
    padding: 0.2rem 0.35rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-s, 4px);
    background: var(--sunken);
    color: var(--text);
  }

  .where .port {
    width: 6rem;
  }

  .state {
    margin: 0;
  }

  .streaming {
    color: var(--danger);
  }

  .row,
  .scenes {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .scenes button[aria-pressed="true"] {
    border-color: var(--active);
    color: var(--active);
  }

  .dim,
  .how {
    color: var(--text-dim);
  }

  .how {
    margin: 0.1rem 0 0;
    font-size: 0.85em;
    line-height: 1.4;
  }

  .error {
    margin: 0;
    color: var(--danger);
  }
</style>
