<script lang="ts">
  /**
   * §122 step 4b: the DJ's own MilkDrop presets — added from disk, each
   * offered as a look ("Yours: …") wherever looks are picked.
   *
   * MilkDrop's own `.milk` files and Butterchurn's `.json` are taken, each only
   * if djmanzo can draw it; one that cannot is said, with why. The community's
   * presets are their authors' works: djmanzo draws them for the DJ who chose
   * them and never sends them anywhere — see `docs/VISUALS.md`.
   */
  import { open } from "@tauri-apps/plugin-dialog";
  import { milkdropAdd, milkdropPresets, milkdropRemove, type MilkdropYours } from "./api";

  let { onchange }: { onchange?: () => void } = $props();

  let presets = $state<MilkdropYours[]>([]);
  let refused = $state<string[]>([]);
  let error = $state("");

  $effect(() => {
    void milkdropPresets()
      .then((all) => (presets = all))
      .catch(() => {});
  });

  async function add() {
    error = "";
    try {
      const picked = await open({
        multiple: true,
        filters: [{ name: "MilkDrop presets", extensions: ["milk", "json"] }],
      });
      if (!picked) return;
      const paths = Array.isArray(picked) ? picked : [picked];
      if (paths.length === 0) return;
      const added = await milkdropAdd(paths);
      presets = added.presets;
      refused = added.refused;
      onchange?.();
    } catch (e) {
      error = String(e);
    }
  }

  async function remove(name: string) {
    error = "";
    try {
      presets = await milkdropRemove(name);
      refused = [];
      onchange?.();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="presets" data-milkdrop-presets>
  <div class="head">
    <span>Your MilkDrop presets</span>
    <button onclick={() => void add()}>Add MilkDrop presets…</button>
  </div>
  {#if presets.length === 0}
    <p class="hint">
      MilkDrop's own <code>.milk</code> files, or Butterchurn's <code>.json</code>. Each becomes a look,
      "Yours: …", behind the words or on the projector. They are yours to answer for: djmanzo draws
      them and never sends them anywhere, and a pack of them is made only when you make one.
    </p>
  {:else}
    <ul>
      {#each presets as preset (preset.file)}
        <li data-preset={preset.name}>
          <span class="name">{preset.name}</span>
          {#if preset.problem}
            <span class="problem" role="note">Cannot be drawn: {preset.problem}</span>
          {:else if preset.note}
            <span class="note">{preset.note}</span>
          {/if}
          <button class="quiet" onclick={() => void remove(preset.name)} aria-label={`Remove ${preset.name}`}
            >Remove</button
          >
        </li>
      {/each}
    </ul>
  {/if}
  {#each refused as why (why)}
    <p class="problem" role="alert">Not added — {why}</p>
  {/each}
  {#if error}
    <p class="problem" role="alert">{error}</p>
  {/if}
</div>

<style>
  .presets {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.9em;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  li {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.4rem;
  }

  .name {
    font-weight: 600;
  }

  .note,
  .hint {
    color: var(--text-dim);
    margin: 0;
  }

  .problem {
    color: var(--danger);
    margin: 0;
  }
</style>
