<script lang="ts">
  /**
   * Panels on other screens.
   *
   * A DJ with two screens does not want a different arrangement on each — they
   * want the same interface, spread out. So a panel is not moved by changing a
   * layout; it is taken out of this window and given one of its own, which the
   * desktop then puts wherever it is dragged.
   *
   * There is no monitor picker here and there will not be one. djmanzo never
   * asks how many screens there are, never positions a window on one, and never
   * has to cope with one being unplugged mid-set.
   */
  import { onMount } from "svelte";
  import { listPanels, detachPanel, attachPanel, type PanelInfo } from "./api";

  let panels = $state<PanelInfo[]>([]);
  let error = $state<string | null>(null);
  let busy = $state<string | null>(null);

  async function refresh() {
    try {
      panels = await listPanels();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function toggle(panel: PanelInfo) {
    busy = panel.id;
    try {
      if (panel.detached) {
        await attachPanel(panel.id);
      } else {
        await detachPanel(panel.id);
      }
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  onMount(() => {
    void refresh();
  });
</script>

<section class="screens">
  <h3>Screens</h3>
  <p class="note">
    Give a panel its own window and drag it wherever you like. It is the same
    application — same decks, same engine, same everything.
  </p>

  <ul>
    {#each panels as panel (panel.id)}
      <li>
        <span class="name">{panel.title.replace(/^djmanzo - /, "")}</span>
        <button
          class:active={panel.detached}
          disabled={busy === panel.id}
          onclick={() => toggle(panel)}
        >
          {#if busy === panel.id}
            …
          {:else if panel.detached}
            Bring back
          {:else}
            Detach
          {/if}
        </button>
      </li>
    {/each}
  </ul>

  {#if error}
    <p class="warn">{error}</p>
  {/if}
</section>

<style>
  .screens {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  h3 {
    margin: 0;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    font-size: 0.75rem;
  }

  li button {
    min-width: 6rem;
    font-size: 0.7rem;
  }

  li button.active {
    background: var(--accent, #4a90a4);
    color: var(--panel);
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
</style>
