<script lang="ts">
  /**
   * The keyboard, drawn.
   *
   * Two jobs. One is the sheet a DJ reads to learn the layout — grouped, so
   * the mirror between the two hands is visible rather than something you
   * work out. The other is live: a key lights while it is held, which is the
   * only way to see that a censor is on when the deck is doing the thing you
   * asked for and looks like it is playing normally.
   */
  import { grouped, pretty, type Keyboard } from "./keyboard.svelte";
  import { setKeyboardEnabled } from "./api";
  import IconButton from "./controls/IconButton.svelte";

  let { keyboard, onclose }: { keyboard: Keyboard; onclose: () => void } = $props();

  const groups = $derived(grouped(keyboard.bindings));

  function toggle() {
    keyboard.enabled = !keyboard.enabled;
    // The backend keeps the same switch so a controller mapping that binds a
    // "keyboard off" action agrees with the interface about the answer.
    void setKeyboardEnabled(keyboard.enabled).catch(() => {});
    if (!keyboard.enabled) keyboard.releaseAll();
  }
</script>

<section class="shortcuts">
  <header>
    <h2>Keyboard</h2>
    <div class="controls">
      <IconButton active={keyboard.enabled} onClick={toggle} title={keyboard.enabled ? "Listening" : "Off"}>
        {keyboard.enabled ? "Listening" : "Off"}
      </IconButton>
      <IconButton icon="fa-solid fa-xmark" title="Close" onClick={onclose} />
    </div>
  </header>

  <p class="hint">
    The keyboard is a controller like any other — the same vocabulary, the same
    file format. Put a <code>.toml</code> in your mappings folder to change it.
    Keys are named by position, so this layout holds on an AZERTY or QWERTZ
    keyboard.
  </p>

  {#if !keyboard.enabled}
    <p class="off">
      Not listening. Keys go to the interface instead — which is what you want
      while typing, and not what you want while playing.
    </p>
  {/if}

  <div class="groups">
    {#each groups as [name, keys] (name)}
      <div class="group">
        <h3>{name}</h3>
        <ul>
          {#each keys as key (key.chord)}
            <li class:down={keyboard.isDown(key.chord)} class:held={key.held}>
              <kbd>{pretty(key.chord)}</kbd>
              <span>{key.label}</span>
            </li>
          {/each}
        </ul>
      </div>
    {/each}
  </div>
</section>

<style>
  .shortcuts {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    min-height: 0;
    overflow-y: auto;
  }

  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
  }

  h2 {
    margin: 0;
    font-size: 1rem;
  }

  .controls {
    display: flex;
    gap: 0.4rem;
  }

  .hint {
    margin: 0;
    font-size: 0.75rem;
    line-height: 1.5;
    color: var(--text-dim);
  }

  .off {
    margin: 0;
    padding: 0.5rem 0.65rem;
    border-radius: var(--radius-s, 4px);
    background: var(--warn-bg, rgb(255 200 80 / 0.12));
    font-size: 0.75rem;
    line-height: 1.5;
  }

  .groups {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr));
    gap: 0.75rem 1rem;
  }

  h3 {
    margin: 0 0 0.35rem;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-dim);
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  li {
    display: grid;
    grid-template-columns: 3.75rem 1fr;
    align-items: center;
    gap: 0.5rem;
    padding: 0.15rem 0.3rem;
    border-radius: var(--radius-s, 4px);
    font-size: 0.75rem;
    /*
      Only the background moves, and only between two flat colours: this list
      is 76 rows and it redraws whenever a key goes down. See ADR-0004 — a
      transition on a property that triggers layout would cost a frame every
      time a pad is hit.
    */
    background: transparent;
  }

  li.down {
    background: var(--accent, #4ea1ff);
    color: var(--bg, #111);
  }

  kbd {
    justify-self: start;
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
    border: 1px solid var(--line, rgb(255 255 255 / 0.15));
    background: var(--surface-2, rgb(255 255 255 / 0.05));
    font: inherit;
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  li.down kbd {
    border-color: currentColor;
    background: transparent;
    color: inherit;
  }

  /* A held key is a different instrument from a latching one, so it reads as one. */
  li.held span::after {
    content: " ·";
    color: var(--text-dim);
  }

  li.down span::after {
    color: inherit;
  }
</style>
