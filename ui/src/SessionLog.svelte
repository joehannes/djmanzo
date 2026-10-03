<script lang="ts">
  /**
   * The session log: every action in order with its timestamp, saved as a
   * set and two takes compared (ADR-0003). Its own component so that it can
   * be a window of its own (§121) as well as a surface in the main window —
   * it needs nothing from either but its three commands. It asks for the log
   * again every two seconds while it is shown: a log on a screen of its own,
   * beside the set being played, that stopped at the moment it was opened
   * would be a record of nothing.
   */
  import { sessionDiff, sessionLog, sessionSave, type DivergenceLine, type SessionSummary } from "./api";

  const EVERY_MS = 2_000;

  let log = $state<string[]>([]);
  let savePath = $state("");
  let saved = $state<SessionSummary | null>(null);
  let diffA = $state("");
  let diffB = $state("");
  let divergence = $state<DivergenceLine[] | null>(null);
  let logError = $state<string | null>(null);

  $effect(() => {
    const read = () => {
      sessionLog()
        .then((lines) => (log = lines))
        .catch(() => {});
    };
    read();
    const timer = setInterval(read, EVERY_MS);
    return () => clearInterval(timer);
  });
</script>

  <!--
    `tabindex` and a name because this scrolls. §33 asks for keyboard
    operation, and a region that overflows and cannot be focused is one a
    keyboard cannot reach the bottom of — axe calls it
    `scrollable-region-focusable` and it is right. It went unnoticed until a
    taller Assistant panel pushed this one into overflowing: the defect was
    always here, and the layout was hiding it.

    The `svelte-ignore` is the two rules disagreeing rather than a shortcut.
    Svelte's `a11y_no_noninteractive_tabindex` is a good general heuristic —
    do not make static things focusable — and it has no way to know this one
    scrolls. A focusable scroll container is the documented fix for the axe
    rule, and the `role` and name are what keep it from being a bare
    focusable div: a screen reader announces a named region rather than
    landing somewhere with nothing to say.
  -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="log" tabindex="0" role="region" aria-label="Session log">
    <p class="hint">
      Every action, in order, with its timestamp. This log is what makes a set
      replayable — see ADR-0003.
    </p>
    <pre class="mono">{log.length ? log.join("\n") : "(nothing yet)"}</pre>

    <!--
      Saving and comparing. The path is typed rather than picked from a
      dialog: this panel is a developer and practice tool, and a file picker
      here would be more ceremony than the thing is worth.
    -->
    <div class="log-tools">
      <label>
        Save as
        <input
          type="text"
          bind:value={savePath}
          placeholder="/home/you/sets/friday.djset"
          spellcheck="false"
        />
      </label>
      <button
        disabled={!savePath || log.length === 0}
        onclick={async () => {
          try {
            saved = await sessionSave(savePath);
            logError = null;
          } catch (e) {
            logError = String(e);
          }
        }}>Save</button
      >
    </div>

    {#if saved}
      <p class="hint">
        {saved.events} events over {saved.seconds.toFixed(0)}s, {saved.tracks}
        {saved.tracks === 1 ? "track" : "tracks"} → <span class="mono">{saved.path}</span>
      </p>
    {/if}

    <div class="log-tools">
      <label>
        Compare
        <input type="text" bind:value={diffA} placeholder="take one" spellcheck="false" />
      </label>
      <label>
        against
        <input type="text" bind:value={diffB} placeholder="take two" spellcheck="false" />
      </label>
      <button
        disabled={!diffA || !diffB}
        onclick={async () => {
          try {
            divergence = await sessionDiff(diffA, diffB);
            logError = null;
          } catch (e) {
            logError = String(e);
            divergence = null;
          }
        }}>Diff</button
      >
    </div>

    {#if logError}
      <p class="hint error">{logError}</p>
    {/if}

    {#if divergence}
      {#if divergence.length === 0}
        <p class="hint">The two takes are the same set, move for move.</p>
      {:else}
        <ul class="divergence">
          {#each divergence as line, i (line.kind + line.event + i)}
            <li class={line.kind}>
              <span class="mono">{line.event}</span>
              <span class="delta">
                {#if line.kind === "drift"}
                  {line.seconds > 0 ? "+" : ""}{line.seconds.toFixed(2)}s
                {:else if line.kind === "only_in_first"}
                  only in the first
                {:else}
                  only in the second
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  </div>

<style>
  .hint {
    margin: 0 0 0.6rem;
    color: var(--text-dim);
    font-size: 0.8em;
  }

  .error {
    color: var(--danger);
  }

  .log {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.9rem;
    overflow: auto;
    flex: 1;
    min-height: 0;
  }

  .log pre {
    margin: 0;
    font-size: 0.8em;
    white-space: pre-wrap;
    user-select: text;
    -webkit-user-select: text;
  }
  .log-tools {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-top: 0.4rem;
  }

  .log-tools label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.8rem;
  }

  .log-tools input {
    min-width: 14rem;
    font-family: inherit;
    font-size: 0.8rem;
  }

  .divergence {
    list-style: none;
    margin: 0.4rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    font-size: 0.8rem;
  }

  .divergence li {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.1rem 0.3rem;
    border-left: 3px solid transparent;
  }

  /* Drift is the ordinary case and reads as neutral; a move present in only
     one take is the thing worth spotting. */
  .divergence li.drift {
    border-left-color: var(--accent, rgba(128, 128, 128, 0.5));
  }

  .divergence li.only_in_first,
  .divergence li.only_in_second {
    border-left-color: var(--warn, #d97706);
  }

  .delta {
    opacity: 0.75;
    white-space: nowrap;
  }
</style>
