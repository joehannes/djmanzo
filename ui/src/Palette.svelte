<script lang="ts">
  /**
   * The command palette: `Ctrl/Cmd + K`, and everything djmanzo can do.
   *
   * # What it is for
   *
   * §51 asks for a command surface, and closes by saying it "can also become
   * the semantic interface exposed to voice/AI". That sentence decides the
   * design. The palette is not a list of pretty labels somebody typed: every
   * entry comes from `dj_core::vocabulary` — the same 82 verbs the parser
   * accepts, the assistant is told about, and a MIDI mapping produces — or
   * from a surface the cockpit publishes.
   *
   * The consequence worth stating: **what you type is a first-class entry.**
   * `deck 2 loop 8` parses, so the top row runs it. That is the only way the
   * verbs taking an argument — a loop length, a key shift, a pitch — can be
   * reached at all, because a list of buttons would have to invent the number.
   *
   * # Why the matching is not here
   *
   * Rust ranks and filters. A palette whose matcher lived in the interface
   * would be a second opinion about which command you meant, and the
   * suggester's lesson is that two rankings over the same thing eventually
   * disagree. The interface draws the answer and sends the key presses.
   */
  import { palette, type PaletteEntry } from "./api";

  let {
    enabled,
    deckCount = 2,
    onAction,
    onSurface,
  }: {
    enabled: boolean;
    deckCount?: number;
    /** Send an action through the bus — the same path a button takes. */
    onAction: (action: string) => void;
    /** Open or close a surface by name. */
    onSurface: (surface: string) => void;
  } = $props();

  let open = $state(false);
  let query = $state("");
  let entries = $state<PaletteEntry[]>([]);
  let chosen = $state(0);
  let field = $state<HTMLInputElement | null>(null);
  let error = $state<string | null>(null);
  /**
   * True until the first answer for the current query is in.
   *
   * Without it the empty list reads "Nothing matches that." while the answer
   * is still in flight, which is a false statement rather than a slow one —
   * and on the first open it is the *only* thing on screen. Seen under Xvfb,
   * where software rendering made the round trip take about two seconds; on a
   * machine with a GPU it would have been too quick to catch and would have
   * shipped.
   */
  let asking = $state(false);

  /**
   * Ask Rust what to offer, whenever the query changes.
   *
   * A round trip per keystroke, which is affordable because the answer is
   * arithmetic over a static table — no database, no file, no network. It is
   * also the only way the "what you typed is a real action" tier can work: the
   * parser is in Rust and the interface must not grow a second copy of it.
   */
  $effect(() => {
    if (!open) return;
    const asked = query;
    asking = true;
    void palette(asked, deckCount)
      .then((found) => {
        // A slower answer to an older query must not overwrite a newer one.
        if (asked !== query) return;
        entries = found;
        chosen = 0;
        error = null;
        asking = false;
      })
      .catch((e) => {
        if (asked !== query) return;
        entries = [];
        error = String(e);
        asking = false;
      });
  });

  function show() {
    open = true;
    query = "";
    chosen = 0;
    asking = true;
    // After the element exists.
    queueMicrotask(() => field?.focus());
  }

  function hide() {
    open = false;
    entries = [];
    error = null;
  }

  function run(entry: PaletteEntry) {
    if (entry.kind === "surface") onSurface(entry.run);
    else onAction(entry.run);
    hide();
  }

  /**
   * The one global key binding djmanzo takes.
   *
   * `Ctrl/Cmd + K` only, and only when a text field does not already have the
   * keys: a DJ typing a search into the browser and pressing K should get a K.
   * Escape closes, and closes nothing else — the palette is the innermost
   * thing on screen while it is open.
   */
  function onKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement | null;
    const typing =
      target?.tagName === "INPUT" ||
      target?.tagName === "TEXTAREA" ||
      target?.isContentEditable === true;

    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      if (open) hide();
      else show();
      return;
    }
    if (!open) return;
    if (event.key === "Escape") {
      event.preventDefault();
      hide();
      return;
    }
    // Everything below is the palette's own list, and only while its field has
    // the keys — so Escape works from anywhere but the arrows do not steal
    // from a field behind it.
    if (!typing || target !== field) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      chosen = entries.length === 0 ? 0 : (chosen + 1) % entries.length;
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      chosen = entries.length === 0 ? 0 : (chosen - 1 + entries.length) % entries.length;
    } else if (event.key === "Enter") {
      event.preventDefault();
      const entry = entries[chosen];
      if (entry) run(entry);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <!--
    A scrim, and the panel near the top rather than centred: a DJ opens this
    mid-transition and the decks behind it are what they are still watching.
  -->
  <div
    class="scrim"
    role="button"
    tabindex="-1"
    aria-label="Close the command palette"
    onclick={hide}
    onkeydown={(e) => e.key === "Enter" && hide()}
  ></div>
  <div class="palette" role="dialog" aria-modal="true" aria-label="Command palette">
    <input
      bind:this={field}
      bind:value={query}
      class="field"
      type="text"
      placeholder="What do you want to do?  (or type an action: deck 2 loop 8)"
      aria-label="Command"
      autocomplete="off"
      spellcheck="false"
    />
    {#if error}
      <p class="error">{error}</p>
    {:else if entries.length === 0}
      <p class="empty">{asking ? "…" : "Nothing matches that."}</p>
    {:else}
      <ul class="entries">
        {#each entries as entry, i (entry.kind + entry.run)}
          <li>
            <button
              class:chosen={i === chosen}
              disabled={!enabled && entry.kind === "action"}
              onclick={() => run(entry)}
              onmouseenter={() => (chosen = i)}
            >
              <span class="label">{entry.label}</span>
              <span class="about">{entry.about}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
    <p class="hint">↑↓ to choose · Enter to run · Esc to close</p>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: var(--scrim, rgba(0, 0, 0, 0.55));
    border: 0;
    padding: 0;
    z-index: 40;
  }

  /*
    Near the top, not centred. This opens mid-transition and what is behind it
    — the decks, the waveforms — is what the DJ is still watching.
  */
  .palette {
    position: fixed;
    top: 12vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(620px, 92vw);
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    background: var(--panel);
    border: 1px solid var(--border-strong, var(--border));
    border-radius: 10px;
    padding: 0.8rem;
    z-index: 41;
    box-shadow: 0 18px 60px rgba(0, 0, 0, 0.5);
  }

  .field {
    width: 100%;
    box-sizing: border-box;
    font-size: 1rem;
    padding: 0.5rem 0.6rem;
    background: var(--control, var(--panel-raised));
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text);
  }

  .entries {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-height: 0;
    overflow-y: auto;
  }

  .entries button {
    width: 100%;
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    text-align: left;
    background: none;
    border: 1px solid transparent;
    border-radius: 5px;
    padding: 0.3rem 0.45rem;
    color: var(--text);
    cursor: pointer;
  }

  /* One row is chosen, and it is the one Enter runs. Marked by fill rather
     than by an outline, so it does not read as a focus ring on a control the
     keyboard is not actually on. */
  .entries button.chosen {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    border-color: var(--accent);
  }

  .entries button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .label {
    flex: none;
    font-weight: 600;
    font-size: 0.85rem;
  }

  .about {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.75rem;
    color: var(--text-dim);
  }

  .hint,
  .empty,
  .error {
    margin: 0;
    font-size: 0.72rem;
    color: var(--text-dim);
  }

  .error {
    color: var(--danger, #dc2626);
  }
</style>
