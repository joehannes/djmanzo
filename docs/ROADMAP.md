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

- `dj-hid` — **done for MIDI and HID**: `midir` and `hidapi` in, in four layers of
  which only the last touches hardware, so the whole mapping engine is testable on a
  machine with nothing plugged in.
- Mapping engine over the action bus — **done**. TOML mapping files, every action in them
  parsed through `Action::parse` *when the file loads*, so a typo is a message when you
  choose the mapping rather than a control that silently does nothing an hour into a set.
  The consequence worth stating: **a mapping cannot do anything the interface cannot**,
  which is what makes a file from a stranger safe to open.
- **Lua — done, and the sandbox is the point.** A table cannot say "this pad
  does one thing normally and another while shift is held", because that is a
  decision and a decision needs an `if`. Marked controls go to a script;
  everything else stays a table entry, so a mapping is not all-or-nothing.
  A scripting language is exactly how "a mapping cannot do anything the
  interface cannot" gets lost, so: nothing reaches outside the process; every
  action a script returns goes through the same parser a table entry's does;
  and a script is stopped after a hundred thousand instructions, because it
  runs on the MIDI thread where a `while true do end` would take the controller
  with it.
  Asking Lua for *no* standard library turned out not to be enough — mlua
  installs the base library regardless, so `dofile` and `loadfile` were
  reachable until a test enumerated them by name and they were taken away.
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
- **Outbound feedback — done for LEDs and pad colours.** A `[[feedback]]` block
  names a parameter by the same stable name the interface and the network API
  use, and the name is resolved *when the file loads*, so a typo is a message
  at the moment a DJ chooses the mapping rather than a light that never comes
  on — the same promise the inbound side makes about actions. Only what changed
  is sent: a DIN cable carries about a thousand messages a second, and sixty
  lit controls refreshed at the snapshot rate would be 3,600, so the pad a DJ
  just hit would queue behind fifty-nine that had not moved. A reconnected
  device is told everything again, and letting go of one turns its lights off
  rather than leaving a board showing last night's set. Segment displays still
  to come.
- **Motorised platters — done.** A platter that reports where it *is* rather
  than how far it moved is a different control kind, and `dj-hid::platter` is
  it: `platter = "..."` plus a `resolution` from the device's manual. The angle
  wraps at zero, and reading a wrap as movement would play a whole revolution
  of audio backwards every time the record went round — so the short way is
  taken, which is safe because a platter covers three thousandths of a turn
  between reports at playing speed and half a turn is physically impossible.
  Past a tenth of a turn the reading is a dropped packet rather than a
  movement, and the honest answer is nothing rather than a lurch. The motor is
  the transport: these decks spin when the deck plays, so `deck.N.playing`
  drives it through the feedback path and a motor that could disagree with the
  transport does not exist.
- **Jog modes — done: scratch, bend and search, vinyl and CDJ.** One piece of
  plastic doing three jobs, and which one you get depends on the mode, whether
  the top is touched and whether the deck is playing. A turn of the wheel is a
  turn of a record -- 1.8 seconds at 33 1/3 RPM -- which is the number that
  makes a scratch feel right. Scratching is **position** control, applied the
  frame it arrives, because any smoothing there is latency and latency is what
  makes a scratch feel like rubber; bending is **rate** control, estimated from
  wheel speed and smoothed, so it does not depend on how chatty the controller
  is or how big the audio buffer is. A paused deck searches and makes sound
  while it does, because finding a cue point by ear is what the wheel is for
  when nothing is playing. On screen the platter answers to being *turned*
  rather than dragged, so a movement is the same near the hub or out at the rim.
  Motorised platters -- position in, torque out -- still to come.
- **Controller-specific audio setup presets — done.** A controller with a
  built-in soundcard has a fixed arrangement of sockets and its manual says
  what it is; djmanzo otherwise works it out from the channel count. That guess
  is right for most devices and wrong for the ones that differ, and wrong here
  has one meaning: the room hears what you are cueing. So an `[audio]` block in
  the same file as the pads states master, cue and booth, and **a layout where
  the master and the cue share a channel is refused when the file loads** --
  there is no later moment at which finding out is any use. The arrangement is
  applied when the controller connects and re-applied after every audio device
  change, because opening a device builds a fresh engine that knows nothing
  about what is plugged in; and it is re-checked against the device's real
  channel count every block, so a routing written for six outputs falls back to
  the guess on a stereo laptop rather than writing past the end of the buffer.
  A mapping that does not fit is *said out loud* in the Controllers panel
  rather than half-applied.
- **HID — done.** The reason to want it is one thing only: **resolution.** A
  7-bit MIDI control gives 128 steps across a jog wheel's whole travel; a
  16-bit HID field gives 65,536, which is the difference between a wheel that
  scratches and one that steps.
  Everything else about HID is harder than MIDI and this is why. **MIDI is
  edge-based** -- a pad sends a note-on when it goes down and says nothing in
  between. **HID is level-based**: the device sends the state of every control
  it has, in one packet, up to a thousand times a second, and nothing in the
  packet says what changed. So the whole job is turning level into edge: each
  field is compared with the last value seen and only a *change* becomes an
  action. Without that, holding play would send "play" a thousand times a
  second. Two details that took a test each: a switch with nothing remembered
  reads as **off**, so the first packet does not fire a release for every
  button nobody is touching; a range has no such default, because a fader's
  position is a fact worth knowing the moment the device appears.
  A HID packet is also **anonymous bytes** -- nothing in it is labelled, and
  where a control lives is decided by whoever wrote the firmware. djmanzo does
  not guess: the mapping states the offset, `hid 1 word-le 2`, exactly as it
  states a note number for MIDI. Which is unwritable by hand for an
  undocumented controller, so the editor learns it: two consecutive reports are
  diffed and the field that moved is named. One bit is a button, several bits
  of one byte is a fader, two adjacent bytes is the 16-bit control HID exists
  for, and **three or more is a DJ brushing two controls, where guessing would
  bind the wrong one** -- so it says nothing and asks again.
  Two things are refused when the file loads rather than discovered later: an
  encoder on a HID field (`turn_up` describes an event, a HID field is a
  level, and there is no honest way to read one from the other), and a platter
  finer than the field it reads (3,600 steps in one byte would wrap fourteen
  times a revolution).
  On Linux the backend is **pure Rust** -- no C hidapi, no `libudev-dev` to
  install before building, which is what keeps the `.deb` build buildable on a
  plain machine. See RESEARCH.md for the licence election on macOS.
- **The Controllers panel — done, and it was missing.** `control_status` had no
  consumer at all: djmanzo could read a controller and edit a mapping, and
  offered no way to see whether the thing on the table was connected. A DJ
  pressing a dead pad had three candidate problems -- no port open, the wrong
  mapping, a missing binding -- and no way to tell them apart. The panel names
  which, and distinguishes "nothing is plugged in" from "this machine has no
  MIDI service", because only one of those is fixed by plugging something in.
