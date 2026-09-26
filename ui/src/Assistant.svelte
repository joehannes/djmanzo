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
  import Conduct from "./Conduct.svelte";
  import AiSetup from "./AiSetup.svelte";
  import {
    ask,
    assistantSight,
    assistantState,
    type Answer,
    type AssistantState,
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
  let state_ = $state<AssistantState | null>(null);
  let showSetup = $state(false);

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
    state_ = await assistantState();
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
    <!-- §120: the same setup Settings shows, in one component. -->
    <div class="setup-panel">
      <AiSetup onchange={(next) => (state_ = next)} />
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

  /*
    §124: "also the basic controls / top toolbar/search" stay in view while a
    view scrolls. In a short panel this whole section scrolls in the panel's
    body, and the model, the spend and Setup went up with it; the box to ask
    in went down. Both hold to their edge of the panel now, over whatever is
    scrolling between them.
  */
  header {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    position: sticky;
    top: 0;
    z-index: 2;
    padding-block: 0.25rem;
    background: var(--panel);
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
    position: sticky;
    bottom: 0;
    z-index: 2;
    padding-block: 0.3rem;
    background: var(--panel);
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
