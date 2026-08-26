<script lang="ts">
  /**
   * What was thought while the records played.
   *
   * # The gesture this is built around
   *
   * Mark now, write afterwards. A DJ noticing the floor empty has both hands
   * busy and about ninety seconds of attention to spare; asking them to
   * compose a sentence loses the observation. So the topbar button takes the
   * moment — the time, and what was on the decks — and this is where the words
   * get added, later, when there is time.
   *
   * That makes a note with no words a complete thing rather than a
   * half-finished one, and this panel treats it as one: an unwritten mark is
   * shown with its cursor ready, not as an error.
   *
   * # Why the notes are not editable in place beyond their body
   *
   * The time and what was playing are what the note *is* — a record of a
   * moment. Letting them be changed would make it a claim about a moment
   * instead, and a journal you can rewrite is not one worth keeping.
   */
  import {
    currentSession,
    listNotes,
    listSessions,
    noteCounts,
    noteDelete,
    noteWrite,
    type JournalNote,
    type Session,
  } from "./api";
  import IconButton from "./controls/IconButton.svelte";
  import { onMount } from "svelte";

  interface Props {
    enabled: boolean;
  }

  let { enabled }: Props = $props();

  /**
   * How often tonight's notes are re-read.
   *
   * Five seconds. Notes arrive at the speed of a person noticing something,
   * and the panel is open while the marks are being made from the topbar, so
   * it has to notice them without the DJ going and finding a refresh.
   */
  const REFRESH_MS = 5000;

  let tonight = $state("");
  /** Which night is being read. Empty means tonight. */
  let chosen = $state("");
  let notes = $state<JournalNote[]>([]);
  let sessions = $state<Session[]>([]);
  let counts = $state<Record<string, number>>({});
  let error = $state("");
  /** Bodies being edited, by note id, so typing is not overwritten by a poll. */
  let drafts = $state<Record<number, string>>({});

  let reading = $derived(chosen || tonight);
  let isTonight = $derived(reading === tonight && tonight !== "");

  async function load() {
    try {
      notes = await listNotes(reading || undefined);
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  onMount(async () => {
    if (!enabled) return;
    try {
      tonight = await currentSession();
      sessions = await listSessions();
      counts = Object.fromEntries(await noteCounts());
    } catch (e) {
      error = String(e);
    }
  });

  /*
    Re-read on a timer, but only for tonight: an old night cannot gain notes
    while it is being read, so polling one would be a query per five seconds
    that can never return anything new.
  */
  $effect(() => {
    if (!enabled || !reading) return;
    void load();
    if (!isTonight) return;
    const timer = setInterval(() => void load(), REFRESH_MS);
    return () => clearInterval(timer);
  });

  /** The body as shown: the draft if one is being typed, else what is stored. */
  function bodyOf(note: JournalNote): string {
    return drafts[note.id] ?? note.body;
  }

  async function save(note: JournalNote) {
    const body = drafts[note.id];
    if (body === undefined || body === note.body) return;
    try {
      await noteWrite(note.id, body);
      delete drafts[note.id];
      await load();
    } catch (e) {
      error = String(e);
    }
  }

  async function remove(note: JournalNote) {
    try {
      await noteDelete(note.id);
      delete drafts[note.id];
      await load();
    } catch (e) {
      error = String(e);
    }
  }

  /** Local time: a DJ reads a night against the night they played it. */
  function clock(unixSeconds: number): string {
    return new Date(unixSeconds * 1000).toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  /** Nights that have notes, plus tonight, most recent first. */
  let nights = $derived(
    sessions
      .filter((s) => counts[s.id] > 0 || s.id === tonight)
      .map((s) => ({ id: s.id, count: counts[s.id] ?? 0 })),
  );
</script>

<div class="journal">
  {#if nights.length > 1}
    <div class="nights">
      {#each nights as night (night.id)}
        <button
          class="night"
          class:chosen={reading === night.id}
          onclick={() => (chosen = night.id)}
        >
          {night.id === tonight ? "Tonight" : night.id}
          {#if night.count > 0}<span class="count">{night.count}</span>{/if}
        </button>
      {/each}
    </div>
  {/if}

  {#if error}
    <p class="error">{error}</p>
  {:else if notes.length === 0}
    <p class="empty">
      Nothing marked{isTonight ? " tonight" : ""}. Use <strong>Mark</strong> in the
      top bar while a record is playing — it takes the moment and what was on the
      decks, and you write it up here afterwards.
    </p>
  {:else}
    <ul class="notes">
      {#each notes as note (note.id)}
        <li class:bare={note.bare && drafts[note.id] === undefined}>
          <div class="moment">
            <span class="mono time">{clock(note.at)}</span>
            <!--
              What was playing, not a link to it. The note is about the night;
              the record may have left the library since.
            -->
            <span class="playing" title={note.playing}>{note.playing || "—"}</span>
            <IconButton
              icon="fa-solid fa-trash"
              title="Delete this note"
              onClick={() => remove(note)}
            />
          </div>
          <!--
            A textarea rather than an input: a note is a sentence or three,
            and one that scrolls sideways in a single line is one nobody
            re-reads.
          -->
          <textarea
            rows="2"
            placeholder="What happened?"
            value={bodyOf(note)}
            oninput={(e) => (drafts[note.id] = e.currentTarget.value)}
            onblur={() => save(note)}
            aria-label="Note taken at {clock(note.at)}"
          ></textarea>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .journal {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    flex: 1;
    min-height: 0;
  }

  .nights {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    flex: none;
  }

  .night {
    font-size: 0.8em;
    padding: 0.15rem 0.45rem;
  }

  .night.chosen {
    border-color: var(--accent);
    color: var(--accent);
  }

  .count {
    margin-left: 0.3rem;
    opacity: 0.65;
  }

  /* The list scrolls, not the panel: the night picker above must stay put. */
  .notes {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .notes li {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.45rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 6px;
  }

  /*
    A mark with nothing written on it is not an error, it is the normal state
    of a note taken thirty seconds ago. Marked as unfinished, quietly.
  */
  .notes li.bare {
    border-style: dashed;
  }

  .moment {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.78em;
  }

  .time {
    color: var(--text-dim);
    flex: none;
  }

  .playing {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--accent);
  }

  textarea {
    width: 100%;
    resize: vertical;
    font: inherit;
    font-size: 0.82em;
    line-height: 1.45;
  }

  .empty,
  .error {
    margin: 0;
    font-size: 0.82em;
    line-height: 1.55;
    color: var(--text-dim);
  }

  .error {
    color: var(--danger, #dc2626);
  }
</style>
