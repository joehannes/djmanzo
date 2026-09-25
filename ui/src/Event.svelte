<script lang="ts">
  /**
   * §118: preparing one night, step by step, and keeping it for the night.
   *
   * > make a workflow and an activity to specifically prepare a set/show for
   * > an event ... the topic, the genre, the location, the music, the songs,
   * > the transitions, the duration, the fallbacks, the extras, the wishes,
   * > the flexibility, the alternatives ... the emergency cases ... the
   * > specials, the after bonus ... and store every step in a decent way ...
   * > and save his preparation/work to load it for the live event
   *
   * Everything this panel knows about an event is `dj_app::gig`'s: what a
   * step still lacks, which ideas a kind of night is offered, the path from
   * learning the moves to playing the night, the running order. It draws the
   * answer and sends edits back; it decides none of it, so the list of what
   * is missing can never disagree with the rules that make it.
   *
   * **Every edit is kept as it is made.** A preparation is written over an
   * evening, a week, a month; a panel that needed a Save press is one that
   * loses the rain plan the night the laptop went to sleep. Edits are sent a
   * moment after typing stops, and the answer replaces what was drawn only if
   * nothing was typed while it was on its way.
   */
  import { onMount } from "svelte";
  import {
    eventOptions,
    eventView,
    forgetEvent,
    listEvents,
    listPlaylists,
    newEvent,
    saveEvent,
    takeEventIdea,
    type Gig,
    type GigAdds,
    type GigIdea,
    type GigOptions,
    type GigStep,
    type GigSummary,
    type GigTrouble,
    type GigView,
    type Playlist,
  } from "./api";

  let events = $state<GigSummary[]>([]);
  let options = $state<GigOptions | null>(null);
  let playlists = $state<Playlist[]>([]);
  let view = $state<GigView | null>(null);
  /** What the DJ is editing: the view's event, ahead of what was last saved. */
  let draft = $state<Gig | null>(null);
  let error = $state<string | null>(null);

  let naming = $state(false);
  let newTitle = $state("");
  let newDate = $state("");
  let forgetting = $state(false);

  /** Edits since the last answer, so a slow answer does not undo typing. */
  let edits = 0;
  let pending: ReturnType<typeof setTimeout> | null = null;
  let saving: Promise<void> | null = null;

  async function refreshList() {
    try {
      events = await listEvents();
    } catch (e) {
      error = String(e);
    }
  }

  async function open(id: string) {
    await flush();
    try {
      show(await eventView(id));
      forgetting = false;
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  function show(next: GigView) {
    view = next;
    draft = structuredClone(next.gig);
  }

  onMount(() => {
    void (async () => {
      try {
        [options, events] = await Promise.all([eventOptions(), listEvents()]);
        playlists = (await listPlaylists().catch(() => [])).filter((p) => p.kind !== "folder");
        // The soonest first, which is the one being prepared.
        if (events.length > 0) await open(events[0].id);
      } catch (e) {
        error = String(e);
      }
    })();
    return () => {
      void flush();
    };
  });

  /**
   * Dates and times as text, in the form Rust keeps them.
   *
   * Native date and time fields were the first version, and WebKitGTK draws
   * an empty one as today's date and a time of day -- so an event with no
   * date looked dated, while its own step said the date was still to do.
   * Text says exactly what is there. An edit is held while one of them is
   * half typed, rather than sent to be refused on every keystroke.
   */
  const DATE = /^\d{4}-\d{2}-\d{2}$/;
  const TIME = /^\d{2}:\d{2}$/;
  const dateOk = (text: string) => text === "" || DATE.test(text);
  const timeOk = (text: string) => text === "" || TIME.test(text);

  /** Why an edit is being held, or nothing when it can be kept. */
  const held = $derived.by(() => {
    if (!draft) return null;
    if (!dateOk(draft.date)) return "The date reads year-month-day: 2026-10-03.";
    if (!timeOk(draft.starts)) return "Times read hours:minutes: 21:00.";
    if (draft.moments.some((m) => !timeOk(m.at))) return "Times read hours:minutes: 21:00.";
    return null;
  });

  /** Keep an edit a moment after typing stops. */
  function edited() {
    edits += 1;
    if (pending) clearTimeout(pending);
    pending = setTimeout(() => {
      pending = null;
      if (held) return;
      saving = send();
    }, 400);
  }

  async function send() {
    if (!draft) return;
    const sent = edits;
    try {
      const answer = await saveEvent($state.snapshot(draft) as Gig);
      error = null;
      view = answer;
      // Replaced only when nothing was typed meanwhile: the answer is the
      // event as it was when it left, and what was typed since is newer.
      if (edits === sent) draft = structuredClone(answer.gig);
      void refreshList();
    } catch (e) {
      error = String(e);
    }
  }

  /** Send what is waiting, now: before switching events, taking an idea, leaving. */
  async function flush() {
    if (pending) {
      clearTimeout(pending);
      pending = null;
      saving = send();
    }
    if (saving) await saving;
  }

  async function create() {
    const title = newTitle.trim();
    if (!title) return;
    await flush();
    try {
      show(await newEvent(title, newDate));
      naming = false;
      newTitle = "";
      newDate = "";
      error = null;
      await refreshList();
    } catch (e) {
      error = String(e);
    }
  }

  async function take(adds: GigAdds) {
    if (!draft) return;
    await flush();
    try {
      show(await takeEventIdea(draft.id, adds));
      await refreshList();
    } catch (e) {
      error = String(e);
    }
  }

  async function forget() {
    if (!draft) return;
    try {
      events = await forgetEvent(draft.id);
      view = null;
      draft = null;
      forgetting = false;
      if (events.length > 0) await open(events[0].id);
    } catch (e) {
      error = String(e);
    }
  }

  const ideasFor = (step: GigStep): GigIdea[] => view?.ideas.filter((i) => i.step === step) ?? [];

  /** A list kept one per line in a text box. */
  const lines = (list: string[]) => list.join("\n");
  const fromLines = (text: string) =>
    text
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0);

  function planFor(trouble: GigTrouble): string {
    return draft?.fallbacks.find((f) => f.trouble === trouble)?.plan ?? "";
  }

  function setPlan(trouble: GigTrouble, plan: string) {
    if (!draft) return;
    const slot = draft.fallbacks.find((f) => f.trouble === trouble);
    if (slot) slot.plan = plan;
    else draft.fallbacks.push({ trouble, plan });
    edited();
  }

  function toggleIn(list: string[], name: string) {
    const at = list.indexOf(name);
    if (at >= 0) list.splice(at, 1);
    else list.push(name);
    edited();
  }

  function when(summary: GigSummary): string {
    return [summary.date, summary.starts].filter(Boolean).join(" ") || "no date yet";
  }

  /**
   * The name field takes the keyboard as it appears. Without it the first
   * letters typed went to the page -- where letters and Space are djmanzo's
   * shortcuts -- and the name field stayed empty. Found by driving it.
   */
  function focusNow(node: HTMLElement) {
    node.focus();
  }

  const hours = $derived(draft ? Math.floor(draft.minutes / 60) : 0);
  const minutesPart = $derived(draft ? draft.minutes % 60 : 0);
</script>

<section class="event" data-event={draft?.id ?? ""}>
  <!--
    Which event, and a new one. A picker rather than a list: a DJ has one
    night in front of them at a time, and the rest are a menu away.
  -->
  <div class="head">
    {#if events.length > 0}
      <select
        aria-label="Event"
        value={draft?.id ?? ""}
        onchange={(e) => void open(e.currentTarget.value)}
      >
        {#each events as summary (summary.id)}
          <option value={summary.id}>
            {summary.title} · {when(summary)}{summary.lacking > 0 ? ` · ${summary.lacking} to prepare` : " · ready"}
          </option>
        {/each}
      </select>
    {/if}
    {#if naming}
      <input
        use:focusNow
        class="new-title"
        aria-label="Name of the new event"
        placeholder="Anna and Ben's wedding"
        bind:value={newTitle}
        onkeydown={(e) => {
          if (e.key === "Enter" && dateOk(newDate)) void create();
          if (e.key === "Escape") naming = false;
        }}
      />
      <input
        class="new-date"
        inputmode="numeric"
        placeholder="2026-10-03"
        aria-label="Date of the new event"
        class:wrong={!dateOk(newDate)}
        bind:value={newDate}
      />
      <button type="button" class="primary" disabled={!newTitle.trim() || !dateOk(newDate)} onclick={() => void create()}>
        Start
      </button>
      <button type="button" onclick={() => (naming = false)}>Cancel</button>
    {:else}
      <button type="button" class="primary" onclick={() => (naming = true)}>New event</button>
    {/if}
  </div>

  {#if held}
    <p class="held" role="status">{held} Kept once it reads right.</p>
  {:else if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  {#if !draft || !view}
    <p class="empty">
      Prepare a night here, a step at a time: the event, the music, the running order,
      what to do if it goes wrong, and what comes after. Each step says what it still
      lacks, a kind of night brings its own ideas, and everything is kept as you type — so
      the night itself starts exactly as you planned it.
    </p>
  {:else}
    <!--
      The owner's workflow for this night: learn the moves, practise them,
      build the set, prepare the event, play it. Each stop is done by what is
      stored, never by a tick -- Rust's `gig::path`.
    -->
    <ol class="path" aria-label="The path to the night">
      {#each view.path as stop (stop.stop)}
        <li class:done={stop.done} data-stop={stop.stop} title={stop.about}>
          <span class="mark" aria-hidden="true">{stop.done ? "✓" : "·"}</span>
          <span class="stop-title">{stop.title}</span>
          <span class="stop-about">{stop.about}</span>
        </li>
      {/each}
    </ol>

    {#each view.steps as stepState (stepState.step)}
      <details class="step" data-step={stepState.step} open={stepState.missing.length > 0 || stepState.step === "event"}>
        <summary>
          <span class="step-title">{stepState.title}</span>
          {#if stepState.missing.length > 0}
            <span class="lacks" data-lacks={stepState.missing.length}>{stepState.missing.length} to go</span>
          {:else if !stepState.optional}
            <span class="ready">ready</span>
          {/if}
        </summary>
        <p class="about">{stepState.about}</p>
        {#if stepState.missing.length > 0}
          <p class="missing">Still to do: {stepState.missing.join(", ")}.</p>
        {/if}

        {#if stepState.step === "event"}
          <div class="grid">
            <label class="wide">Name
              <input bind:value={draft.title} oninput={edited} />
            </label>
            <label>Date
              <input
                inputmode="numeric"
                placeholder="2026-10-03"
                class:wrong={!dateOk(draft.date)}
                bind:value={draft.date}
                oninput={edited}
              />
            </label>
            <label>You start
              <input
                inputmode="numeric"
                placeholder="21:00"
                class:wrong={!timeOk(draft.starts)}
                bind:value={draft.starts}
                oninput={edited}
              />
            </label>
            <label>You play
              <span class="duration">
                <input
                  type="number"
                  min="0"
                  max="24"
                  aria-label="Hours you play"
                  value={hours}
                  oninput={(e) => {
                    draft!.minutes = Math.max(0, Number(e.currentTarget.value) || 0) * 60 + minutesPart;
                    edited();
                  }}
                />h
                <input
                  type="number"
                  min="0"
                  max="59"
                  step="15"
                  aria-label="Minutes you play"
                  value={minutesPart}
                  oninput={(e) => {
                    draft!.minutes = hours * 60 + Math.min(59, Math.max(0, Number(e.currentTarget.value) || 0));
                    edited();
                  }}
                />m
              </span>
            </label>
            <label>Kind of night
              <select
                value={draft.setting ?? ""}
                onchange={(e) => {
                  draft!.setting = e.currentTarget.value || null;
                  edited();
                }}
              >
                <option value="">Choose…</option>
                {#each options?.nights ?? [] as night (night.slug)}
                  <option value={night.slug}>{night.title}</option>
                {/each}
              </select>
            </label>
            <label class="wide">Where
              <input bind:value={draft.place} oninput={edited} placeholder="The venue, the town" />
            </label>
            <div class="wide choice" role="group" aria-label="Under the sky">
              {#each options?.skies ?? [] as sky (sky.slug)}
                <button
                  type="button"
                  class:on={draft.sky === sky.slug}
                  aria-pressed={draft.sky === sky.slug}
                  onclick={() => {
                    draft!.sky = sky.slug as Gig["sky"];
                    edited();
                  }}>{sky.title}</button
                >
              {/each}
            </div>
            <label class="wide">Who is coming, and what for
              <textarea rows="2" bind:value={draft.crowd} oninput={edited}></textarea>
            </label>
          </div>
        {:else if stepState.step === "music"}
          <div class="grid">
            <label class="wide">What the set is about
              <input bind:value={draft.topic} oninput={edited} placeholder="Songs they met to; a summer of disco" />
            </label>
            <div class="wide">
              <span class="field">Genres</span>
              <div class="chips">
                {#each draft.genres as name (name)}
                  <button type="button" class="chip on" onclick={() => toggleIn(draft!.genres, name)} title="Take {name} off">{name} ×</button>
                {/each}
                <select
                  aria-label="Add a genre"
                  value=""
                  onchange={(e) => {
                    const name = e.currentTarget.value;
                    if (name && !draft!.genres.includes(name)) toggleIn(draft!.genres, name);
                    e.currentTarget.value = "";
                  }}
                >
                  <option value="">+ genre</option>
                  {#each (options?.genres ?? []).filter((g) => !draft!.genres.includes(g)) as name (name)}
                    <option value={name}>{name}</option>
                  {/each}
                </select>
              </div>
            </div>
            <div class="wide">
              <span class="field">Not tonight</span>
              <div class="chips">
                {#each draft.avoid as name (name)}
                  <button type="button" class="chip avoid" onclick={() => toggleIn(draft!.avoid, name)} title="Allow {name} again">{name} ×</button>
                {/each}
                <select
                  aria-label="Add a genre not to play"
                  value=""
                  onchange={(e) => {
                    const name = e.currentTarget.value;
                    if (name && !draft!.avoid.includes(name)) toggleIn(draft!.avoid, name);
                    e.currentTarget.value = "";
                  }}
                >
                  <option value="">+ genre to avoid</option>
                  {#each (options?.genres ?? []).filter((g) => !draft!.avoid.includes(g) && !draft!.genres.includes(g)) as name (name)}
                    <option value={name}>{name}</option>
                  {/each}
                </select>
              </div>
            </div>
            <label>Asked for, one a line
              <textarea
                rows="3"
                aria-label="Records asked for"
                value={lines(draft.wishes)}
                oninput={(e) => {
                  draft!.wishes = fromLines(e.currentTarget.value);
                  edited();
                }}
              ></textarea>
            </label>
            <label>Never, whatever is asked
              <textarea
                rows="3"
                aria-label="Records never to play"
                value={lines(draft.never)}
                oninput={(e) => {
                  draft!.never = fromLines(e.currentTarget.value);
                  edited();
                }}
              ></textarea>
            </label>
            <div class="wide choice" role="group" aria-label="How closely the night keeps to the plan">
              {#each options?.leeways ?? [] as leeway (leeway.slug)}
                <button
                  type="button"
                  class:on={draft.leeway === leeway.slug}
                  aria-pressed={draft.leeway === leeway.slug}
                  onclick={() => {
                    draft!.leeway = leeway.slug as Gig["leeway"];
                    edited();
                  }}>{leeway.title}</button
                >
              {/each}
            </div>
          </div>
        {:else if stepState.step === "order"}
          <!-- The moments with a time: the owner's specials. -->
          <div class="moments">
            {#each draft.moments as moment, index (index)}
              <div class="moment" data-moment={index}>
                <input
                  inputmode="numeric"
                  placeholder="21:30"
                  aria-label="When: {moment.what || 'moment'}"
                  class:wrong={!timeOk(moment.at)}
                  bind:value={moment.at}
                  oninput={edited}
                />
                <input aria-label="What happens" bind:value={moment.what} oninput={edited} placeholder="First dance" />
                <input aria-label="The record for it" bind:value={moment.record} oninput={edited} placeholder="The record" />
                <button
                  type="button"
                  class="drop"
                  title="Take this moment off"
                  aria-label="Take {moment.what || 'this moment'} off"
                  onclick={() => {
                    draft!.moments.splice(index, 1);
                    edited();
                  }}>×</button
                >
              </div>
            {/each}
            <button
              type="button"
              onclick={() => {
                draft!.moments.push({ at: "", what: "", record: "" });
                edited();
              }}>+ moment</button
            >
          </div>

          {#if view.timeline.length > 0}
            <ol class="timeline" aria-label="Running order">
              {#each view.timeline as mark, index (index)}
                <li class:outside={mark.outside} title={mark.outside ? "Outside the hours you play" : `${mark.after} minutes in`}>
                  <span class="mono">{mark.at || "--:--"}</span> {mark.what}
                </li>
              {/each}
            </ol>
          {/if}

          <!-- The moves the night needs: the owner's transitions. -->
          <div class="moves">
            <span class="field">Moves for this night</span>
            {#each view.moves as move (move.name)}
              <div class="move" data-move={move.name}>
                <span class="move-name" title="{move.what} — {move.when}">{move.name}</span>
                <button
                  type="button"
                  class="rehearse"
                  class:on={move.rehearsed}
                  aria-pressed={move.rehearsed}
                  title={move.rehearsed ? "Rehearsed for this night" : "Mark it rehearsed, once you have"}
                  onclick={() => toggleIn(draft!.rehearsed, move.name)}
                >{move.rehearsed ? "rehearsed ✓" : "rehearse"}</button>
                <button type="button" class="drop" aria-label="Take {move.name} off" onclick={() => toggleIn(draft!.techniques, move.name)}>×</button>
              </div>
            {/each}
            <select
              aria-label="Add a move"
              value=""
              onchange={(e) => {
                const name = e.currentTarget.value;
                if (name && !draft!.techniques.includes(name)) toggleIn(draft!.techniques, name);
                e.currentTarget.value = "";
              }}
            >
              <option value="">+ move</option>
              {#each (options?.moves ?? []).filter((m) => !draft!.techniques.includes(m.name)) as move (move.name)}
                <option value={move.name} title={move.what}>{move.name}</option>
              {/each}
            </select>
          </div>

          <!-- The set: the playlist the plan panel saved, by reference. -->
          <label class="set">The set
            <select
              aria-label="The set for this night"
              value={draft.setlist?.playlist ?? ""}
              onchange={(e) => {
                const id = Number(e.currentTarget.value);
                const chosen = playlists.find((p) => p.id === id);
                draft!.setlist = chosen ? { playlist: chosen.id, name: chosen.name } : null;
                edited();
              }}
            >
              <option value="">Not built yet — build it in the plan panel</option>
              {#each playlists as list (list.id)}
                <option value={list.id}>{list.name} ({list.track_count})</option>
              {/each}
            </select>
          </label>
        {:else if stepState.step === "trouble"}
          <!-- The owner's fallbacks, alternatives and emergencies. -->
          <div class="troubles">
            {#each view.troubles as row (row.trouble)}
              <label class="trouble" data-trouble={row.trouble}>
                <span class="field">{row.title}</span>
                <textarea
                  rows="2"
                  placeholder={row.usual}
                  value={planFor(row.trouble)}
                  oninput={(e) => setPlan(row.trouble, e.currentTarget.value)}
                ></textarea>
              </label>
            {/each}
          </div>
        {:else if stepState.step === "extras"}
          <div class="grid">
            <label class="wide">Extras
              <textarea rows="2" bind:value={draft.extras} oninput={edited}></textarea>
            </label>
            <label class="wide">After the set
              <textarea rows="2" bind:value={draft.after} oninput={edited}></textarea>
            </label>
            <label class="wide">Who to call
              <textarea rows="2" bind:value={draft.contacts} oninput={edited} placeholder="The host, the venue, the sound engineer"></textarea>
            </label>
            <label class="wide">Notes
              <textarea rows="2" bind:value={draft.notes} oninput={edited}></textarea>
            </label>
          </div>
        {/if}

        <!--
          Ideas for this step, from written rules about this kind of night --
          labelled as suggestions, because a DJ who knows the couple knows
          better. One press each, and gone once taken.
        -->
        {#if ideasFor(stepState.step).length > 0}
          <div class="ideas" role="group" aria-label="Ideas for {stepState.title}">
            <span class="field">Ideas</span>
            {#each ideasFor(stepState.step) as idea, index (index)}
              <button type="button" class="idea" title={idea.because} onclick={() => void take(idea.adds)}>
                + {idea.text}
              </button>
            {/each}
          </div>
        {/if}
      </details>
    {/each}

    <div class="foot">
      {#if forgetting}
        <span>Forget “{draft.title}” and everything prepared for it?</span>
        <button type="button" class="danger" onclick={() => void forget()}>Forget it</button>
        <button type="button" onclick={() => (forgetting = false)}>Keep it</button>
      {:else}
        <button type="button" class="quiet" onclick={() => (forgetting = true)}>Forget this event…</button>
      {/if}
    </div>
  {/if}
</section>

<style>
  .event {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    font-size: 0.85em;
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    align-items: center;
  }

  .head select {
    flex: 1 1 12rem;
    min-width: 0;
  }

  .new-title {
    flex: 1 1 10rem;
    min-width: 0;
  }

  .primary {
    background: var(--selected);
    border-color: var(--selected);
    color: var(--on-selected);
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 0.9em;
  }

  .held {
    margin: 0;
    color: var(--warn);
    font-size: 0.9em;
  }

  input.wrong {
    border-color: var(--warn);
  }

  .new-date {
    width: 7.5rem;
  }

  .empty,
  .about {
    margin: 0;
    color: var(--text-dim);
    line-height: 1.5;
  }

  .about {
    font-size: 0.9em;
    margin-bottom: 0.3rem;
  }

  /* The path: five stops in a row, done ones lit. */
  .path {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 0.25rem;
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .path li {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    padding: 0.3rem 0.35rem;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    min-width: 0;
  }

  .path li.done {
    border-color: var(--active);
  }

  .path .mark {
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .path li.done .mark {
    color: var(--active);
  }

  .stop-title {
    font-weight: 600;
  }

  .stop-about {
    font-size: 0.78em;
    color: var(--text-dim);
    line-height: 1.3;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .step {
    border-top: 1px solid var(--border);
    padding-top: 0.35rem;
  }

  .step summary {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    padding: 0.15rem 0;
  }

  .step-title {
    font-weight: 600;
  }

  .lacks {
    font-size: 0.78em;
    color: var(--warn);
  }

  .ready {
    font-size: 0.78em;
    color: var(--active);
  }

  .missing {
    margin: 0 0 0.35rem;
    font-size: 0.85em;
    color: var(--warn);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(9rem, 1fr));
    gap: 0.4rem 0.5rem;
  }

  .grid label,
  .troubles label,
  .set {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    font-size: 0.85em;
    color: var(--text-dim);
    min-width: 0;
  }

  .grid .wide {
    grid-column: 1 / -1;
  }

  .grid input,
  .grid select,
  .grid textarea,
  .troubles textarea,
  .set select {
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
    font: inherit;
    color: var(--text);
  }

  .duration {
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }

  .duration input {
    width: 3.2rem;
  }

  .field {
    font-size: 0.85em;
    color: var(--text-dim);
    margin-right: 0.3rem;
  }

  .choice {
    display: flex;
    gap: 0.25rem;
    flex-wrap: wrap;
  }

  .choice button.on,
  .chip.on,
  .rehearse.on {
    background: var(--selected);
    border-color: var(--selected);
    color: var(--on-selected);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    align-items: center;
  }

  .chip {
    padding: 0.1rem 0.45rem;
    border-radius: 999px;
    font-size: 0.9em;
  }

  .chip.avoid {
    border-color: var(--danger);
    color: var(--danger);
  }

  .moments,
  .moves,
  .troubles {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    margin-bottom: 0.4rem;
  }

  .moment {
    display: grid;
    grid-template-columns: 5.5rem minmax(0, 1fr) minmax(0, 1fr) auto;
    gap: 0.25rem;
  }

  .moment input {
    min-width: 0;
  }

  .drop {
    padding: 0 0.4rem;
  }

  .timeline {
    list-style: none;
    margin: 0 0 0.4rem;
    padding: 0.3rem 0.5rem;
    border-left: 2px solid var(--selected);
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .timeline li.outside {
    color: var(--warn);
  }

  .move {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .move-name {
    flex: 1;
    min-width: 0;
  }

  .rehearse {
    font-size: 0.85em;
    padding: 0.05rem 0.4rem;
  }

  .ideas {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    align-items: center;
    margin: 0.35rem 0 0.2rem;
  }

  .idea {
    font-size: 0.85em;
    padding: 0.1rem 0.45rem;
    border-style: dashed;
    color: var(--text-dim);
    text-align: left;
    max-width: 100%;
  }

  .idea:hover {
    color: var(--text);
  }

  .foot {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    flex-wrap: wrap;
    border-top: 1px solid var(--border);
    padding-top: 0.4rem;
  }

  .quiet {
    background: none;
    border: none;
    color: var(--text-dim);
    padding: 0;
  }

  .danger {
    border-color: var(--danger);
    color: var(--danger);
  }
</style>
