# Roadmap

Nine core milestones, plus a six-milestone assistant track. The ordering rule is simple: **every milestone ends with something you can
actually use.** M0 plays a file. M2 is a playable two-deck mixer. Nothing is a six-month
foundation with no output.

Feature-to-milestone assignments are in [FEATURES.md](FEATURES.md). Architecture is in
[ARCHITECTURE.md](ARCHITECTURE.md).

---

## M0 — Foundations and walking skeleton

Prove the stack end to end before building anything on it.

- Cargo workspace with the crate skeleton from [ARCHITECTURE.md §10](ARCHITECTURE.md#10-crate-map).
- Tauri 2 shell, Svelte 5 + TypeScript UI, dev loop working on both platforms.
- CI: build + test on **macOS arm64 and Ubuntu**, from the first commit.
- `dj-audio`: device enumeration, open an output stream via `cpal`.
- `dj-decode`: decode a file with `symphonia` into a ring buffer.
- `dj-engine`: one deck, one output, play/pause.
- `dj-control`: Action enum and bus, `ParameterRegistry` skeleton, `rtrb` bridge to the engine.

**Done when:** on both macOS and Xubuntu, you can pick an audio device in the UI, load a file,
press play, and hear it — with the audio thread doing no allocation, verified by an
allocation-denying allocator in the test build.

---

## M1 — Two-deck core

The first genuinely usable build.

- Two decks with independent transport, pitch fader, pitch bend, keylock
  (Signalsmith Stretch), and cue behaviour.
- Mixer: channel faders, crossfader with curve, 3-band EQ + kills, filter, gain, VU, limiter.
- Routing: main, booth, headphone PFL with cue/master blend and split-cue.
- Audio setup: 4-channel single device **and** dual-device with clock-drift correction.
  The correction is a control loop on the queue depth between the two callbacks, with the
  resampling ratio clamped and heavily smoothed; verified by simulating a twenty-minute set
  at ±50 and ±400 ppm. **Not yet verified against two real sound cards.**
- `dj-render`: scrolling waveform tiles rasterised on the CPU and served as PNG over a
  custom URI scheme, CSS-transform scrolling in the UI. (`wgpu` was the original plan;
  measurement said the CPU is fast enough and a GPU context is not free — see ADR-0004.)
- Light and dark themes.

**Done when:** you can beat-match two tracks by ear and mix them, cueing in headphones, on both
platforms — and the waveform holds 60 fps on real Xubuntu hardware. That benchmark is a gate,
not a nice-to-have: if the webview cannot do it, this is where we take the native-window escape
hatch, not at M7.

---

## M2 — Analysis and sync

- `dj-analysis`: onsets, BPM, beat grid with confidence, key (standard + Camelot), EBU R128
  loudness, multi-resolution waveform data with spectral colouring.
- **Labelled regression set** of tracks with hand-verified grids; beat-tracking accuracy scored
  in CI from here on.
- Beat grid overlay, editable — shift, scale, tap.
- Sync (tempo + phase), quantize, beat jump. Low grid confidence disables auto-sync instead of
  silently misbehaving.
- Hot cues, manual and auto loops, loop adjust/move/halve/double, saved loops.
- Overview waveform, stacked/parallel multi-deck view, zoom.
- 4 decks.
- Analysis cache keyed by content hash.

**Done when:** load an unknown track, get a correct grid and key without touching anything, and
sync it. Regression suite green.

### Where M2 actually stands

Done: `dj-analysis` itself, and the wiring — analysis runs on a worker at load, the result is
cached by content hash on disk, BPM and Camelot key appear in the deck header, the rejected
octave is offered as a one-click alternative, and auto-gain trims each track to the reference
loudness through the action bus (so it lands in the session log and can be overridden).
A weak grid is shown but marked, and `sync_worthy` is false, so nothing can auto-sync to it.

The beat grid is drawn too — rasterised into the tiles in the same pass as the waveform, so
the two cannot disagree, with every fourth beat emphasised, individual beats dropping out
before the lines merge into a wash, and a low-confidence grid drawn faintly rather than
hidden (you can only correct a grid you can see).

Sync, quantize and beat jump are in. Sync picks its leader automatically — the deck already
playing is the one the room hears and the one that must not move — matches tempo (offering half
and double, so a 70 BPM track meets a 140 one at 70 rather than being stretched to double
speed), aligns phase once, and then *holds* the tempo as the leader's pitch fader moves.
A grid below the confidence threshold refuses to sync in either direction, and the button is
disabled rather than silently failing. Quantize snaps beat jumps to the grid.

Hot cues and loops are in. Eight cues per deck, set and jumped from one pad the way a
controller sends them, cleared on right-click, and honouring quantize. Loops come in three
forms — auto loops in beats, manual in/out (which needs no beat grid at all, because a loop is
stored in frames), and halve/double/move — and the playhead is folded back into the loop on
**every frame** of both render paths, because at a sixteenth of a beat a loop is shorter than
one callback and folding per block would play straight past it. Cue markers and the loop band
are DOM inside the scrolling strip rather than rasterised into the tiles: there are at most
nine of them, they ride the same transform for free, and moving one costs no tile re-render.

Four decks and the overview waveform are in. The engine always ran four; the interface now
shows two by default and four on a click, wrapping to two rows rather than four columns
(a quarter-width deck cannot hold eight pads and two jump rows and still be readable at arm's
length in a dark booth). The overview is the *same* Rust renderer at a different zoom — one
tile spanning the whole track — so it inherits the beat grid, the spectral colouring and the
theme for free, with cue markers and the loop band as percentage-positioned DOM over it.
Its tile width is quantised to 32 px so that dragging the window does not mint several hundred
of the most expensive tiles in the application.

Two things the overview exposed and fixed. The grid's minimum line spacing went from 6 px to
14 px: at the overview's zoom a 105 BPM track puts beats 7 px apart, which is arithmetically
"not overlapping" and visually a picket fence that hides exactly the structure an overview
exists to show. The floor only bites at the zoomed-out end — the scrolling lane's beats are
80–100 px apart — so bar lines carry the phrasing there on their own. And the playhead
interpolation between snapshots was assuming a 48 kHz device rate; it now derives the track's
own rate from its length in frames and seconds, so a 44.1 kHz track no longer runs 8.8% fast
between one snapshot and the next.

Per-channel **crossfader assignment** came with the fourth deck, because it had to: a fixed
"deck 1 left, deck 2 right" mapping leaves decks 3 and 4 permanently outside the one control
a DJ uses without looking. Each deck now carries an A / — / B switch, defaulting to the
convention (1 left, 2 right, the rest through), and *through* means full gain rather than the
curve's midpoint — a deck brought in mid-set should be audible when its channel fader is up,
not attenuated because the crossfader happens to be parked.

Grid editing is in. Six verbs — put a beat on the playhead, nudge in milliseconds, scale the
tempo, set it outright, tap it in, and reset to what the analyser found — reachable from a row
under each deck and, because they are actions rather than commands, from a controller, a
script and the assistant too. Every edit marks the grid certain: the DJ looked at the waveform
and said where the beat is, which outranks a correlation score and is the entire point of
editing a grid the analyser was unsure of.

Three decisions worth recording. The edits are computed in the host rather than on the audio
thread, because editing needs the analyser's original to reset to and a run of taps to
average, and neither belongs in a callback that must not allocate; the result reaches the
engine as `SetGrid`, which is the same path the analyser's own finding takes, so there is
still exactly one place a grid is decided. Taps are recorded as *playhead* positions rather
than wall-clock times, which is what makes them survive a pitch-fader move and stay meaningful
on a paused deck. And the deck header now reads its confidence live from the engine instead of
from the cached analysis — otherwise a grid the DJ had just fixed would still be labelled
"weak" beside an enabled Sync button.

Saved loops landed with M3's persistence.

The **labelled regression set** is half done, and the half that is missing is the half this
repository cannot supply. The corpus needs licensed music and a human who can hear whether a
downbeat is on the downbeat; it can be collected, not written. Everything *around* the corpus
now exists as `dj_analysis::regression`: the label format, the metrics, and the scoring.

The metrics keep two accuracies apart on purpose. **Exact** is the estimate within 2% of the
truth; **octave-tolerant** also accepts a half, double or triplet relation. Autocorrelation
genuinely cannot tell 80 from 160 — a curve periodic at one is periodic at the other — so an
octave error costs the DJ one click on the alternative the analyser already offers, while a
tempo that is neither means the grid is noise. A single blended score would hide exactly the
distinction that decides what to fix. Declines are counted separately again, because *an
analyser that is confidently wrong is worse than one that says it does not know* is this
crate's founding rule and adding the two together would erase it.

`best_confidence_threshold` is what closes the loop: it sweeps the confidences a run produced
and returns the cut maximising Youden's J, which is the calibration
`CERTAIN_CORRELATION` in `crates/dj-analysis/src/tempo.rs` has been waiting for — currently
interpolated between a synthetic click track (0.95) and white noise (0.014) with nothing real in
between. It returns `None` when the corpus is all one class, rather than a number somebody would
then act on.

---

## M3 — Library

- SQLite schema, content-hash track identity, tags via `lofty`.
- Browser: folder tree + song list + **SideView** (Sidelist, Sampler placeholder, Automix
  placeholder, Clone).
- Instant search, sortable columns, colour-coding, filtering, batch tag editing.
- Playlists, crates, smart folders. Play history, session export, duplicate detection.
- Importers: rekordbox (`rekordcrate`), Serato (clean-room), Traktor NML, iTunes XML, folders.
- Harmonic key display and compatible-key filtering.
- Layout presets (Starter / Essentials / Pro / Performance) and the skin system.

**Done when:** a DJ can import an existing rekordbox or Serato library with cues and grids
intact, and find any track in their collection in under a second.

### Where M3 actually stands

The `dj-library` crate is in: the schema, migrations, track identity, tag reading, folder
scanning, and instant search.

**Identity is the audio, not the file.** A track's primary key is the BLAKE3 hash of its
decoded samples, so moving or renaming a file keeps its cues, its corrected grid and its play
history; two copies in different folders are one row; and the same recording as FLAC and as an
MP3 made from that FLAC are two rows, correctly — a cue placed on one is milliseconds out on
the other. This also retired the FNV-based hash `dj-decode` had been using: four interleaved
64-bit lanes widened to 32 bytes is fine for a cache key, where a collision costs one wasted
re-analysis, and not fine for identity, where a collision puts one track's cues under
another's waveform.

**A scan is two halves, and only the cheap one is synchronous.** Identifying a track costs a
full decode — seconds per file, hours for a real collection — so a scan walks the folders,
reads the tags and records what it found, which takes seconds and leaves the collection
browsable and searchable immediately. Identification then runs in the background, promoting
rows out of a `pending_files` staging table into `tracks` as it goes. A rescan compares path,
size and modification time, so it re-reads only what actually changed; a file that fails to
decode is recorded with a reason rather than retried forever, and a later edit clears the
failure. A symlink loop is walked once, not indefinitely.

Search is FTS5 over the tags, with every word turned into a prefix match so it narrows as you
type. Punctuation in the search box is stripped rather than escaped: FTS5 treats `"`, `*`,
`-` and `:` as syntax, and a DJ typing `AC/DC` means a band, not a query.

The schema also carries the tables the rest of M3 needs — playlists as one tree covering
lists, folders and smart queries; playlist membership with position in the key, because a
playlist is a sequence and not a set; play history; and saved loops, which closes the one M2
item that was waiting for somewhere to put it.

The library is wired into the application. It opens in the *config* directory rather than the
cache one — a cache is something the system may delete to reclaim space, and a DJ's cues,
corrected grids and play history are not that — and a background worker drains the identify
queue one file at a time, decoding, hashing, analysing and promoting each into `tracks`.

Analysis happens during identification rather than at load. The expensive part is having the
decoded samples in memory and they already are, so leaving it until a track reaches a deck
would mean decoding the same file twice — and, worse, a DJ who imported their collection last
night would still have no BPM or key to sort by this evening. A library you cannot sort by
tempo is most of the reason to have one.

The browser is a tab alongside the sources search, and is the one that opens first: a
streaming search is what you do when your own crate does not have it. It is a dense sortable
table rather than a list of cards, sorted client-side because the rows are already there, with
search debounced at 120 ms and nulls sorted last in every column — an unanalysed track has no
BPM, and burying those among the 60s and the 180s would be worse than keeping them together
where they can be seen.

Three bugs that only showed up against real audio. `upsert_track` deliberately never writes
the analysis columns, so that a rescan reading tags off disk cannot erase a grid the DJ
corrected — which meant promotion silently dropped the analysis identification had just done,
and the collection filled up with tracks that never got a BPM. Promotion now writes analysis
too, in the same transaction, but only where there is none, so a second copy of the same audio
turning up in another folder cannot replace a corrected grid with a fresh guess. Silence
measures as negative infinity, which `serde_json` refuses, so one silent track would have
broken the whole browser payload; it stores as absent. And the panel polled the track *count*
without re-reading the rows, so a DJ would have watched the number climb to four thousand over
an empty table.

Verified end to end against generated click tracks at 90, 120 and 140 BPM: scan, identify,
analyse, store, serve and display, reading back 90.1, 120.1 and 139.9.

Cues, grids and saved loops now belong to the track. Set a cue, correct a grid, save a loop —
take the record off and put it back next week, and they are where you left them.

They reach the library by two routes, because they are two shapes of event. A **grid edit** is
a discrete action with a known result, so it is written where it happens. A **hot cue** is set
by the *engine*, at a playhead quantize may have moved, so the host cannot know where it landed
until the audio thread has published it — cues are noticed by watching the snapshot, which is
the one place that reads engine state after the fact. The comparison is eight optional floats
per deck per tick; anything that moved goes to a writer thread, because the snapshot pump runs
at 60 Hz and a stalled pump is a frozen interface.

Three things that had to be right. Ejecting is not "the DJ cleared every cue" — the watcher
forgets the deck rather than writing an empty set, or a track would lose its cues every time it
left a deck. A freshly loaded track is not a change either: its cues have just come *out* of
the library, and on the tick before the restore reaches the engine they are still empty.
And restoring replaces the whole set rather than filling slots, or the previous track's cues
survive in the slots this one does not use. Each has a test, and each test was checked by
breaking the feature to confirm it fails.

Loading a file now also adds it to the library. Partly because a cue row has a foreign key to
its track, so without it every cue set on a file opened from disk would be silently discarded —
and partly because a track you played is part of your collection whether or not you ever
pointed a scan at the folder it lives in.

**Saved loops are in**, which closes the last M2 item. `loop_save` and `loop_recall` take a
slot; the region is read from the registry rather than from the action, because quantize may
have snapped it and the loop the DJ can hear is the snapped one. Four pads under each deck —
click recalls, shift-click saves — because the destructive gesture should be the deliberate
one. A saved loop belongs to the track, so one saved on deck 1 recalls on deck 2.

Playlists and history are in. Playlists, crates and folders are one table with a `kind`,
because to a DJ they are the same gesture — a named thing in a sidebar holding either tracks or
other named things — and the difference between rekordbox's playlists-and-folders and Serato's
crates-and-subcrates is only which one may contain the other. The sidebar builds the tree from
a flat list with parent ids: nesting it in Rust would mean rebuilding the structure on every
change and turning "move this node" into a rewrite.

Positions in a playlist are ordered but not contiguous. A track can appear twice — DJs do that
on purpose — so removal names a *position* rather than a track, and appending is one insert
rather than a renumber. Only an explicit reorder pays for rewriting the list. Moving a folder
inside itself or inside its own child is refused rather than performed: the rows would survive
and nothing in the sidebar could reach them.

**A play is recorded when the track was played, not when it was loaded.** Thirty seconds, or a
quarter of the track if it is shorter — a DJ auditions constantly, and a history full of
four-bar previews is one nobody can read. Measured from the playhead rather than from elapsed
time, because the playhead is what the room heard: a deck parked at the drop for five minutes
has not been played, and one started from a cue point has. Pausing does not re-arm the count,
which was a real bug caught in testing — reporting no track while paused made the watcher
forget, so every pause and resume would have been another row.

Three interface details worth keeping. The search box is *absent* in the history rather than
present and inert, because searching a history is a different question and a box that silently
does nothing is worse than one that is not there. The count above the rows says what is on
screen first — "2 in Peak · 3 in your collection" — since the collection count above a two-row
playlist reads as a bug however true it is. And `.entry { flex: 1 }`, written for a row sharing
a horizontal line with a delete button, also applied to the two top-level sidebar buttons in a
*column* flex container, which stretched "All tracks" into a block and squeezed the tree out of
the panel.

Smart folders are in, with a filter language of their own:

```
bpm > 120 and key compatible 8A
artist = "Juan Luis Guerra" or (year >= 1990 and rating >= 4)
not genre contains reggaeton
```

**A language rather than stored SQL.** A smart folder needs a condition that outlives the
session, which means storing it — and storing SQL would make the database file an executable
script, so anything that could write a playlist row could run statements against a DJ's
collection. The filter parses into a typed tree and *compiles* to a parameterised `WHERE`
clause; every value the DJ typed leaves as a bound parameter, and there is a test that a filter
containing `DROP TABLE tracks` matches nothing and drops nothing.

**Absence means two different things, and the language honours both.** An absent *tag* is a
value: a track with no genre genuinely is not reggaeton, and `not genre contains reggaeton`
should find it — so the text columns read through `COALESCE`. An absent *analysis* is an
unknown, and a filter must not assert anything about it: `bpm > 120` cannot be true of a track
nobody has analysed, and neither can `not bpm > 120`. Wrapping the comparison in SQL's `NOT`
would invert the null guard along with the comparison and put every unanalysed track into a
filter for slow ones — a DJ would find a 150 BPM record in their warm-up crate at the worst
possible moment. Negation is pushed down to the leaves by De Morgan instead, so `not bpm > 120`
compiles as `bpm <= 120` and still requires a tempo to exist.

`key compatible 8A` is the one worth having: it matches the key, its relative major or minor,
and one step either way round the Camelot wheel — harmonic mixing as a crate. The filter is
checked as it is typed, so a typo is reported while the DJ is looking at the box rather than
when the folder turns up empty, and it is evaluated on every open rather than cached, because a
smart folder is a question about the collection and a track added since belongs in it.

Three importers are in — **rekordbox XML, Traktor NML and iTunes XML** — chosen by what the
file contains rather than by its extension, because rekordbox and iTunes both write `.xml` and
a DJ who renamed theirs should still get their collection.

**An import is another way of filling the identify queue.** It names tracks by path; our
identity is the hash of the decoded audio, so an import cannot write a track row, a cue row or
a playlist row at the moment it runs. Decoding first would mean a DJ waiting hours before
seeing anything. So an import fills `pending_files` — the same queue a folder scan fills — with
the cues, loops and grid it found riding along as a payload, and records playlist membership by
path. The playlist *tree* is created immediately, because it needs no track ids: the sidebar
fills in at once and the tracks appear underneath as they are identified. A path already in the
library skips the queue entirely and has everything applied on the spot.

Each format's own peculiarities are handled at its own edge, so nothing downstream has to know
where a value came from: Traktor stores cue positions in **milliseconds** and splits paths
across `DIR`/`FILE` with `/:` as the separator; rekordbox counts hot cues from zero and marks a
loop by giving a cue an `End`; iTunes recognises its own built-in playlists by marker keys
rather than by name, since the names are localised and a list of English ones would import
"Musik" as a crate. Keys arrive as `Am`, `A min`, `8A` or a Traktor integer and all reach the
same place; anything unreadable is absent rather than guessed.

**The end-to-end run found the bug that would have made the feature pointless.** Every imported
grid was declined, because the analyser had already run on everything the scan found and the
rule was "never overwrite an existing analysis". The library could not tell a measurement from
a judgement. Grids now record where they came from — analysis, import, or a hand edit here —
and the rule is the obvious one: a hand edit outranks an import, an import outranks an
analysis, and an analysis fills in a blank. Re-analysing may improve an analysis and
re-importing may correct an import, so equal sources replace themselves. A source that knows
the grid but not the key no longer blanks a key the analyser had found.

Cues are the other way round: an import never replaces cues that are already there. We never
invent a cue, so any cue on a track is one the DJ placed, and an import from the software they
left behind must not overwrite it.

**Serato is in too**, written from the format's published structure rather than from
`triseratops`, which is AGPL-3.0-or-later and therefore out of bounds under
[ADR-0002](adr/0002-clean-room-permissive-licensing.md). The format is a tagged-chunk
container — four ASCII bytes, a big-endian length, a payload, repeated — with text as UTF-16
big-endian, and that description is short enough to state in full in the module that
implements it.

Serato is also the one importer that is a *folder* rather than a file: there is no export, only
a `_Serato_` directory. Pointing at that folder, at the music folder containing it, or at the
drive all work, because those are the three places somebody would reasonably click. Nesting is
spelled in the crate filenames — `Latin%%Warm-up.crate` is "Warm-up" inside "Latin" — so the
folders are implied rather than stored, and are created once however many crates name them.
Paths are stored relative to the volume, so `Users/dj/Music/a.flac` regains its leading slash.

A length field is four bytes of input asking for an allocation, so a chunk claiming to be
larger than any real library file stops the read rather than being obeyed; trailing padding,
which real files carry, is ignored rather than costing the library.

Serato's hot cues are **not** in its library — it writes them into the audio files themselves,
as a `GEOB` tag — so they are not something a DJ imports at all. They are something a file
*has*, and they are read at identification, which is the moment a file is first opened and the
moment it first has an id to hang them on. A DJ who never exported anything, and who simply
copies their music onto a new machine, still arrives with their cues.

They go through the same path as an import and answer to the same rules: they fill in a grid
the analyser guessed at, and they never replace cues set here. A payload written deliberately
into a library export outranks one found in a tag, so an import applies first.

Nothing writes markers back. A DJ's music is theirs, and a library that silently rewrites the
tags of every track it touches is one bad release away from a disaster.

M3's own "done when" — *import an existing rekordbox or Serato library with cues and grids
intact, and find any track in under a second* — is met.

**Duplicate detection got back what identity throws away.** A track is one row per distinct
piece of audio and `tracks.path` is where it was last seen — right for playing, since the cues
belong to the music, but it meant the second copy silently replaced the first's path and a DJ
could never learn they had two. Every path a track's audio has been seen at is now remembered
alongside; a track with more than one is a duplicate. Removing one only forgets the library's
memory of it — nothing here deletes anybody's music — and if it was the path the track played
from, the track moves to one it still has rather than pointing at a file nobody owns.

Batch editing sets fields across a selection, where absent means *leave it alone*. Clearing is
its own verb, because "set this to nothing" is a different intention and a single method
expressing both would have to invent a sentinel for one of them. Colour is a stripe down the
edge of a row rather than a filled row: a DJ colours tracks to find them at a glance, and six
saturated rows are harder to read than six marks. The edit bar appears only while something is
selected — a row of tag fields above an unselected table is an invitation to a mistake.

Sessions export as a plain set list with times relative to the first track, since what somebody
reading one wants is how far into the night it was, not the wall clock of a machine in another
time zone.

**A screenshot found the bug worth recording.** Serato's in-file markers are usually cues with
no tempo, and applying one blanked the grid the analyser had already found — the write is an
overwrite, not a merge, and every grid field in a cues-only payload is null. A payload with
nothing to say about the grid now says nothing.

**SideView is a real playlist, not a widget with a list in it.** The Sidelist is stored as a
playlist row flagged `system`, so it survives a restart, reorders through the same code as
every other list, and cannot be renamed or deleted out from under the panel that shows it.
System playlists are hidden from the ordinary playlist listing for the same reason: a DJ
looking at their crates should not find one they did not make. Sampler and Automix are named
placeholders pointing at M5 and M6 rather than dead tabs — an empty tab that says why is
information, and one that just sits there is a bug report waiting to be filed.

**A layout preset and a skin are the same thing.** Building presets as special cases in the
interface and skins as a file format on top would have been two ways of saying one thing,
eventually disagreeing about some detail nobody meant. So the four presets — Starter,
Essentials, Pro, Performance — are ordinary layouts that happen to ship, and a DJ's own layout
is JSON loaded by the same code from `layouts/` beside their presets. Every field has a
default, so a layout file names only what it changes; absurd values are clamped rather than
refused, because a layout is a preference and somebody who wrote `density: 40` wants big text,
not an error; and a malformed file is skipped with a warning rather than costing them the
other nine at the start of a set. A layout says what is on screen and how densely — it cannot
execute anything or change what a control does, which is what makes one somebody sent you safe
to load. The choice is remembered by *name*, not by copying the layout, so a DJ editing their
own file sees the edit take.

**Three layout bugs were only visible once a layout was applied.** The topbar crushed the
Connect button to zero width instead of wrapping — `flex: 1` with `min-width: 0` let the device
group shrink toward nothing, so everything appeared to fit while the one control that matters
before a device is open had no width left. The browser's track table sized to its own content
rather than taking the free width, wasting half the panel on a wide window. And the SideView's
tabs shared width equally regardless of label, clipping the Sidelist's count off its own edge.
None of the three is visible in the code; all three are obvious in a screenshot.

**Drag-and-drop landed in the consolidation pass**, as an addition to the select rather than a
replacement. A drag needs two hands and a surface, cannot be done from a keyboard, and is
invisible to a screen reader; a select is none of those things. Both, always. Dragging a selected
row takes the whole selection, because that is what a DJ who has just ticked eight boxes means;
dragging an unselected row takes only it and leaves the selection alone, because picking
something up should not silently change what was chosen.

The pass also found two bugs in shipped M3 code, both quiet:

**`add_to_playlist` accepted anything.** A folder holds lists and a smart folder holds a *query*,
but neither was refused — and a track filed into a smart folder becomes a row that folder's own
filter will never select, so it goes in and never comes back out. The guard is in the store
rather than only the interface, because the interface is not the only caller: importers, the
assistant and the network API all reach it.

**A looped track was never recorded as played.** The play watcher measured the playhead's
*position*, on the stated reasoning that the playhead is what the room heard — right for a track
cued near its end and played out in twenty seconds, which elapsed time would miss. But the
reverse case is just as common: a DJ looping an intro through a whole transition never moves the
playhead, so the track was silently absent from the set list however long it ran. Either counts
now. The original objection to elapsed time — a deck parked at the drop for five minutes — never
applied, because time only accrues while the deck is *playing*, which was already checked. Found
by querying the database after a run: two decks played, one looping, one history row.

**The crate tree and the browser kept separate copies of the playlist list**, and only the
browser's was refreshed. Importing a rekordbox library with forty playlists put forty rows in
the database and showed none of them in the tree until the panel remounted, so an import looked
like it had half worked.

**M3 is complete.**

---

## M4 — Controllers

The milestone that makes the hardware in your hands work.

- `dj-hid` — **done for MIDI**: `midir` MIDI in, in four layers of which only the last
  touches hardware, so the whole mapping engine is testable on a machine with nothing
  plugged in. `hidapi` HID still to come.
- Mapping engine over the action bus — **done**. TOML mapping files, every action in them
  parsed through `Action::parse` *when the file loads*, so a typo is a message when you
  choose the mapping rather than a control that silently does nothing an hour into a set.
  The consequence worth stating: **a mapping cannot do anything the interface cannot**,
  which is what makes a file from a stranger safe to open. Lua for real logic still to come.
- Inbound — **done** for 7-bit, 14-bit pitch-bend and relative encoders. The encoder
  convention is **declared, not guessed** (`signed`, `offset`, `absolute`): the same byte
  means opposite things in two of them, and reading it blind sent a DJ turning an absolute
  encoder *down* a beat jump *forward*. Touch detection still to come.
- **The keyboard as a controller** — done, and the piece that matters most for a laptop with
  nothing plugged into it. Keys are named by physical position (`KeyQ`, not `q`) so a layout
  holds on AZERTY and QWERTZ. The bundled mapping is 76 keys laid out so the two hands
  mirror each other, tested against its own promises: every key labelled "(hold)" has a
  release, the two decks have the same moves, and nothing takes a Command chord because
  Cmd-Q would quit the application mid-set. Held keys are let go when the window loses
  focus — hold the bass kill, hit Cmd-Tab, and the key-up goes to whatever you switched to.
- Outbound feedback: LEDs, pad colours, displays.
- **Motorized platters** as a first-class control kind — absolute high-res position in, motor
  start/stop ramp and torque out.
- Jog modes: scratch, bend, search. Vinyl vs CDJ mode.
- Controller-specific audio setup presets.
- In-app mapping editor (learn a control, bind an action).
- Mappings — bundled rather than installed, so a fresh install works with nothing
  configured. A user file of the same name in the config directory replaces the bundled one.

**Done when:** a full set can be played from the controller without touching the laptop, and
adding a new controller requires editing a file, not rebuilding the app.

---

## M5 — Performance

- **Pad zone with pages** — done: Cues, Loops, Roll, Slicer, Saved, Sampler, FX. All seven.
- **Sampler** — done: four banks of eight, four trigger modes, tempo sync, per-slot level
  and routing, its own pad page, and recording into a slot from the master or from a deck.
  The deck tap is pre-fader, so a hook can be lifted off a track the room is not hearing yet.
- **FX rack** — done: three chained slots per deck and on the master, pre/post-fader placement,
  beat-synced timing, and chain presets. A chain needed no new kind of preset — a preset is
  already action text and the rack is already in the action vocabulary — only a way to read
  the rack back *out*, which is what `dj_app::rackcapture` does.
- Core built-in effects — echo, delay, reverb, gate, crush, flanger, phaser and filter done.
  Roll shipped as the loop roll, on its own pad page. Brake and backspin are done and live on
  the deck rather than in a slot — they are **transport rather than signal**: they change how
  fast the record turns, not what comes off it.
- **CLAP plugin hosting** — done, as one insert on the master between the effect rack and
  the limiter. CLAP rather than VST for a licensing reason before a technical one: VST3's SDK
  is GPL-or-commercial and VST2's is gone, so ADR-0002 rules both out before the first
  question is asked. CLAP is MIT, its threading model is written down, and a host can drive
  one without a GUI.

  The plugin's own window is **not** hosted, and will not be soon: a plugin's interface is a
  native child window — an X11 window on Linux, an `NSView` on macOS — and there is nowhere
  to put one inside a webview. Parameters are exposed generically instead and djmanzo draws
  them. That is a real loss for a plugin whose panel *is* the product and a real gain
  everywhere else: a generic control is in djmanzo's own vocabulary, so a controller can be
  mapped to it, a preset can save it, and the assistant can move it. A plugin window can do
  none of those.

  The honest caveat: a CLAP plugin's `process` is only *supposed* to be free of allocation,
  locks and I/O. Nothing in the specification enforces it and nothing here can. The
  allocation-counting harness catches a badly behaved plugin the moment one is loaded, but
  only for that plugin on that machine. A DJ loading a third-party plugin into the master
  chain is taking on that risk.

  Also not yet done: a plugin telling the host its parameters have changed — after loading a
  preset in its own window, say — is recorded and not acted on. It matters for a plugin with
  its own preset browser and not at all for one driven from here.

  Tested against a real plugin, not a mock: `dj-clap` compiles a CLAP plugin into its own
  test binary and hosts it through the same path a plugin read off a disk would take. There
  are no `.clap` bundles on a CI machine, and a host tested only against whatever happens to
  be installed is a host tested nowhere.
- **Slip mode, reverse/censor, loop roll** — done. **6 decks** done: the engine always
  builds `MAX_DECKS`, and the interface shows two, four or six. Six is three rows of two
  rather than two of three — deck width is what decides whether pads are readable, and a
  six-deck rig is a scrolling one.
- **Microphone/aux input with ducking** — done. A channel strip rather than a deck: gain,
  a switch, a headphone send, and a ducker sidechained from itself. The input arrives on the
  operating system's own callback, so a lock-free ring carries it to the render thread —
  the mirror image of the recording path, and the same discipline: neither end of that ring
  is ever dropped on an audio thread.

  The part that decides whether talkover is usable is the **hold**. Speech is mostly gaps,
  and a ducker that recovers whenever the microphone falls quiet surges the music back up
  into every pause between two words. Half a second of hold bridges the gaps inside a
  sentence and still lets the music back promptly at the end of one. The music is ducked and
  the microphone is not — a ducker that ducked its own sidechain would be a gate — and the
  headphone mix is never ducked at all, because the DJ already has the voice in their ears
  and pulling the music down there removes the only reference they have.

  Aux is the same strip with talkover switched off, which is why there is one of these and
  not two. Microphone effects are still to come.
- **Automix with configurable transition style** — done. Not a feature of the audio engine:
  nothing in it is realtime and nothing in it touches a sample. It watches where the playing
  track has got to and, at the right moment, sends the same actions a DJ would send by hand.
  Every action it emits already existed in the vocabulary, which is the design — everything
  automix can do, a person could have done, and it takes the same path through `perform` as
  a button press.

  Four styles: **cut** (one stops, the next starts), **fade** (a straight crossfade),
  **blend** (a crossfade with the outgoing bass pulled out as the incoming one arrives —
  the default, because both kicks at once is what makes an automatic mix sound automatic)
  and **echo** (an echo thrown over the outgoing track so it dissolves rather than ends).

  Two decisions worth recording. It moves the **channel faders, not the crossfader**: a
  crossfader only cuts decks assigned to one of its halves, so a crossfader automix would
  work on decks 1 and 2 and silently do nothing on 3 and 4 — and a DJ who parked the
  crossfader hard left would hand over to a system fading in a deck the crossfader is
  already silencing. Automix sets the decks it is using to *through* when it takes over.
  And a transition's progress comes from the **outgoing deck's playhead**, not from
  elapsed wall-clock time: a transition timed off the interface pump would stretch whenever
  the machine got busy, which is exactly when a transition is happening.

  It plays from the **Sidelist**, because a DJ already has somewhere they put what plays
  next. It does not know where a track's outro is — the handover point is the end of the
  file minus the transition length, which is right for a track that ends when the music
  does and wrong for one with a minute of applause on it. Finding the real end is analysis
  work and is not done yet.
- **Recording to disk** — done: `record on` streams the master, post-limiter, into a
  16-bit WAV beside the settings. Lock-free ring out of the callback, dither on the writer
  thread, sizes rewritten every five seconds so a crash costs seconds rather than the file.
  A disk that cannot keep up loses samples and the interface says how many, because the
  audio thread will not wait for it. Icecast/Shoutcast broadcast still to come.
- **Multi-monitor / detachable panels** — done. A DJ with two screens does not want a
  different arrangement on each; they want the same interface, spread out. So a panel is not
  moved by changing a layout — it is taken out of the main window and given one of its own,
  which the desktop's own window management then puts wherever it is dragged. Six panels can
  go: browser, waveforms, effects, sampler, assistant, watershed.

  djmanzo never asks how many screens there are, never positions a window on one, and never
  has to cope with one being unplugged mid-set. It opens a window; the desktop decides where
  windows go. Every attempt to be cleverer than that ends with an application that puts a
  panel on a projector — which is also why *which* panels are detached is remembered and
  *where* they were is not. A saved position is wrong the moment a screen is unplugged, and
  restoring one onto a monitor that is no longer there is how a panel ends up invisible with
  no way to get it back.

  A detached window is the same application: same state, same action bus, same snapshot.
  Tauri's `emit` reaches every window, so a detached waveform is drawn from exactly the same
  sixty-times-a-second stream the main window draws from, and there is no second path to
  keep in step.

### Slip, reverse and censor

One engine concept delivers all three: **a shadow playhead that keeps running at
the track's natural forward rate while something diverts the audible one.**
Slip is that shadow made available; reverse flips the sign of the step; and a
censor is the two composed — momentary reverse that always slips, because
hiding a word *and landing back on the beat* is the whole gesture and it cannot
do the second half without a shadow.

Three decisions the tests pin:

- **Arming slip mid-loop starts the shadow now.** Pretending it had been running
  since the loop began would jump the track forward by however long the DJ had
  been looping before they reached for the button.
- **Disarming slip mid-loop leaves the playhead alone**, because "stay here" is
  what turning slip off means.
- **Releasing a censor inside a loop returns to the loop, not out of it** — the
  loop is still diverting the playhead, so the diversion has not ended.

Allocation-free on both audio paths, proven by `rt_safety.rs` on the direct path
and again on the keylocked one, which advances the shadow per block rather than
per frame.

### Brake and backspin

One state for both, because they are one move with a different push: a brake
starts at full speed and coasts to a stop, a backspin starts four times faster
in the other direction and coasts to the same place.

**Linear decay, not exponential.** A platter slows against roughly constant
friction, so its speed falls in a straight line and it stops at a definite
moment — which is why a brake has a *length* a DJ can put on a beat. An
exponential decay would approach zero and never arrive, and the record would
still be crawling four bars later.

**A coast bypasses keylock.** The sound of a brake is the pitch falling;
keylock exists to stop the pitch falling; a keylocked brake is a brake that does
not brake. The shifter also works on blocks, and a rate that changes every frame
is not something a block-based shifter can follow — so a spinning deck goes down
the direct path whatever keylock says.

**The step is read per frame, not per block.** Once per block a one-beat brake
would fall in about ninety audible steps: a zipper rather than a slowdown.

Two things the tests caught, both worth recording.

The first version of the brake test only checked that each block travelled no
further than the last, and then that the deck had stopped. A mutation removing
the coast multiplier entirely **passed it** — a constant rate satisfies "no
faster than before", and the deck stopped anyway because the coast's own timer
ended it. The test now requires the record to end up crawling compared with how
it started, and the mutation fails it.

That stronger test then found a real bug: when the coast ended part-way through
a block, the spin was cleared and the *remaining frames of that block played at
full speed*. Up to a whole buffer of audio jumping back to pitch as the record
came to rest — five milliseconds at 256 frames, and worse at bigger ones. The
frame loop now stops when the deck does.

### The sampler

Four banks of eight, and a sample is **a deck with almost everything taken
away**: a source, a playhead, a level. It reuses `TrackSource` rather than
inventing a second kind of audio, so a sample is loaded, retired and
interpolated by exactly the code that does those things for a track — and gets
the same fractional reads, which is what lets it follow the tempo.

Four decisions worth keeping:

- **Four trigger modes, and a test that stops there being a fifth.** Two modes
  are the same behaviour if they agree on both questions a mode answers: does
  releasing stop it, and does a second press restart it. `no_two_modes_behave_identically`
  checks every pair differs on at least one, because a mode that duplicates
  another is a choice a DJ has to make for no reason. Hold and stutter differ
  only on the second question — small on paper, the entire effect in practice.
- **A sample with no tempo is never stretched.** Sync is a switch, but a
  two-second vocal stab has no tempo to sync *from*, and stretching it by
  whatever ratio was handy is worse than not stretching it. The switch is hidden
  on such a slot rather than greyed out.
- **A load names its bank.** A file takes a moment to decode, and a DJ who
  switches banks in the meantime should not find their sample in the bank they
  moved to.
- **Banks are a view, not a mute.** A loop keeps running when the DJ switches
  away from its bank — which is what banks are *for*, and also why `stop_all`
  exists: eight loops running with no way to stop them in one gesture is a
  sampler that will one day be the loudest thing in the room.

One bug the restructuring caught: the sampler's own level was being applied to
the whole main bus, decks included, because the mixing loop had slots outside
and frames inside — and by then the decks had already added to that bus. Frames
outside fixes it, and the sampler now gains only what it put there.

Firing a pad allocates nothing, proven the same way everything else is. So does
loading: the buffer is built on the host thread and the audio thread swaps two
pointers and hands the old one back.

The Sampler pad page is the one whose pads address the **mixer** rather than the
deck. A sample belongs to the set, not to a deck — but the pads that fire it are
the deck's, exactly as on hardware. Both are true at once, so a pad's action
became either kind.

### The pad zone

Eight pads and a row of page tabs, replacing five separate fixed rows — hot
cues, auto loops, saved loops, roll — each of which took vertical space whether
or not the DJ wanted it.

The point is not the space saved. It is that **a page is a mapping from pad
number to action, and that mapping lives in Rust** (`crates/dj-core/src/pads.rs`).
The same table has two consumers that must not disagree: the eight buttons on
screen and the eight rubber pads on a controller in M4. Written twice, they
drift, and a DJ ends up with a pad that does one thing under their finger and
another on the display.

Three consequences fall out of keeping it small:

- **A pad names the condition that lights it**, in `Lit`, rather than the
  interface carrying a branch per page. Adding a page is rows in a table, not
  cases in a component.
- **A blank pad has no action at all**, rather than a do-nothing one. A verb
  that does nothing would have to exist in the vocabulary and be explained to
  everything that reads it.
- **The loop and roll pages walk the same ladder** — a sixteenth doubling to
  eight beats — so halving or doubling is one pad left or right, and a DJ who
  has learnt one page has learnt the other. `LoopBeats` became fractional to
  make that true, which also collapsed the engine's two loop entry points into
  one: halving already reached a sixteenth of a beat, so the length was never
  really an integer.

Pages that need a beat grid are hidden on a track without one rather than greyed
out — the same rule the FX beat control follows. Cues is the default page
precisely because it is the one that works on an unanalysed track.

### The effect rack

Three slots per deck and three on the master, run as a **chain** rather than a
parallel bank: slot 1 feeds slot 2 feeds slot 3. That is what makes stacking
mean anything — a gate after an echo chops the repeats, a gate before it feeds
the echo chopped audio, and a DJ can hear which one they built.

Four decisions carry the design.

**A slot owns the buffer, not the effect.** Every time-based effect wants
somewhere to keep the recent past. Giving each its own would make rack memory
scale with the size of the effect *catalogue* rather than with how many effects
can run at once, and would make switching one cost an allocation. The slot holds
one delay line and lends it to whichever effect is loaded.

**An effect is an enum variant, not a `Box<dyn Effect>`.** Switching an effect is
a normal DJ move — it is what an FX select knob does — and it happens on the
audio thread like every other action. A boxed effect would have to be built on
the control thread and posted across a queue, or built in the callback and
allocate. A variant assignment does neither. `rt_safety.rs` switches every effect
into every slot on a deck and the master two thousand times inside the callback
and counts zero allocations.

**Timing is in beats, and the beat comes from `effective_bpm`.** A quarter-beat
echo stays a quarter-beat echo when the DJ rides the pitch fader, because the
tempo the effect measures already has the fader in it. The deck's rack borrows
its deck's tempo; the master rack has none of its own, so it borrows from **the
loudest playing deck that has a grid** — loudest rather than lowest-numbered
because that is the deck the room is hearing, and during a transition it follows
the incoming track as it comes up, which is exactly when a master effect gets
thrown.

**One verb with a sub-grammar, not thirty-six verbs.** `deck 1 fx 2 echo`,
`deck 1 fx 2 wet 0.5`, `master fx 1 beats 1/4`. Three slots times seven controls
times two targets would have been thirty-six entries in a vocabulary that is read
by a model on every request, where every token is paid for.

Eight effects, and two decisions inside them worth keeping:

- **A slot owns two buffers, not one.** A reverb needs several short delays at
  once, at lengths chosen so their repeats do not pile up into a flutter, and
  the single line cannot be all of them. So the slot gained a tank. The
  principle is unchanged — the *slot* owns the memory, not the effect — and
  rack memory still scales with how many effects can run rather than with the
  size of the catalogue.
- **Echo and delay are two effects, not one with a knob at zero.** An echo has
  feedback and builds; a delay does not and can sit under a whole mix at a wet
  setting that would make an echo unlistenable. They are different tools and
  a DJ reaches for them at different moments.

Two tests in this slice failed for reasons worth recording rather than fixing
quietly. The reverb's comb lengths were chosen as pretty millisecond values,
which at 44.1 kHz round to frame counts sharing a factor of two — an audible
flutter. They are now the next prime above each target, so any two are mutually
prime at every sample rate. And the reverb's size knob barely did anything,
because damping was climbing as fast as feedback and cancelling it: a big room
does absorb more treble, but it still rings longer, and the control has to make
that true.

Placement is per slot: pre-fader hears the DJ's EQ moves and its tail survives
the fader coming down, post-fader hears what the room hears and dies with the
fader. Pre-fader listen stays pre-fader either way — a post-fader effect changes
what reaches the master and must not change what reaches the headphones.

### Loop roll

A fourth move from the same shadow, and the one that shows the concept was the
right one: a roll is a held loop that always slips, so letting go lands you where
the track would have been. No new engine machinery — `rolling` joins `slip` and
`censoring` in what counts as slipping, and the loop does the rest.

Two things this slice settled:

- **Rolls are fractions.** `set_loop_beats` took an integer, but halving a loop
  already reaches a sixteenth of a beat, so the length was never really an
  integer — only the way of asking for one was. `set_loop_length` takes beats as
  a float and `set_loop_beats` delegates to it; the action accepts `roll 1/4` as
  well as `roll 0.25`, because a sub-beat loop has a spoken name.
- **The pads had the wrong choke point.** Slip was armed in `set_loop_region`,
  which `set_loop_beats`, `loop_out`, halve and double all bypass — so slip
  engaged from the test's entry point and from nothing a DJ actually presses.
  `begin_diversion`/`end_diversion` now live in `enter_loop`/`exit_loop`, which
  every path goes through. Reverting the move fails three tests.

In the interface the roll pads are held, not clicked, and they take pointer
capture: dragging off a pad mid-roll keeps rolling until the finger lifts, the
way a hardware pad does. Releasing a roll ends its loop, so the release is
guarded by which pad started it — a pointer merely crossing the row must not
cancel a loop the DJ set on purpose.

**Done when:** djmanzo is at feature parity with VirtualDJ for a standard club set.

---

## M6 — Stems

- `dj-stems`: HT-Demucs via ONNX (`ort`), CoreML on macOS, CUDA/DirectML where present.
- **The separated track reaches the audio thread without a lock — done.** It
  used to be an `RwLock<Vec<StemFrame>>` the worker appended to, read with
  `try_read` so the audio thread could never block. It could not block, but it
  could *fail*, and a failed read falls back to the unseparated mix: while the
  worker held the write lock for a 1024-frame crossfade and a fifteen-megabyte
  `extend_from_slice`, a DJ holding the vocal muted heard it come back — once
  per chunk, for the whole track. Measured at 0.90 → **0**. The stems are now
  an immutable table of chunks, published by an atomic swap; the deck loads it
  wait-free and the audio thread takes no lock at all.
- **A built-in separator, so stems work on a fresh install — done.** The
  downloaded model is the quality option, not the only option: `dj-stems::hpss`
  implements Fitzgerald's harmonic/percussive separation over an FFT and splits
  the harmonic part by band and by centredness, giving four stems from
  arithmetic alone — no model, no runtime, nothing to fetch. Both separators
  sit behind one `Separator` trait, so the worker and the cache do not know
  which is running. The interface names it and says what a model would improve.
- **Availability is a first-class state — done.** The model is a download, not
  part of the package, so "no stems on this machine" is normal rather than an
  error. `dj-stems` opens ONNX Runtime itself before letting `ort` near it,
  because `ort` panics on a missing library and that panic poisons a mutex its
  own exit handler locks — aborting the process at exit, long after the code
  that asked for a stem. The application now starts without a model, reports
  why through `stems_status`, and greys out the stem controls with the reason
  beside them instead of offering pads that do nothing.
- Look-ahead separation with a rolling window and content-hashed disk cache, bounded with LRU
  eviction, chunk boundaries crossfaded.
- Graceful pending state: original mix plays while the first window is separating.
- **Stem pads page — started**: the action vocabulary now has the four stems and the pad table exposes a Stems page with mute toggles over held solos, so the screen, controllers and assistant share the same verbs before separated buffers arrive. Per-stem volume, EQ and effects still to come.
- Stem-aware transitions; stem swapping across decks.
- Per-deck and per-stem outputs for external processing.

**Done when:** load a track, and within a couple of seconds you can drop the vocal — with no
change in audio latency, and instantly on the next load.

---

## M7 — Network

**Foundation landed:** `dj-net` now gives every future transport a single,
tested boundary: JSON control messages parse into the existing action bus and
parameter registry, MIDI clock has bounded input/output timing utilities, and a
phase follower applies only gentle tempo corrections. The documented control
schema is transport-neutral so WebSocket and OSC adapters cannot grow private
engine APIs. Pioneer/Denon discovery and actual socket adapters remain the
next protocol-specific slices.

- **Pro DJ Link** — join a Pioneer CDJ/XDJ network as a peer: device announcement, beat/tempo
  sync, on-air state, track metadata.
- **StagelinQ** — Denon Prime discovery and state map.
- Network tempo sync (Ableton Link, or a clean-room implementation — see
  [RESEARCH.md](RESEARCH.md#2-open-source-prior-art) for the licensing decision).
- MIDI clock in/out.
- WebSocket + OSC control API over the same Actions and Parameters the UI uses; documented.
- Art-Net / DMX output driven by beat and structure data.

**Done when:** djmanzo can be plugged into a running club setup and stay in phase, and an
external system can drive it over the network without a private API.

---

## M8 — Beyond VirtualDJ

- Structure/phrase detection; phrase markers on the waveform; phrase-locked loops and jumps.
- AI transition planner — suggests where and how, with stated reasoning.
- Next-track suggestions ranked by harmonic compatibility, energy trajectory and phrase fit.
- **Deterministic set replay and offline re-render** from the action log; practice loops; take
  diffing.
- Lyrics extraction and waveform display; karaoke.
- Video mixing / VJ output.

**Done when:** there are things djmanzo does that no other DJ application does at all.

---

## M9 — Context, memory and the audience

The milestone about everything around the mix: knowing about what is playing,
remembering what worked, and closing the loop with the room.

Grouped together because they share one substrate — a **session record**. A
session is already a timestamped action log (see
[ADR-0003](adr/0003-action-bus-and-parameter-registry.md)); M9 hangs context on
it. Notes, photos, reactions and outcomes all attach to the same timeline, which
is what makes the reflective statistics possible at all.

### Knowing about what is playing

- **Live info feed.** While a track plays, pull what is known about it —
  release, label, personnel, the story — so a request from a party-goer can be
  answered rather than nodded at. Sources: **MusicBrainz** (CC0 data, open API),
  **Discogs** (credits and pressings), **Wikidata/Wikipedia**, **Cover Art
  Archive**. All open, all citable. Commercial editorial feeds mostly are not
  licensable for this, and are not planned.
- **Genre detection**, from tags where present and from audio where not, feeding
  the same panel and the suggester below.
- **Dictionary and acronym lookup** for the selected or playing track: what
  *dembow*, *perico ripiao*, *guira*, *mambo* mean in this repertoire, and the
  label and remix shorthand that turns up in filenames.

### Reading the room

- **Session phases with preset packs** — warm up, fiesta, peak, slow-down, chill
  out, close. A phase is a named set of constraints (tempo band, energy, genre
  weighting, transition style) rather than a playlist, so it steers rather than
  dictates.
- **Similar-music proposer**, configurable, considering the event, the previous
  and next tracks, the session so far, and the current phase. Suggestions state
  their reasoning, per [ADR-0005](adr/0005-assistant-speaks-only-actions.md).

### Remembering

- **Note-taking sidekick** — notes attached to a moment in the session, not to a
  file. "This transition landed", "the floor emptied here", "the birthday girl
  wanted this".
- **Webcam capture** — periodic stills or clips attached to the same timeline,
  so a set can be reviewed against what the room was actually doing.
- **Reflective statistics.** The data stays local and stays the DJ's. What it
  can answer: which transitions you reach for and which actually work, how your
  energy curve compares across nights, which tracks you always play and which
  you always skip, how long your phases really last versus how long you plan
  them, which requests you got and which you played.
- **An assistant that learns you** — built on the same record, so its
  suggestions come from your sets rather than from a generic model of DJing.

### Closing the loop with the room

- **Social posting** to Instagram and Facebook, on command or automatically, and
  **auto-stitched short video** for TikTok.
- **Live audience reactions** — requests and reactions from people actually in
  the room, surfaced as bubbles that open a sidebar, so a request can be
  answered on the mic and played.

> **Feasibility, stated up front.** Instagram and Facebook posting requires a
> Business or Creator account plus Meta app review before it works for anyone
> but the developer. TikTok's Content Posting API requires an audit before posts
> can be public rather than private-only. And **reading live comment streams is
> restricted or forbidden on most platforms** — which is why the primary channel
> for requests is planned as a djmanzo-hosted page behind a QR code on the
> booth, with social as a secondary feed. That inverts the dependency: the
> interactive feature works on its own, and social makes it wider.

**Done when:** a DJ can finish a set, look at what happened, and learn something
they did not already know — and a party-goer can ask for a song and hear it.

---

## Presets, everywhere

Not a milestone — a **capability that lands early and is then used by every
milestone after it**, because the alternative is six subsystems each inventing
their own idea of a saved configuration.

A preset is a named, layered set of actions and parameter values. Since every
intent in djmanzo is already an `Action` on one bus
([ADR-0003](adr/0003-action-bus-and-parameter-registry.md)), a preset is just an
ordered list of them plus the parameters they should settle at — which means it
is data, it is diffable, it is shareable as a file, and applying one is
indistinguishable from a very fast pair of hands.

**Layering** is what makes it useful rather than rigid: a pack sets a baseline,
a preset within it overrides part of that, and anything the DJ touches
afterwards wins. Nothing a preset does is unrecoverable, and nothing it sets is
hidden.

What gets packs:

| Area | Examples |
|---|---|
| **Session phases** | warm up · fiesta · peak · slow-down · chill out · close (see M9) |
| **Mix defaults** | transition length, EQ swap style, crossfader curve, gain target |
| **FX chains** | per-genre chains and beat-synced timings, per pad page |
| **Controller mappings** | per device, as data files (M4) |
| **Layouts and skins** | Starter · Essentials · Pro · Performance (M3) |
| **Audio setup** | per interface, per venue: device, buffer, bus layout |
| **Genre packs** | bachata · merengue · típico · dembow · reggaetón tempo bands, grid hints, transition rules |
| **Social and presentation** | post templates, overlays, artwork treatments, hashtag sets |
| **Assistant behaviour** | how forward it is, what it may do unasked, spend caps |

**Three levels of automation**, and the distinction matters:

1. **Configured** — the DJ picks a pack. Deterministic, no network, no model.
2. **Contextual** — djmanzo proposes a pack from what it can observe: time of
   night, tempo drift, phase, what is loaded. Still rule-based and explainable.
3. **Learned** — the assistant proposes from the DJ's own session record (M9).
   Suggestions state their reasoning, and are proposals rather than actions,
   per [ADR-0005](adr/0005-assistant-speaks-only-actions.md).

The order is deliberate. Level 1 works with no assistant at all, which means the
feature is useful before any of the intelligence exists — and it stays useful on
the night the network is down.

---

## The interface layer

M3 shipped layouts and skins as a flat set of feature flags, which can show, hide, resize and
scale a fixed set of components. That is where the ceiling is: a skin cannot move, reorder or
restyle anything, a DJ's layout file cannot name a widget the binary does not already know, and
M5's detachable panels have no mechanism waiting for them.

[ADR-0008](adr/0008-one-widget-vocabulary.md) decides the replacement — **a widget registry, and
a layout as a tree of addressed instances placed in named slots.** It is the move
[ADR-0003](adr/0003-action-bus-and-parameter-registry.md) already made for behaviour, applied to
what is on screen: one vocabulary, so the interface, controllers, the network API and the
assistant all address components by the same names, instead of the interface being the one layer
nothing else can reach.

| Step | What |
|---|---|
| **W1** | The registry in `dj-app`: names, slots, prop types with ranges and defaults, validation. Rust, not TypeScript — the network API and the assistant need it without a webview running. |
| **W2** | The layout tree format, the upconverter from today's flat `Layout`, and the token set a skin may restyle within. Nobody's existing file breaks. |
| **W3** | `Deck.svelte` and `App.svelte` stop being layouts and become renderers over the tree. This is the expensive step and the reason the ADR came first. |
| **W4** | What it unlocks: detachable panels and multi-monitor (M5) as a subtree with a window; widget addressing over the network API; assistant-composed layouts as proposals, per [ADR-0005](adr/0005-assistant-speaks-only-actions.md). |

Sequenced before the richer control rendering below, because a component that has no name is a
component a skin cannot place — and building the visual layer twice is the thing to avoid.

**The standing cost to remember:** widget names leak into every DJ's saved layout file the day
this ships. They are a compatibility surface and need the same care action names got.

---

## Visuals and motion

Requested as its own thread: richer visual representation of the controls and of
the audio, WebGL-driven visualisation, and motion throughout the interface.

**This has since been answered by measurement and turned into a design.** See
[ADR-0009](adr/0009-the-living-interface.md) for the decision and
[VISUAL-LANGUAGE.md](VISUAL-LANGUAGE.md) for the system; the sequencing note
below is kept because it is what prompted the measurement, and because its
reasoning still holds for anything WebGL-only.

The measurement, on the same no-GPU floor ADR-0004 used, with identical motion:

| shapes | DOM (`transform`) | Canvas 2D | WebGL |
|---|---|---|---|
| 60 | 59.8 fps | 60.0 fps | 59.5 fps |
| 240 | **27.0 fps** | 57.8 fps | 60.0 fps |
| 960 | **18.6 fps** | 45.4 fps | 59.7 fps |

The interface's *current* DOM approach is the worst of the three and the only one
that collapses — one self-repainting surface beats N animating layers, because
the cost ADR-0004 found was document invalidation rather than fill rate. The
WebGL run also reported its renderer as "Apple GPU" on a headless Linux box with
no GPU, which settles a question ADR-0004 left open: **the driver string cannot
detect a software fallback**, and the frame probe is the only honest detector.

### A second measurement, in the application rather than in isolation

20 August 2026, same no-GPU floor, four decks with controls, pads, FX rows and
the watershed all on screen. The frame-rate banner was made to report a *live*
figure first — it had been driven by an edge-triggered callback, so the number
in it was whatever it had been when things first went bad and never moved again.

| | fps |
|---|---|
| decks playing | 6 |
| decks paused | 26 |

The first reading of that was "the scrolling waveform costs four times the rest
of the interface put together", and it was wrong. Pausing a deck stops far more
than the waveform: the time readouts, the progress bars, the cue meters, the
beat phase and every other per-snapshot value stop changing too, so almost all
DOM churn everywhere stops at once.

The waveform was then rewritten from a strip of `<img>` moved by `translate3d`
into a single self-repainting `<canvas>` — the shape the table above endorses.
It measured **7 fps playing and 25 paused**, against 6 and 26. No effect either
way, and **the change was reverted**: an unproven benefit does not justify
replacing a working design, which is the same rule this document already applies
to the wgpu escape hatch.

Bisecting from there gave a real breakdown rather than a hunch. Toggling the
deck count and the watershed, four decks playing:

| | fps | implied cost per frame |
|---|---|---|
| 4 decks | 7 | — |
| 2 decks | 11 | **~26 ms per playing deck** |
| 2 decks, watershed off | 17 | **~32 ms for the watershed** |
| 4 decks, both paused | 26 | ~1.5 ms per paused deck |

A playing deck costing seventeen times a paused one is the sharp fact, and the
loops are not the reason: both `Waveform` and `Overview` run their
`requestAnimationFrame` whether or not the deck moves. What changes is whether
the transform they compute *differs from the last one*. A paused deck writes
the identical string, the browser sees no change, and nothing repaints.

**So the cost is per element whose transform actually moves, and the fix is to
move fewer of them, less often.** An overview playhead crosses a whole track in
minutes — a fraction of a pixel per frame — so rounding it to the pixel it
would be rasterised at anyway means the great majority of frames write nothing.
Doing that in both lanes took four decks from **7 fps to 11**.

Together with taking the audio out of the control pipeline, the interface went
from **4 fps to 11** on this floor without changing a single rendering
strategy.

### How much of this to believe

Single readings drift. Six samples five seconds apart, one build, one session,
nothing touched between them: **11, 11, 11, 10, 11, 12.** So the figure is
11 ± 1, and any change measuring inside two frames a second has measured
nothing. Two of this session's attempts died on that band after first appearing
to work — a 30 Hz cap on the watershed, and skipping the watershed's label
writes when unchanged, which read 13 on one sample and 11 on six.

Take three or more samples before believing a number in this table, and treat
anything under about three frames a second as noise.

Two things that did survive it: the audio leaving the control pipeline
(4 → 6) and whole-pixel transforms in the lanes (7 → 11).

The watershed's canvas has not been touched. Its earlier ~32 ms figure came
from toggling the whole component off, which removes its DOM and its layout as
well as its drawing, so it overstates what the drawing itself costs.

The original note follows.

Sequenced behind one thing, honestly: **[ADR-0004](adr/0004-waveform-rendering-strategy.md)
was written specifically about the hazard WebGL under WebKitGTK presents** —
contexts that create successfully and are then backed by a software rasteriser,
with no reliable way to detect it. The waveform benchmark that would settle it
is still open (it needs real Xubuntu hardware). Building an interface that
depends on WebGL before that answer is known risks building it twice.

So the order is:

1. **Motion that costs nothing** — transforms and opacity only, which the
   compositor handles without a paint. State changes that currently snap get
   transitions that *communicate* rather than decorate: a fader settling, a cue
   arming, a deck taking over the master, a track ending.
2. **Richer control rendering** — spectral waveform colouring, meter ballistics
   with real integration times, jog and platter rendering, phrase shading. All
   of it in the Rust tile renderer, which already sidesteps the webview.
3. **WebGL audio visualisation** — the beat- and audio-reactive layer, and the
   karaoke visuals from [K2](KARAOKE.md). **Every one of these must degrade to
   something static rather than breaking the interface**, and the frame-rate
   monitor already in the shell is what will notice when it needs to.

The rule that survives all three: *the words must render even if every visual
effect fails* — stated for karaoke in [KARAOKE.md](KARAOKE.md), and it
generalises. Nothing decorative may be load-bearing.

---

## The assistant track

The AI layer runs as its own track, because most of it depends on the library
and analyser rather than on the mixer. See [ASSISTANT.md](ASSISTANT.md) for the
feature design and [ADR-0005](adr/0005-assistant-speaks-only-actions.md) for the
constraint that shapes all of it: **the assistant can only emit action text onto
the existing bus.** It is a client, not a component.

| # | Milestone | Depends on | Definition of done |
|---|---|---|---|
| **A1** | Assistant foundation | M0 | Secrets in the OS keychain. Provider abstraction over OpenRouter, Anthropic, OpenAI, Google, Groq and local models, with live model lists tagged free/paid. A chat panel that turns plain language into actions and executes them. Per-session spend cap. |
| **A2** | Voice | A1 | Mic capture on its own stream. Wake phrase via openWakeWord, configurable. Local speech-to-text via whisper.cpp, Spanish and English. Push-to-talk shortcut for loud rooms. A local intent matcher that handles common commands with no network call. |
| **A3** | Music intelligence | A1, **M2**, **M3** | The Dominican/Caribbean domain pack — genres, tempo ranges, and which transitions actually work. Session planner taking absolute and relative instruction. Templates. Live steering that adjusts the remaining plan without discarding it. Suggestions that state their reasoning. |
| **A4** | Sources | A1, M3 | `SourceProvider` abstraction per [ADR-0006](adr/0006-music-sources-and-licensing.md). Local pool search. Spotify for discovery and planning only — never audio. YouTube search, with optional acquisition off by default. A provider slot ready for a licensed streaming partner. |
| **A5** | Generated music | A1 | Kaggle credentials and notebook deployment. HeartMuLa job lifecycle. Spoken song requests. Results land in a Generated container, analysed like any other track. |
| **A6** | Sharing | A5 | Export and hand off to WhatsApp with a prefilled message. djmanzo prepares the share; the user presses send. |

### Karaoke

Two milestones, designed in [KARAOKE.md](KARAOKE.md).

| # | Milestone | Depends on | Definition of done |
|---|---|---|---|
| **K1** | Karaoke, no models needed | M1, M3 | Band-limited centre cancellation -- cancels the vocal band only, so centred kick and bass survive. Lyrics from tags, sidecar `.lrc`, and [LRCLIB](https://lrclib.net/) (free, MIT, no API key). Karaoke screen on a second monitor with timed wipe-highlight display, next-line preview, count-in and artwork from Cover Art Archive. |
| **K2** | Karaoke, full quality | K1, M6, A2 | Stem-based vocal removal, plus vocal *reduction* for a guide vocal. Transcription over the isolated vocal stem -- far more accurate than over a mix. **Forced alignment** turning unsynced lyrics into synced ones. Beat- and microphone-reactive visuals that degrade gracefully. Voice control and a singer queue. |

**K1 delivers a usable karaoke night on its own**: centre cancellation plus
LRCLIB covers a great deal of real repertoire with no model, no GPU and no
cache.

**A1 and A2 are buildable now.** A3 is deliberately gated behind M2 and M3: a
planner needs beatgrids, keys, energy and play history to reason about, and
building it earlier would produce a planner with nothing to reason about.

---

---

## P — The polish pass

Runs **after the feature milestones**, as a deliberate second pass over
everything already built rather than as work interleaved into each milestone.
Requested explicitly, and worth its own phase: shipping features and making them
feel like one coherent instrument are different jobs, and doing the second one
concurrently tends to mean doing it badly.

Strictly in this order, because polishing something unusable just makes it
prettily unusable.

| # | Pass | What it covers |
|---|---|---|
| **P1** | Does it work? | Every feature exercised end to end on real hardware. Bugs fixed, edge cases safeguarded, failure modes made survivable rather than silent. Every "needs the user to verify" item from earlier milestones actually verified. |
| **P2** | Is it usable? | Every feature reachable and worth reaching. Missing affordances added, dead ends removed, defaults reconsidered against real use. |
| **P3** | Does it flow? | The whole daily workflow as one motion: load → cue → beatmatch → mix → next, with each step handing off cleanly to the one after it. Keyboard and controller paths as complete as the mouse path. The interface should stop being a set of panels and start being an instrument. |
| **P4** | Is it beautiful? | Only once P1–P3 land. Colour, form, spacing, hierarchy, motion, feedback. Animation that communicates state rather than decorating it. Consistency between light and dark, and legibility in a dark booth at arm's length. |

**P4 is last on purpose.** Visual polish applied before the workflow is settled
gets thrown away when the workflow changes, and it disguises usability problems
by making them look intentional.

---

## Working agreements

These hold from M0, not from "once it settles down":

- **CI on both platforms from the first commit.** A macOS-only build is a broken build.
- **The audio thread is sacred.** No allocation, no locks, no I/O, no logging, no panics.
  Enforced by tooling; xruns in the integration suite fail the build.
- **Analysis quality is measured, not asserted.** Beat-tracking accuracy is a number in CI.
- **Mappings and skins are data.** Adding hardware or a theme never requires a rebuild.
- **Every third-party dependency carries a recorded licence** and a note on why it is
  compatible. See [ADR-0002](adr/0002-clean-room-permissive-licensing.md).
- **Nothing is copied from VirtualDJ or from GPL projects.** Ideas, not source; our own art.
- **The assistant never gets a privileged path.** It emits action text like every
  other input source, so its work is auditable, reversible and replayable.
- **Licensing constraints are stated, not worked around.** Where a service
  forbids what we would like to do, the UI says so plainly rather than failing
  mysteriously.
