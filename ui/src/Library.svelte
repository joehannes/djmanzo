<script lang="ts">
  /**
   * The DJ's own collection.
   *
   * # Why this looks like a table and not like the sources panel
   *
   * A streaming search is a question with an answer. A library is a place you
   * *know*: the same tracks in the same order, sortable by the columns a DJ
   * actually sorts by, findable by typing three letters. So it is a dense table
   * rather than a list of cards, and it sorts client-side because the rows are
   * already here and a round trip to re-sort five hundred of them would be
   * slower than doing it in place.
   *
   * # The two-phase scan, in the interface
   *
   * Adding a folder walks it and shows the tracks in seconds; identifying them
   * takes a full decode each and runs in the background. So the panel states
   * both numbers, always. A DJ who adds 4,000 files and sees "4,000 found,
   * 12 identified" understands what is happening; one who sees an empty
   * collection concludes it is broken.
   */
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import Crates, { type Selection } from "./Crates.svelte";
  import {
    addToPlaylist,
    checkFilter,
    formatTime,
    libraryAddFolder,
    libraryRemoveFolder,
    librarySearch,
    libraryRescan,
    libraryStatus,
    listPlaylists,
    loadTrack,
    playHistory,
    playlistTracks,
    removeFromPlaylist,
    setPlaylistQuery,
    smartPlaylistTracks,
    type LibraryStatus,
    type LibraryTrack,
    type PlayRecord,
    type Playlist,
  } from "./api";

  let { enabled, deckCount = 2 }: { enabled: boolean; deckCount?: number } = $props();

  /**
   * Left-to-right mark. Prefixed to any path shown in a `.path` box — see the
   * comment on that rule for why a right-to-left box needs it.
   */
  const LTR = "\u200e";

  let tracks = $state<LibraryTrack[]>([]);
  /**
   * What the sidebar has selected. The rows below follow it: the whole
   * collection, one playlist, or the history.
   */
  let selection = $state<Selection>({ kind: "all" });
  let history = $state<PlayRecord[]>([]);
  let playlists = $state<Playlist[]>([]);
  let status = $state<LibraryStatus | null>(null);
  let query = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);
  /** Path of the row being loaded, so it can say so. */
  let loading = $state<string | null>(null);

  type Column = "title" | "artist" | "album" | "bpm" | "key" | "duration_seconds";
  let sortBy = $state<Column>("artist");
  let ascending = $state(true);

  /**
   * How often the panel re-reads the identifier's progress.
   *
   * Two seconds, not sixty times a second: this is a background job measured in
   * minutes, and a counter that ticks smoothly would cost a database round trip
   * per frame to tell the DJ nothing they did not already know.
   */
  const STATUS_INTERVAL_MS = 2000;

  /**
   * How long to wait after a keystroke before searching.
   *
   * Long enough that typing a word is one query rather than five, short enough
   * that it still feels like the results are following your fingers.
   */
  const DEBOUNCE_MS = 120;

  let debounce: ReturnType<typeof setTimeout> | undefined;

  async function refresh() {
    try {
      if (selection.kind === "history") {
        history = await playHistory();
        tracks = [];
      } else if (selection.kind === "smart") {
        // Evaluated now, not stored: a smart folder is a question about the
        // collection, so a track added since it was written belongs in it.
        tracks = await smartPlaylistTracks(selection.id);
      } else if (selection.kind === "playlist") {
        const entries = await playlistTracks(selection.id);
        // Search still filters inside a playlist, client-side: the rows are
        // already here, and a DJ narrowing a 200-track set does not want a
        // round trip per keystroke.
        const needle = query.trim().toLowerCase();
        const matching = needle
          ? entries.filter(
              (e) =>
                e.title.toLowerCase().includes(needle) ||
                e.artist.toLowerCase().includes(needle),
            )
          : entries;
        // The position rides on each row: `PlaylistEntry` is a `LibraryTrack`
        // plus its position, so removal can name the entry rather than the
        // track -- which matters when the same track is in the set twice.
        tracks = matching;
      } else {
        tracks = await librarySearch(query);
      }
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function refreshPlaylists() {
    try {
      playlists = await listPlaylists();
    } catch (e) {
      error = String(e);
    }
  }

  async function addTo(playlist: number, track: LibraryTrack) {
    try {
      await addToPlaylist(playlist, track.id);
      await refreshPlaylists();
      if (selection.kind === "playlist" && selection.id === playlist) await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function removeAt(position: number) {
    if (selection.kind !== "playlist") return;
    try {
      await removeFromPlaylist(selection.id, position);
      await Promise.all([refresh(), refreshPlaylists()]);
    } catch (e) {
      error = String(e);
    }
  }

  /** The filter being edited, and what is wrong with it. */
  let filterText = $state("");
  let filterError = $state<string | null>(null);
  let filterDirty = $state(false);

  /**
   * Check as it is typed, save on blur or Enter.
   *
   * Checking is a parse, which is microseconds — so the DJ finds out about a
   * typo while looking at the box, rather than when the folder turns up empty.
   * Saving is separate because a half-typed filter is not one to store.
   */
  async function checkFilterText() {
    filterDirty = true;
    if (!filterText.trim()) {
      filterError = null;
      return;
    }
    try {
      await checkFilter(filterText);
      filterError = null;
    } catch (e) {
      filterError = String(e);
    }
  }

  async function saveFilter() {
    if (selection.kind !== "smart" || !filterDirty || filterError) return;
    try {
      await setPlaylistQuery(selection.id, filterText);
      selection = { ...selection, query: filterText };
      filterDirty = false;
      await Promise.all([refresh(), refreshPlaylists()]);
    } catch (e) {
      filterError = String(e);
    }
  }

  /**
   * Follow the sidebar into a smart folder.
   *
   * Keyed on the id as well as the text so that switching between two folders
   * loads the second one's filter rather than keeping the first's.
   */
  $effect(() => {
    if (selection.kind === "smart") {
      filterText = selection.query;
      filterError = null;
      filterDirty = false;
    }
  });

  /** Local time, since a DJ reads a history against the night they played. */
  function whenPlayed(unixSeconds: number): string {
    return new Date(unixSeconds * 1000).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  /**
   * Re-read the status, and the rows with it when the collection has grown.
   *
   * The count and the table have to move together. Polling only the count would
   * show a DJ "4,000 tracks" over an empty table until they happened to type
   * something — the collection filling up in a number while the thing they are
   * looking at stays blank, which is worse than showing nothing at all.
   *
   * Only when the count actually changed: identification takes seconds per
   * file, so most polls have nothing to report and re-running the query on
   * every one would be five hundred rows of IPC every two seconds for nothing.
   */
  async function refreshStatus() {
    try {
      const next = await libraryStatus();
      const grew = status != null && next.tracks !== status.tracks;
      status = next;
      if (grew) await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  function onQuery() {
    clearTimeout(debounce);
    debounce = setTimeout(refresh, DEBOUNCE_MS);
  }

  async function addFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== "string") return;
    busy = true;
    error = null;
    try {
      const report = await libraryAddFolder(picked);
      if (report.unreadable_dirs > 0) {
        error = `${report.unreadable_dirs} folder${
          report.unreadable_dirs === 1 ? "" : "s"
        } could not be read — a permissions problem, or a drive that is not mounted.`;
      }
      await Promise.all([refresh(), refreshStatus()]);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function removeFolder(path: string) {
    try {
      await libraryRemoveFolder(path);
      await refreshStatus();
    } catch (e) {
      error = String(e);
    }
  }

  async function rescan() {
    busy = true;
    error = null;
    try {
      await libraryRescan();
      await Promise.all([refresh(), refreshStatus()]);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function toDeck(track: LibraryTrack, deck: number) {
    loading = track.path;
    error = null;
    try {
      await loadTrack(deck, track.path);
    } catch (e) {
      // The commonest cause by far: the file moved or the drive is unplugged.
      // Say that, rather than relaying a decoder error nobody can act on.
      error = `Could not load ${track.title}. ${String(e)}`;
    } finally {
      loading = null;
    }
  }

  function sort(column: Column) {
    if (sortBy === column) {
      ascending = !ascending;
    } else {
      sortBy = column;
      ascending = true;
    }
  }

  const sorted = $derived.by(() => {
    const rows = [...tracks];
    const direction = ascending ? 1 : -1;
    rows.sort((a, b) => {
      const x = a[sortBy];
      const y = b[sortBy];
      // Nulls last whichever way the column is sorted. An unanalysed track has
      // no BPM, and burying those among the 60s or the 180s would be worse than
      // keeping them together at the end where they can be seen.
      if (x == null && y == null) return 0;
      if (x == null) return 1;
      if (y == null) return -1;
      if (typeof x === "number" && typeof y === "number") return (x - y) * direction;
      return String(x).localeCompare(String(y), undefined, { numeric: true }) * direction;
    });
    return rows;
  });

  const decks = $derived(Array.from({ length: deckCount }, (_, i) => i + 1));

  onMount(() => {
    void refresh();
    void refreshStatus();
    void refreshPlaylists();
    const timer = setInterval(refreshStatus, STATUS_INTERVAL_MS);
    return () => {
      clearInterval(timer);
      clearTimeout(debounce);
    };
  });
</script>

<div class="with-sidebar">
<Crates bind:selection onchange={() => void refresh()} />

<div class="library">
  <div class="controls">
    <!--
      Absent in the history rather than present and inert. Searching a history
      is a different question — "when did I last play this" — and a box that
      silently does nothing is worse than one that is not there.
    -->
    {#if selection.kind !== "history"}
      <input
        type="search"
        placeholder={selection.kind === "playlist" || selection.kind === "smart"
          ? `Search ${selection.name}…`
          : "Search your collection…"}
        bind:value={query}
        oninput={onQuery}
        aria-label="Search the library"
      />
    {:else}
      <span class="viewing">Everything played, most recent first.</span>
    {/if}
    <button onclick={addFolder} disabled={busy}>Add folder…</button>
    <button onclick={rescan} disabled={busy || !status?.folders.length}>
      {busy ? "Scanning…" : "Rescan"}
    </button>
  </div>

  {#if status}
    <!--
      Both halves of the scan, always. "4,000 found, 12 identified" is a DJ
      waiting; an empty collection with no explanation is a DJ filing a bug.
    -->
    <p class="status">
      <!--
        What is on screen first, the collection second. The collection count
        above a two-row playlist reads as a bug, however true it is.
      -->
      {#if selection.kind === "smart"}
        <strong>{sorted.length.toLocaleString()}</strong> matching
        · {status.tracks.toLocaleString()} in your collection
      {:else if selection.kind === "playlist"}
        <strong>{sorted.length.toLocaleString()}</strong> in {selection.name}
        · {status.tracks.toLocaleString()} in your collection
      {:else if selection.kind === "history"}
        <strong>{history.length.toLocaleString()}</strong> play{history.length === 1 ? "" : "s"}
      {:else}
        <strong>{status.tracks.toLocaleString()}</strong> track{status.tracks === 1 ? "" : "s"}
      {/if}
      {#if status.pending > 0}
        · <strong>{status.pending.toLocaleString()}</strong> waiting to be analysed
        {#if status.working}<em>(working…)</em>{/if}
      {/if}
      {#if status.identified > 0}
        · {status.identified.toLocaleString()} done this session
      {/if}
    </p>

    {#if status.pending > 0}
      <p class="hint">
        Identifying a track means decoding it, which takes a few seconds each. It runs in
        the background — the tracks above are already playable, and the rest appear as they
        finish.
      </p>
    {/if}

    {#if status.path == null}
      <p class="warn">
        No writable config directory, so the library is in memory only and will be empty
        again after a restart.
      </p>
    {/if}

    {#if status.folders.length > 0}
      <div class="folders">
        {#each status.folders as folder (folder)}
          <span class="folder">
            <span class="path" title={folder}>{LTR}{folder}</span>
            <button
              class="remove"
              onclick={() => removeFolder(folder)}
              title="Stop watching this folder. The tracks already in the library stay."
              aria-label="Stop watching {folder}"
            >×</button>
          </span>
        {/each}
      </div>
    {/if}

    {#if status.failed.length > 0}
      <details class="failed">
        <summary>{status.failed.length} file{status.failed.length === 1 ? "" : "s"} could not be read</summary>
        <ul>
          {#each status.failed.slice(0, 20) as file (file.path)}
            <li><span class="path">{LTR}{file.path}</span> — {file.reason}</li>
          {/each}
        </ul>
      </details>
    {/if}
  {/if}

  {#if selection.kind === "smart"}
    <div class="filter">
      <input
        class:invalid={filterError != null}
        bind:value={filterText}
        oninput={checkFilterText}
        onblur={saveFilter}
        onkeydown={(e) => e.key === "Enter" && saveFilter()}
        placeholder="bpm > 120 and key compatible 8A"
        aria-label="Filter for {selection.name}"
      />
      {#if filterDirty && !filterError}
        <button onclick={saveFilter}>Save</button>
      {/if}
    </div>
    {#if filterError}
      <p class="error">{filterError}</p>
    {:else}
      <p class="hint">
        <code>bpm</code>, <code>key</code>, <code>artist</code>, <code>title</code>,
        <code>album</code>, <code>genre</code>, <code>label</code>, <code>year</code>,
        <code>rating</code>, <code>plays</code> — compared with
        <code>&gt; &lt; = !=</code>, <code>contains</code>, <code>starts</code>,
        <code>ends</code>, or <code>compatible</code> for harmonic neighbours. Combine
        with <code>and</code>, <code>or</code>, <code>not</code> and brackets.
      </p>
    {/if}
  {/if}

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if selection.kind === "history"}
    {#if history.length === 0}
      <p class="empty">
        Nothing played yet. A track counts once it has been playing for thirty
        seconds — long enough that auditioning the first four bars of everything
        does not fill this up.
      </p>
    {:else}
      <div class="table-scroll">
        <table>
          <thead>
            <tr>
              <th>Played</th>
              <th>Title</th>
              <th>Artist</th>
            </tr>
          </thead>
          <tbody>
            {#each history as play, index (play.track_id + ":" + play.played_at + ":" + index)}
              <tr>
                <td class="mono">{whenPlayed(play.played_at)}</td>
                <td class="title">{play.title}</td>
                <td>{play.artist}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {:else if sorted.length === 0}
    <p class="empty">
      {#if query.trim()}
        Nothing matches “{query}”.
      {:else if status && status.pending > 0}
        Still identifying. Tracks appear here as they finish.
      {:else}
        No music yet. Add a folder to get started.
      {/if}
    </p>
  {:else}
    <div class="table-scroll">
      <table>
        <thead>
          <tr>
            {#each [["title", "Title"], ["artist", "Artist"], ["album", "Album"], ["bpm", "BPM"], ["key", "Key"], ["duration_seconds", "Time"]] as [column, heading] (column)}
              <th>
                <button
                  class="sort"
                  class:active={sortBy === column}
                  onclick={() => sort(column as Column)}
                >
                  {heading}{#if sortBy === column}<span class="arrow">{ascending ? "▲" : "▼"}</span>{/if}
                </button>
              </th>
            {/each}
            <th class="load-heading">Load</th>
          </tr>
        </thead>
        <tbody>
          {#each sorted as track (track.id)}
            <tr class:unanalysed={!track.analysed}>
              <td class="title" title={track.path}>{track.title}</td>
              <td>{track.artist}</td>
              <td>{track.album ?? ""}</td>
              <!--
                A blank rather than a zero when the analyser has not run. A
                plausible-looking 0.0 is worse than an obvious gap, because a
                DJ reads these at a glance and will not stop to wonder.
              -->
              <td class="mono">{track.bpm != null ? track.bpm.toFixed(1) : ""}</td>
              <td class="mono">{track.key ?? ""}</td>
              <td class="mono">{formatTime(track.duration_seconds)}</td>
              <td class="load">
                <!--
                  Adding to a playlist is a select rather than a drag. Drag is
                  the gesture DJs know, and it will come — but a control that
                  works with one hand on a trackpad in a dark booth should not
                  wait for it.
                -->
                {#if playlists.some((p) => p.kind !== "folder")}
                  <select
                    class="add-to"
                    aria-label="Add {track.title} to a playlist"
                    onchange={(event) => {
                      const target = event.currentTarget;
                      const id = Number(target.value);
                      target.value = "";
                      if (id) void addTo(id, track);
                    }}
                  >
                    <option value="">+</option>
                    {#each playlists.filter((p) => p.kind !== "folder") as list (list.id)}
                      <option value={list.id}>{list.name}</option>
                    {/each}
                  </select>
                {/if}
                {#if selection.kind === "playlist" && "position" in track}
                  <button
                    class="drop"
                    onclick={() => removeAt((track as { position: number }).position)}
                    title="Take this out of {selection.name}"
                    aria-label="Remove from playlist"
                  >−</button>
                {/if}
                {#each decks as deck (deck)}
                  <button
                    onclick={() => toDeck(track, deck)}
                    disabled={!enabled || loading === track.path}
                    title="Load onto deck {deck}"
                  >
                    {loading === track.path ? "…" : deck}
                  </button>
                {/each}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
</div>

<style>
  /*
    Sidebar and rows side by side, both scrolling inside themselves so the
    controls above stay reachable however long either gets.
  */
  .with-sidebar {
    display: flex;
    gap: 0.7rem;
    flex: 1;
    min-height: 0;
  }

  .add-to {
    font-size: 0.85em;
    padding: 0.05rem 0.1rem;
    max-width: 3.5rem;
  }

  .library {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    min-height: 0;
    height: 100%;
  }

  .controls {
    display: flex;
    gap: 0.5rem;
  }

  .controls input {
    flex: 1;
    min-width: 0;
  }

  .filter {
    display: flex;
    gap: 0.4rem;
  }

  .filter input {
    flex: 1;
    min-width: 0;
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 0.9em;
  }

  /* Marked, not blocked: a half-typed filter is normal, and taking the box
     away mid-sentence would be worse than colouring it. */
  .filter input.invalid {
    border-color: var(--danger, #dc2626);
  }

  .hint code {
    font-size: 0.95em;
    opacity: 0.85;
  }

  .viewing {
    flex: 1;
    align-self: center;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .status {
    margin: 0;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .status strong {
    color: var(--text);
  }

  .hint,
  .warn,
  .empty {
    margin: 0;
    font-size: 0.85em;
    color: var(--text-dim);
    line-height: 1.45;
  }

  .warn {
    color: var(--warn, #d97706);
  }

  .error {
    margin: 0;
    font-size: 0.85em;
    color: var(--danger, #dc2626);
  }

  .folders {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .folder {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    background: var(--panel-raised);
    border-radius: 4px;
    padding: 0.1rem 0.2rem 0.1rem 0.45rem;
    font-size: 0.8em;
    max-width: 100%;
  }

  /*
    A long path truncates from the *left*: the folder name at the end is what
    distinguishes two paths, and the shared prefix is what does not.

    `direction: rtl` is what moves the ellipsis to the start, and on its own it
    also reorders the punctuation -- the bidi algorithm treats a leading `/` as
    neutral and floats it to the visual end, so `/tmp/music` renders as
    `tmp/music/`. The U+200E prefix in the markup pins the string as one
    left-to-right run so the slashes stay where they were typed, while the box
    keeps clipping at the start.
  */
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
    max-width: 22rem;
  }

  .folder .remove {
    padding: 0 0.3rem;
    line-height: 1.2;
  }

  .failed {
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .failed ul {
    margin: 0.3rem 0 0;
    padding-left: 1.1rem;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  /*
    The table scrolls inside its own box rather than growing the panel. A
    collection is thousands of rows and the controls above must stay reachable.
  */
  .table-scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 6px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85em;
  }

  thead th {
    position: sticky;
    top: 0;
    background: var(--panel-raised);
    text-align: left;
    padding: 0;
    border-bottom: 1px solid var(--border);
    /* Above the rows, which scroll under it. */
    z-index: 1;
  }

  .sort {
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-radius: 0;
    padding: 0.35rem 0.5rem;
    font: inherit;
    color: var(--text-dim);
    cursor: pointer;
  }

  .sort.active {
    color: var(--text);
  }

  .arrow {
    margin-left: 0.25rem;
    font-size: 0.75em;
  }

  tbody td {
    padding: 0.25rem 0.5rem;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 18rem;
  }

  tbody tr:hover td {
    background: var(--panel-raised);
  }

  /* Dimmed, not hidden: the track is playable, it just has no numbers yet. */
  tbody tr.unanalysed td {
    color: var(--text-dim);
  }

  .mono {
    font-variant-numeric: tabular-nums;
  }

  .load,
  .load-heading {
    text-align: right;
    white-space: nowrap;
  }

  .load button {
    padding: 0.1rem 0.4rem;
    font-size: 0.9em;
    margin-left: 0.15rem;
  }
</style>
