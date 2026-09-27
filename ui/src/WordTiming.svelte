<script lang="ts">
  /**
   * §122: the words of a record placed in time, for the singers' screen to
   * wipe word by word — by whisper.cpp, compiled into djmanzo.
   *
   * > replace it with the model download for the whisper.cpp version. let it
   * > be a dropdown button for different useful models ... and hint at
   * > download size/time as of network, also at the usefulness as of
   * > recognition/reliability and analysis time per song approx as per the
   * > current machine
   *
   * The models are downloaded on the DJ's choice, from a list that says what
   * each costs to fetch and to run here and how well it hears. Each run says
   * how long it took against the owner's minute and a half rather than
   * promising it. The rules are `dj_app::whispercpp` and `dj_app::wordtimes`;
   * this draws them.
   */
  import {
    wordTiming,
    wordTimingChoose,
    wordTimingDownload,
    wordTimingForgetWhisperx,
    wordTimingRemove,
    wordTimingRun,
    wordTimingSpeed,
    type DeckState,
    type WordModel,
    type WordTiming,
    type WordTimingReport,
  } from "./api";

  let { decks = [], deckCount = 2 }: { decks?: DeckState[]; deckCount?: number } = $props();

  let timing = $state<WordTiming | null>(null);
  let running = $state<number | null>(null);
  let report = $state<WordTimingReport | null>(null);
  let error = $state("");
  let picking = $state(false);
  let measuring = $state(false);
  /** Two letters, or empty to let Whisper listen for it. */
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

  // While a model downloads or a record is listened to, read how far every
  // second.
  $effect(() => {
    if (!timing?.progress.downloading && running === null) return;
    const timer = setInterval(() => void refresh(), 1000);
    return () => clearInterval(timer);
  });

  // The first time the list opens, measure the download speed, so each
  // model can say how long it would take to fetch.
  $effect(() => {
    if (!picking || measuring || !timing || timing.progress.speed !== null) return;
    measuring = true;
    wordTimingSpeed()
      .then((next) => (timing = next))
      .catch(() => {});
  });

  async function act(what: () => Promise<WordTiming>) {
    error = "";
    try {
      timing = await what();
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
      void refresh();
    }
  }

  const loaded = $derived(decks.slice(0, deckCount).filter((deck) => deck.loaded));
  const chosen = $derived(timing?.models.find((model) => model.chosen) ?? null);
  const downloading = $derived(timing?.models.find((model) => model.id === timing?.progress.downloading) ?? null);

  const seconds = (value: number) => `${value.toFixed(1)} s`;
  const megabytes = (bytes: number) =>
    bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${Math.round(bytes / 1e6)} MB`;
  /** A duration as a DJ says it: "about 40 s", "about 1 min 10 s". */
  function about(value: number): string {
    const total = Math.max(1, Math.round(value / 5) * 5);
    if (total < 60) return `about ${total} s`;
    const minutes = Math.floor(total / 60);
    const rest = total % 60;
    return rest === 0 ? `about ${minutes} min` : `about ${minutes} min ${rest} s`;
  }
  function fetchTime(model: WordModel): string {
    if (model.download_seconds !== null) return `${about(model.download_seconds)} to download here`;
    return measuring ? "measuring your connection…" : "download time not measured";
  }
</script>

<section class="timing" aria-labelledby="word-timing">
  <h3 id="word-timing">Words in time</h3>
  <p class="hint">
    Whisper places each word of a record in time, so the singers' screen can wipe the words as they are sung. It is
    part of djmanzo; what it needs is one model, downloaded once.
  </p>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  {#if timing?.cpu}
    <p class="error" role="alert">{timing.cpu}</p>
  {/if}

  {#if timing?.old_whisperx}
    <p class="old" role="note">
      An earlier djmanzo installed WhisperX here ({megabytes(timing.old_whisperx)}), which nothing uses now.
      <button type="button" class="quiet" onclick={() => void act(wordTimingForgetWhisperx)}>Delete it</button>
    </p>
  {/if}

  {#if timing}
    <details class="picker" bind:open={picking}>
      <summary>
        {#if chosen}
          Model: <strong>{chosen.name}</strong>
        {:else}
          Choose a model to download
        {/if}
      </summary>
      <ul aria-label="Models">
        {#each timing.models as model (model.id)}
          <li data-model={model.id} data-chosen={model.chosen} data-installed={model.installed}>
            <div class="head">
              <span class="name">{model.name}</span>
              {#if model.recommended}<span class="badge">recommended</span>{/if}
              <span class="size mono">{megabytes(model.bytes)}</span>
            </div>
            <p class="reliability">{model.reliability}</p>
            <p class="costs">
              <span class="fetch">{model.installed ? "downloaded" : fetchTime(model)}</span>
              ·
              <span class="analysis" data-over={model.song_seconds > timing.budget_seconds}>
                {about(model.song_seconds)} for a 4-minute song {model.song_measured ? "(measured here)" : "(estimated)"}
              </span>
            </p>
            <div class="actions">
              {#if model.chosen}
                <span class="in-use">In use</span>
              {:else if model.installed}
                <button type="button" onclick={() => void act(() => wordTimingChoose(model.id))}>Use {model.name}</button>
              {:else}
                <button
                  type="button"
                  disabled={timing.progress.downloading !== null}
                  onclick={() => void act(() => wordTimingDownload(model.id))}>Download {model.name}</button
                >
              {/if}
              {#if model.installed}
                <button type="button" class="quiet" onclick={() => void act(() => wordTimingRemove(model.id))}
                  >Remove</button
                >
              {/if}
            </div>
          </li>
        {/each}
      </ul>
      <p class="hint">Kept in {timing.folder}.</p>
    </details>

    {#if downloading}
      <div class="step" role="status">
        Downloading {downloading.name} — {megabytes(timing.progress.downloaded)} of {megabytes(
          timing.progress.download_total || downloading.bytes,
        )}
        <progress max={timing.progress.download_total || downloading.bytes} value={timing.progress.downloaded}
        ></progress>
      </div>
    {/if}
    {#if timing.progress.error && !error}
      <p class="error" role="alert">{timing.progress.error}</p>
    {/if}

    {#if chosen}
      <div class="run">
        <label
          >Language <input
            bind:value={language}
            maxlength="3"
            size="3"
            placeholder="auto"
            title="Two letters — en, es, de… — or empty to let Whisper listen for it"
          /></label
        >
        {#each loaded as deck (deck.number)}
          <button type="button" disabled={running !== null} onclick={() => void time(deck.number)}>
            {running === deck.number
              ? `Listening to deck ${deck.number}… ${timing.progress.percent}%`
              : `Time the words on deck ${deck.number}`}
          </button>
        {:else}
          <span class="hint">Load a record to time its words.</span>
        {/each}
      </div>
    {/if}
  {/if}

  {#if report && timing && running === null}
    <p class="report" role="status" data-within={report.within_budget}>
      {report.words} words {report.mode === "align" ? "placed" : "found and placed"} ({report.language}) by
      {timing.models.find((model) => model.id === report?.model)?.name ?? report.model} in
      {seconds(report.seconds)} for {Math.round(report.record_seconds)} s of record —
      {report.within_budget
        ? `within the ${timing.budget_seconds}-second limit`
        : `over the ${timing.budget_seconds}-second limit`}.
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
  .step,
  .old {
    color: var(--text-dim);
    font-size: 0.85em;
    margin: 0;
  }

  .error {
    color: var(--danger);
    margin: 0;
  }

  .picker {
    border: 1px solid var(--border);
    border-radius: var(--radius, 6px);
    background: var(--panel-raised);
  }

  .picker summary {
    cursor: pointer;
    padding: 0.35rem 0.6rem;
    font-size: 0.9em;
  }

  .picker ul {
    list-style: none;
    margin: 0;
    padding: 0 0.4rem 0.4rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .picker li {
    border: 1px solid var(--border);
    border-radius: var(--radius, 6px);
    padding: 0.35rem 0.5rem;
    background: var(--panel);
  }

  .picker li[data-chosen="true"] {
    border-color: var(--accent);
  }

  .picker .hint {
    padding: 0 0.6rem 0.4rem;
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
  }

  .name {
    font-weight: 600;
  }

  .badge {
    font-size: 0.75em;
    color: var(--accent);
    border: 1px solid var(--accent);
    border-radius: 999px;
    padding: 0 0.4rem;
  }

  .size {
    margin-left: auto;
    color: var(--text-dim);
    font-size: 0.85em;
  }

  .reliability,
  .costs {
    margin: 0.15rem 0 0;
    font-size: 0.82em;
    color: var(--text-dim);
  }

  .analysis[data-over="true"] {
    color: var(--warn);
  }

  .actions {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.3rem;
    align-items: center;
  }

  .in-use {
    font-size: 0.85em;
    color: var(--accent);
  }

  button.quiet {
    background: none;
    border: none;
    color: var(--text-dim);
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
    font: inherit;
  }

  progress {
    width: 100%;
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
