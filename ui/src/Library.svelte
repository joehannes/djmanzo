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
  import { SvelteSet } from "svelte/reactivity";
  import { open } from "@tauri-apps/plugin-dialog";
  import Journal from "./Journal.svelte";
  import Requests from "./Requests.svelte";
  import Memory from "./Memory.svelte";
  import ShareSet from "./ShareSet.svelte";
  import Crates, { type Selection } from "./Crates.svelte";
  import IconButton from "./controls/IconButton.svelte";
  import { setAside } from "./prepare.svelte";
  import {
    addToPlaylist,
    checkFilter,
    clearTrackField,
    coverUrl,
    defaultMusicFolder,
    editTracks,
    findDuplicates,
    forgetTrackPath,
    formatTime,
    importLibrary,
    similarTo,
    type Suggestion,
    listSessions,
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
    type DeckState,
    type Duplicate,
    type LibraryStatus,
    type LibraryTrack,
    type PlayRecord,
    type Playlist,
    type Session,
    setTrackFunctions,
    trackFunctions,
    type TrackFunction,
  } from "./api";

  let {
    enabled,
    deckCount = 2,
    decks = [],
  }: { enabled: boolean; deckCount?: number; decks?: DeckState[] } = $props();

  /**
   * The drag payload for tracks.
   *
   * A custom type rather than `text/plain` so a drop target knows what it is
   * being handed: the crate tree also drags nodes around to reparent them, and
   * telling the two apart by the payload's shape is how a folder ends up with
   * a track id in it.
   */
  const TRACKS = "application/x-djmanzo-tracks";

  /**
   * Start dragging a track — or the whole selection, if it is part of one.
   *
   * Dragging a selected row takes everything selected, because that is what a
   * DJ who has just ticked eight boxes means. Dragging an *unselected* row
   * takes only that row, and does not disturb the selection: picking something
   * up should not silently change what was chosen.
   */
  function startDrag(event: DragEvent, track: { id: string }) {
    const ids = selected.has(track.id) ? [...selected] : [track.id];
    event.dataTransfer?.setData(TRACKS, JSON.stringify(ids));
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "copy";
  }

  /**
   * Setting a track aside now goes through a module rune rather than down a
   * prop, because Prepare is no longer a child of this panel -- it is a dock
   * surface of its own, which is what §21's "first class" asks for. See
   * `prepare.svelte.ts` for why that is a rune and not a context or an event.
   */
  /**
   * The track "more like this" is currently answering for, if any.
   *
   * The results replace the table rather than opening a list of their own.
   * A second list would need its own sorting, its own deck buttons and its
   * own set-aside, and would have fewer of them than the table it sat beside —
   * a worse version of the thing already on screen.
   */
  let likeThis = $state<string | null>(null);
  let likeThisTitle = $state("");
  let alike = $state<Suggestion[]>([]);
  /** Reasons per track id, so a row can say why it is here. */
  let alikeWhy = $state<Record<string, string[]>>({});

  async function showAlike(id: string) {
    if (likeThis === id) {
      likeThis = null;
      return;
    }
    try {
      const found = await similarTo(id);
      alike = found;
      alikeWhy = Object.fromEntries(found.map((s) => [s.track.id, s.reasons]));
      likeThisTitle = tracks.find((t) => t.id === id)?.title ?? "that track";
      likeThis = id;
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

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
  let duplicates = $state<Duplicate[]>([]);
  let sessions = $state<Session[]>([]);
  /** Which night the share sheet is open for, if any. */
  let sharing = $state<string | null>(null);

  /**
   * Which rows are selected, by track id.
   *
   * A set rather than a flag on each row: the rows are replaced wholesale on
   * every search, and a flag would be lost with them — which is exactly when a
   * DJ has typed to narrow the list *in order to* select something.
   */
  let selected = $state<Set<string>>(new SvelteSet());
  /** The batch edit being composed. */
  let edit = $state<{ genre: string; colour: string; rating: string }>({
    genre: "",
    colour: "",
    rating: "",
  });
  /**
   * What a record is *for*, which is not what it is.
   *
   * Genre says a record is bachata; it does not say whether it opens a room or
   * rescues an empty floor. The vocabulary is ten words fixed in Rust, and the
   * labels come from there so the assistant and this panel cannot describe the
   * same thing differently. See `dj_library::functions`.
   */
  let functions = $state<TrackFunction[]>([]);
  /** The functions being applied to the selection, by slug. */
  let chosenFunctions = $state<Set<string>>(new SvelteSet());

  $effect(() => {
    void trackFunctions()
      .then((got) => {
        functions = got;
      })
      // A library that is not open yet answers with an error, and the picker
      // simply is not offered until it is.
      .catch(() => {});
  });

  /**
   * Apply what is ticked to everything selected.
   *
   * The whole answer rather than a change to it: Rust replaces the set, so
   * unticking a function removes it. Separate from the genre/colour/rating
   * apply because it is a different kind of edit -- those write a field, this
   * writes a judgement -- and because folding it in would mean a DJ colouring
   * eight tracks silently cleared their functions.
   */
  async function applyFunctions() {
    if (selected.size === 0) return;
    busy = true;
    try {
      await setTrackFunctions([...selected], [...chosenFunctions]);
      functions = await trackFunctions();
      await refresh();
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  let status = $state<LibraryStatus | null>(null);
  let query = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);
  /** Path of the row being loaded, so it can say so. */
  let loading = $state<string | null>(null);

  type Column =
    | "title"
    | "artist"
    | "album"
    | "genre"
    | "year"
    | "bpm"
    | "key"
    | "key_name"
    | "loudness_lufs"
    | "duration_seconds"
    | "rating"
    | "play_count"
    | "last_played"
    | "phrase_beats";

  /**
   * The columns, what to call them, and how to read one off a record.
   *
   * One list, read by the table's headings, its cells, the card view's sort
   * control and the column picker. Two lists would eventually offer different
   * columns in different places, and a DJ who sorted by BPM in the table and
   * could not in cards would conclude the cards were the lesser browser —
   * which is exactly what §20 is asking us not to build.
   *
   * `show` returns the string to draw, and **empty means "not known"** rather
   * than zero: an unanalysed record has no BPM, and a plausible-looking 0.0 is
   * read at a glance and believed. Sorting reads the raw field, so nulls go to
   * the end however the column is sorted, whatever `show` does with them.
   *
   * `numeric` is about alignment and nothing else — a column of tabular
   * figures is scannable and a column of proportional ones is not.
   */
  type ColumnSpec = {
    key: Column;
    heading: string;
    show: (track: LibraryTrack) => string;
    numeric?: boolean;
    /** Shown unless the DJ says otherwise. Six, which is what fits. */
    byDefault?: boolean;
  };

  const COLUMNS: ColumnSpec[] = [
    { key: "title", heading: "Title", show: (t) => t.title, byDefault: true },
    { key: "artist", heading: "Artist", show: (t) => t.artist, byDefault: true },
    { key: "album", heading: "Album", show: (t) => t.album ?? "", byDefault: true },
    { key: "genre", heading: "Genre", show: (t) => t.genre ?? "" },
    { key: "year", heading: "Year", show: (t) => (t.year != null ? String(t.year) : ""), numeric: true },
    {
      key: "bpm",
      heading: "BPM",
      show: (t) => (t.bpm != null ? t.bpm.toFixed(1) : ""),
      numeric: true,
      byDefault: true,
    },
    { key: "key", heading: "Camelot", show: (t) => t.key ?? "", numeric: true, byDefault: true },
    { key: "key_name", heading: "Key", show: (t) => t.key_name ?? "", numeric: true },
    {
      // §20 calls this column "energy". This library measures loudness, which
      // is the honest thing it has: an integrated LUFS is a number an
      // instrument took, and an energy rating out of ten is one somebody
      // invented. The Pair view already labels it the same way.
      key: "loudness_lufs",
      heading: "Energy",
      show: (t) => (t.loudness_lufs != null ? `${t.loudness_lufs.toFixed(1)} LU` : ""),
      numeric: true,
    },
    {
      key: "duration_seconds",
      heading: "Time",
      show: (t) => formatTime(t.duration_seconds),
      numeric: true,
      byDefault: true,
    },
    {
      key: "rating",
      heading: "Rating",
      show: (t) => (t.rating != null ? "\u2605".repeat(t.rating) : ""),
    },
    { key: "play_count", heading: "Plays", show: (t) => String(t.play_count), numeric: true },
    {
      key: "last_played",
      heading: "Last played",
      // Never played is blank rather than "never": the column is full of dates
      // and a word among them reads as a date nobody recognises.
      show: (t) => (t.last_played != null ? whenPlayed(t.last_played) : ""),
    },
    {
      key: "phrase_beats",
      heading: "Phrase",
      show: (t) => (t.phrase_beats != null ? `${t.phrase_beats}` : ""),
      numeric: true,
    },
  ];
  let sortBy = $state<Column>("artist");
  let ascending = $state(true);

  /**
   * Which columns are on, in the order [`COLUMNS`] declares them.
   *
   * §20 asks for *instant custom column configuration*, and this is it: a
   * checkbox each, applied as it is pressed, remembered. Order is not
   * configurable and that is deliberate for now — a DJ who can reorder columns
   * can also lose the title off the left-hand edge, and the value of that is
   * not obvious enough to spend the drag-and-drop on.
   *
   * Stored as a list of names rather than a set of booleans, so a column added
   * later is off for people who have already chosen and on for people who
   * have not — which is the right way round. A stored name that no longer
   * exists is dropped on the way in.
   */
  const CHOSEN_KEY = "djmanzo.library.columns";

  function rememberedColumns(): Set<Column> {
    const stored = safeStorage()?.getItem(CHOSEN_KEY);
    if (stored) {
      const names = new Set(stored.split(","));
      const kept = COLUMNS.filter((c) => names.has(c.key)).map((c) => c.key);
      // An empty choice would be a table of nothing but checkboxes and load
      // buttons, which is a state to refuse rather than to store.
      if (kept.length > 0) return new SvelteSet(kept);
    }
    return new SvelteSet(COLUMNS.filter((c) => c.byDefault).map((c) => c.key));
  }

  let chosenColumns = $state<Set<Column>>(rememberedColumns());
  /** The chosen columns, in the declared order. */
  const shown = $derived(COLUMNS.filter((c) => chosenColumns.has(c.key)));
  let pickingColumns = $state(false);

  function toggleColumn(column: Column) {
    if (chosenColumns.has(column)) {
      // Never all of them off. The last one stays, for the reason above.
      if (chosenColumns.size === 1) return;
      chosenColumns.delete(column);
    } else {
      chosenColumns.add(column);
    }
    try {
      safeStorage()?.setItem(CHOSEN_KEY, [...chosenColumns].join(","));
    } catch {
      // Storage denied. The choice still applies; it is forgotten by the next
      // launch, which is not worth a message.
    }
  }

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
        sessions = await listSessions();
        tracks = [];
      } else if (selection.kind === "duplicates") {
        duplicates = await findDuplicates();
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

  /** The crate tree, so it can be told when the playlists change. */
  let crates = $state<Crates | null>(null);

  /**
   * Re-read the playlists — *both* copies of them.
   *
   * The tree and this panel each hold their own list of the same rows, and for
   * a while only this one was refreshed. The symptom was quiet and bad:
   * importing a rekordbox library with forty playlists put forty rows in the
   * database and showed none of them in the tree until the panel remounted, so
   * an import looked like it had half worked.
   *
   * Both in one function rather than two calls at each site, because the bug
   * was somebody remembering one of them.
   */
  async function refreshPlaylists() {
    try {
      playlists = await listPlaylists();
      await crates?.refresh();
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

  function toggleSelected(id: string) {
    if (selected.has(id)) selected.delete(id);
    else selected.add(id);
  }

  async function applyEdit() {
    if (selected.size === 0) return;
    const ids = [...selected];
    try {
      const rating = edit.rating === "" ? undefined : Number(edit.rating);
      await editTracks(ids, {
        genre: edit.genre || undefined,
        colour: edit.colour || undefined,
        rating,
      });
      edit = { genre: "", colour: "", rating: "" };
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function clearField(field: string) {
    if (selected.size === 0) return;
    try {
      await clearTrackField([...selected], field);
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function forgetCopy(track: string, path: string) {
    try {
      await forgetTrackPath(track, path);
      duplicates = await findDuplicates();
    } catch (e) {
      error = String(e);
    }
  }

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
    await scanFolder(picked);
  }

  /**
   * The one-click first run.
   *
   * `null` until the backend answers, and hidden once there is anything in the
   * collection — an offer to scan your music folder is help on a first launch
   * and clutter on every one after it.
   */
  let musicFolder = $state<string | null>(null);
  $effect(() => {
    void defaultMusicFolder()
      .then((found) => (musicFolder = found))
      .catch(() => {});
  });

  async function scanFolder(path: string) {
    busy = true;
    error = null;
    try {
      const report = await libraryAddFolder(path);
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

  /** What the last import did, kept until the DJ does something else. */
  let imported = $state<string | null>(null);

  async function importFrom(folder: boolean) {
    // Serato has no export file — it keeps a `_Serato_` folder — so importing
    // from it means choosing a directory. The two buttons say which, rather
    // than one button that sometimes wants a file and sometimes a folder.
    const picked = await open(
      folder
        ? { directory: true, multiple: false }
        : {
            multiple: false,
            filters: [
              // Named by what a DJ recognises, not by extension: they know
              // they exported from rekordbox, and may not know what it wrote.
              {
                name: "Library export (rekordbox, Traktor, iTunes)",
                extensions: ["xml", "nml"],
              },
            ],
          },
    );
    if (typeof picked !== "string") return;
    busy = true;
    error = null;
    imported = null;
    try {
      const result = await importLibrary(picked);
      const parts = [`${result.format}: ${result.tracks} tracks`];
      if (result.already_known > 0) parts.push(`${result.already_known} already here`);
      if (result.queued > 0) parts.push(`${result.queued} being analysed`);
      if (result.playlists > 0) {
        parts.push(`${result.playlists} playlist${result.playlists === 1 ? "" : "s"}`);
      }
      if (result.skipped.length > 0) {
        parts.push(`${result.skipped.length} skipped`);
      }
      imported = parts.join(" · ");
      // The skipped list is the part a DJ needs to act on, so it goes where
      // errors go rather than into the summary line.
      if (result.skipped.length > 0) {
        error = `Could not import: ${result.skipped.slice(0, 5).join("; ")}${
          result.skipped.length > 5 ? ` and ${result.skipped.length - 5} more` : ""
        }`;
      }
      await Promise.all([refresh(), refreshStatus(), refreshPlaylists()]);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function rescan() {
    busy = true;
    error = null;
    try {
      await libraryRescan();
      await Promise.all([refresh(), refreshStatus(), refreshPlaylists()]);
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
    // "More like this" is already ranked, and re-sorting it by BPM would throw
    // away the ranking that is the whole answer. The column headers still
    // work — a DJ who sorts is asking to see it a different way — but the
    // order it arrives in is the suggester's.
    if (likeThis) return alike.map((s) => s.track);
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

  /** Deck *numbers*, for the load buttons — distinct from `decks`, which is
   * their live state. */
  const deckNumbers = $derived(Array.from({ length: deckCount }, (_, i) => i + 1));

  // -------------------------------------------------------------------------
  // The two representations
  // -------------------------------------------------------------------------

  /**
   * Table or cards.
   *
   * The directive's §20 asks for several representations of the same
   * collection, and this is the same rows either way: the same sort, the same
   * search, the same selection, the same actions. A card view that quietly
   * showed a different set of records — or offered fewer things to do with one
   * — would be a second browser rather than a second view of this one.
   *
   * Cards are for when the sleeve is the fastest way to find a record, which
   * is most of how a physical collection is searched and none of how a
   * spreadsheet is.
   */
  type Layout = "table" | "cards";
  const LAYOUT_KEY = "djmanzo.library.layout";

  function safeStorage(): Storage | null {
    try {
      return typeof localStorage === "undefined" ? null : localStorage;
    } catch {
      return null;
    }
  }

  /**
   * Remembered, because it is a working preference and not a mode.
   *
   * Somebody who prefers sleeves prefers them tomorrow as well, and being
   * handed the table again on every launch is the kind of small daily friction
   * that makes a person stop using the view they wanted.
   */
  let layout = $state<Layout>(safeStorage()?.getItem(LAYOUT_KEY) === "cards" ? "cards" : "table");

  function choose(next: Layout) {
    layout = next;
    try {
      safeStorage()?.setItem(LAYOUT_KEY, next);
    } catch {
      // A browser with storage denied still gets the view it asked for; it
      // just forgets by the next launch. Not worth a message.
    }
  }

  /**
   * Records whose sleeve came back 404, so the card stops asking.
   *
   * Most collections are part-tagged and a missing sleeve is not a failure —
   * see `coverUrl`. Without this the browser would re-request the same missing
   * picture on every scroll back.
   */
  let sleeveless = $state<Set<string>>(new SvelteSet());

  /**
   * The lettering a card falls back to.
   *
   * Deliberately monochrome. Colour already means something in this browser —
   * it is the marking the DJ applied — and a second, generated colour language
   * on the same tile would be two things that look like the same statement.
   */
  function initials(track: LibraryTrack): string {
    const letter = (of: string) => of.trim()[0]?.toUpperCase() ?? "";
    // Artist then title, not the first two words of either: "JL" for both Juan
    // Luis Guerra records tells you nothing, and "JB" and "JO" tell you which.
    return `${letter(track.artist)}${letter(track.title)}`;
  }

  /**
   * Somewhere to put a record without choosing a deck.
   *
   * A deck that is **loaded but not playing** is what the autopilot and the
   * automix both mean by a place to mix into, so staging from the browser is
   * the same statement they are already reading. First rather than best: with
   * two decks there is no interesting choice, and with four the DJ who cares
   * uses the numbered buttons beside this one.
   */
  const freeDeck = $derived(
    decks.find((deck) => !deck.playing && deck.number <= deckCount)?.number ?? null,
  );

  async function stage(track: LibraryTrack) {
    if (freeDeck === null) return;
    await toDeck(track, freeDeck);
  }

  /**
   * Five stars, toggled.
   *
   * §20 asks a card for "favorite", and this library already has a rating a DJ
   * sorts and filters by. A separate favourite flag would be a second opinion
   * about the same judgement, and the two would drift the first time somebody
   * used one and not the other. So a favourite *is* five stars, and the star
   * on the card says so.
   */
  async function favourite(track: LibraryTrack) {
    try {
      if (track.rating === 5) {
        await clearTrackField([track.id], "rating");
      } else {
        await editTracks([track.id], { rating: 5 });
      }
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

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

{#snippet actions(track: LibraryTrack)}
    <!--
      Adding to a playlist is a select rather than a drag. Drag is
      the gesture DJs know, and it will come — but a control that
      works with one hand on a trackpad in a dark booth should not
      wait for it.
    -->
    <!--
      Plain lists only. A folder holds lists, and a smart folder
      holds a query — a track filed into one is a member its own
      filter does not select, so it goes in and never comes back
      out. The store refuses both now; this stops the interface
      offering a target it knows will be refused.
    -->
    {#if playlists.some((p) => p.kind === "list")}
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
        {#each playlists.filter((p) => p.kind === "list") as list (list.id)}
          <option value={list.id}>{list.name}</option>
        {/each}
      </select>
    {/if}
    <button
      class="aside"
      onclick={() => setAside(track.id)}
      title="Set aside in the Sidelist"
      aria-label="Set aside {track.title}"
    >→</button>
    <!--
      A favourite is five stars. See `favourite` for why this writes the
      rating a DJ already sorts by rather than a flag of its own.
    -->
    <button
      class="star"
      class:on={track.rating === 5}
      onclick={() => favourite(track)}
      title={track.rating === 5 ? "Remove from favourites" : "Favourite — five stars"}
      aria-pressed={track.rating === 5}
      aria-label="Favourite {track.title}"
    >{track.rating === 5 ? "\u2605" : "\u2606"}</button>
    <!--
      "More like this" sits beside "set aside" because both are
      things you do *with* a track rather than *to* a deck, and the
      deck buttons at the end of the row should stay a block a hand
      can find without reading.
    -->
    <button
      class="alike"
      class:on={likeThis === track.id}
      onclick={() => showAlike(track.id)}
      title="Find records like {track.title}"
      aria-label="Find records like {track.title}"
    >≈</button>
    {#if selection.kind === "playlist" && "position" in track}
      <button
        class="drop"
        onclick={() => removeAt((track as { position: number }).position)}
        title="Take this out of {selection.name}"
        aria-label="Remove from playlist"
      >−</button>
    {/if}
    <!--
      Somewhere to put it without choosing a deck. Disabled rather than hidden
      when every deck is playing: a control that vanishes mid-set reads as a
      bug, and the reason is in the tooltip.
    -->
    <button
      class="stage"
      onclick={() => void stage(track)}
      disabled={!enabled || freeDeck === null || loading === track.path}
      title={freeDeck === null
        ? "Every deck is playing"
        : `Stage on deck ${freeDeck}, ready to mix into`}
      aria-label="Stage {track.title}"
    >&#8681;</button>
    {#each deckNumbers as deck (deck)}
      <button
        onclick={() => toDeck(track, deck)}
        disabled={!enabled || loading === track.path}
        title="Load onto deck {deck}"
        aria-label="Load {track.title} onto deck {deck}"
      >
        {loading === track.path ? "…" : deck}
      </button>
    {/each}
{/snippet}

<div class="with-sidebar">
<Crates bind:this={crates} bind:selection onchange={() => void refresh()} />

<div class="library">
  <div class="controls">
    <!--
      Absent in the history rather than present and inert. Searching a history
      is a different question — "when did I last play this" — and a box that
      silently does nothing is worse than one that is not there.
    -->
    {#if selection.kind !== "history" && selection.kind !== "duplicates" && selection.kind !== "notes" && selection.kind !== "requests" && selection.kind !== "memory"}
      <input
        type="search"
        placeholder={selection.kind === "playlist" || selection.kind === "smart"
          ? `Search ${selection.name}…`
          : "Search your collection…"}
        bind:value={query}
        oninput={onQuery}
        aria-label="Search the library"
      />
    {:else if selection.kind === "history"}
      <span class="viewing">Everything played, most recent first.</span>
    {:else if selection.kind === "notes"}
      <span class="viewing">What you thought while it was playing.</span>
    {:else if selection.kind === "requests"}
      <span class="viewing">What the room is asking for.</span>
    {:else if selection.kind === "memory"}
      <span class="viewing">A record from half of what you remember.</span>
      <!--
        Named rather than left to the tail: a catch-all here describes whatever
        view is newest as "duplicates", which is how the Requests panel opened
        under a sentence about audio files being in two places.
      -->
    {:else if selection.kind === "duplicates"}
      <span class="viewing">Tracks whose audio is in more than one place.</span>
    {/if}
    <!--
      The two representations of §20. A segmented pair rather than one button
      that toggles: which view you are in should be readable without pressing
      anything, and a single icon that means "you are in the table" and "go to
      cards" at the same time is the control nobody can read at a glance.
    -->
    <!--
      Sorting, where a card view can reach it. The table sorts from its column
      headings and cards have none, so without this the second representation
      would be the one you cannot order -- and the two share `sortBy`, so
      switching views keeps the order you chose. Picking a column here does not
      flip the direction the way clicking a heading twice does: a heading is
      one control that means both, a menu and an arrow are two that mean one
      each.
    -->
    {#if layout === "cards"}
      <label class="sorting">
        <span class="by">Sort</span>
        <select
          value={sortBy}
          onchange={(event) => (sortBy = event.currentTarget.value as Column)}
          aria-label="Sort the collection by"
        >
          {#each COLUMNS as column (column.key)}
            <option value={column.key}>{column.heading}</option>
          {/each}
        </select>
        <button
          class="direction"
          onclick={() => (ascending = !ascending)}
          title={ascending ? "Sorted up — reverse it" : "Sorted down — reverse it"}
          aria-label="Reverse the order"
        >{ascending ? "\u25b2" : "\u25bc"}</button>
      </label>
    {/if}
    <!--
      §20's "instant custom column configuration". Only in the table: a card
      has a shape rather than columns, and offering the control where it does
      nothing is worse than not offering it.
    -->
    {#if layout === "table"}
      <div class="columns-picker">
        <IconButton
          icon="fa-solid fa-table-cells"
          title="Which columns to show"
          aria-label="Which columns to show"
          active={pickingColumns}
          onClick={() => (pickingColumns = !pickingColumns)}
        />
        {#if pickingColumns}
          <div class="chooser" role="group" aria-label="Columns">
            {#each COLUMNS as column (column.key)}
              <label>
                <input
                  type="checkbox"
                  checked={chosenColumns.has(column.key)}
                  onchange={() => toggleColumn(column.key)}
                />
                {column.heading}
              </label>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
    <div class="views" role="group" aria-label="How to show the collection">
      <IconButton
        icon="fa-solid fa-list"
        title="Show as a table"
        aria-label="Show as a table"
        active={layout === "table"}
        onClick={() => choose("table")}
      />
      <IconButton
        icon="fa-solid fa-table-cells"
        title="Show as cards with their sleeves"
        aria-label="Show as cards"
        active={layout === "cards"}
        onClick={() => choose("cards")}
      />
    </div>
    <IconButton icon="fa-solid fa-folder-plus" title="Add folder…" onClick={addFolder} disabled={busy} />
    <IconButton icon="fa-solid fa-repeat" title={busy ? "Scanning…" : "Rescan"} onClick={rescan} disabled={busy || !status?.folders.length} />
    <IconButton icon="fa-solid fa-file-import" title="Import a rekordbox, Traktor or iTunes library export" onClick={() => importFrom(false)} disabled={busy} />
    <IconButton icon="fa-solid fa-compact-disc" title="Import from Serato" onClick={() => importFrom(true)} disabled={busy} />
  </div>

  {#if imported}
    <p class="status">Imported — {imported}</p>
  {/if}

  <!--
    Hidden while the share sheet is open. It reports how many plays there
    are, which the sheet restates as "N of M" beside the records themselves,
    and the panel is short enough that a redundant line costs a control.
  -->
  {#if status && !sharing}
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
      {:else if selection.kind === "duplicates"}
        <strong>{duplicates.length.toLocaleString()}</strong>
        with more than one copy · {status.tracks.toLocaleString()} in your collection
      <span style="display:inline-flex; gap:0.4rem; margin-left:0.4rem;">
        <IconButton icon="fa-solid fa-folder-plus" title="Add folder" onClick={addFolder} disabled={busy} />
        <IconButton icon="fa-solid fa-repeat" title="Rescan" onClick={rescan} disabled={busy || !status?.folders.length} />
      </span>
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

    <!--
      Which folders were scanned is a fact about the collection. In the Journal
      it is one more thing between the DJ and what they wrote, in a panel that
      is short to begin with.
    -->
    {#if status.folders.length > 0 && selection.kind !== "notes" && selection.kind !== "requests" && selection.kind !== "memory"}
      <div class="folders">
        {#each status.folders as folder (folder)}
          <span class="folder">
            <span class="path" title={folder}>{LTR}{folder}</span>
            <IconButton
              icon="fa-solid fa-xmark"
              title="Stop watching this folder"
              aria-label={`Stop watching ${folder}`}
              onClick={() => removeFolder(folder)}
            />
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
            <IconButton icon="fa-solid fa-floppy-disk" title="Save filter" onClick={saveFilter} />
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

  {#if selected.size > 0}
    <!--
      Only while something is selected. A row of tag fields sitting above an
      unselected table is an invitation to a mistake, and a DJ who has not
      chosen anything has not asked to change anything.
    -->
    <div class="batch">
      <span class="count">{selected.size} selected</span>
      <input placeholder="Genre" bind:value={edit.genre} aria-label="Genre" />
      <input
        type="color"
        bind:value={edit.colour}
        aria-label="Colour"
        title="Colour these tracks"
      />
      <select bind:value={edit.rating} aria-label="Rating">
        <option value="">Rating…</option>
        {#each [0, 1, 2, 3, 4, 5] as stars (stars)}
          <option value={String(stars)}>{"★".repeat(stars) || "none"}</option>
        {/each}
      </select>
      <IconButton icon="fa-solid fa-check" title="Apply edits" onClick={applyEdit} />
      <IconButton icon="fa-solid fa-eraser" title="Empty the genre on these tracks" onClick={() => clearField("genre")} />
      <IconButton icon="fa-solid fa-palette" title="Clear colour" onClick={() => clearField("colour")} />
      <IconButton icon="fa-solid fa-ban" title="Deselect" onClick={() => selected.clear()} />
    </div>

    <!--
      What these records are *for*.

      Its own row rather than another field in the one above, because it is a
      different kind of edit: the row above writes what a record *is* -- genre,
      colour, a rating -- and this writes a judgement about when to play it.
      Folding them together would also mean a DJ colouring eight tracks
      silently replaced their functions, since this call sends the whole set.

      Every function is offered even at zero, with its count: a picker that
      hides what you have never used never suggests using it, and "four openers
      and eighty peaks" is the sort of thing nobody notices until it is
      counted.
    -->
    {#if functions.length > 0}
      <div class="batch functions" role="group" aria-label="What these records are for">
        <span class="count">For</span>
        {#each functions as fn (fn.slug)}
          <IconButton
            active={chosenFunctions.has(fn.slug)}
            title="{fn.label} — {fn.about} ({fn.count} in your collection)"
            aria-pressed={chosenFunctions.has(fn.slug)}
            onClick={() => {
              if (chosenFunctions.has(fn.slug)) chosenFunctions.delete(fn.slug);
              else chosenFunctions.add(fn.slug);
            }}
          >
            {fn.label}
          </IconButton>
        {/each}
        <IconButton
          icon="fa-solid fa-check"
          title="Set these {selected.size} records to exactly what is lit. Unticking one removes it."
          onClick={applyFunctions}
          disabled={busy}
        />
      </div>
    {/if}
  {/if}

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if selection.kind === "duplicates"}
    {#if duplicates.length === 0}
      <p class="empty">
        No duplicates. Two files count as duplicates when they hold
        byte-for-byte the same audio — the same recording as a FLAC and as an
        MP3 made from it does not, and correctly so: a cue placed on one is
        milliseconds out on the other.
      </p>
    {:else}
      <p class="hint">
        Removing a copy here only forgets it. Delete the file yourself first —
        nothing in djmanzo deletes your music.
      </p>
      <div class="table-scroll">
        <table>
          <thead>
            <tr><th>Title</th><th>Artist</th><th>Copies</th></tr>
          </thead>
          <tbody>
            {#each duplicates as dup (dup.id)}
              <tr>
                <td class="title">{dup.title}</td>
                <td>{dup.artist}</td>
                <td>
                  {#each dup.paths as copy (copy)}
                    <div class="copy">
                      <span class="path" title={copy}>{LTR}{copy}</span>
                      <button
                        onclick={() => forgetCopy(dup.id, copy)}
                        title="Forget this copy. The file is not touched."
                      >Forget</button>
                    </div>
                  {/each}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {:else if selection.kind === "notes"}
    <Journal {enabled} />
  {:else if selection.kind === "requests"}
    <!--
      A request handed to the search box, and the view switched to the
      collection: the whole point of "Find it" is that the next thing the DJ
      does is look at records, not at the list they just read.
    -->
    <Requests
      {enabled}
      onFind={(text) => {
        query = text;
        selection = { kind: "all" };
        // Setting the box is not searching it: the query only runs from the
        // input's own handler, so a request handed over this way filled the
        // field and left the table showing everything.
        void refresh();
      }}
    />
  {:else if selection.kind === "memory"}
    <Memory
      {enabled}
      onFind={(text) => {
        query = text;
        selection = { kind: "all" };
        void refresh();
      }}
    />
  {:else if selection.kind === "history"}
    {#if sessions.length > 0}
      <div class="sessions">
        {#each sessions as session (session.id)}
          <button
            class="session"
            class:chosen={sharing === session.id}
            aria-pressed={sharing === session.id}
            onclick={() => (sharing = sharing === session.id ? null : session.id)}
            title="Share or export {session.id}"
          >
            {session.id} · {session.tracks}
          </button>
        {/each}
      </div>
    {/if}
    {#if sharing}
      <!--
        Selecting a night opens the share sheet rather than a file dialog. The
        chip used to export straight to disk, which was one click fewer and
        gave the DJ no sight of what they were handing over and no choice of
        where it went.

        The sheet takes the table's place rather than sitting above it. A
        browser panel with the decks open is about three hundred pixels tall,
        and a sheet stacked on top of a table put its own buttons below the
        window edge with nothing to scroll. Replacing it costs nothing: the
        preview *is* the tracklist, so the table underneath was showing the
        same records twice.
      -->
      <ShareSet session={sharing} onclose={() => (sharing = null)} />
    {:else if history.length === 0}
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
        {#if musicFolder && !status?.folders.length}
          <!--
            The one thing a first launch needs. Every operating system already
            knows where music lives, so this is a click rather than a hunt
            through a file dialog for a folder you did not choose the location
            of.
          -->
          Nothing here yet.
          <button class="offer" onclick={() => scanFolder(musicFolder!)} disabled={busy}>
            Scan {musicFolder}
          </button>
          <span class="or">or add a folder of your own.</span>
        {:else}
          No music yet. Add a folder to get started.
        {/if}
      {/if}
    </p>
  {:else}
    {#if likeThis}
      <!--
        Says what is being shown and how to leave. Without it the table has
        silently become a different list, which is the worst thing a browser
        can do to somebody who looked away for a moment.
      -->
      <p class="alike-banner">
        Records like <strong>{likeThisTitle}</strong>
        <button onclick={() => (likeThis = null)}>show everything</button>
      </p>
    {/if}
    {#if layout === "cards"}
      <!--
        §20's second view. The same rows as the table — the same sort, the same
        search, the same selection, the same actions — laid out so the sleeve
        does the finding. A card is operational: everything the row offers is
        on it, from one snippet, so the two views cannot drift apart.
      -->
      <ul class="cards">
        {#each sorted as track (track.id)}
          <li
            class="card"
            class:unanalysed={!track.analysed}
            class:picked={selected.has(track.id)}
            draggable="true"
            ondragstart={(event) => startDrag(event, track)}
          >
            <div class="sleeve">
              <!--
                The lettering is underneath rather than instead of. A missing
                sleeve is the ordinary case in a part-tagged collection, and
                swapping the image out after it fails would flash a broken-image
                icon across the grid on every scroll.
              -->
              <span class="lettering" aria-hidden="true">{initials(track)}</span>
              {#if !sleeveless.has(track.id)}
                <img
                  src={coverUrl(track.id)}
                  alt=""
                  loading="lazy"
                  onerror={() => sleeveless.add(track.id)}
                />
              {/if}
              {#if track.colour}
                <span class="swatch" style="background: {track.colour}"></span>
              {/if}
              <input
                class="pick"
                type="checkbox"
                checked={selected.has(track.id)}
                onchange={() => toggleSelected(track.id)}
                aria-label="Select {track.title}"
              />
            </div>
            <div class="what">
              <strong class="title" title={track.path}>{track.title}</strong>
              <span class="who">{track.artist}</span>
              <!--
                Blank rather than zero where the analyser has not run, for the
                same reason the table's cells are: a plausible 0.0 is read at a
                glance and believed.
              -->
              <span class="facts mono">
                {track.bpm != null ? track.bpm.toFixed(1) : "—"}
                · {track.key ?? "—"}
                · {formatTime(track.duration_seconds)}
              </span>
              {#if likeThis && alikeWhy[track.id]?.length}
                <span class="why">
                  {#each alikeWhy[track.id].slice(0, 2) as reason (reason)}
                    <span class="reason">{reason}</span>
                  {/each}
                </span>
              {/if}
            </div>
            <div class="does">{@render actions(track)}</div>
          </li>
        {/each}
      </ul>
    {:else}
      <div class="table-scroll">
        <table>
          <thead>
            <tr>
              <th class="pick"></th>
              {#each shown as column (column.key)}
                <th class:mono={column.numeric}>
                  <button
                    class="sort"
                    class:active={sortBy === column.key}
                    onclick={() => sort(column.key)}
                  >
                    {column.heading}{#if sortBy === column.key}<span class="arrow">{ascending ? "▲" : "▼"}</span>{/if}
                  </button>
                </th>
              {/each}
              <th class="load-heading">Load</th>
            </tr>
          </thead>
          <tbody>
            {#each sorted as track (track.id)}
              <tr
                class:unanalysed={!track.analysed}
                class:picked={selected.has(track.id)}
                draggable="true"
                ondragstart={(event) => startDrag(event, track)}
              >
                <td class="pick">
                  <input
                    type="checkbox"
                    checked={selected.has(track.id)}
                    onchange={() => toggleSelected(track.id)}
                    aria-label="Select {track.title}"
                  />
                </td>
                <!--
                  One cell per chosen column, drawn from the same list the
                  headings come from, so a header and its cells cannot drift
                  apart — which is what happens the moment they are two lists.
                -->
                {#each shown as column (column.key)}
                  <td
                    class:title={column.key === "title"}
                    class:mono={column.numeric}
                    title={column.key === "title" ? track.path : undefined}
                  >
                    {#if column.key === "title"}
                      <!--
                        The colour is a stripe rather than a filled row: a DJ
                        colours tracks to find them at a glance, and a table of
                        six saturated rows is harder to read than one with six
                        marks down its edge.
                      -->
                      {#if track.colour}
                        <span class="swatch" style="background: {track.colour}"></span>
                      {/if}{track.title}<!--
                        Why this record is here, under its name and only while
                        the question is being asked. In the ordinary collection
                        view these would be noise on every row; in an answer
                        they are the answer.
                      -->{#if likeThis && alikeWhy[track.id]?.length}<span class="why"
                        >{#each alikeWhy[track.id].slice(0, 2) as reason (reason)}<span
                          class="reason">{reason}</span
                        >{/each}</span
                      >{/if}
                    {:else}{column.show(track)}{/if}
                  </td>
                {/each}
                <td class="load">
                  {@render actions(track)}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
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

  .batch {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-wrap: wrap;
    padding: 0.3rem 0.4rem;
    background: var(--panel-raised);
    border-radius: 5px;
    font-size: 0.85em;
  }

  /* The genre field, which carries no explicit type. */
  .batch input:not([type]) {
    width: 8rem;
  }

  .batch .count {
    color: var(--text-dim);
    white-space: nowrap;
  }

  /* Reads as a statement about the table, not a control bar above it. */
  .alike-banner {
    margin: 0 0 0.4rem;
    font-size: 0.82em;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--text-dim);
  }

  .alike-banner button {
    font-size: 0.9em;
    padding: 0.1rem 0.45rem;
  }

  .alike.on {
    border-color: var(--accent);
    color: var(--accent);
  }

  .why {
    display: block;
    margin-top: 0.1rem;
    font-size: 0.72em;
    font-weight: 400;
    color: var(--text-dim);
  }

  /* Chips, not prose: read at a glance beside a name, never a sentence. */
  .reason + .reason::before {
    content: " · ";
  }

  .sessions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .session {
    font-size: 0.8em;
    padding: 0.15rem 0.45rem;
  }

  /* The open sheet belongs to one chip, and the chip says which. */
  .session.chosen {
    border-color: var(--accent);
    color: var(--accent);
  }

  .copy {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .copy button {
    padding: 0 0.35rem;
    font-size: 0.85em;
  }

  th.pick,
  td.pick {
    width: 1.6rem;
    text-align: center;
    padding-right: 0;
  }

  tbody tr.picked td {
    background: color-mix(in srgb, var(--accent-2) 18%, transparent);
  }

  .swatch {
    display: inline-block;
    width: 0.5rem;
    height: 0.85em;
    border-radius: 2px;
    margin-right: 0.35rem;
    vertical-align: -0.1em;
  }

  .aside {
    padding: 0.05rem 0.35rem;
    font-size: 0.9em;
  }

  .add-to {
    font-size: 0.85em;
    padding: 0.05rem 0.1rem;
    max-width: 3.5rem;
  }

  /* ---------------------------------------------------------------------
     §20's two representations
     --------------------------------------------------------------------- */

  /*
    Both buttons are always there and the current one is lit, so which view you
    are in is readable without pressing anything -- a single button that
    toggled would have to mean "you are in the table" and "go to cards" at
    once. `flex-shrink: 0` because they sit beside a search box that takes the
    rest of the row: without it they were squeezed to an eight-pixel sliver,
    which is what driving this actually looked like.
  */
  .views {
    display: flex;
    gap: 0.2rem;
    flex-shrink: 0;
  }

  .columns-picker {
    position: relative;
    flex-shrink: 0;
  }

  /*
    Over the rows rather than pushing them down. A control bar that changed
    height when opened would move the table under a hand reaching for it,
    which is the reflow §18 exists to prevent.
  */
  .chooser {
    position: absolute;
    top: calc(100% + 0.25rem);
    right: 0;
    z-index: 10;
    display: grid;
    grid-template-columns: repeat(2, minmax(6.5rem, 1fr));
    gap: 0.1rem 0.6rem;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    background: var(--panel-raised);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    white-space: nowrap;
  }

  .chooser label {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.85em;
    cursor: pointer;
  }

  /* Beside the view buttons, and no wider than it needs to be. */
  .sorting {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    flex-shrink: 0;
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .sorting select {
    font-size: 1em;
    padding: 0.1rem 0.2rem;
  }

  .sorting .direction {
    padding: 0.1rem 0.35rem;
    font-size: 0.9em;
  }

  /*
    `auto-fill` rather than a fixed count: this panel docks along the bottom of
    a laptop screen and down the side of a booth monitor, and a grid that
    insisted on four columns would be four slivers in the narrow one. 9.5rem is
    the width at which a sleeve is still recognisable and its buttons still fit
    on one row.
  */
  .cards {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(9.5rem, 1fr));
    gap: 0.6rem;
    /* Scrolls inside itself, so the controls above stay reachable. */
    overflow-y: auto;
    flex: 1;
    min-height: 0;
    /* Short collections stay at the top instead of stretching to fill. */
    align-content: start;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    min-width: 0;
    padding: 0.4rem;
    border-radius: 6px;
    background: var(--panel-raised);
  }

  .card.picked {
    outline: 1px solid var(--accent-2);
  }

  /* The same statement the table makes about a record nothing has analysed. */
  .card.unanalysed .facts {
    color: var(--text-dim);
  }

  .sleeve {
    position: relative;
    aspect-ratio: 1;
    border-radius: 4px;
    overflow: hidden;
    background: var(--panel);
    display: grid;
    place-items: center;
  }

  /*
    Over the lettering rather than instead of it -- see the markup. `cover`
    because sleeves are square and scans are not, and a letterboxed grid reads
    as broken.
  */
  .sleeve img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .lettering {
    font-size: 1.5rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--text-dim);
  }

  /* The DJ's own marking, kept where it cannot be mistaken for the sleeve. */
  .card .swatch {
    position: absolute;
    left: 0.25rem;
    bottom: 0.25rem;
    width: 0.4rem;
    height: 1rem;
    margin: 0;
  }

  .sleeve .pick {
    position: absolute;
    top: 0.2rem;
    right: 0.2rem;
    margin: 0;
  }

  .what {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  /*
    Two lines, then an ellipsis. A card grid whose rows are different heights
    because one record has a long remix title is much harder to scan than one
    that clips.
  */
  .card .title {
    font-size: 0.85em;
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .who,
  .facts {
    font-size: 0.78em;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .does {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.15rem;
    margin-top: auto;
  }

  .does button {
    padding: 0.05rem 0.3rem;
    font-size: 0.85em;
  }

  /* A star is a state, so it says which one it is in. */
  .star.on {
    color: var(--accent-2);
    border-color: var(--accent-2);
  }

  /*
    Takes whatever the crate tree and the side view leave. Without `flex: 1` it
    sized to its own content, so on a wide window the track table stopped
    mid-panel and the rest was empty -- the columns a DJ actually reads
    (title, artist, BPM, key) squeezed while there was room going spare.
  */
  .library {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    flex: 1;
    min-width: 0;
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
  /* The first-run offer: a button that reads as the obvious next thing. */
  .offer {
    margin: 0 0.35rem;
    font-weight: 600;
  }

  .or {
    color: var(--text-dim);
  }

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