- **In-app mapping editor — done.** M4's promise is that adding a controller
  means editing a file rather than rebuilding the application, which is only
  half true while the only way to write the file is by hand from a manual. Now:
  press the control, say what it should do, save. Learning **suppresses** the
  action the control already has, because learning the play button by pressing
  the play button would otherwise start the deck sixty times over a mapping
  session. A binding is checked at the moment it is made, so a typo is a
  message while the DJ is still looking at the pad they pressed. What is saved
  is an ordinary mapping file — hand-editable afterwards, and proved to reload
  *before* it is written, so a file the loader would refuse never reaches the
  directory the loader scans.
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
- **Look-ahead separation that follows the playhead — done.** The content-hashed
  disk cache with LRU eviction was already here; the *look-ahead* was not.
  Separation walked the file from chunk 0, which is the wrong order for the one
  thing a DJ actually does with a fresh track: load it and cue straight to the
  drop. The worker would be twenty seconds in while the playhead sat at three
  minutes, and the stem pads quietly did nothing there for as long as it took to
  grind through everything in between.
  What blocked it was the stem table, which accepted a chunk only at the next
  index in order — so out-of-order results had nowhere to go. It is sparse now:
  a chunk lands at the index it names, the gaps are holes, and a hole reads as
  "not yet" exactly as the end of a partly-separated track already did.
  The feeder re-reads the playhead on every round rather than planning an order
  once, so a seek mid-separation redirects the work instead of being ignored;
  at equal distance the chunk *ahead* wins, because the playhead moves one way.
  The one invariant that mattered is kept exactly: a lookup is a division, not a
  search, so every chunk but the last must still be full length.
- **Chunk boundaries no longer glitch — done.** Separation is a windowed
  transform, so the first and last windows of any buffer have no neighbours to
  overlap-add with. Chunks were being separated independently and butted
  together, which put a **large** discontinuity at every seam — measured at 3.77
  on material peaking at 0.9, in every stem, once every ten seconds, for the
  whole track.
  Fixed by separating *more* than the chunk and keeping only the middle, rather
  than by crossfading the seam: with enough context either side the interior is
  simply correct, and there is nothing left to fade between.
  The margin is measured rather than guessed — 0 frames deviates by 3.77, 1024
  by 0.014, 2048 and beyond by 0.0053 and flat. What remains at the flat end is
  the median filters' longer-range statistics, a real difference rather than an
  edge artefact, at half a percent of full scale. The constant is 4096: the
  turn in the curve with a factor of two in hand, costing under two percent more
  work on a ten-second chunk.
  The stem cache is versioned with it. Entries written before this hold the
  glitched audio, and reading one back would reintroduce the fault silently on
  any machine that had run djmanzo before.
- Graceful pending state: original mix plays while the first window is separating.
- **Stem pads page — done**: the action vocabulary has the four stems and the
  pad table exposes a Stems page with mute toggles over held solos, so the
  screen, controllers and assistant share the same verbs.
- **Per-stem volume, EQ and filter — done.** The per-stem EQ and filter were
  the sharpest example of this project's recurring bug: they *ran on every
  frame*, in the audio path, with the coefficients the constructor gave them —
  and no verb existed that could reach them. Four knobs per stem that could
  never move.
  Now `stem_eq_low|mid|high vocal:1.5` and `stem_filter vocal:-0.4`, sharing
  one parser with `stem_volume` so the three ranges cannot drift apart — a
  filter clamped to a volume's `0.0..=1.0` would be a filter that only sweeps
  one way.
  **Composed with the deck's own EQ rather than replacing it.** The deck's EQ
  is the channel strip; a stem's is a trim within it, so the gains multiply and
  the filter sweeps add. That matters more than it sounds: on a separated track
  the deck's `shape` is skipped entirely and its EQ reaches the audio *only*
  through the stem channels, so a per-stem EQ that overwrote them would have
  silently disabled the deck EQ the moment a track finished separating. An
  untouched stem reads 1.0 and behaves exactly as it did before any of this
  existed.
  Published as the DJ's own trim rather than the effective coefficient, because
  a knob showing the product would jump every time the channel strip moved.
- **Stem swapping across decks — done.** The gesture stems exist for: deck 1's
  vocal over deck 2's mix, as one move rather than four pad presses.
  `stem_swap vocal 1 2` keeps only that stem on the source and takes the
  receiving deck's own copy of it away, so the two do not fight over the same
  part of the spectrum. A mixer action rather than a deck action, because it is
  about **two** decks and an action living on one would have to name the other
  anyway.
  Latching, with `stem_swap_off` to undo it — and **undone to what the DJ
  chose, not to nothing**: the receiving deck's own mute pattern is remembered
  before the swap takes it, because by the time you want it back it has been
  changed. A second swap replaces the first rather than stacking, since
  stacking loses the first deck's snapshot and then nothing can undo it. And it
  is published, because a state nothing can *see* is a state nothing can
  release — which is exactly how the Acapella button used to leave a deck dead
  for the rest of a set.
  Stem-aware transitions shipped earlier as the automix VocalDrop.
- **Per-stem outputs for external processing — done.** One deck leaves as four
  stereo pairs — vocals on 1–2, drums on 3–4, bass on 5–6, everything else on
  7–8 — for an external mixer or a DAW.
  A **tap, not a signal path**: the parts go out before the deck's EQ, filter,
  fader, effects and keylock, because what a processor on the other end wants
  is the separated parts, not the parts with djmanzo's tone shaping already on
  them — and the alternative would mean four independent time-stretchers for a
  result nobody asked for. The per-stem volume and mute *are* applied, because
  those are stem controls.
  The mix is computed normally first and the eight channels are overwritten at
  the very end of `render`, so the decks advance, the meters move and the
  waveform scrolls exactly as in a set; only the *output* is replaced. That
  ordering is what makes the feature unable to regress the mix path.
  Eight channels is the floor and it is refused below that — there is no honest
  way to send three stems of four — but the *choice* is remembered either way
  and takes effect when a wider interface arrives, because a DJ sets this up
  before plugging in, not after. An unseparated track sends silence rather than
  the mix: four copies of the same full track down four cables would be worse
  than nothing.
  This is also what surfaced the **four-channel cap in the audio host**: every
  device was opened at four channels however many sockets it had, so the booth
  bus (six) and this (eight) were reachable in the engine's own tests and
  unreachable from the application, on any hardware. Devices now open at their
  full width, rounded down to a stereo pair and capped at eight.
- **Per-deck outputs for external processing — done.** Each deck on a stereo
  pair of its own instead of the mix: deck 1 on outputs 1–2, deck 2 on 3–4, and
  so on, for a DJ who mixes on an external mixer and wants djmanzo to be four
  turntables rather than a mixer.
  **Pre-fader**, taken at the same point the headphone cue and the deck
  recorder take: the mixer on the other end has its own fader, and sending it a
  signal ours had already closed would be two faders in series with the second
  invisible to the person standing at it. A deck on its own socket is
  deliberately *not* also summed into a master — being heard twice, once
  through our crossfader and once through the external mixer, is the failure
  this exists to avoid.
  Mutually exclusive with stem out, which wants the same sockets; whichever is
  asked for last wins, in the engine and in the application both, so the panel
  cannot show an arrangement the audio is not in.
  The master chain does not run at all in this mode — no master gain, no
  microphone, no ducker, no limiter, no plugin insert — because there is no
  master. The microphone ring is still drained on purpose, since the loop that
  usually does it is the one being skipped and a backed-up ring would replay
  everything said while the decks were out.
  Found a bug on the way in: the master chain is **two** frame loops, not one,
  and the second writes the booth send from the master. On an eight-channel
  device the booth is channels 3–4, which is deck 2's cable — the decks were
  reaching their sockets and two of them were being wiped a few lines later.

