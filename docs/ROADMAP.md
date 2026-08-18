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

Not yet: saved loops (they need M3's persistence) and the labelled regression set.
`CERTAIN_CORRELATION` in `crates/dj-analysis/src/tempo.rs` was
calibrated against synthetic click tracks (0.95) and white noise (0.014); real music sits
between those and the constant should be re-derived once there are hand-verified grids.

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

Not yet: a session export.

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

Drag-and-drop onto a playlist is a select for now — the gesture DJs know is a drag, and it will
come, but a control that works one-handed on a trackpad should not wait for it.

**M3 is complete.**

---

## M4 — Controllers

The milestone that makes the hardware in your hands work.

- `dj-hid`: `midir` MIDI I/O and `hidapi` HID I/O on both platforms.
- Mapping engine over the Action bus; TOML mapping files with optional Lua for real logic.
- Inbound: 7-bit, 14-bit and high-resolution absolute controls; relative encoders; touch
  detection.
- Outbound feedback: LEDs, pad colours, displays.
- **Motorized platters** as a first-class control kind — absolute high-res position in, motor
  start/stop ramp and torque out.
- Jog modes: scratch, bend, search. Vinyl vs CDJ mode.
- Keyboard shortcut mapping. Controller-specific audio setup presets.
- In-app mapping editor (learn a control, bind an action).
- Mappings for the hardware actually on hand, committed to `mappings/`.

**Done when:** a full set can be played from the controller without touching the laptop, and
adding a new controller requires editing a file, not rebuilding the app.

---

## M5 — Performance

- Pad zone with pages: Cues, Loops, Loop Roll, Slicer, Sampler, FX.
- Sampler: banks, pad grid, trigger modes, tempo sync, record from deck or master.
- FX rack: multiple slots, per-deck and master routing, pre/post-fader placement, beat-synced
  timing, presets.
- Core built-in effects (echo, reverb, delay, flanger, phaser, filter, gate, bitcrush, roll,
  brake, backspin).
- **CLAP** plugin hosting.
- Slip mode, reverse/censor, 6 decks.
- Microphone/aux input with ducking.
- Automix with configurable transition style.
- Recording to disk; Icecast/Shoutcast broadcast.
- Multi-monitor / detachable panels.

**Done when:** djmanzo is at feature parity with VirtualDJ for a standard club set.

---

## M6 — Stems

- `dj-stems`: HT-Demucs via ONNX (`ort`), CoreML on macOS, CUDA/DirectML where present.
- Look-ahead separation with a rolling window and content-hashed disk cache, bounded with LRU
  eviction, chunk boundaries crossfaded.
- Graceful pending state: original mix plays while the first window is separating.
- Stem pads page; per-stem volume, EQ and effects.
- Stem-aware transitions; stem swapping across decks.
- Per-deck and per-stem outputs for external processing.

**Done when:** load a track, and within a couple of seconds you can drop the vocal — with no
change in audio latency, and instantly on the next load.

---

## M7 — Network

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
