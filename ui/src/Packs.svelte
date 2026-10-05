<script lang="ts">
  /**
   * §122 step 7: packs — looks and the MilkDrop presets they draw, made by
   * someone and signed by them. A pack is read and checked whole by Rust
   * (`dj_app::packs`) before anything is installed, and its licence, author,
   * looks and key are shown first: the DJ installs it, or does not. See
   * `docs/VISUALS.md`, step 7.
   */
  import { open } from "@tauri-apps/plugin-dialog";
  import { packInspect, packInstall, packRemove, packsInstalled, type PackInspected, type PackInstalled } from "./api";

  let { onchange }: { onchange?: () => void } = $props();

  let packs = $state<PackInstalled[]>([]);
  /** A pack read and waiting for the DJ's yes or no. */
  let pending = $state<{ path: string; seen: PackInspected } | null>(null);
  let error = $state("");

  $effect(() => {
    void packsInstalled()
      .then((all) => (packs = all))
      .catch(() => {});
  });

  async function add() {
    error = "";
    try {
      const picked = await open({ multiple: false, filters: [{ name: "djmanzo packs", extensions: ["djpack"] }] });
      if (!picked || Array.isArray(picked)) return;
      pending = { path: picked, seen: await packInspect(picked) };
    } catch (e) {
      error = String(e);
    }
  }

  async function install() {
    if (!pending) return;
    error = "";
    try {
      packs = await packInstall(pending.path);
      pending = null;
      onchange?.();
    } catch (e) {
      error = String(e);
    }
  }

  async function remove(id: string) {
    error = "";
    try {
      packs = await packRemove(id);
      onchange?.();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="packs" data-packs>
  <div class="head">
    <span>Packs</span>
    <button onclick={() => void add()}>Add a pack…</button>
  </div>
  {#if pending}
    <section class="pending" aria-label="The pack to install" data-pack-pending>
      {#if pending.seen.problem}
        <p class="problem" role="alert">{pending.seen.name} cannot be installed: {pending.seen.problem}</p>
        <button onclick={() => (pending = null)}>Close</button>
      {:else}
        <p class="title">
          <strong>{pending.seen.name}</strong>
          {pending.seen.version} — by {pending.seen.author}
        </p>
        <p class="licence" data-pack-licence>Licence: <strong>{pending.seen.licence}</strong></p>
        {#if pending.seen.description}<p class="hint">{pending.seen.description}</p>{/if}
        <p class="hint">
          {pending.seen.looks.length === 1 ? "One look" : `${pending.seen.looks.length} looks`}:
          {pending.seen.looks.join(", ")}
        </p>
        <p class="hint" data-pack-key>
          Signed by the key <code>{pending.seen.key}</code>: compare it with the one its maker publishes.
        </p>
        {#if pending.seen.installed}
          <p class="hint">Replaces version {pending.seen.installed}, from the same key.</p>
        {/if}
        <div class="buttons">
          <button onclick={() => void install()}>Install</button>
          <button class="quiet" onclick={() => (pending = null)}>Cancel</button>
        </div>
      {/if}
    </section>
  {/if}
  {#if packs.length === 0 && !pending}
    <p class="hint">
      A pack is someone's looks and presets in one <code>.djpack</code> file, signed by its maker. djmanzo checks
      every file before installing it, and shows you its licence first.
    </p>
  {:else}
    <ul>
      {#each packs as pack (pack.id)}
        <li data-pack={pack.id}>
          <span class="name">{pack.name}</span>
          {#if pack.problem}
            <span class="problem" role="note">Not offered: {pack.problem}</span>
          {:else}
            <span class="note">{pack.version} · {pack.author} · {pack.licence} · {pack.looks} looks</span>
          {/if}
          <button class="quiet" onclick={() => void remove(pack.id)} aria-label={`Remove ${pack.name}`}>Remove</button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if error}
    <p class="problem" role="alert">{error}</p>
  {/if}
</div>

<style>
  .packs {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.9em;
  }

  .head,
  .buttons {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .pending {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    padding: 0.5rem;
    border: 1px solid var(--border, currentColor);
    border-radius: 0.4rem;
  }

  .pending p {
    margin: 0;
  }

  .licence {
    font-size: 1.05em;
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
    opacity: 0.75;
  }

  .problem {
    color: var(--danger, #e05a5a);
  }

  code {
    font-size: 0.95em;
  }
</style>
