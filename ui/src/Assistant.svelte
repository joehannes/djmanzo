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
  import { onMount } from "svelte";
  import IconButton from "./controls/IconButton.svelte";
  import Conduct from "./Conduct.svelte";
  import {
    ask,
    assistantSight,
    assistantState,
    listLlmModels,
    listLlmProviders,
    openSignupLink,
    resetSpend,
    setAssistantModel,
    setSecret,
    setSpendCap,
    type Answer,
    type AssistantState,
    type LlmModel,
    type LlmProvider,
    type Sight,
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

  /**
   * §40: what the assistant is told about the night, and what it is not.
   *
   * Read from Rust rather than written here. The unseen half is the reason
   * this is on screen at all — an answer that ignored your history reads very
   * differently once you know it could not see it — and a hand-written copy of
   * that half is the one nobody would keep true.
   */
  let sight = $state<Sight[]>([]);
  let showSight = $state(false);
  const seen = $derived(sight.filter((item) => item.told));
  const unseen = $derived(sight.filter((item) => !item.told));

  $effect(() => {
    void refresh();
  });

  async function refresh() {
    [providers, state_] = await Promise.all([listLlmProviders(), assistantState()]);
  }

  onMount(async () => {
    // Once, on mount: §40's list is a table in Rust and does not change while
    // djmanzo is running. Asked here rather than in `refresh`, which runs on
    // every answer.
    try {
      sight = await assistantSight();
    } catch {
      // A panel that cannot say what the assistant sees says nothing, rather
      // than claiming it sees everything.
      sight = [];
    }
  });

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

  <!--
    How much it does, and taking it back. Above the conversation on purpose:
    what the assistant is *allowed* to do is a decision you make before you ask
    it anything, and the pair of takeover buttons should be the first thing your
    hand finds when this panel is open.
  -->
  <Conduct {enabled} />


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
                  A button, not a link. `target="_blank"` inside a Tauri
                  window opens nothing at all on Linux, so this looked
                  like a link and behaved like dead text.
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

  <!--
    `tabindex` and a name because this scrolls. §33 asks for keyboard operation,
    and a conversation that overflows and cannot be focused is one a keyboard
    cannot read the top of — axe calls it `scrollable-region-focusable`. It went
    unnoticed until §80's learned preferences made this panel tall enough to
    overflow in the audit's window: the defect was always here and the layout
    was hiding it.

    The `svelte-ignore` is two rules disagreeing rather than a shortcut.
    Svelte's `a11y_no_noninteractive_tabindex` is a good general heuristic and
    has no way to know this one scrolls; a focusable scroll container is the
    documented fix for the axe rule, and the `role` and name are what keep it
    from being a bare focusable div.
  -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="log" tabindex="0" role="region" aria-label="Assistant conversation">
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

  <!--
    §40: what it is looking at while it answers.

    Folded, and below the conversation rather than above it: this is what you
    open when an answer surprised you, not something to read before asking.
    Both halves, because the useful one is the second — "it could not see your
    history" is the single most informative thing about an answer that ignored
    your history.
  -->
  {#if sight.length > 0}
    <details class="sight" bind:open={showSight}>
      <summary>
        What it can see
        <span class="count">{seen.length} of {sight.length}</span>
      </summary>
      <ul class="seen">
        {#each seen as item (item.name)}
          <li title={item.about}>
            <span class="what">{item.name}</span>
            <span class="from">{item.source}</span>
          </li>
        {/each}
      </ul>
      {#if unseen.length > 0}
        <p class="blind-heading">What it cannot see</p>
        <ul class="blind">
          {#each unseen as item (item.name)}
            <li>
              <span class="what">{item.name}</span>
              <span class="why">{item.source}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </details>
  {/if}

  <div class="compose">
    <input
      aria-label="Ask the assistant"
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
  /* §40's two halves. The unseen one is not styled as an error: it is the
     honest shape of an assistant that runs on a laptop, and colouring it red
     would read as something being broken. */
  .sight {
    border-top: 1px solid var(--edge, rgba(255, 255, 255, 0.12));
    padding: 0.4rem 0.6rem;
    font-size: 0.78rem;
  }
  .sight > summary {
    cursor: pointer;
    color: var(--text-dim, rgba(255, 255, 255, 0.6));
  }
  .sight .count {
    margin-left: 0.4rem;
    opacity: 0.75;
  }
  .sight ul {
    list-style: none;
    margin: 0.4rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .sight li {
    display: flex;
    gap: 0.5rem;
    align-items: baseline;
  }
  .sight .what {
    flex: none;
    min-width: 9rem;
    color: var(--text, #e6e6e6);
  }
  .sight .from,
  .sight .why {
    color: var(--text-dim, rgba(255, 255, 255, 0.55));
  }
  .sight .blind-heading {
    margin: 0.6rem 0 0;
    color: var(--text-dim, rgba(255, 255, 255, 0.6));
  }
  .sight .blind li {
    align-items: flex-start;
  }

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
