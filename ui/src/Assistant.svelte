<script lang="ts">
  /**
   * Talking to the assistant.
   *
   * Two things this panel is careful about, both from
   * `docs/adr/0005-assistant-speaks-only-actions.md`:
   *
   * 1. **It shows the actions.** Every answer lists exactly what was dispatched,
   *    in the same text a script or a controller mapping would use. Nothing the
   *    assistant does is hidden from you, and anything it did you can undo by
   *    hand.
   * 2. **It shows what was rejected.** A model producing plausible-but-invalid
   *    commands is a prompt problem, and hiding the evidence makes it
   *    unfixable.
   *
   * The `local` badge is worth watching: it means the answer cost nothing and
   * took no round trip, which is true for most of what gets typed here.
   */
  import IconButton from "./controls/IconButton.svelte";
  import {
    ask,
    assistantState,
    listLlmModels,
    listLlmProviders,
    resetSpend,
    setAssistantModel,
    setSecret,
    setSpendCap,
    type Answer,
    type AssistantState,
    type LlmModel,
    type LlmProvider,
  } from "./api";

  let { enabled }: { enabled: boolean } = $props();

  interface Entry {
    question: string;
    answer: Answer | null;
    error: string | null;
  }

  let history = $state<Entry[]>([]);
  let text = $state("");
  let busy = $state(false);
  let providers = $state<LlmProvider[]>([]);
  let models = $state<LlmModel[]>([]);
  let state_ = $state<AssistantState | null>(null);
  let showSetup = $state(false);
  let modelsError = $state<string | null>(null);
  let loadingModels = $state(false);
  let draft = $state<Record<string, string>>({});

  $effect(() => {
    void refresh();
  });

  async function refresh() {
    [providers, state_] = await Promise.all([listLlmProviders(), assistantState()]);
  }

  async function loadModels(provider: string) {
    loadingModels = true;
    modelsError = null;
    try {
      models = await listLlmModels(provider);
      // Free first: the whole point of listing them is that you can start
      // without paying, and burying the free ones defeats it.
      models.sort((a, b) => Number(b.free) - Number(a.free) || a.name.localeCompare(b.name));
    } catch (e) {
      models = [];
      modelsError = String(e);
    } finally {
      loadingModels = false;
    }
  }

  async function choose(provider: string, model: string) {
    state_ = await setAssistantModel(provider, model);
  }

  async function saveKey(id: string) {
    const value = (draft[id] ?? "").trim();
    if (!value) return;
    await setSecret(id, value);
    draft[id] = "";
    await refresh();
  }

  async function send() {
    const question = text.trim();
    if (!question || busy) return;
    text = "";
    busy = true;
    const entry: Entry = { question, answer: null, error: null };
    history = [...history, entry];

    try {
      const answer = await ask(question);
      entry.answer = answer;
    } catch (e) {
      entry.error = String(e);
    } finally {
      history = [...history];
      state_ = await assistantState();
      busy = false;
    }
  }

  const capReached = $derived(
    state_ !== null && Number.isFinite(state_.cap_usd) && state_.spent_usd >= state_.cap_usd,
  );
</script>

