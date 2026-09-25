<script lang="ts">
  /**
   * §120: which AI the assistant uses, and the keys it needs.
   *
   * > I don't know what AI can be used right now, but I want to be able to use
   * > any OpenRouter AI, ChatGPT, Gemini Free and other models, Claude if
   * > feasible via some token or something else ... so there shall be a config
   * > as well to configure tokens/accounts ... and enable/select AI
   *
   * djmanzo already spoke to all of them — OpenRouter, a local model through
   * Ollama, Anthropic's Claude, OpenAI's ChatGPT models, Google's Gemini and
   * Groq — with keys kept in the system's keychain (`dj-secrets`), never in a
   * settings file. What the owner could not do was find it: it lived behind a
   * "Setup" button inside the assistant's own panel, and with the toolbars
   * off (§117) nothing on screen led there. So it is one component now, drawn
   * in that panel and in Settings, where a DJ looks for an account.
   *
   * What each provider needs, whether it is ready, and where to get a key are
   * Rust's (`dj_app::assistant::list_llm_providers`); this draws them.
   *
   * **A model belongs to the provider it was listed from.** Choosing one used
   * to set the provider to whichever was *first* ready, so with OpenRouter and
   * Google both keyed, a Gemini model went to OpenRouter under a name
   * OpenRouter does not have. The list now remembers whose it is.
   */
  import IconButton from "./controls/IconButton.svelte";
  import {
    assistantState,
    listLlmModels,
    listLlmProviders,
    openSignupLink,
    resetSpend,
    setAssistantModel,
    setSecret,
    setSpendCap,
    type AssistantState,
    type LlmModel,
    type LlmProvider,
  } from "./api";

  let {
    onchange,
  }: {
    /** Told whenever the choice or the spend changes, so a header can follow. */
    onchange?: (state: AssistantState) => void;
  } = $props();

  let providers = $state<LlmProvider[]>([]);
  let models = $state<LlmModel[]>([]);
  /** The provider whose models are listed, by id. */
  let modelsFor = $state<string | null>(null);
  let current = $state<AssistantState | null>(null);
  let modelsError = $state<string | null>(null);
  let loadingModels = $state<string | null>(null);
  let draft = $state<Record<string, string>>({});

  const label = (id: string) => providers.find((p) => p.id === id)?.label ?? id;

  function keep(next: AssistantState) {
    current = next;
    onchange?.(next);
  }

  async function refresh() {
    const [list, now] = await Promise.all([listLlmProviders(), assistantState()]);
    providers = list;
    keep(now);
  }

  $effect(() => {
    void refresh();
  });

  async function loadModels(provider: string) {
    loadingModels = provider;
    modelsError = null;
    try {
      const list = await listLlmModels(provider);
      // Free first: the whole point of listing them is that you can start
      // without paying, and burying the free ones defeats it.
      list.sort((a, b) => Number(b.free) - Number(a.free) || a.name.localeCompare(b.name));
      models = list;
      modelsFor = provider;
    } catch (e) {
      models = [];
      modelsFor = null;
      modelsError = String(e);
    } finally {
      loadingModels = null;
    }
  }

  async function choose(model: string) {
    if (!modelsFor) return;
    keep(await setAssistantModel(modelsFor, model));
  }

  async function saveKey(id: string) {
    const value = (draft[id] ?? "").trim();
    if (!value) return;
    await setSecret(id, value);
    draft[id] = "";
    await refresh();
  }
</script>