**Done when:** load a track, and within a couple of seconds you can drop the vocal — with no
change in audio latency, and instantly on the next load.

---

## H — Hardware djmanzo can be built for without owning it

- **Timecode vinyl (DVS) — decoder done, not yet wired.** `dj-dvs` turns a
  control signal into a speed, a direction and an absolute position. It is the
  rare hardware feature that can be *finished and proved* without the hardware,
  because the signal can be generated: `Synth` writes a timecode and `Decoder`
  reads it back, so the encoding, the shift register, the quadrature and the
  position lookup are all pinned by round-trip tests over synthetic audio.
  **`xwax` was not read.** It is GPL-2.0, ADR-0002 forbids linking or copying
  it, and that includes reading it closely enough to reproduce. This is written
  from the prose descriptions RESEARCH.md records as safe input.
  Three things it does that a first attempt would not:
  - **Direction comes from the edge, not the sign.** The trailing channel's
    sign alternates every half cycle whichever way the record turns; read as
    the direction on its own it makes normal play average out to a fifth of
    normal speed. What carries the direction is the sign *paired with* whether
    the crossing was rising or falling.
  - **Speed and position arrive at different times, and are reported
    separately.** Speed is available within a couple of cycles; an absolute
    position needs a whole register's worth of bits, about twenty milliseconds.
    A DJ dropping the needle gets sound at once and the playhead a moment later,
    which is what dropping a needle feels like.
  - **A lifted needle drops its window.** Half a position from before the lift
    spliced onto half from after is a position on neither.
  **Not verified against a pressed record**, and the interface will say so. The
  bit ordering, the register's direction of travel and the sense of the
  quadrature are conventions, and a convention agreed with oneself is agreed
  with nobody. See RESEARCH.md on why the published Serato parameters are not
  shipped.
  **Wired to a deck.** `Command::SetTimecode` installs a decoder and a ring the
  host's input callback fills; the engine reads it at the *top* of the block,
  before the decks move, because the record decides what the deck is about to
  do and reading it afterwards would apply this block's hand movement to the
  next block's audio — a buffer of lag on the one control where lag is the
  whole complaint.
  Two modes, and **relative is not the lesser one**: with absolute tracking a
  DJ who nudges the record to beatmatch finds the playhead snapping back to
  where the vinyl says it should be. Relative follows the movement and leaves
  the position alone, which is what using vinyl as a jog wheel means. Absolute
  is for dropping the needle at a known point, and even then it only moves the
  playhead when the disagreement is more than a quarter second — below that the
  record and the track are telling the same story.
  The decoder's four-megabyte table is built on the host thread and handed
  over, and comes back through the retirement queue rather than being freed on
  the audio thread. Proved allocation-free by `rt_safety`.
  **Reachable from the application.** `timecode_status`, `start_timecode`,
  `stop_timecode` and `timecode_formats` are the door `dj-dvs` did not have:
  until they existed, `Command::SetTimecode` could be sent by nothing. The
  settings panel carries an input picker, the relative/absolute switch, a
  per-deck on/off, and a live reading that distinguishes **three** states from
  one number -- not on a record, on one and hearing nothing, and reading -- so a
  DJ whose deck will not move is told whether to check the cable or the
  cartridge.
  **`write_timecode_signal` writes the control signal to a WAV.** This is what
  makes "djmanzo ships no vendor format" a choice rather than a dead end: render
  it, burn it to a CD or put it on a phone, and any turntable, CD deck or media
  player drives a deck without buying a record. Proved by writing 16-bit PCM to
  disk and decoding the file back -- write level, clamp, integer conversion and
  chunk seams all in the path.
  **A device change used to leave every input dangling.** Opening an output
  builds a fresh engine, and each input is half of a ring whose other half
  belonged to the engine being dropped -- so the microphone went silently dead
  on a reconnect while still holding a sound card open, and no test could see
  it, because until the null backend could *capture*, no input path was
  reachable without hardware. Both are fixed: `NullBackend` has an input device,
  and the count of open captures is asserted across a reconnect.
  Still to do: run it against a pressed record on a real turntable.
- **Pro DJ Link** and **StagelinQ** — not started, and honestly gated. Neither
  vendor publishes an SDK; Pioneer's official route is a certification and
  licensing partnership, and both protocols are reverse-engineered by the
  community. The parsing and state machines could be built and tested against a
  simulated peer, but whether a real CDJ emits what the documents say is not
  something a container can answer.
- **CDJs are usable today without either of those.** A CDJ in MIDI mode is a
  MIDI controller, and CDJs are class-compliant USB audio interfaces — so they
  work now, with a mapping. What Pro DJ Link adds is the network layer: link
  sync between players, the players' screens showing your library, on-air from
  the mixer. HID mode, which is what gives Serato its jog resolution, needs the
  device in hand — `dj_hid::report::changed_field` exists to learn one.

---

## M7 — Network

**Foundation landed, and now reachable.** `dj-net` gives every future transport
a single, tested boundary: JSON control messages parse into the existing action
bus and parameter registry, MIDI clock has bounded input/output timing
utilities, and a phase follower applies only gentle tempo corrections. The
control schema is transport-neutral so WebSocket and OSC adapters cannot grow
private engine APIs.

It was, for four days, a boundary with no door. **Nothing depended on the
crate** -- and it could not have: `ControlService::new` took the bus and the
registry *by value*, and djmanzo holds both in `Arc`s because there is one of
each. Any service built from it would have dispatched into a private ring
buffer nobody reads. It took shared handles and a socket to make the tests mean
anything.

- **A local control server — done.** One JSON object per line over TCP, in
  Settings → Remote control. Off unless switched on; `127.0.0.1:7654` by
  default; **a passphrase required the moment the address is not loopback**,
  refused in `ControlServer::start` rather than left to the panel. Verified by
  driving a running djmanzo from a separate process: `deck.1.volume` 1.0 → 0.42
  over the socket. WebSocket is a framing layer over this and remains a slice;
  the capability came first.

- **Pro DJ Link** — join a Pioneer CDJ/XDJ network as a peer: device announcement, beat/tempo
  sync, on-air state, track metadata.
- **StagelinQ** — Denon Prime discovery and state map.
- **Network tempo sync between djmanzo instances — done.** The licensing
  decision deferred in RESEARCH.md is settled by not needing to make it:
  **this is not Ableton Link.** Link is GPLv2-or-proprietary and ADR-0002 rules
  out the former; its protocol is documented well enough to reimplement, but a
  reimplementation calling itself Link-compatible without ever having been
  tested against Live, Serato or any real Link peer would be a claim nobody
  here can stand behind. So djmanzo syncs to djmanzo and says so in the panel.
  Link interop stays open, needing either the commercial licence or a machine
  with a Link peer on it.
  What it covers is the case a second laptop actually creates: two DJs back to
  back, or a main rig and its backup. Every instance announces *and* listens —
  a peer that only listened would be invisible to the peers it was following.
  There is no election and no master: each follows the others through
  `PhaseFollower`, which takes a fraction of the error each time, so two peers
  converge on each other and one arriving or leaving costs nothing.
  **`PhaseFollower` was the third piece of `dj-net` with no door.** Like
  `ControlService` and the MIDI clock before it, it was written, tested and
  exported, and nothing could reach it.
  No passphrase, and unlike the control server that is defensible rather than
  deferred: UDP has no handshake to carry one. What a hostile packet achieves
  is bounded by the follower instead — a tempo more than six percent away is
  ignored outright and the nudge is clamped to one percent, against a control
  port that can load tracks and open devices.
  The nudge is **published, not dispatched**: letting a network thread move a
  deck's pitch without the DJ opting in is not a decision a background thread
  gets to make.