<section class="assistant">
  <header>
    <span class="who">Assistant</span>
    {#if state_}
      <span class="mono model" title="{state_.provider} / {state_.model}">
        {state_.model}
      </span>
      <span class="mono spend" class:over={capReached}>
        ${state_.spent_usd.toFixed(3)} / {Number.isFinite(state_.cap_usd)
          ? `$${state_.cap_usd.toFixed(2)}`
          : "∞"}
      </span>
      {#if state_.unpriced_calls > 0}
        <!-- $0.00 after fifty calls is ignorance, not thrift. Say which. -->
        <span class="unpriced" title="This provider does not report pricing">
          +{state_.unpriced_calls} unpriced
        </span>
      {/if}
    {/if}
    <button class="setup" onclick={() => (showSetup = !showSetup)}>
      {showSetup ? "Hide setup" : "Setup"}
    </button>
  </header>

  {#if showSetup}
    <div class="setup-panel">
      <p class="hint">
        A local model needs no key and no internet — the right default if you
        would rather not send your track list anywhere. Everything else needs a
        key, and OpenRouter's free models are the easiest way to try this
        properly.
      </p>

      {#each providers as provider (provider.id)}
        <div class="provider" class:recommended={provider.recommended}>
          <div class="row">
            <span class="name">{provider.label}</span>
            {#if provider.recommended}<span class="badge">start here</span>{/if}
            <span class="status {provider.status}">{provider.status_detail}</span>
            <button
              disabled={provider.status !== "ready"}
              onclick={() => loadModels(provider.id)}
            >
              {loadingModels ? "…" : "Models"}
            </button>
          </div>
          <p class="summary">{provider.summary}</p>

          {#if provider.credential}
            <div class="row">
              <input
                type="password"
                autocomplete="off"
                placeholder={provider.is_set ? `Replace ${provider.hint}` : provider.credential_label}
                bind:value={draft[provider.credential]}
                onkeydown={(e) => e.key === "Enter" && saveKey(provider.credential!)}
              />
              <IconButton icon="fa-solid fa-floppy-disk" title="Save key" onClick={() => saveKey(provider.credential!)} />
              {#if provider.signup_url}
                <a href={provider.signup_url} target="_blank" rel="noreferrer">Get one →</a>
              {/if}
            </div>
            {#if provider.free_tier}
              <p class="free-tier">{provider.free_tier}</p>
            {/if}
          {/if}
        </div>
      {/each}

      {#if modelsError}
        <p class="error">{modelsError}</p>
      {/if}

      {#if models.length > 0}
        <div class="models">
          {#each models.slice(0, 40) as model (model.id)}
            <button
              class="model-pick"
              class:free={model.free}
              class:chosen={state_?.model === model.id}
              onclick={() => choose(providers.find((p) => p.status === "ready")?.id ?? "local", model.id)}
              title={model.id}
            >
              {model.name}
              {#if model.free}<em>free</em>{/if}
            </button>
          {/each}
        </div>
      {/if}

      <div class="row cap">
        <label for="cap">Spend cap</label>
        <input
          id="cap"
          type="number"
          min="0"
          step="0.5"
          value={state_ && Number.isFinite(state_.cap_usd) ? state_.cap_usd : 2}
          onchange={async (e) => {
            state_ = await setSpendCap(Number(e.currentTarget.value));
          }}
        />
        <button onclick={async () => (state_ = await resetSpend())}>Reset spend</button>
      </div>
    </div>
  {/if}

  <div class="log">
    {#if history.length === 0}
      <p class="empty">
        Try <em>“play deck 2”</em>, <em>“kill the bass on deck 1”</em>, or
        <em>“pon el deck 2”</em>. Commands like those are understood on this
        machine — no key, no network, no cost.
      </p>
    {/if}

    {#each history as entry, i (i)}
      <div class="turn">
        <p class="question">{entry.question}</p>

        {#if entry.error}
          <p class="error">{entry.error}</p>
        {:else if entry.answer}
          <p class="reply">
            {entry.answer.reply}
            <span class="source {entry.answer.source}">
              {entry.answer.source === "local" ? "no model needed" : "model"}
            </span>
          </p>

          {#if entry.answer.actions.length > 0}
            <!-- Exactly what was dispatched, in the same text a script uses. -->
            <ul class="actions mono">
              {#each entry.answer.actions as action (action)}
                <li>{action}</li>
              {/each}
            </ul>
          {/if}

          {#if entry.answer.rejected.length > 0}
            <p class="rejected">
              Ignored {entry.answer.rejected.length}
              {entry.answer.rejected.length === 1 ? "line" : "lines"} that were not
              valid commands: <span class="mono">{entry.answer.rejected.join("; ")}</span>
            </p>
          {/if}
        {:else}
          <p class="reply pending">…</p>
        {/if}
      </div>
    {/each}
  </div>

  <div class="compose">
    <input
      type="text"
      placeholder={enabled ? "Ask for something…" : "Connect a device first"}
      bind:value={text}
      disabled={busy}
      onkeydown={(e) => e.key === "Enter" && send()}
    />
    <button class="primary" onclick={send} disabled={busy || !text.trim()}>
      {busy ? "…" : "Send"}
    </button>
  </div>
</section>

<style>
  .assistant {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    flex: 1;
    min-height: 0;
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
  }

  .who {
    font-weight: 600;
  }

  .model,
  .spend {
    font-size: 0.75em;
    color: var(--text-dim);
  }

  .spend.over {
    color: var(--danger);
  }

  .unpriced {
    font-size: 0.7em;
    color: var(--warn);
  }

  .setup {
    margin-left: auto;
    padding: 0.2rem 0.5rem;
    font-size: 0.75em;
  }

  .setup-panel {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.8rem;
    max-height: 45%;
    overflow: auto;
  }

  .provider {
    border-top: 1px solid var(--border);
    padding: 0.55rem 0 0.2rem;
  }

  .provider.recommended {
    border-top-color: color-mix(in srgb, var(--accent) 50%, var(--border));
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
    border-color: var(--accent);
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

  .log {
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.8rem;
  }

  .empty {
    margin: 0;
    font-size: 0.82em;
    line-height: 1.6;
    color: var(--text-dim);
  }

  .empty em {
    font-style: normal;
    color: var(--text);
  }

  .turn {
    margin-bottom: 0.8rem;
  }

  .question {
    margin: 0 0 0.2rem;
    font-size: 0.85em;
    color: var(--text);
  }

  .question::before {
    content: "› ";
    color: var(--accent);
  }

  .reply {
    margin: 0;
    font-size: 0.82em;
    color: var(--text-dim);
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }

  .reply.pending {
    opacity: 0.6;
  }

  .source {
    font-size: 0.85em;
    padding: 0.05rem 0.3rem;
    border-radius: 3px;
    border: 1px solid var(--border);
  }

  /* Free and instant. Worth a colour, because it is most of what happens. */
  .source.local {
    color: var(--accent-2);
    border-color: color-mix(in srgb, var(--accent-2) 45%, var(--border));
  }

  .actions {
    margin: 0.25rem 0 0;
    padding-left: 1rem;
    font-size: 0.76em;
    color: var(--accent);
    user-select: text;
    -webkit-user-select: text;
  }

  .rejected {
    margin: 0.25rem 0 0;
    font-size: 0.74em;
    color: var(--warn);
    line-height: 1.5;
  }

  .compose {
    display: flex;
    gap: 0.4rem;
  }

  .compose input {
    flex: 1;
    min-width: 0;
  }

  .error {
    margin: 0.25rem 0 0;
    font-size: 0.78em;
    color: var(--danger);
  }
</style>
