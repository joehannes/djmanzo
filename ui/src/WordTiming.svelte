<script lang="ts">
  /**
   * §122: the words of a record placed in time by WhisperX, for the singers'
   * screen to wipe word by word.
   *
   * The owner chose WhisperX for its word timestamps. It is installed once,
   * on the DJ's say-so, into a folder of djmanzo's own; each run says how
   * long it took against the owner's fifteen seconds rather than promising
   * it. The rule is `dj_app::wordtimes`; this draws it.
   */
  import {
    wordTiming,
    wordTimingInstall,
    wordTimingRun,
    type DeckState,
    type WordTiming,
    type WordTimingReport,
  } from "./api";

  let { decks = [], deckCount = 2 }: { decks?: DeckState[]; deckCount?: number } = $props();

  let timing = $state<WordTiming | null>(null);
  let running = $state<number | null>(null);
  let report = $state<WordTimingReport | null>(null);
  let error = $state("");
  /** Two letters, or empty to let WhisperX listen for it. */
  let language = $state("");

  async function refresh() {
    try {
      timing = await wordTiming();
      report ??= timing.progress.last;
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    void refresh();
  });

  // While it installs, read the steps every two seconds.
  $effect(() => {
    if (!timing?.progress.installing) return;
    const timer = setInterval(() => void refresh(), 2000);
    return () => clearInterval(timer);
  });

  async function install() {
    error = "";
    try {
      timing = await wordTimingInstall();
    } catch (e) {
      error = String(e);
    }
  }

  async function time(deck: number) {
    error = "";
    running = deck;
    try {
      report = await wordTimingRun(deck, language.trim() || null);
    } catch (e) {
      error = String(e);
    } finally {
      running = null;
    }
  }

  const loaded = $derived(decks.slice(0, deckCount).filter((deck) => deck.loaded));
  const seconds = (value: number) => `${value.toFixed(1)} s`;
</script>

<section class="timing" aria-labelledby="word-timing">
  <h3 id="word-timing">Words in time</h3>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  {#if timing && !timing.installed}
    <p class="hint">
      WhisperX places each word of a record in time, so the singers' screen can wipe the words as they are
      sung. It is installed once, into a folder of djmanzo's own — Python, PyTorch and WhisperX, about two
      gigabytes.
    </p>
    {#if timing.progress.installing}
      <p class="step" role="status">{timing.progress.step ?? "Installing"}…</p>
    {:else}
      <button type="button" onclick={() => void install()}>Install WhisperX</button>
      {#if timing.progress.error}
        <p class="error" role="alert">{timing.progress.error}</p>
      {/if}
    {/if}
  {:else if timing}
    <div class="run">
      <label
        >Language <input
          bind:value={language}
          maxlength="3"
          size="3"
          placeholder="auto"
          title="Two letters — en, es, de… — or empty to let WhisperX listen for it"
        /></label
      >
      {#each loaded as deck (deck.number)}
        <button type="button" disabled={running !== null} onclick={() => void time(deck.number)}>
          {running === deck.number ? `Timing deck ${deck.number}…` : `Time the words on deck ${deck.number}`}
        </button>
      {:else}
        <span class="hint">Load a record to time its words.</span>
      {/each}
    </div>
  {/if}

  {#if report && timing}
    <p class="report" role="status" data-within={report.within_budget}>
      {report.words} words {report.mode === "align" ? "placed" : "found and placed"} ({report.language}) in
      {seconds(report.seconds)} for {Math.round(report.record_seconds)} s of record —
      {report.within_budget
        ? `within the ${timing.budget_seconds}-second budget`
        : `over the ${timing.budget_seconds}-second budget`}.
      {report.heard === "vocals"
        ? "Heard from the separated vocals."
        : "Heard from the whole mix: the vocals were not separated yet, so words under loud music may be placed less well."}
    </p>
    <ul class="stages" aria-label="How long each stage took">
      {#each report.stages as [stage, took] (stage)}
        <li><span>{stage}</span> <span class="mono">{seconds(took)}</span></li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .timing {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    border-top: 1px solid var(--border);
    padding-top: 0.6rem;
  }

  h3 {
    margin: 0;
    font-size: 1em;
  }

  .hint,
  .step {
    color: var(--text-dim);
    font-size: 0.85em;
    margin: 0;
  }

  .error {
    color: var(--danger);
    margin: 0;
  }

  .run {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
  }

  label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  input {
    font: inherit;
    background: var(--panel-raised);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.2rem 0.3rem;
    width: 3.5rem;
  }

  .report {
    margin: 0;
    font-size: 0.9em;
  }

  .report[data-within="false"] {
    color: var(--warn);
  }

  .stages {
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 0.2rem 0.8rem;
    margin: 0;
    padding: 0;
    font-size: 0.8em;
    color: var(--text-dim);
  }
</style>