- **A master phase to sync to — done.** `master_bpm` had no companion, so there
  was a tempo to follow and no downbeat. `master_phase` comes from the same
  deck, by the same loudest-playing rule — a phase from one deck and a tempo
  from another would describe a beat nobody is playing.
- **MIDI clock in/out — done.** The oldest sync protocol there is, and still
  the one most likely to be on the other end of a cable in a small club.
  `dj-net` had the arithmetic and, like `ControlService`, nothing used it.
  **Out:** djmanzo is the clock master, twenty-four pulses to the beat at
  whatever the loudest playing deck is doing. That figure existed inside the
  engine as `master_bpm()` and was kept there; it is now `master_bpm` in the
  registry, because a clock outside the audio thread needs it and so does
  anything else that has to be in time with the music rather than with a deck.
  The pulses go out on a dedicated thread rather than the audio callback:
  sending MIDI is I/O, and a callback firing in 5.3 ms lumps would jitter a
  pulse by up to a whole buffer.
  Two details a follower would notice. Finding a tempo sends **continue**, not
  start -- `start` means "from the top", and a drum machine told that mid-set
  jumps to bar one. And pausing every deck sends `stop` while the **pulses keep
  going**, because a follower that simply stops hearing them decides it has
  lost its master altogether.
  **In:** djmanzo follows a drum machine or a second DJ, and while it does the
  external clock **outranks every deck as the sync leader** -- a DJ who plugged
  the room's clock in wants the room's clock, not deck 1. Unplugging it hands
  the lead back rather than freezing a synced deck at the tempo of a clock
  nobody is sending.
  The timing is proved rather than listened to: `ClockDriver` takes an elapsed
  time and a sink, so "one minute at 120 BPM is exactly 2,880 pulses" is a test.
  So is the drift: ten minutes of 5.333 ms intervals -- 256 frames at 48 kHz,
  deliberately not a whole tick -- comes out within one pulse.
- **OSC — done.** The protocol TouchOSC, Lemur and QLab already speak, so a DJ
  with an iPad has a control surface already. djmanzo invents no address space:
  **the action grammar is the address space**, `/deck/1/volume` with a float,
  which makes a layout readable next to a controller mapping.
  Loopback only, and that is not a default — OSC is UDP, so there is no
  handshake to carry a passphrase and nothing to authenticate with. A port
  facing the network is refused rather than protected badly.
  Bundles are refused rather than partly applied; a bundle exists to make
  several messages take effect together, and applying the first alone would be
  a scene change that half happened.
- **Rate limiting — done.** Sixty requests a second with a hundred and twenty in
  hand: a scene change firing a dozen at once is not throttled, a runaway script
  hits the wall immediately, and the answer is `too_fast` with the connection
  left open.
- WebSocket, for a browser client. A framing layer over the line protocol.
- Art-Net / DMX output driven by beat and structure data.

**Done when:** djmanzo can be plugged into a running club setup and stay in phase, and an
external system can drive it over the network without a private API.

---

## M8 — Beyond VirtualDJ

- **Phrase detection — done in `dj-analysis::structure`.** Finds how long a
  phrase is (8, 16 or 32 beats) and which beat starts one, from beat-synchronous
  novelty over four frequency bands. Phrase markers on the waveform and
  phrase-locked loops and jumps are the next slice; they need this first, and
  now have it.

  Three things it does that a first attempt would not, each found by a test:

  - **A z-score is not an effect size.** On a track whose beats are nearly
    identical, a periodic ripple of a few percent is wildly unlikely to be
    chance — and inaudible. Scored on significance alone, a metronome came back
    with a sixteen-beat phrase at thirteen z. A boundary now also has to be big.
  - **A 16-beat track satisfies a 32-beat test**, because every boundary of a 32
    is also a boundary of a 16. Length cannot be decided by boundary strength;
    what separates them is whether the *midpoint* between boundaries is quiet.
  - **Hop quantisation invents phrases.** A beat is 43.07 hops at 120 BPM, so
    beat spans drift against the hop grid with a period of about sixteen beats
    — exactly the range being searched. Beat features are energy *densities* and
    novelty is *relative*, so neither span length nor absolute level can leak in.

  Honest note on the tests: three fixtures had to be rewritten because each was
  manufacturing the structure it was meant to disprove. A `MIN_LIFT` gate was
  removed rather than kept, because no test could be built that it passed and
  the remaining gates failed.

- **Phrases are reachable — done.** The analyser's finding now survives a
  restart (a cache version and a library migration), reaches the engine paired
  with the grid it was measured against, and is drawn and played:

  - **`phrasejump`** in the action vocabulary, on `alt+shift+Q/W` and `U/I` —
    completing the beat → bar → phrase progression already on those keys. It
    lands **on** a boundary rather than moving a fixed distance, which is the
    whole reason it is not `beatjump 16`.
  - **Phrase markers on the waveform**, drawn in the same Rust pass as the beat
    grid so the two cannot disagree by a pixel, in their own colour rather than
    a third shade of the same white — and drawn at overview zoom where the beat
    and bar lines are correctly suppressed, because there the markers *are* the
    structure.

  A phrase counts beats from the grid's anchor, so the two travel together in
  one command: pairing a phrase with a different grid is unrepresentable rather
  than merely discouraged. Editing a grid clears the phrase; resetting one
  restores it.

- **Phrase-locked loops — done.** `loop_phrase`, on `alt+shift+E` and `O` and on
  the deck panel. It starts at the phrase boundary the playhead is inside, not
  at the playhead: pressed three beats into a phrase, a 16-beat *beat* loop runs
  from beat 3 to beat 19 — a fragment beginning in the middle of one musical
  idea and ending in the middle of the next — while this loops the phrase.
  Fractional lengths keep the alignment, so half a phrase is still phrase-start
  aligned.

- **Next-track suggestions — done.** `dj_library::suggest` ranks the library
  against what is playing, and the **Next** tab in SideView shows the result:
  the panel about the next twenty minutes, which is what a suggestion is for.

  Deterministic and local — no model, no network, no learned weights. That is a
  floor rather than a placeholder: a DJ deciding what to drop at 01:40 needs an
  answer in the time it takes to look down, and needs to see why it was given.

  **Reasons are typed data, not prose.** `Reason::KeyClash { from, to }` can be
  shown as a chip, sorted on, and disagreed with; a rendered sentence cannot.
  Same principle as [ADR-0005](adr/0005-assistant-speaks-only-actions.md), one
  layer down. The interface shows every reason including the bad ones — a DJ who
  can see "key clash" is a DJ who can decide to do it anyway.

  **Trajectory** (lift, hold, ease) is the one input the ranking cannot infer,
  because the same two records are the right and the wrong answer depending on
  where the night is going.

  Two things it does not claim to know, stated in the code: **energy is
  approximated by loudness** and they are not the same thing — a sparse, tense
  record can be quieter than a wall-of-sound filler and carry a room better; and
  **phrase compatibility is nearly free**, since 8, 16 and 32 all divide each
  other, so the real risk is a track with no structure at all, which is what the
  phrase reason actually reports.

