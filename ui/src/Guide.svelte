<script lang="ts">
  /**
   * §117: the guide — where the DJ is after each key, and every key from here.
   *
   * > a visual feedback guide/legend where we are after each key in the
   * > shortcut chain with all the possible breadcrumbs/outcomes
   *
   * Drawn along the bottom of the window, over the mixer rather than the
   * decks, and only while a chain is being typed: the breadcrumb says how the
   * DJ got here, each row says the key and what it does, a group says so with
   * `›` and a colour, and the letter a key stands for is underlined in its
   * word. It takes no clicks for granted — a click on a row does what the key
   * would — and it never stays: a leaf run, Escape, or a click outside, and
   * it is gone.
   */
  import type { LeaderNode } from "./api";
  import { keyLabel, mnemonicAt, type Leader } from "./leader.svelte";
  import { CHORDS, modName } from "./platform";

  const mod = modName();

  let { leader }: { leader: Leader } = $props();

  const here = $derived(leader.here);
  /** The keys that led here, each with where it went. */
  const trail = $derived(leader.path.slice(1));

  /** A label cut around its mnemonic letter, for the underline. */
  function parts(node: LeaderNode): [string, string, string] {
    const at = mnemonicAt(node.label, node.key);
    if (at < 0) return [node.label, "", ""];
    return [node.label.slice(0, at), node.label[at], node.label.slice(at + 1)];
  }
</script>

{#if leader.open && leader.shown && here}
  <!-- Outside the panel, a click closes it: the guide is never in the way. -->
  <div class="scrim" aria-hidden="true" onclick={() => leader.close()}></div>
  <div class="guide" role="dialog" aria-label="Keys from here" data-guide>
    <header>
      <ol class="trail" aria-label="Where you are">
        <li><kbd>Space</kbd></li>
        {#each trail as step, i (i)}
          <li><span class="sep" aria-hidden="true">›</span><kbd>{keyLabel(step.key)}</kbd> {step.label}</li>
        {/each}
      </ol>
      <span class="how">
        {#if leader.missed}
          <span class="missed" role="status">Nothing on <kbd>{keyLabel(leader.missed)}</kbd> here</span>
        {/if}
        <kbd>Esc</kbd> close · <kbd>⌫</kbd> back
      </span>
    </header>
    <ul class="keys">
      {#each here.children as node (node.key)}
        {@const [before, letter, after] = parts(node)}
        <li class:group={!node.run} class:mine={node.mine}>
          <button type="button" data-key={node.key} onclick={() => leader.choose(node.key)}>
            <kbd>{keyLabel(node.key)}</kbd>
            <span class="label">{before}{#if letter}<u>{letter}</u>{/if}{after}{#if !node.run}<span class="more" aria-hidden="true"> ›</span><span class="sr-only">, more</span>{/if}</span>
          </button>
        </li>
      {/each}
    </ul>
    <footer aria-label="Also, anywhere">
      {#each CHORDS as chord (chord.code)}
        <span><kbd>{mod}</kbd><kbd>{chord.code === "Comma" ? "," : chord.code === "Slash" ? "/" : chord.code.slice(3)}</kbd> {chord.label}</span>
      {/each}
      <span><kbd>{mod}</kbd><kbd>1</kbd>–<kbd>9</kbd> an activity, even while typing</span>
    </footer>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 70;
  }

  .guide {
    position: fixed;
    left: 50%;
    bottom: 0.75rem;
    transform: translateX(-50%);
    z-index: 71;
    width: min(62rem, calc(100vw - 1.5rem));
    max-height: 44vh;
    overflow: auto;
    padding: 0.6rem 0.75rem 0.7rem;
    border: 1px solid var(--border-strong, var(--border));
    border-radius: 0.6rem;
    /* Opaque: the mixer is under it, and a guide the faders show through
       is a guide nobody can read in a dark booth. A theme's panel colour is
       often translucent, so it is laid over the theme's own background. */
    background:
      linear-gradient(var(--panel-raised), var(--panel-raised)),
      var(--bg);
    box-shadow: 0 0.6rem 2rem rgb(0 0 0 / 0.45);
    color: var(--text);
    font-size: 0.9rem;
  }

  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
    padding-bottom: 0.45rem;
    margin-bottom: 0.45rem;
    border-bottom: 1px solid var(--border);
  }

  .trail {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin: 0;
    padding: 0;
    list-style: none;
    color: var(--text-dim);
  }

  .trail li:last-child {
    color: var(--text);
  }

  .sep {
    margin-right: 0.3rem;
    color: var(--text-dim);
  }

  .how {
    display: flex;
    gap: 0.35rem;
    align-items: baseline;
    color: var(--text-dim);
    font-size: 0.8rem;
    white-space: nowrap;
  }

  .missed {
    margin-right: 0.75rem;
    color: var(--warn);
  }

  .keys {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(13.5rem, 1fr));
    gap: 0.15rem 0.9rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .keys button {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    width: 100%;
    padding: 0.2rem 0.3rem;
    border: 0;
    border-radius: 0.3rem;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .keys button:hover,
  .keys button:focus-visible {
    background: var(--panel-hover);
  }

  .more {
    margin-left: 0.25em;
  }

  .group .label {
    color: var(--accent);
    font-weight: 600;
  }

  .mine kbd {
    border-color: var(--accent);
  }

  kbd {
    display: inline-block;
    min-width: 1.6em;
    padding: 0 0.35em;
    border: 1px solid var(--border-strong, var(--border));
    border-radius: 0.25rem;
    background: var(--panel-raised);
    font-family: var(--mono, monospace);
    font-size: 0.85em;
    text-align: center;
  }

  footer {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem 1rem;
    margin-top: 0.45rem;
    padding-top: 0.4rem;
    border-top: 1px solid var(--border);
    color: var(--text-dim);
    font-size: 0.78rem;
  }

  footer kbd + kbd {
    margin-left: 0.15em;
  }

  /* Read aloud, not drawn: a group's "›" says the same to the eye. */
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  u {
    text-decoration-thickness: 2px;
    text-underline-offset: 2px;
  }
</style>
