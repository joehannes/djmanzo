<script lang="ts">
  /**
   * §122 step 7: packs — looks and the MilkDrop presets they draw, made by
   * someone and signed by them. A pack is read and checked whole by Rust
   * (`dj_app::packs`) before anything is installed, and its licence, author,
   * looks and key are shown first: the DJ installs it, or does not. See
   * `docs/VISUALS.md`, step 7.
   *
   * Step 7b: the DJ makes a pack of their own presets, signed with their own
   * key — made the first time and kept beside the settings — once they have
   * said the presets are theirs to share under the licence they name.
   *
   * Step 7c: *Browse packs* reads the DJ's indexes — only when pressed — and
   * lists their packs against what is installed; *Get…* downloads one and
   * shows it here as *Add a pack…* does, licence first, refused when it is
   * not the pack the index lists.
   */
  import { open, save } from "@tauri-apps/plugin-dialog";
  import {
    milkdropPresets,
    packGet,
    packIndexAdd,
    packIndexRemove,
    packIndexes,
    packInspect,
    packInstall,
    packKey,
    packMake,
    packRemove,
    packsBrowse,
    packsInstalled,
    type PacksBrowsed,
    type PackInspected,
    type PackInstalled,
    type PackKey,
    type PackMade,
  } from "./api";

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
      const name = pending.seen.name;
      browsing?.read?.packs.forEach((pack) => {
        if (pack.name === name) pack.state = "installed";
      });
      pending = null;
      onchange?.();
    } catch (e) {
      error = String(e);
    }
  }

  /** The browser, while it is open: the DJ's indexes and what they list. */
  let browsing = $state<{ urls: string[]; read: PacksBrowsed | null; reading: boolean; adding: string } | null>(null);

  async function startBrowsing() {
    error = "";
    try {
      browsing = { urls: await packIndexes(), read: null, reading: false, adding: "" };
      await readIndexes();
    } catch (e) {
      error = String(e);
    }
  }

  async function readIndexes() {
    if (!browsing) return;
    browsing.reading = true;
    try {
      browsing.read = await packsBrowse();
    } catch (e) {
      error = String(e);
    } finally {
      if (browsing) browsing.reading = false;
    }
  }

  async function addIndex() {
    if (!browsing) return;
    error = "";
    try {
      browsing.urls = await packIndexAdd(browsing.adding);
      browsing.adding = "";
      await readIndexes();
    } catch (e) {
      error = String(e);
    }
  }

  async function removeIndex(url: string) {
    if (!browsing) return;
    error = "";
    try {
      browsing.urls = await packIndexRemove(url);
      await readIndexes();
    } catch (e) {
      error = String(e);
    }
  }

  async function get(index: string, name: string) {
    error = "";
    try {
      const got = await packGet(index, name);
      pending = { path: got.path, seen: got.seen };
    } catch (e) {
      error = String(e);
    }
  }

  const STATES: Record<string, string> = {
    installed: "Installed",
    update: "An update to yours",
    other_key: "Yours is signed by another key",
  };

  /** Licences a DJ is likely to share presets under; any other can be typed. */
  const LICENCES = ["CC BY 4.0", "CC BY-SA 4.0", "CC BY-NC 4.0", "CC0 1.0", "MIT"];

  /** The maker form, while it is open. */
  let making = $state<{
    key: PackKey;
    presets: string[];
    name: string;
    author: string;
    licence: string;
    version: string;
    description: string;
    chosen: string[];
    theirs: boolean;
  } | null>(null);
  let made = $state<PackMade | null>(null);

  const ready = $derived(
    making !== null &&
      [making.name, making.author, making.licence, making.version].every((field) => field.trim() !== "") &&
      making.chosen.length > 0 &&
      making.theirs,
  );

  async function startMaking() {
    error = "";
    made = null;
    try {
      const [key, yours] = await Promise.all([packKey(), milkdropPresets()]);
      making = {
        key,
        presets: yours.filter((preset) => !preset.problem).map((preset) => preset.name),
        name: "",
        author: "",
        licence: "",
        version: "1.0.0",
        description: "",
        chosen: [],
        theirs: false,
      };
    } catch (e) {
      error = String(e);
    }
  }

  function toggle(name: string, on: boolean) {
    if (!making) return;
    making.chosen = on ? [...making.chosen, name] : making.chosen.filter((chosen) => chosen !== name);
  }

  async function make() {
    if (!making || !ready) return;
    error = "";
    try {
      const path = await save({
        defaultPath: `${making.name.trim()}.djpack`,
        filters: [{ name: "djmanzo packs", extensions: ["djpack"] }],
      });
      if (!path) return;
      made = await packMake(
        {
          name: making.name,
          author: making.author,
          licence: making.licence,
          version: making.version,
          description: making.description,
          presets: making.presets.filter((name) => making?.chosen.includes(name)),
          theirs: making.theirs,
        },
        path,
      );
      making = null;
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
    <button class="quiet" onclick={() => void startMaking()} disabled={making !== null}>
      Make a pack of your presets…
    </button>
    <button class="quiet" onclick={() => (browsing ? (browsing = null) : void startBrowsing())}>
      {browsing ? "Close the browser" : "Browse packs…"}
    </button>
  </div>
  {#if browsing}
    <section class="pending" aria-label="Packs from your indexes" data-pack-browser>
      <p class="hint">
        Read from these indexes when you open this, and not otherwise. Each pack is checked against its maker's
        signature and against the key its index names before you see its licence.
      </p>
      <ul data-pack-indexes>
        {#each browsing.urls as url (url)}
          {@const seen = browsing.read?.indexes.find((index) => index.url === url)}
          <li data-pack-index={url}>
            <span class="name">{seen?.name || url}</span>
            {#if seen?.problem}
              <span class="problem" role="note">Not read: {seen.problem}</span>
            {:else if seen}
              <span class="note"
                >{seen.packs === 1 ? "one pack" : `${seen.packs} packs`}{seen.left_out
                  ? ` · ${seen.left_out} left out, not whole`
                  : ""}</span
              >
            {/if}
            <button class="quiet" onclick={() => void removeIndex(url)} aria-label={`Stop reading ${url}`}
              >Remove</button
            >
          </li>
        {/each}
      </ul>
      <div class="buttons">
        <input
          bind:value={browsing.adding}
          aria-label="Another index's address"
          onkeydown={(event) => event.key === "Enter" && void addIndex()}
        />
        <button onclick={() => void addIndex()} disabled={browsing.adding.trim() === ""}>Add an index</button>
      </div>
      {#if browsing.reading}
        <p class="hint" role="status">Reading the indexes…</p>
      {:else if browsing.read && browsing.read.packs.length === 0}
        <p class="hint">These indexes list no packs yet.</p>
      {:else if browsing.read}
        <ul data-pack-listed>
          {#each browsing.read.packs as pack (pack.name)}
            <li data-pack-offer={pack.name}>
              <span class="name">{pack.name}</span>
              <span class="note">{pack.version} · {pack.author} · {pack.licence}</span>
              {#if STATES[pack.state]}<span class="note" data-pack-state>{STATES[pack.state]}</span>{/if}
              {#if pack.state !== "installed" && pack.state !== "other_key"}
                <button
                  class="quiet"
                  onclick={() => void get(pack.index, pack.name)}
                  aria-label={`Get ${pack.name}`}>Get…</button
                >
              {/if}
              {#if pack.description}<span class="hint description">{pack.description}</span>{/if}
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}
  {#if making}
    <section class="pending" aria-label="Make a pack of your presets" data-pack-maker>
      {#if making.presets.length === 0}
        <p class="hint">
          You have no presets of your own to put in a pack yet: add some under <em>Your MilkDrop presets</em> first.
        </p>
        <button onclick={() => (making = null)}>Close</button>
      {:else}
        <label>Its name <input bind:value={making.name} /></label>
        <label>Made by <input bind:value={making.author} /></label>
        <label>
          Licence
          <input bind:value={making.licence} list="pack-licences" />
          <datalist id="pack-licences">
            {#each LICENCES as licence (licence)}<option value={licence}></option>{/each}
          </datalist>
        </label>
        <label>Version <input bind:value={making.version} /></label>
        <label>What it is <textarea bind:value={making.description} rows="2" maxlength="600"></textarea></label>
        <fieldset>
          <legend>Your presets in it — one look each</legend>
          {#each making.presets as name (name)}
            <label class="check">
              <input
                type="checkbox"
                checked={making.chosen.includes(name)}
                onchange={(event) => toggle(name, event.currentTarget.checked)}
              />
              {name}
            </label>
          {/each}
        </fieldset>
        <label class="check" data-pack-theirs>
          <input type="checkbox" bind:checked={making.theirs} />
          These presets are mine to share under this licence: I made them, or their own licence lets me pass them on
          under it.
        </label>
        <p class="hint" data-pack-own-key>
          Signed with your key <code>{making.key.fingerprint}</code>, kept at <code>{making.key.file}</code>. Keep a
          copy: a new version of this pack is taken as an update only when it is signed with the same key. Give the
          fingerprint with the pack, so those who install it can check it.
        </p>
        <div class="buttons">
          <button onclick={() => void make()} disabled={!ready}>Make…</button>
          <button class="quiet" onclick={() => (making = null)}>Cancel</button>
        </div>
      {/if}
    </section>
  {/if}
  {#if made}
    <p class="made" role="status" data-pack-made>
      Made <strong>{made.name}</strong>
      {made.version} — {made.looks === 1 ? "one look" : `${made.looks} looks`}, signed by
      <code>{made.key}</code> — at <code>{made.path}</code>.
    </p>
  {/if}
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
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
  }

  .head button {
    white-space: nowrap;
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

  .pending label {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .pending label.check {
    flex-direction: row;
    align-items: baseline;
    gap: 0.4rem;
  }

  fieldset {
    margin: 0;
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--border, currentColor);
    border-radius: 0.3rem;
  }

  .made {
    margin: 0;
  }

  .description {
    flex-basis: 100%;
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
    overflow-wrap: anywhere;
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
