<script lang="ts">
  /**
   * §119: what the room and the stream said, on the records they said it
   * about.
   *
   * > the song shall have comments and icons in places with the timestamp
   * > of messages where people comment heavily they like the moment of the
   * > music ... the user shall be able to save a (live) session ... insights
   * > into the reactions of the live crowd and the mood ... goals a DJ ...
   * > might want to define himself
   *
   * Everything read here is Rust's (`dj_app::crowd`): what a reaction is
   * about and which way it leans, the record and the part of it it was
   * placed on, the night read back, the goals answered. This draws it, and
   * keeps the DJ's hands on the controls that do not move: the stable ones
   * first, the lists that grow as the night does after them.
   */
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    crowdAdd,
    crowdImport,
    crowdSettingsSave,
    crowdView,
    formatTime,
    type CrowdAbout,
    type CrowdGoal,
    type CrowdMeasure,
    type CrowdPart,
    type CrowdPlaced,
    type CrowdView,
  } from "./api";

  let view = $state<CrowdView | null>(null);
  let session = $state<string | null>(null);
  let error = $state<string | null>(null);
  let note = $state("");
  let noteField = $state<HTMLInputElement | undefined>();
  let importing = $state(false);
  let began = $state("");
  let adding = $state(false);
  let goalName = $state("");
  let goalMeasure = $state<CrowdMeasure>("warmth");
  let goalTarget = $state("");
  let goalAtMost = $state(false);

  async function refresh() {
    try {
      view = await crowdView(session);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    void session;
    void refresh();
  });

  // The night being played is still being added to; one asked about
  // afterwards is not. Every ten seconds, and only while this is open.
  $effect(() => {
    if (!view?.current) return;
    const every = setInterval(refresh, 10_000);
    return () => clearInterval(every);
  });

  async function noteIt() {
    const text = note.trim();
    if (!text) return;
    try {
      view = await crowdAdd(text, null, "other");
      note = "";
      error = null;
      // Back to the field, whether it was sent with Enter or the button: a
      // DJ who presses *Note it* and types the next one must be typing into
      // this, not into the booth, where letters are shortcuts -- driving it,
      // the letters after the press paused a deck and opened the palette.
      noteField?.focus();
    } catch (e) {
      error = String(e);
    }
  }

  /** "HH:MM" on the night's own date, as unix seconds. */
  function startOf(clock: string): number | null {
    const m = /^(\d{1,2}):(\d{2})$/.exec(clock.trim());
    if (!m || Number(m[1]) > 23 || Number(m[2]) > 59) return null;
    const base = view?.played[0] ? new Date(view.played[0].at * 1000) : new Date();
    base.setHours(Number(m[1]), Number(m[2]), 0, 0);
    return Math.round(base.getTime() / 1000);
  }

  function openImport() {
    importing = !importing;
    if (importing && !began && view?.played[0]) {
      const first = new Date(view.played[0].at * 1000);
      began = `${String(first.getHours()).padStart(2, "0")}:${String(first.getMinutes()).padStart(2, "0")}`;
    }
  }

  async function bringIn() {
    const start = startOf(began);
    if (start === null) {
      error = "When the stream began reads hours:minutes: 21:00.";
      return;
    }
    try {
      const path = await open({ multiple: false, filters: [{ name: "Chat logs", extensions: ["txt", "log", "csv"] }] });
      if (typeof path !== "string") return;
      view = await crowdImport(view?.session ?? null, path, start, "youtube");
      importing = false;
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function saveSettings(goals: CrowdGoal[], delay: number) {
    try {
      view = await crowdSettingsSave(view?.session ?? null, { delay, goals });
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  function addGoal() {
    if (!view) return;
    const target = Number(goalTarget);
    const measure = goalMeasure;
    const name = goalName.trim() || view.measures.find((m) => m.measure === measure)?.title || "";
    if (!Number.isFinite(target) || goalTarget.trim() === "") {
      error = "A goal needs a number.";
      return;
    }
    // Warmth is a share: 80 means 80%.
    const value = measure === "warmth" && target > 1 ? target / 100 : target;
    void saveSettings([...view.goals.map((a) => a.goal), { name, measure, target: value, at_most: goalAtMost }], view.delay);
    adding = false;
    goalName = "";
    goalTarget = "";
    goalAtMost = false;
  }

  function removeGoal(at: number) {
    if (!view) return;
    void saveSettings(
      view.goals.map((a) => a.goal).filter((_, i) => i !== at),
      view.delay,
    );
  }

  /** A past night by when it began: its id carries the moment, in seconds. */
  function nightOf(session: string): string {
    const m = /^session-(\d+)$/.exec(session);
    if (!m) return session;
    return new Date(Number(m[1]) * 1000).toLocaleString(undefined, {
      weekday: "short",
      day: "numeric",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  const percent = (x: number | null) => (x === null ? "—" : `${Math.round(x * 100)}%`);
  const shown = (measure: CrowdMeasure, x: number | null) =>
    x === null ? "—" : measure === "warmth" ? percent(x) : String(Math.round(x * 10) / 10);

  const PART: Record<CrowdPart, string> = {
    drop: "the drop",
    breakdown: "the breakdown",
    voice: "the voice coming in",
    record: "",
  };

  /** Where a reaction landed, in words. */
  function where(p: CrowdPlaced): string {
    if (p.record === null || !view) return "before the first record";
    const record = view.played[p.record];
    const title = record?.title || "a record";
    if (p.part !== "record" && p.part_at !== null) return `${title} · ${PART[p.part]} at ${formatTime(p.part_at)}`;
    return `${title} · ${formatTime(p.into)} in`;
  }

  /** Each subject's picture: a flame, a question, a raised hand, a wave, a dot. */
  const ICON: Record<CrowdAbout, string> = {
    moment: "M12 3c1 4 5 5 5 10a5 5 0 0 1-10 0c0-3 2-4 2-6 2 1 3 3 3 3s0-4 0-7z",
    track: "M9 9a3 3 0 1 1 4 3c-1 .5-1 1-1 2M12 18v.5",
    request: "M8 13V6a1.5 1.5 0 0 1 3 0v5M11 11V5a1.5 1.5 0 0 1 3 0v6M14 11V6.5a1.5 1.5 0 0 1 3 0V14c0 4-2.5 7-6 7-2.5 0-4-1.5-5-3l-2-4a1.5 1.5 0 0 1 2.5-1.5L8 14",
    greeting: "M5 12c2-4 6-7 9-7M7 15c3-5 8-8 12-7M9 18c3-3 7-5 10-5",
    other: "M12 11.5a.5.5 0 1 0 0 1a.5.5 0 1 0 0-1z",
  };

  const ABOUT: Record<CrowdAbout, string> = {
    moment: "The moment",
    track: "The record",
    request: "A request",
    greeting: "A hello",
    other: "Something else",
  };

  const feed = $derived((view?.placed ?? []).slice(-40).reverse());
  const tallest = $derived(Math.max(1, ...(view?.summary.pace ?? [0])));
</script>

<div class="crowd">
  {#if view}
    <div class="controls">
      <label>Night
        <select
          value={view.session}
          onchange={(e) => (session = e.currentTarget.value)}
          aria-label="Which night"
        >
          {#each view.sessions as s, i (s)}
            <option value={s}>{i === 0 ? "This night" : nightOf(s)}</option>
          {/each}
        </select>
      </label>
      <label>Stream delay
        <input
          type="number"
          min="0"
          max="120"
          value={view.delay}
          onchange={(e) => view && void saveSettings(view.goals.map((a) => a.goal), Number(e.currentTarget.value))}
          aria-label="Stream delay in seconds"
        /> s
      </label>
    </div>

    {#if view.current}
      <form
        class="note"
        onsubmit={(e) => {
          e.preventDefault();
          void noteIt();
        }}
      >
        <input
          type="text"
          bind:this={noteField}
          bind:value={note}
          placeholder="Note a reaction: a cheer, a shout, 🔥"
          aria-label="Note a reaction"
        />
        <button type="submit" disabled={!note.trim()}>Note it</button>
      </form>
    {/if}
    <div class="bring">
      <button type="button" onclick={openImport} aria-expanded={importing}>Bring in a chat log</button>
      {#if importing}
        <label>It began at
          <input type="text" bind:value={began} placeholder="HH:MM" aria-label="When the stream began" class="clock" />
        </label>
        <button type="button" class="primary" onclick={() => void bringIn()}>Choose the file</button>
      {/if}
    </div>

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    <div class="tiles" data-summary>
      <div class="tile"><strong data-count="reactions">{view.summary.reactions}</strong><span>reactions</span></div>
      <div class="tile"><strong data-count="busiest">{view.summary.busiest_minute}</strong><span>busiest minute</span></div>
      <div class="tile"><strong data-count="warmth">{percent(view.summary.warmth)}</strong><span>warm</span></div>
      <div class="tile"><strong data-count="moments">{view.summary.moments}</strong><span>moments</span></div>
      <div class="tile"><strong data-count="asked">{view.summary.asked}</strong><span>asked what it was</span></div>
      <div class="tile"><strong data-count="requests">{view.summary.requests}</strong><span>requests</span></div>
    </div>

    {#if view.summary.pace.length > 0}
      <svg class="pace" viewBox="0 0 {view.summary.pace.length} 10" preserveAspectRatio="none" role="img" aria-label="Reactions per minute">
        {#each view.summary.pace as n, i (i)}
          <rect x={i + 0.1} width="0.8" y={10 - (n / tallest) * 10} height={(n / tallest) * 10} />
        {/each}
      </svg>
    {/if}

    <h3>Goals</h3>
    <ul class="goals">
      {#each view.goals as answered, i (i)}
        <li data-goal={answered.goal.measure} data-met={answered.met === null ? "unknown" : String(answered.met)}>
          <span class="mark" aria-hidden="true">{answered.met === null ? "·" : answered.met ? "✓" : "✗"}</span>
          <span class="name">{answered.goal.name}</span>
          <span class="value">
            {shown(answered.goal.measure, answered.value)} · goal {answered.goal.at_most ? "≤" : "≥"}
            {shown(answered.goal.measure, answered.goal.target)}
          </span>
          <button type="button" class="remove" aria-label="Remove the goal {answered.goal.name}" onclick={() => removeGoal(i)}>×</button>
        </li>
      {/each}
    </ul>
    {#if adding}
      <div class="goal-form">
        <select bind:value={goalMeasure} aria-label="What the goal measures">
          {#each view.measures as m (m.measure)}
            <option value={m.measure}>{m.title}</option>
          {/each}
        </select>
        <label class="atmost"><input type="checkbox" bind:checked={goalAtMost} /> at most</label>
        <input type="text" inputmode="decimal" bind:value={goalTarget} placeholder={goalMeasure === "warmth" ? "80 (%)" : "10"} aria-label="The number" class="target" />
        <input type="text" bind:value={goalName} placeholder="In your words (optional)" aria-label="The goal in your words" />
        <button type="button" class="primary" onclick={addGoal}>Add</button>
      </div>
    {:else}
      <button type="button" onclick={() => (adding = true)}>+ A goal of your own</button>
    {/if}

    {#if view.summary.records.length > 0}
      <h3>Records</h3>
      <ul class="records">
        {#each view.summary.records as r (r.track_id)}
          <li data-record={r.track_id}>
            <div class="line">
              <strong>{r.title}</strong>
              <span class="artist">{r.artist}</span>
              <span class="counts">{r.reactions} · <span class="up">{r.up} warm</span>{#if r.down > 0} · <span class="down">{r.down} cold</span>{/if}{#if r.asked > 0} · {r.asked} asked{/if}</span>
            </div>
            {#if r.moments.length > 0}
              <div class="moments">
                {#each r.moments.slice(0, 4) as m (m.part + m.at)}
                  <span class="moment" data-part={m.part}>{m.part === "record" ? `${formatTime(m.at)} in` : `${PART[m.part]} ${formatTime(m.at)}`} ×{m.count}</span>
                {/each}
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}

    <h3>What was said</h3>
    {#if feed.length === 0}
      <p class="hint">Nothing yet. Requests from the room arrive here by themselves; a stream's chat can be brought in as a log.</p>
    {:else}
      <ul class="feed">
        {#each feed as p, i (i)}
          <li data-about={p.about} data-lean={p.lean} data-source={p.reaction.source}>
            <svg viewBox="0 0 24 24" aria-label={ABOUT[p.about]} role="img"><path d={ICON[p.about]} /></svg>
            <div class="said">
              <span class="text">{p.reaction.text}</span>
              <span class="where">{p.reaction.who ? `${p.reaction.who} · ` : ""}{where(p)}</span>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  {:else if error}
    <p class="error">{error}</p>
  {/if}
</div>

<style>
  .crowd {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.5rem;
    overflow: auto;
    min-height: 0;
    height: 100%;
  }

  .controls,
  .note,
  .bring,
  .goal-form {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
  }

  .controls label,
  .bring label {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .controls input[type="number"] {
    width: 4rem;
  }

  .note input {
    flex: 1 1 10rem;
    min-width: 0;
  }

  .clock {
    width: 4.5rem;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.35rem;
  }

  .tile {
    display: flex;
    flex-direction: column;
    padding: 0.35rem 0.45rem;
    border: 1px solid var(--border);
    border-radius: 0.45rem;
    min-width: 0;
  }

  .tile strong {
    font-size: 1.3em;
  }

  .tile span {
    font-size: 0.75em;
    color: var(--text-dim);
  }

  .pace {
    width: 100%;
    height: 2.5rem;
    fill: var(--active);
  }

  h3 {
    margin: 0.4rem 0 0;
    font-size: 0.95em;
  }

  .goals,
  .records,
  .feed {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .goals li {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .goals .mark {
    width: 1rem;
    text-align: center;
    font-weight: 700;
  }

  .goals li[data-met="true"] .mark {
    color: var(--success, var(--active));
  }

  .goals li[data-met="false"] .mark {
    color: var(--warn);
  }

  .goals .name {
    flex: 1 1 auto;
    min-width: 0;
  }

  .goals .value {
    font-family: var(--mono);
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .remove {
    padding: 0 0.35rem;
  }

  .goal-form select,
  .goal-form input:not([type="checkbox"]) {
    min-width: 0;
    flex: 1 1 8rem;
  }

  .goal-form .target {
    flex: 0 1 5rem;
  }

  .atmost {
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .records .line {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.35rem;
  }

  .artist,
  .counts,
  .where,
  .hint {
    color: var(--text-dim);
    font-size: 0.8em;
  }

  .up {
    color: var(--success, var(--active));
  }

  .down {
    color: var(--warn);
  }

  .moments {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin-top: 0.15rem;
  }

  .moment {
    font-size: 0.75em;
    padding: 0.05rem 0.4rem;
    border-radius: 999px;
    border: 1px solid var(--accent-warm, var(--active));
  }

  .feed li {
    display: flex;
    align-items: flex-start;
    gap: 0.45rem;
  }

  .feed svg {
    flex: none;
    width: 1.3rem;
    height: 1.3rem;
    fill: none;
    stroke: var(--text-dim);
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .feed li[data-about="moment"] svg {
    stroke: var(--accent-warm, var(--active));
  }

  .feed li[data-lean="up"] svg {
    fill: none;
  }

  .feed li[data-lean="down"] .text {
    color: var(--warn);
  }

  .said {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .text {
    overflow-wrap: anywhere;
  }

  .error {
    margin: 0;
    color: var(--warn);
  }
</style>
