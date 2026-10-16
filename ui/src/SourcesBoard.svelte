<script lang="ts">
  /**
   * §128: the ⚙ in the bar — every source djmanzo can use, by what it is
   * for: finding songs, playing them, buying them, lyrics, songs like this
   * one, covers and artists. Rust builds the list from the tables that
   * already describe the sources (`dj_app::sources_board`); this draws it,
   * says for each whether it works now, and takes the host to where it is
   * set up. Nothing here connects anything by itself.
   */
  import { onMount } from "svelte";
  import { sourcesBoard, type SourcesBoard, type SourceReady } from "./sourcesBoard";

  let {
    onclose,
    onsetup,
  }: {
    onclose: () => void;
    /** Open Settings where this kind of source is set up. */
    onsetup: (where: "sources" | "assistant") => void;
  } = $props();

  let board = $state<SourcesBoard | null>(null);
  let error = $state("");

  onMount(() => {
    sourcesBoard()
      .then((answer) => (board = answer))
      .catch((e) => (error = String(e)));
  });

  const WORD: Record<SourceReady, string> = {
    ready: "Ready",
    needs_setup: "Set up",
    partner_gated: "Partners only",
    off: "Off",
  };
  const ICON: Record<string, string> = {
    search: "🔎",
    play: "▶",
    buy: "🛒",
    lyrics: "🎤",
    similar: "✨",
    imagery: "🖼",
  };
</script>

<div
  class="backdrop"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) onclose();
  }}
>
  <div class="board" role="dialog" aria-modal="true" aria-labelledby="sources-title">
    <header>
      <h2 id="sources-title">Sources and connections</h2>
      <button class="close" aria-label="Close sources and connections" onclick={onclose}>✕</button>
    </header>
    <p class="lede">
      Where djmanzo finds, plays, buys, and dresses your songs — what works now, and what needs setting up.
    </p>
    {#if error}
      <p class="problem" role="alert">{error}</p>
    {:else if !board}
      <p class="lede">Asking…</p>
    {:else}
      <div class="jobs">
        {#each board.jobs as job (job.job)}
          <section class="job" aria-label={job.title} data-job={job.job}>
            <h3><span aria-hidden="true">{ICON[job.job]}</span> {job.title}</h3>
            <ul>
              {#each job.rows as row (row.name)}
                <li data-ready={row.ready}>
                  <div class="row-head">
                    <span class="name">{row.name}</span>
                    <span class="chip">{WORD[row.ready]}</span>
                  </div>
                  <p class="what">{row.what}</p>
                  <p class="detail">
                    {row.detail}
                    {#if row.setup}
                      <button
                        class="setup"
                        onclick={() => onsetup(row.setup === "assistant" ? "assistant" : "sources")}
                        >Set up →</button
                      >
                    {/if}
                  </p>
                </li>
              {/each}
            </ul>
          </section>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: start center;
    padding: 4vh 1rem;
    background: rgb(0 0 0 / 0.45);
    overflow: auto;
  }

  .board {
    width: min(68rem, 100%);
    background: var(--panel);
    border: 1px solid var(--accent);
    border-radius: 14px;
    padding: 1rem 1.2rem 1.2rem;
    box-shadow: 0 1.2rem 3rem rgb(0 0 0 / 0.5);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h2 {
    margin: 0;
    font-size: 1.15rem;
  }

  .close {
    width: 2rem;
    height: 2rem;
    padding: 0;
    border-radius: 50%;
  }

  .lede,
  .problem {
    margin: 0.3rem 0 0.8rem;
    color: var(--text-dim);
  }

  .jobs {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
    gap: 0.8rem;
  }

  .job {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.6rem 0.7rem;
    background: color-mix(in srgb, var(--accent) 4%, var(--panel));
  }

  h3 {
    margin: 0 0 0.4rem;
    font-size: 0.95rem;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  li {
    border-left: 3px solid var(--border);
    padding-left: 0.5rem;
  }

  li[data-ready="ready"] {
    border-left-color: var(--accent);
  }

  li[data-ready="needs_setup"] {
    border-left-color: var(--warn, var(--accent-2));
  }

  .row-head {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
    align-items: baseline;
  }

  .name {
    font-weight: 600;
  }

  .chip {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
    white-space: nowrap;
  }

  li[data-ready="ready"] .chip {
    color: var(--accent);
  }

  .what,
  .detail {
    margin: 0.1rem 0 0;
    font-size: 0.82rem;
    color: var(--text-dim);
  }

  .setup {
    margin-left: 0.3rem;
    padding: 0.05rem 0.4rem;
    font-size: 0.78rem;
  }
</style>