<div class="ai-setup">
  {#if current}
    <p class="in-use" data-ai-in-use>
      In use: <strong>{label(current.provider)}</strong>
      <span class="mono">{current.model}</span>
    </p>
  {/if}
  <p class="hint">
    A local model needs no key and no internet — the right default if you would
    rather not send your track list anywhere. Everything else needs a key from
    the provider; OpenRouter's free models, and Google's free Gemini tier, are the
    easiest way to try it properly. Claude needs a key from Anthropic's console:
    a Claude.ai subscription cannot be used by other applications.
  </p>

  {#each providers as provider (provider.id)}
    <div
      class="provider"
      class:recommended={provider.recommended}
      class:current={current?.provider === provider.id}
      data-provider={provider.id}
    >
      <div class="row">
        <span class="name">{provider.label}</span>
        {#if provider.recommended}<span class="badge">start here</span>{/if}
        {#if current?.provider === provider.id}<span class="badge using">in use</span>{/if}
        <span class="status {provider.status}">{provider.status_detail}</span>
        <button
          type="button"
          disabled={provider.status !== "ready" || loadingModels !== null}
          onclick={() => loadModels(provider.id)}
        >
          {loadingModels === provider.id ? "…" : "Models"}
        </button>
      </div>
      <p class="summary">{provider.summary}</p>

      {#if provider.credential}
        <div class="row">
          <input
            aria-label="{provider.label} — {provider.credential_label ?? 'key'}"
            type="password"
            autocomplete="off"
            placeholder={provider.is_set ? `Replace ${provider.hint}` : provider.credential_label}
            bind:value={draft[provider.credential]}
            onkeydown={(e) => e.key === "Enter" && saveKey(provider.credential!)}
          />
          <IconButton icon="fa-solid fa-floppy-disk" title="Save key" onClick={() => saveKey(provider.credential!)} />
          {#if provider.signup_url}
            <!--
              A button, not a link. `target="_blank"` inside a Tauri window
              opens nothing at all on Linux, so this looked like a link and
              behaved like dead text.
            -->
            <button type="button" class="signup" onclick={() => openSignupLink(provider.signup_url!)}>
              Get one →
            </button>
          {/if}
        </div>
        {#if provider.free_tier}
          <p class="free-tier">{provider.free_tier}</p>
        {/if}
      {/if}

      {#if modelsFor === provider.id && models.length > 0}
        <div class="models" role="group" aria-label="{provider.label} models">
          {#each models.slice(0, 40) as model (model.id)}
            <button
              type="button"
              class="model-pick"
              class:free={model.free}
              class:chosen={current?.provider === provider.id && current?.model === model.id}
              onclick={() => choose(model.id)}
              title={model.id}
            >
              {model.name}
              {#if model.free}<em>free</em>{/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {/each}

  {#if modelsError}
    <p class="error">{modelsError}</p>
  {/if}

  <div class="row cap">
    <label for="ai-cap">Spend cap</label>
    <input
      id="ai-cap"
      type="number"
      min="0"
      step="0.5"
      value={current && Number.isFinite(current.cap_usd) ? current.cap_usd : 2}
      onchange={async (e) => keep(await setSpendCap(Number(e.currentTarget.value)))}
    />
    <button type="button" onclick={async () => keep(await resetSpend())}>Reset spend</button>
  </div>
</div>

<style>
  .in-use {
    margin: 0 0 0.4rem;
    font-size: 0.82em;
  }

  .in-use .mono {
    margin-left: 0.3rem;
    color: var(--text-dim);
  }

  .provider {
    border-top: 1px solid var(--border);
    padding: 0.55rem 0 0.2rem;
  }

  .provider.recommended {
    border-top-color: color-mix(in srgb, var(--accent) 50%, var(--border));
  }

  .provider.current {
    border-top-color: var(--selected);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: 0.3rem;
  }

  .row input {
    flex: 1;
    min-width: 0;
  }

  .name {
    font-weight: 600;
    font-size: 0.9em;
  }

  .badge {
    font-size: 0.62em;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
    background: var(--accent);
    color: var(--on-accent);
  }

  .badge.using {
    background: var(--selected);
    color: var(--on-selected);
  }

  .status {
    font-size: 0.72em;
    color: var(--text-dim);
    flex: 1;
    min-width: 0;
  }

  .status.ready {
    color: var(--accent-2);
  }

  .status.needs_key {
    color: var(--warn);
  }

  .summary,
  .free-tier,
  .hint {
    margin: 0 0 0.3rem;
    font-size: 0.76em;
    line-height: 1.5;
    color: var(--text-dim);
  }

  /*
    Was an anchor until it turned out a webview anchor reaches nothing. It is a
    button now and still has to read as a link, because the DJ's understanding
    of it -- "this takes me somewhere" -- was the only correct part before.
  */
  .signup {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--accent);
    white-space: nowrap;
    cursor: pointer;
  }
  .signup:hover {
    text-decoration: underline;
  }

  .models {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin: 0.5rem 0;
  }

  .model-pick {
    font-size: 0.72em;
    padding: 0.2rem 0.45rem;
  }

  .model-pick.free em {
    font-style: normal;
    color: var(--accent-2);
    margin-left: 0.25rem;
  }

  .model-pick.chosen {
    border-color: var(--selected);
  }

  .cap {
    margin-top: 0.6rem;
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .cap input {
    width: 6rem;
    flex: none;
  }

  .error {
    color: var(--danger, var(--warn));
    font-size: 0.8em;
  }
</style>
