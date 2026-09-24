<script lang="ts">
  /**
   * §108: going live — the record on a stream.
   *
   * Two ways in, because OBS has two sources that carry text: a Browser
   * source pointed at djmanzo's overlay page (transparent, the record in large
   * type, served on this machine only), and a Text source reading the file
   * djmanzo keeps up to date. Which record is `dj_app::live::lead`: the one
   * the room hears most, changing hands once per blend. Off until switched on.
   */
  import { liveStatus, setLive, type LiveStatus } from "./api";

  let current = $state<LiveStatus | null>(null);
  let error = $state("");
  let copied = $state<string | null>(null);

  async function refresh() {
    try {
      current = await liveStatus();
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    void refresh();
    // What the stream is told changes with the music.
    const poll = setInterval(() => void refresh(), 1_000);
    return () => clearInterval(poll);
  });

  async function change(on: boolean) {
    error = "";
    try {
      current = await setLive(on);
    } catch (e) {
      error = String(e);
    }
  }

  async function copy(what: string) {
    try {
      await navigator.clipboard.writeText(what);
      copied = what;
      setTimeout(() => (copied = null), 1_500);
    } catch {
      // No clipboard: the field is selectable, which is the fallback.
      copied = null;
    }
  }
</script>

<div class="live" data-live>
  {#if current}
    <label class="switch">
      <input
        type="checkbox"
        checked={current.on}
        onchange={(event) => void change(event.currentTarget.checked)}
      />
      Tell the stream what is playing
    </label>
    {#if current.on}
      <p class="saying" data-saying>
        {#if current.saying}
          On the stream now: <strong>{current.saying}</strong>
        {:else}
          Nothing on the stream: no deck is being heard.
        {/if}
      </p>
      <div class="way">
        <span class="label">Browser source</span>
        {#if current.overlay}
          <input class="mono" readonly aria-label="Overlay address for an OBS Browser source" value={current.overlay} data-overlay onfocus={(e) => e.currentTarget.select()} />
          <button onclick={() => void copy(current?.overlay ?? "")}>{copied === current.overlay ? "Copied" : "Copy"}</button>
        {:else}
          <span class="dim">starting…</span>
        {/if}
      </div>
      <div class="way">
        <span class="label">Text source</span>
        {#if current.file}
          <input class="mono" readonly aria-label="File for an OBS Text source" value={current.file} data-file onfocus={(e) => e.currentTarget.select()} />
          <button onclick={() => void copy(current?.file ?? "")}>{copied === current.file ? "Copied" : "Copy"}</button>
        {/if}
      </div>
      <p class="how">
        In OBS, add a <em>Browser</em> source with the address above (800 × 140
        suits it; the page's ground is transparent), or a <em>Text</em> source
        with <em>Read from file</em> ticked and the file above. Both follow the
        music by themselves.
      </p>
    {/if}
    {#if current.problem}
      <p class="error" role="alert">{current.problem}</p>
    {/if}
  {/if}
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
</div>

<style>
  .live {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .switch {
    display: flex;
    gap: 0.4rem;
    align-items: baseline;
  }

  .saying {
    margin: 0;
  }

  .way {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .label {
    min-width: 7.5rem;
    color: var(--text-dim);
  }

  .way input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 0.85em;
    padding: 0.2rem 0.35rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-s, 4px);
    background: var(--sunken);
    color: var(--text);
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
