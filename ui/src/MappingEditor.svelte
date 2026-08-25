<script lang="ts">
  import { onDestroy } from "svelte";
  import IconButton from "./controls/IconButton.svelte";
  import {
    mappingBind,
    mappingDraft,
    mappingDraftFrom,
    mappingLearn,
    mappingRename,
    mappingSave,
    mappingUnbind,
    type MappingDraft,
    type Role,
  } from "./api";

  let { mappings = [] }: { mappings?: { name: string }[] } = $props();

  let draft = $state<MappingDraft>({
    name: "My mapping",
    device: "",
    bindings: [],
    learning: false,
    learned: null,
  });
  let status = $state("");
  let action = $state("deck 1 play_pause");
  let release = $state("");
  let kind = $state<Role["kind"]>("latching");
  let min = $state("");
  let max = $state("");

  /**
   * How often to ask what was touched while learning.
   *
   * A press has to appear to be instant, and a poll is far cheaper than
   * pushing every MIDI message across the bridge — most of which, during a
   * mapping session, are the DJ brushing something on the way to the control
   * they meant. Polling only runs while the panel is learning.
   */
  const POLL_MS = 120;
  let poll: ReturnType<typeof setInterval> | null = null;

  function stopPolling() {
    if (poll !== null) {
      clearInterval(poll);
      poll = null;
    }
  }

  onDestroy(() => {
    stopPolling();
    // Leaving the panel must not leave the controller inert.
    if (draft.learning) void mappingLearn(false);
  });

  async function toggleLearn() {
    draft = await mappingLearn(!draft.learning);
    stopPolling();
    if (draft.learning) {
      poll = setInterval(async () => {
        draft = await mappingDraft();
      }, POLL_MS);
    }
  }

  function role(): Role {
    const bottom = min === "" ? undefined : Number(min);
    const top = max === "" ? undefined : Number(max);
    switch (kind) {
      case "momentary":
        return { kind, press: action, release };
      case "continuous":
        return { kind, action, min: bottom, max: top };
      case "encoder":
        return { kind, up: action, down: release, encoding: "signed" };
      default:
        return { kind: "latching", press: action };
    }
  }

  async function bind() {
    if (!draft.learned) return;
    try {
      draft = await mappingBind(draft.learned, role());
      status = "";
    } catch (error) {
      // The engine checks the action when it is bound, so this is a real
      // message about a real typo rather than a generic failure.
      status = String(error);
    }
  }

  async function save() {
    try {
      status = `Saved to ${await mappingSave()}`;
    } catch (error) {
      status = String(error);
    }
  }
</script>

<section class="editor">
  <header>
    <h2>Mapping editor</h2>
    <p class="blurb">
      Press a control on your controller, say what it should do, and save. The
      file lands beside your presets and can be edited by hand afterwards.
    </p>
  </header>

  <div class="row">
    <label>
      Name
      <input
        value={draft.name}
        onchange={async (e) =>
          (draft = await mappingRename(e.currentTarget.value, draft.device))}
      />
    </label>
    <label>
      Device
      <input
        value={draft.device}
        placeholder="part of the port name"
        onchange={async (e) =>
          (draft = await mappingRename(draft.name, e.currentTarget.value))}
      />
    </label>
    <label>
      Start from
      <select
        onchange={async (e) =>
          (draft = await mappingDraftFrom(e.currentTarget.value || null))}
      >
        <option value="">Nothing</option>
        {#each mappings as mapping (mapping.name)}
          <option value={mapping.name}>{mapping.name}</option>
        {/each}
      </select>
    </label>
  </div>

  <div class="row learn">
    <button class="listen" class:on={draft.learning} onclick={toggleLearn}>
      {draft.learning ? "Listening — press a control" : "Learn a control"}
    </button>
    <code class="seen">{draft.learned ?? "—"}</code>
  </div>

  <div class="row">
    <label>
      Behaves like
      <select bind:value={kind}>
        <option value="latching">Button — stays done</option>
        <option value="momentary">Button — hold</option>
        <option value="continuous">Fader or knob</option>
        <option value="encoder">Endless encoder</option>
      </select>
    </label>
    <label class="wide">
      {kind === "encoder" ? "Turning up" : "Action"}
      <input bind:value={action} spellcheck="false" />
    </label>
    {#if kind === "momentary" || kind === "encoder"}
      <label class="wide">
        {kind === "encoder" ? "Turning down" : "On release"}
        <input bind:value={release} spellcheck="false" />
      </label>
    {/if}
    {#if kind === "continuous"}
      <label class="narrow">Min<input bind:value={min} placeholder="0" /></label>
      <label class="narrow">Max<input bind:value={max} placeholder="1" /></label>
    {/if}
  </div>

  <div class="row">
    <button disabled={!draft.learned} onclick={bind}>
      Bind {draft.learned ?? "a control"}
    </button>
    <button disabled={draft.bindings.length === 0} onclick={save}>
      Save mapping
    </button>
  </div>

  {#if status}
    <p class="status" role="status">{status}</p>
  {/if}

  <ul class="bindings">
    {#each draft.bindings as binding (binding.on)}
      <li>
        <code>{binding.on}</code>
        <span class="does">{binding.does}</span>
        <IconButton
          icon="fa-solid fa-xmark"
          title="Unbind {binding.on}"
          onClick={async () => (draft = await mappingUnbind(binding.on))}
        />
      </li>
    {:else}
      <li class="empty">Nothing bound yet.</li>
    {/each}
  </ul>
</section>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    padding: 0.8rem;
  }
  h2 {
    margin: 0;
    font-size: 0.95rem;
  }
  .blurb {
    margin: 0.2rem 0 0;
    font-size: 0.78rem;
    color: var(--text-dim, rgba(255, 255, 255, 0.6));
    max-width: 46ch;
  }
  .row {
    display: flex;
    gap: 0.6rem;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.72rem;
    color: var(--text-dim, rgba(255, 255, 255, 0.6));
  }
  label.wide {
    flex: 1 1 16rem;
  }
  label.narrow input {
    width: 4.5rem;
  }
  input,
  select {
    font: inherit;
    font-size: 0.8rem;
    padding: 0.25rem 0.4rem;
    border-radius: var(--radius, 6px);
    border: 1px solid var(--edge, rgba(255, 255, 255, 0.12));
    background: var(--panel, #101210);
    color: var(--text, #e6e6e6);
  }
  button {
    font-size: 0.8rem;
    padding: 0.35rem 0.8rem;
    border-radius: var(--radius, 6px);
    border: 1px solid var(--edge, rgba(255, 255, 255, 0.12));
    background: var(--panel-raised, #1a1d1a);
    color: var(--text, #e6e6e6);
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .listen.on {
    background: var(--accent, #4ade80);
    color: var(--on-accent, #06120a);
    border-color: var(--accent-2, #22c55e);
  }
  .seen {
    font-size: 0.85rem;
    padding: 0.3rem 0.5rem;
    border-radius: var(--radius, 6px);
    background: var(--panel, #101210);
    min-width: 9rem;
  }
  .status {
    margin: 0;
    font-size: 0.78rem;
    color: var(--text-dim, rgba(255, 255, 255, 0.7));
  }
  .bindings {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    max-height: 16rem;
    overflow-y: auto;
  }
  .bindings li {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-size: 0.78rem;
  }
  .bindings code {
    min-width: 9rem;
    color: var(--accent, #4ade80);
  }
  .does {
    flex: 1;
    color: var(--text-dim, rgba(255, 255, 255, 0.7));
  }
  .empty {
    color: var(--text-dim, rgba(255, 255, 255, 0.5));
  }
</style>
