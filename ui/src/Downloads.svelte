<script lang="ts">
  /**
   * §111: the downloads folder, and music filed from it.
   *
   * A record bought through one of the "find it to buy" links lands wherever
   * the browser saves things. With this on, djmanzo waits until it has
   * finished arriving, moves it into the music folder under its family and
   * artist — `House/Kerri Chandler/`, `Karaoke/Queen/` — and adds it to the
   * collection. An album that arrives as a `.zip` has its records taken out
   * and filed the same way, and the archive put away in `Unpacked/`. The
   * rules are `dj_app::downloads::place` and `take_album`, tested there. Off
   * until the DJ switches it on: moving somebody's files is never something
   * that starts by itself.
   */
  import { open } from "@tauri-apps/plugin-dialog";
  import { defaultMusicFolder, downloads, setDownloads, type Downloads } from "./api";

  let current = $state<Downloads | null>(null);
  let error = $state("");
  let suggestedMusic = $state<string | null>(null);

  async function refresh() {
    try {
      current = await downloads();
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    void refresh();
    void defaultMusicFolder()
      .then((found) => (suggestedMusic = found))
      .catch(() => {});
    // The list of filings grows while the settings are open.
    const poll = setInterval(() => void refresh(), 5_000);
    return () => clearInterval(poll);
  });

  async function change(watch: string | null, into: string | null, on: boolean) {
    error = "";
    try {
      current = await setDownloads(watch, into, on);
    } catch (e) {
      error = String(e);
    }
  }

  async function choose(which: "watch" | "into") {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== "string" || !current) return;
    await change(
      which === "watch" ? picked : current.watch,
      which === "into" ? picked : current.into,
      current.on,
    );
  }
</script>

<div class="downloads" data-downloads>
  {#if current}
    <div class="row">
      <span class="label">Watch</span>
      <span class="path mono" data-watch>{current.watch ?? "— the folder your browser saves to"}</span>
      <button onclick={() => void choose("watch")}>Choose…</button>
    </div>
    <div class="row">
      <span class="label">File into</span>
      <span class="path mono" data-into>{current.into ?? "— your music folder"}</span>
      <button onclick={() => void choose("into")}>Choose…</button>
      {#if !current.into && suggestedMusic}
        <button class="quiet" onclick={() => void change(current?.watch ?? null, suggestedMusic, false)}
          >Use {suggestedMusic}</button
        >
      {/if}
    </div>
    <label class="switch">
      <input
        type="checkbox"
        checked={current.on}
        disabled={!current.watch || !current.into}
        onchange={(event) => void change(current?.watch ?? null, current?.into ?? null, event.currentTarget.checked)}
      />
      File new music automatically — under its family and artist, karaoke and a cappellas on their own, an album
      taken out of its .zip
    </label>
    {#if current.filed.length > 0}
      <ul class="filed" aria-label="Filed lately">
        {#each current.filed as filing, i (i)}
          <li class:problem={filing.problem}>
            <span class="mono">{filing.arrived}</span>
            {#if filing.problem}
              — left where it was: {filing.problem}
            {:else}
              → <span class="mono">{filing.to}</span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
</div>

<style>
  .downloads {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  .label {
    min-width: 4.5rem;
    color: var(--text-dim);
  }

  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .switch {
    display: flex;
    gap: 0.4rem;
    align-items: baseline;
  }

  .filed {
    margin: 0.2rem 0 0;
    padding-left: 1rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .filed .problem {
    color: var(--danger);
  }

  .error {
    margin: 0;
    color: var(--danger);
  }
</style>