- **The transition planner — done.** `dj_app::plan` proposes where to start a
  mix, how long to take, and which of the five styles to use, with typed reasons
  like the suggester's. Reachable from the Automix panel.

  It lives **beside** automix rather than inside it. Automix *runs* a transition
  on a style chosen in advance; the planner *decides* one from what the two
  records actually are. Keeping them apart means the planner can be asked for an
  opinion without anything moving — and "Use this" sets the style and length but
  does not start the mix, because a plan that acted on being agreed with would
  be an instruction, not a proposal.

  What it chooses, and why: the last phrase boundary that leaves room for the
  whole transition plus a tail margin, because a human presses the button a beat
  or two late. `Blend` when tempo and key both work, `Echo` when the keys fight
  (tolerable for a moment, tiring for eight bars), `Cut` when the tempos do not
  — nothing overlapping helps there, and `Cut` is the honest answer.

  Two things it cannot know, stated in the code: **where the outgoing track's
  outro actually is** — a phrase boundary near the end is the best structural
  guess available and it is a guess, so the plan names the boundary it chose and
  how much track is left, letting a DJ disagree with the specific thing rather
  than the whole answer; and **whether the two records suit each other**, since
  key and tempo are arithmetic and taste is not.

- **Set files, and take diffing — done.** A set is already a recording, because
  every action goes through one bus with a timestamp (ADR-0003). `dj_app::session`
  turns that into a file, and the Session log panel saves and compares them.

  **The file is text**, one event per line, in the same words an action is
  written in everywhere else — mapping files, the assistant's output, `Display`.
  So a set is readable, hand-editable and diffable, and comments and blank lines
  survive a round trip because annotating a set by hand is the point.

  **A set is not reproducible from its actions alone.** Loading is deliberately
  outside the action vocabulary — it carries an `Arc` and nothing external should
  invent one — so replaying "deck 1 play" against an empty deck reproduces
  silence, perfectly deterministically. The log is therefore wider than the
  vocabulary: `SessionEvent::Load` records what went *on* the decks. That is the
  only addition needed; ejecting looked like a second one and is not, since
  `deck 1 eject` is already an ordinary action.

  **Take diffing is not `diff`.** Two takes of one mix are the same decisions at
  different times, and a line comparison of a file whose first column is a
  timestamp calls every line changed. `session::diff` matches moves and reports
  which are missing and how far the rest drifted.

- **Deterministic replay and offline re-render — done.** `dj_app::replay` drives
  the engine headless against a frame counter rather than a sound card, and
  `session_render` writes a set back out as a WAV.

  **The same file and the same records produce byte-identical audio, every
  time.** That is not luck with floating point: the engine has no wall clock in
  its signal path — envelopes, filters, the crossfader and the beat clock all
  move per frame — so the same frames and commands in the same order give the
  same samples. A gig mixed through a cheap interface can be re-rendered
  afterwards at full quality with nothing dropped, because a replay runs to no
  deadline and cannot underrun.

  **Events land on their frame, not the block edge.** A 1024-frame block at
  48 kHz is 21 ms, a third of a beat at 174 BPM; firing whatever is due at each
  edge would quantise every move in the set to that grid — still deterministic,
  and audibly not what was played. Blocks are split at each event instead.

  Two things are refused rather than approximated: a **missing record** stops
  the replay by name instead of rendering a silent deck into a file that is
  quietly wrong, and a **live input** (microphone, aux, timecode vinyl) cannot
  be reproduced at all — such a set replays as the actions the vinyl produced,
  which is a fair record of what was played and not a recording of the night.

**Wired up — the assistant is reachable.** `Action::touches` says which control
an action moves, so every action arriving from a person records a touch and the
takeover is real rather than theoretical. `AppState::conduct` holds posture,
occasion and takeover under one lock — read together on every tick, changed
together when a pack is chosen — and seven commands drive it.

The **Conduct panel** sits above the conversation in the Assistant, and its
layout is the design rather than an arrangement:

- **The two takeover buttons never move and are always present.** Not
  shown-when-relevant: a control that appears and disappears is one you cannot
  build muscle memory for, and muscle memory is what you have at 01:40. "Hand
  back" is *disabled* when nothing is held rather than hidden, so its position
  is learnable even on the nights you never need it.
- **They are the largest controls in the panel**, because reaching for the right
  one under pressure should not require reading. Take-over is marked urgent;
  hand-back is deliberately calm, so it never looks like the thing to press in a
  hurry.
- **What it will do next is shown at every posture**, including the ones that
  will not act. Seeing what it *would* do is how a DJ decides whether to let it.

**The set feeds the autopilot, and it acts on its own.** `Conduct` holds the
setlist and how far through it the night has got — advanced when a record
actually reaches a deck, not when one is chosen, because a staged track the DJ
ejects was never played and counting it would silently skip a record.

A tick looks every half second: fast enough that a mix point cannot pass between
two looks, and slow enough to cost nothing. It exits immediately at Off, Watch
and Suggest, where most sessions will leave it.

The tick calls **exactly the same `decide` and `perform_step` a manual press
does**, so what the assistant does on its own and what it does when asked cannot
drift apart. All the gating lives in `autopilot::next_step`; the loop is only
obedience, and a second posture check inside it would be a second thing to keep
in step with the first.

Staging routes through the same `put_on_deck` a hand-load uses, so a staged
record arrives with its cues, grid and analysis exactly as a manually loaded one
does.

- Still open from the original M8: lyrics/karaoke and video mixing.

### M8 extended — the assistant that mixes, adapts and teaches

Added after the first five slices landed. The AI plumbing is already done (six
providers, keys, model choice, and ADR-0005's rule that the assistant may only
emit actions); this is what it should *do*.

Designed in full in [ASSISTANT.md](ASSISTANT.md) part two. In short, two dials:

- **Posture** — Off, Watch, Suggest, **Prepare**, Assist, Autopilot. Prepare is
  the one most software skips and the one most working DJs would leave it on: a
  DJ two minutes from the end wants the next record already loaded, cued to the
  phrase and gain-matched, so the only remaining act is theirs.
- **Occasion** — Learning, Practice, Experimenting, Warm-up, Peak, Close,
  Background, Requests. Changes the *weights*, not the vocabulary, and comes as
  named packs that can be chosen by hand or moved between by the assistant,
  visibly and undoably.

**Takeover and resume — done.** `dj_assistant::takeover` holds who has which
control. Touching one takes it immediately, per control rather than globally — a
DJ reaching for the bass on deck one has not asked the assistant to stop keeping
deck two in sync, and taking everything would punish them for touching anything.

The asymmetry is the point: **taking is implicit and instant, handing back is
explicit and total.** A hand on a fader is unambiguous; letting go is not a
decision, and a DJ who releases the crossfader to pick up a drink has not asked
the machine to resume. There is a panic gesture that takes everything, one
gesture that hands everything back, and a ten-minute expiry — long enough that
it can never fire during a transition (the longest djmanzo will plan is 64
beats, under two minutes at any danceable tempo) and short enough that a nudge
at the start of a set is not still being honoured at the end.

Plus: **touching a control takes over** — not a mode button, the fader itself —
and handing back is one gesture in one place that never moves.

