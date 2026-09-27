<script lang="ts">
  /**
   * §123: the words of a song of a guest's own, written by the DJ's AI
   * provider, ready to copy into a music service.
   *
   * Only for a guest who agreed to a song being made for them and has sung.
   * What the model is told is `dj_app::guests::brief` — their name, the song,
   * where and when, their favourites and languages, never their contact
   * details — and what comes back is checked in Rust before it is kept.
   */
  import { guestSong, type Guest, type GuestBook } from "./api";

  let { guest, onbook }: { guest: Guest; onbook: (book: GuestBook) => void } = $props();

  /** The song they sang last, and so the one the song is about. */
  const last = $derived(guest.sang[guest.sang.length - 1]);
  const draft = $derived(last?.song ?? null);

  let language = $state("");
  let keywords = $state("");
  let writing = $state(false);
  let error = $state("");
  let copied = $state("");

  async function write() {
    if (!last) return;
    writing = true;
    error = "";
    const date = new Date(last.at * 1000).toLocaleDateString(undefined, {
      weekday: "long",
      day: "numeric",
      month: "long",
      year: "numeric",
    });
    try {
      onbook(await guestSong(guest.id, language.trim(), keywords.trim(), date));
    } catch (e) {
      error = String(e);
    } finally {
      writing = false;
    }
  }

  async function copy(what: string, text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = what;
      setTimeout(() => {
        if (copied === what) copied = "";
      }, 2000);
    } catch (e) {
      error = `Could not copy: ${String(e)}`;
    }
  }
</script>

{#if last}
  <div class="song" role="group" aria-label="A song for {guest.name}">
    <h4>A song for {guest.name}</h4>
    <p class="hint">
      Words about tonight — {last.title}{last.event ? `, ${last.event}` : ""}{last.place ? `, ${last.place}` : ""} —
      written by your AI provider. It is told their name, the song, where and when, their favourites and languages;
      not how to reach them.
    </p>
    <div class="ask">
      <label
        >Sung in <input
          bind:value={language}
          size="8"
          placeholder="its own"
          title="The language of the song they sang — or empty, for the AI to know it from the song"
        /></label
      >
      <label class="wide"
        >Your ideas <input bind:value={keywords} placeholder="first time on stage, the whole table sang along" /></label
      >
      <button type="button" disabled={writing} onclick={() => void write()}>
        {writing ? "Writing…" : draft ? "Write them again" : "Write the words"}
      </button>
    </div>
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    {#if draft}
      <div class="style">
        <span class="label">Style</span>
        <span class="text" data-style>{draft.style}</span>
        <button type="button" onclick={() => void copy("style", draft.style)}
          >{copied === "style" ? "Copied" : "Copy the style"}</button
        >
      </div>
      {#each draft.versions as version (version.language)}
        <section class="version" aria-label="Words in {version.language}">
          <div class="head">
            <h5>{version.language}</h5>
            <button type="button" onclick={() => void copy(version.language, version.lyrics)}
              >{copied === version.language ? "Copied" : `Copy the ${version.language} words`}</button
            >
          </div>
          <pre>{version.lyrics}</pre>
        </section>
      {/each}
    {/if}
  </div>
{/if}

<style>
  .song {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    border-top: 1px solid var(--border);
    padding-top: 0.5rem;
  }

  h4,
  h5 {
    margin: 0;
    font-size: 0.95em;
  }

  .hint {
    margin: 0;
    color: var(--text-dim);
    font-size: 0.85em;
  }

  .error {
    margin: 0;
    color: var(--danger);
  }

  .ask {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
  }

  label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  label.wide {
    flex: 1 1 14rem;
  }

  label.wide input {
    flex: 1;
  }

  input {
    font: inherit;
    background: var(--panel-raised);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.2rem 0.35rem;
    min-width: 0;
  }

  .style {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: baseline;
  }

  .label {
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .text {
    flex: 1 1 12rem;
  }

  .version {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .head {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    justify-content: space-between;
  }

  pre {
    margin: 0;
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 0.9em;
    background: var(--panel-raised);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.4rem 0.5rem;
    max-height: 14rem;
    overflow: auto;
  }
</style>
