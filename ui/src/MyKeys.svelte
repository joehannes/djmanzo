<script lang="ts">
  /**
   * §117: the DJ's own keys under Space.
   *
   * > let the user be capable of creating his own shortcuts/mnemonics as well
   * > and those shall be reflected in the visual guide.
   *
   * Keys are pressed, not typed out: the box takes the keys the DJ presses
   * after Space, as the guide will read them. What they do is found the way
   * everything else is — the palette's own search, so a DJ's key can name
   * anything the palette offers and nothing it does not. Rust checks the
   * pair against the tree before keeping it (`dj_app::leader::check`) and
   * says why when it will not.
   */
  import { forgetMnemonic, keepMnemonic, leaderMine, palette, type MyKey, type PaletteEntry } from "./api";
  import { keyLabel, keyOf } from "./leader.svelte";

  let { decks, onchange }: { decks: number; onchange: () => void } = $props();

  let mine = $state<MyKey[]>([]);
  let keys = $state<string[]>([]);
  let query = $state("");
  let found = $state<PaletteEntry[]>([]);
  let chosen = $state<PaletteEntry | null>(null);
  let label = $state("");
  let refused = $state("");
  /** Whether the name field has just been focused, for its mouseup. */
  let justFocused = false;

  $effect(() => {
    void leaderMine()
      .then((list) => (mine = list))
      .catch(() => {});
  });

  /** A palette entry as a leaf's `run`. */
  function runOf(entry: PaletteEntry): string {
    return entry.kind === "ui" ? `uiop ${entry.run}` : `${entry.kind} ${entry.run}`;
  }

  /** The keys box: each key pressed is one more step, Backspace takes one back. */
  function onKeys(event: KeyboardEvent) {
    if (event.key === "Tab") return;
    event.preventDefault();
    if (event.key === "Backspace") {
      keys = keys.slice(0, -1);
      return;
    }
    if (event.ctrlKey || event.altKey || event.metaKey || keys.length >= 4) return;
    const key = keyOf(event);
    if (key) keys = [...keys, key];
  }

  async function search() {
    chosen = null;
    if (!query.trim()) {
      found = [];
      return;
    }
    try {
      found = (await palette(query, decks)).entries.slice(0, 8);
    } catch {
      found = [];
    }
  }

  function choose(entry: PaletteEntry) {
    chosen = entry;
    // The palette's own words, without its "Run:" for a typed action.
    if (!label.trim()) label = entry.label.replace(/^Run: /, "");
  }

  async function keep() {
    if (!chosen) return;
    refused = "";
    try {
      mine = await keepMnemonic(keys, label, runOf(chosen), decks);
      keys = [];
      query = "";
      found = [];
      chosen = null;
      label = "";
      onchange();
    } catch (why) {
      refused = String(why);
    }
  }

  async function forget(which: string[]) {
    mine = await forgetMnemonic(which);
    onchange();
  }
</script>

<section class="mine" aria-label="Your keys under Space">
  <h3>Your keys under Space</h3>
  {#if mine.length > 0}
    <ul>
      {#each mine as one (one.keys.join(" "))}
        <li data-my-key={one.keys.join(" ")}>
          <span class="chain"><kbd>Space</kbd>{#each one.keys as key, i (i)}<kbd>{keyLabel(key)}</kbd>{/each}</span>
          <span>{one.label}</span>
          <button type="button" onclick={() => void forget(one.keys)} aria-label={`Forget ${one.label}`}>Forget</button>
        </li>
      {/each}
    </ul>
  {/if}
  <div class="form">
    <label>
      <span>Keys after Space</span>
      <input
        readonly
        value={keys.map(keyLabel).join(" ")}
        placeholder="press them here"
        onkeydown={onKeys}
        aria-label="Keys after Space"
      />
    </label>
    <label>
      <span>Does</span>
      <input type="search" bind:value={query} oninput={() => void search()} placeholder="deck 1 loop 8, library, Aurora…" aria-label="What the key does" />
    </label>
    {#if found.length > 0}
      <div class="found" role="radiogroup" aria-label="What it does">
        {#each found as entry (entry.kind + entry.run)}
          <button
            type="button"
            role="radio"
            aria-checked={chosen === entry}
            class:chosen={chosen === entry}
            onclick={() => choose(entry)}>{entry.label}</button
          >
        {/each}
      </div>
    {/if}
    <label>
      <span>Name in the guide</span>
      <!--
        Selected on focus: a suggested name is replaced by typing, not typed
        into. The mouseup of the click that focused it would collapse the
        selection again, so that one is not let through — found by clicking
        into it in the running application and typing a name onto the end
        of the suggestion.
      -->
      <input
        bind:value={label}
        onfocus={(event) => {
          event.currentTarget.select();
          justFocused = true;
        }}
        onmouseup={(event) => {
          if (justFocused) event.preventDefault();
          justFocused = false;
        }}
        aria-label="Name in the guide"
      />
    </label>
    <button type="button" class="keep" disabled={keys.length === 0 || !chosen} onclick={() => void keep()}>Keep</button>
    {#if refused}
      <p class="refused" role="alert">{refused}</p>
    {/if}
  </div>
</section>

<style>
  .mine {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    font-size: 0.75rem;
  }

  h3 {
    margin: 0;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-dim);
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 0.5rem;
  }

  .chain {
    display: flex;
    gap: 0.15rem;
  }

  kbd {
    padding: 0.05rem 0.3rem;
    border: 1px solid var(--line);
    border-radius: 3px;
    font: inherit;
    font-size: 0.7rem;
  }

  .form {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
    gap: 0.4rem 0.75rem;
    align-items: end;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    color: var(--text-dim);
  }

  .found {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }

  .found .chosen {
    background: var(--selected);
    color: var(--on-selected);
  }

  .refused {
    grid-column: 1 / -1;
    margin: 0;
    color: var(--danger);
  }
</style>