**Occasion-aware density — done.** The interface changes with the occasion for
exactly one reason: to make the right thing quick and the destructive thing
hard, because a booth is dark and loud and a mis-click at 01:40 is heard by
everyone.

It is deliberately one narrow rule rather than a second layout. A control that
**cannot be undone by pressing it again** — ejecting a playing deck is the first
of them — becomes a 600 ms hold when the occasion says mistakes are costly, and
stays a plain click when it does not. Everything reversible stays a click
always: making ordinary controls harder would tax a DJ on every action to guard
against a few, which is a worse trade than the accident it prevents.

Three decisions are worth naming, because each is a place this normally goes
wrong:

- **The occasion table has one home.** `mistakes_are_costly` is computed in Rust
  beside the occasions themselves and sent to the interface. A second copy in
  TypeScript could disagree with the first, and the disagreement would surface
  as a control that is hard to press on a night it should not be.
- **The pad learned to hold; no new widget was introduced.**
  [ADR-0008](adr/0008-one-widget-vocabulary.md) says there is one vocabulary,
  and a bespoke hold-button next to a row of pads would read as a different kind
  of thing rather than the same thing being careful. The progress fills the pad
  itself, and the label says `hold`, because a button that needs holding and
  does not say so reads as broken — and the first press is in front of people.
- **A missing hold is an inconvenience; an unexpected one is a broken control.**
  So when the interface cannot reach the backend it falls back to *no* hold
  rather than guessing, and a keypress fires immediately either way: reaching
  for Enter is already deliberate in a way that brushing a touchscreen is not.

**Genre families and set assembly — done.** `dj_core::genre` holds the families;
`dj_library::setlist` builds a whole set from them.

The table encodes three facts, not a list of names. **Felt tempo is not written
tempo** — trap is written at 140 and danced at 70, so it mixes with hip-hop and
not with house, and a comparison on the written number gets that exactly
backwards. **Rhythmic grammar decides whether a blend is possible** — dembow and
four-on-the-floor put their kicks in different places, so holding them together
is a mistake however well the tempos match. **Families cross unevenly** —
amapiano into afro house is nothing, salsa into techno is a statement.

The assembler asks the suggester repeatedly, each answer becoming the next
question, and shapes the result with an **arc**: without one every step is
locally optimal and the set is an hour at a single energy. A Journey climbs,
plateaus, then descends — the plateau matters, because a set that peaks and
immediately drops feels like an accident. Taste **tilts** rather than filters: a
set that never leaves the DJ's four favourite families is the set they would
have built by hand. Avoided genres are strict, because "no country at my
wedding" is not a preference to be balanced.

**DJ technique — done.** `dj_assistant::technique` is a table of twenty-eight
moves, and it is consulted rather than read: `for_situation` takes how the two
records blend and what the DJ has in front of them, and returns the handful that
apply. A manual would hold the same sentences and never be opened in a booth.

Three fields carry the weight. **`Needs` says what a move is impossible
without**, not what it is nicest with — a laptop DJ shown a list half of which
their setup cannot perform learns to ignore the list. **`Kind` decides what the
other record has to be**: holding two records together needs them to agree, so
a blend is withheld across a dembow/four-on-the-floor seam, while a cut is
available always, which is exactly why it is the move that saves a set. And
**every entry carries a metaphor**, because that is what the teaching is made
of — a table entry with no picture is one the learning module cannot use.

The rig is read, never configured: a controller counts when its mapping is
*open*, not when the device is merely plugged in. And when nothing is analysed
yet — a fresh import, the grid still being built — every structural technique
drops away and beatmatching by ear is what is left. That is the moment a table
which assumed a grid would go blank, and it is the moment the DJ most needs it.

**The learning module — done.** `dj_assistant::coach` reads the action log, not
the audio. Every action is already timestamped on one bus
([ADR-0003](adr/0003-action-bus-and-parameter-registry.md)), so what the DJ did
is *known* rather than inferred; a listener guessing a bass swap from a spectrum
would be wrong in the noisy, unarguable way that teaches a learner to distrust
the whole feature. What the log cannot say is whether it sounded good, and the
coach therefore does not claim to.

It does three things, in this order:

- **Names what the hand just did.** Most of early DJing is doing something that
  worked and having no word for it, which means not being able to do it on
  purpose. Some moves are their own name — a backspin is a backspin. A bass swap
  is not: it is one deck's low leaving as another's arrives, close enough
  together to have been one intention. Two lows *arriving* together is not a
  swap but the commonest mistake there is, and calling it one would have the
  coach congratulating a DJ for it.
- **Catches the specific error, one at a time.** Not "that was off": "you came in
  three beats before the phrase on deck 2". Early and late are different
  mistakes made for different reasons — early is nerves about dead air, late is
  not having decided — so the note says which. Two lows up is checked first,
  because it sounds fine in headphones and wrong in the room, which makes it the
  correction that most needs a machine to make it.
- **Sets one thing to practise.** The easiest technique not yet shown, on the rig
  actually present. Easiest, not most impressive: a learner sent at a transformer
  scratch after two nights stops being a learner.

Nothing is scored. No marks, no streak, no percentage — a DJ practising at home
is not revising for an exam, and a number attached to a mix invites playing for
the number.

In the interface it follows the occasion's own verbosity, which is the one part
of the Conduct panel that appears and disappears: loud when learning, brief when
practising, absent in front of people. Coaching at peak time is not help, it is
a second thing competing with the room.
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
- **Similar-music proposer — first half done.** "More like this" from any track
  in the browser: the table becomes the ranked answer, each row carrying the
  reasons it is there, with every affordance the browser already has — sorting,
  set-aside, load to a deck — applying to it. A second, lesser list beside the
  table would have had fewer of them.

  What makes it more than a re-run of the suggester is **taste learned from
  what the DJ actually plays**, in `dj_library::learned`. The insight the module
  is built on: counting plays per family learns the shape of the *collection*,
  not the DJ. Somebody whose library is nine-tenths bachata plays mostly
  bachata whatever they think of it. So a leaning is a **ratio** — how often a
  family is played against how often owning it would predict — and one means no
  information. Plays are weighted by a 180-day half-life, because taste drifts
  and a phase two years gone should stop steering.

  Three rules it will not break:

  - **Taste is added to a score, never multiplied.** The score is signed — a key
    clash is negative — and multiplying a negative by a number above one makes
    it *better*, promoting exactly the records taste should push down.
  - **It is bounded to ±0.75, against a scale where a same-key match is +3 and a
    clash −2.5.** So it can reorder records that would all work, and can never
    lift one that would not. Taste breaks ties; it does not overrule the mixing.
  - **It never learns an avoidance.** A family owned and never played is one the
    DJ has not got round to as easily as one they dislike, and from here the two
    are indistinguishable. Avoiding stays explicit, because it is honoured
    strictly and a wrong guess silently removes music from a night.

  It says nothing at all until about a night's worth of plays, and what it has
  concluded is shown in the Conduct panel — it steers every suggestion, so a DJ
  should be able to see, and disagree with, what it thinks of them.

  Still to come: seeding from the session so far and from the current phase
  rather than from one track.

- **Assembling a set — the commands, not yet the workspace.** `setlist_build`
  already builds a whole night from an arc and a taste; `setlist_steer` now
  adjusts one without throwing it away (lift, ease, hold, favour, avoid, and
  per-record next, later, drop), and `setlist_save` turns a plan into a
  playlist in a single call rather than a create followed by twenty adds — a
  plan half-written because the twelfth call failed is worse than one not
  written at all.

  The plan deliberately lives in the interface and is handed back for each
  change. A draft the backend remembered would be one more thing to fall out of
  step with what is on screen.

  **The workspace — done.** In the browser sidebar beside History and the
  Journal, because all three are about nights rather than about the collection;
  this is the one that looks forwards.

  The **arc is the first control and the shape is the first thing shown.** A set
  built one locally-best answer at a time is an hour at a single energy; the arc
  is what makes it a night. Twenty-five rows of text lose exactly the property
  the DJ came for, so a strip draws the running trajectory — and a Journey
  visibly climbs, plateaus and descends, plateau included.

  Steering rather than rebuilding: "take it up", "hold it", "bring it down", and
  per record "not yet" and "take it out". The opener is protected, which is
  visible in the interface — steer a built set downwards and slot one keeps its
  lift while everything after it turns. A DJ adjusting the shape has not asked
  for a different first record.

  Taste from `learned` is **offered, not imposed**: one checkbox turns it off,
  because a DJ building a set for somebody else's party should not have to
  wonder why their own habits keep surfacing.

  A plan is not written to the library until it is asked for — a draft is not a
  playlist, and every abandoned build would otherwise fill the crate tree. When
  it is asked for it goes two ways: a playlist that outlives the panel, or the
  assistant, which will play it.

### Remembering

- **Note-taking sidekick — done.** Notes attached to a moment in the session,
  not to a file. "This transition landed", "the floor emptied here", "the
  birthday girl wanted this".

  Built around one gesture: **mark now, write afterwards.** A DJ who has just
  watched the floor empty has both hands busy and about ninety seconds of
  attention; asking them to compose a sentence loses the observation. So
  **Mark** in the top bar — beside REC, for the same reason REC is there —
  takes the moment and what is on the decks, and the words are added later in
  the **Journal**, which sits beside History in the browser sidebar because the
  two are halves of the same thing: what was played, and what was thought while
  it played.

  That makes a note with no words a complete row rather than a half-finished
  one, and the interface shows it as one — a dashed border and a cursor ready,
  not an error.

  Three decisions worth naming:

  - **A note belongs to a moment, not to a track.** The same record works at
    midnight and clears the room at two; filed against the track it would say
    the wrong thing on both nights.
  - **What was playing is copied in, not joined.** A note is an observation made
    at a time, like a caption under a photograph: it should not change when the
    library does. Joining `history` by timestamp is fragile exactly where it
    matters — forty seconds into a record, mid-transition with two up, or with
    nothing playing. And a foreign key would be worse: removing a track would
    cascade away the note about the night it was played, which has the ownership
    backwards. There is a test for that.
  - **Writing a note up cannot rewrite the moment.** Only the body is editable.
    A journal you can rewrite is not one worth keeping.
- **Webcam capture** — periodic stills or clips attached to the same timeline,
  so a set can be reviewed against what the room was actually doing.

**Reading the room is built** — without keeping a single frame. Assistant →
*The room*. A camera and a microphone are sampled every two seconds into three
numbers — brightness, how much of the picture changed, and loudness — and the
frame is discarded. Nothing is recorded, written or sent; the preview is there
so the lens can be aimed and can be switched off while the measuring carries
on.

Three decisions worth stating:

- **Every reading is relative to tonight.** Two lenses pointed at the same wall
  report different numbers, and a microphone's level depends on where it was
  put down, so an absolute threshold is a number that means something in one
  venue on one device. "Stiller than it has been all night" is a true sentence
  about a number nobody calibrated; "the floor is quiet" is not.
- **It never names a mood.** A camera measures how much of the frame changed.
  It cannot tell dancing from leaving, and a module reporting "the crowd loves
  this" from a difference of pixels is lying with statistics. The one
  interpretation offered is a *disagreement* — the floor is doing something
  other than the occasion the DJ set — which is a comparison of two things
  djmanzo actually knows.
- **Weather is not here.** It is not a sensor reading; it is a location plus
  somebody else's forecast. The hour is here, because a clock is a real
  instrument.

The eye is djmanzo's own window rather than a phone on a speaker stack, which
is what it should be. A browser will not open a camera on a page served over
plain HTTP, and serving HTTPS would mean shipping `tiny_http`'s TLS — which
pins rustls 0.20 and an unmaintained ring 0.16 — onto a port facing a club's
wifi. The trade was examined and refused; see `docs/RESEARCH.md`. A `tiny_http`
release on a current rustls unblocks it, and nothing else in the design
changes.
**Finding a record from what you remember is built.** Browser → *From
memory*, and it is three ways in feeding one list, because people do not
remember a record one way:

- **A line.** Searched against lyrics fetched for the collection from LRCLIB
  (free, no key, and the only service that permits storing what it returns —
  which searching requires). Folded on both sides so a capital, a comma and a
  missing accent do not stop a match. A record the database has nothing for is
  remembered as asked, so a sweep never asks about the same instrumental twice.
- **A description.** The one way in that can name a record the DJ does not own,
  because it is the assistant. It answers in rows or it answers nothing: a line
  without a separator is dropped, so a model that replies in prose returns an
  empty list rather than a paragraph presented as a shortlist. Every guess
  carries its reason and whether the record is already in the collection.
- **A hum.** Two searches off one recording. It is read through djmanzo's own
  key and tempo detection — the same analysis every track has already been
  through — to narrow the collection, counting half and double time because
  people hum the vocal rather than the kick; and it is compared *as a melody*
  against the pitch contour of every record that has one.

**Matching the tune itself is built** (`dj_analysis::melody`). A pitch contour
is ten points a second of whatever is the strongest periodic thing in a melodic
band, found with YIN rather than plain autocorrelation because autocorrelation
peaks at the octave below about as readily as at the fundamental and an octave
error is a semitone error of twelve. Three measurements shaped the rest of it:

- **Match on the intervals, not the pitches.** Centring each contour on its own
  median does not make the search blind to key, and a failing test is what
  showed why: the hum's median is the median of eight seconds and the record's
  is the median of five minutes, so they are never the same number. The phrase
  sat nine semitones from where the hum thought it was and the search returned
  the intro. Differencing consecutive points cancels any constant offset
  exactly, with no key detection to go wrong.
- **Fold those intervals into an octave.** Three octave slips in one hum flipped
  the ranking — the right record scored 2.13 against the wrong one's 1.42.
  Clamping the outliers barely moved it (0.67 against 0.69). Folding took the
  right record to 0.000.
- **Gate on in-band energy.** A nine-kilohertz tone read as fully voiced,
  because YIN is scale-invariant and so attenuating a band it cannot hear does
  nothing. Voicing now requires the energy actually to be in the melodic band.

The search is subsequence DTW — free start and end in the record — so the
answer is *where in this record*, and the warping is itself what makes it
tempo-independent. A fourth correction came from running the thing rather than
compiling it: the sweep fed the matcher an interleaved stereo buffer, which
does not fail — it makes a contour twice as long as the record, so every
reported timestamp was half the truth while the match itself still landed,
because the intervals are unchanged and the warping absorbs a constant rate.
Contour *length* is now asserted, which is the only place that error is
visible. Contours are stored a quarter-semitone to the byte
(`melodies`, schema 9), about 3 kB for a five-minute record, and are filled in
a sweep at a time from the panel rather than by a background pass that would
compete with analysis for the disk. The panel says how many records have one,
because a shortlist drawn from a third of a collection should say so.

**It still does not identify a record you do not own**, and the panel says so
beside the button. That needs a licensed fingerprint service with tens of
millions of reference melodies. Within the collection it is honest but not
magic: what a contour holds is the loudest periodicity, which is the vocal much
of the time and the bassline some of it, so a hummed vocal will not find a
record whose strongest line is the bass. That is why the result is a shortlist
with key and tempo still ranked beside it, and not an answer.

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

**Requests are built.** djmanzo carries its own HTTP server (`dj_net::web`),
its own request page (`dj_net::page`, English and Spanish, no script and no
fetch so it works on venue wifi with no route out), the book that folds six
spellings of one song into one tally (`dj_net::room`), and the QR code and
printable sticker sheet that get a phone there (`dj_net::sticker`). Browser →
**Requests**.

Two things worth stating plainly about it:

- **The printed sticker problem is solved by answering to a name.**
  `http://djmanzo.local:7331/` is the same at every venue, which is what makes
  a sticker printable in advance; djmanzo answers for that name over multicast
  DNS (`dj_net::announce`). Apple devices resolve `.local` reliably, Android
  has since 12 and not on every build, and a few browsers never will — so the
  plain address is offered beside it with that caveat printed, not hidden, and
  a sheet can be printed for either.
- **The server faces the network on purpose, and is safe for a different
  reason than the control server.** `dj_net::server` is safe because it is off,
  on loopback, and behind a token. None of that is available to a page a
  stranger opens from a sticker, so the safety is structural instead:
  `dj_net::front::Doorman` holds a request book and nothing else — no action
  bus, no registry, no deck. A request from the room cannot reach the audio
  path because there is no path, not because something checks.

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
| **W1** | **Built.** The registry in `dj-app`: 33 names, the slots each may sit in and offer, and every setting with a type, a range and a default. Rust, not TypeScript — the network API and the assistant need it without a webview running. |
| **W2** | **Built.** The tree format, the upconverter from the flat `Layout`, the loader that reads both out of one directory, and the 23-token set a skin may restyle within. Nobody's existing file breaks. |
| **W3** | **Not started.** `Deck.svelte` and `App.svelte` stop being layouts and become renderers over the tree. This is the expensive step and the reason the ADR came first. |
| **W4** | What it unlocks: detachable panels and multi-monitor (M5) as a subtree with a window; widget addressing over the network API; assistant-composed layouts as proposals, per [ADR-0005](adr/0005-assistant-speaks-only-actions.md). |

Two things W1 and W2 settled that the ADR left as intentions.

**The restyling boundary is a whitelist of three shapes, not a list of refusals.** A colour is a
hash and 3, 4, 6 or 8 hex digits; a length is a number and one of `px`, `rem`, `em`; a scale is a
bare number. `url()`, `@import`, a comment, a closing brace and a CSS escape spelling `url(` are
all refused by falling off the end rather than by being named. That distinction earned its keep
immediately: deleting the hex-digit check broke nothing in the first version of the test, because
`#0;url(xy` is exactly eight characters and passed the length check alone.

**Two formats sharing one directory need each reader to leave the other's files alone.** A tree
parses cleanly as a flat `Layout` — every field there has a default — so the flat reader was
offering a tree file back as a fiction: the file's name attached to this struct's idea of
everything else, listed twice in the picker. Both readers now sniff for a non-empty `slots`, one
keeping those files and one skipping them.

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
| **A6** | Sharing | — | **Shipped.** Export and hand off to WhatsApp with a prefilled message. djmanzo prepares the share; the user presses send. Built without waiting for A5: the dependency was on generated music being *shareable*, and a tracklist of records that were actually played is the thing people ask for after a night. |

### Karaoke

Two milestones, designed in [KARAOKE.md](KARAOKE.md).

| # | Milestone | Depends on | Definition of done |
|---|---|---|---|
| **K1** | Karaoke, no models needed | M1, M3 | Band-limited centre cancellation -- cancels the vocal band only, so centred kick and bass survive. Lyrics from tags, sidecar `.lrc`, and [LRCLIB](https://lrclib.net/) (free, MIT, no API key). Karaoke screen on a second monitor with timed wipe-highlight display, next-line preview, count-in and artwork from Cover Art Archive. |
| **K2** | Karaoke, full quality | K1, M6, A2 | Stem-based vocal removal, plus vocal *reduction* for a guide vocal. Transcription over the isolated vocal stem -- far more accurate than over a mix. **Forced alignment** turning unsynced lyrics into synced ones. Beat- and microphone-reactive visuals that degrade gracefully. Voice control and a singer queue. |

**K1 delivers a usable karaoke night on its own**: centre cancellation plus
LRCLIB covers a great deal of real repertoire with no model, no GPU and no
cache.

#### What A6 decided, and why

- **djmanzo prepares, the human sends.** The link opens a compose window with
  no recipient chosen. Nothing is posted, and no contact list is read. That is
  not a limitation working around an API — it is the line: a set going to a
  particular person is a decision a person makes.
- **The preview comes before the send.** Once WhatsApp has the text it belongs
  to a chat window, and "wait, not that one" stops being something software can
  offer. It is also where a four-hour set announces that it will not fit in a
  link, while there is still a file to choose instead.
- **The URL is built in Rust; the interface names a session.** A command that
  took a URL and opened it would hand the webview a general-purpose way to
  launch anything, granted permanently the first time it was convenient. The
  interface says *which night*; djmanzo decides what that means.
- **The length budget is counted in encoded bytes, not characters.** One
  accented character costs nine bytes once percent-encoded, and this repertoire
  is mostly accented. A character count would pass a list the operating system
  then refuses — failing on precisely the sets djmanzo exists for.
- **A truncated share says it was truncated, in the message itself.** The
  person reading it in the chat is the one who would otherwise assume the night
  ended there.

Fixed alongside it: every **"Get one →" link was decorative**. A webview cannot
reach a browser on its own — `target="_blank"` inside a Tauri window opens
nothing at all on Linux — so every link next to a credential field had been
silently doing nothing. They are buttons now, and the address is checked
against djmanzo's own catalogs before the operating system is asked to open it.

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
| **P2** | Is it usable? | Every feature reachable and worth reaching. Missing affordances added, dead ends removed, defaults reconsidered against real use. **Under way.** The keyboard half is done and is now a test — see CONTROLLERS.md — which found that the crossfader could not be assigned, a beat grid could not be corrected, and sync could be engaged and never released. The interface half cannot be audited the same way: scraping Svelte templates for what they dispatch gives both false positives and false negatives, and a test built on it would fail for the wrong reasons and get muted. It is being done by using the thing — which found, among other things, that `class:on` on the REC button had never matched a rule anywhere in the application, so a running recording looked exactly like a stopped one, and that the crossfader had drifted back below the fold at the application's own default window size after a previous pass had fixed it. **Using the thing catches these; nothing stops them coming back.** The gap worth closing is a rendered-geometry budget — a headless browser asserting that the controls a DJ performs a mix with are inside the default viewport — which is measurement rather than template scraping, and is the one form of interface test that would not fail for the wrong reasons. |
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
