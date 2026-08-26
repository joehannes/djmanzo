# Features

Two halves: **parity** — everything VirtualDJ does that djmanzo must do — and **beyond** —
the reasons to use djmanzo instead.

Every parity row carries the milestone that delivers it. Nothing is listed without a home.
Milestone definitions are in [ROADMAP.md](ROADMAP.md).

---

## 1. Parity matrix

### Decks and transport

| Feature | Milestone |
|---|---|
| 2 decks | M1 |
| 4 decks | M2 |
| 6 decks | M5 — **done** |
| Play / pause / cue (CDJ-style cue behaviour) | M1 |
| Pitch fader, configurable range (±6/8/10/16/25/50/100 %) | M1 |
| Pitch bend (temporary nudge) | M1 |
| Keylock / master tempo | M1 |
| Jog wheel: scratch, bend, search modes | M4 |
| Vinyl mode vs CDJ mode | M4 |
| Sync (tempo + phase) | M2 |
| Quantize to beat / bar | M2 |
| Beat jump | M2 |
| Track load, unload, clone deck, instant doubles | M3 |
| Slip mode | M5 — **done** |
| Reverse / censor | M5 — **done** |
| Elapsed / remaining time, end-of-track warning | M1 |
| **Key shift** — transpose in semitones for harmonic mixing, independent of tempo | M2 |
| **Sandbox** — audition a mix in headphones while the master keeps playing | M5 |

### Mixer

| Feature | Milestone |
|---|---|
| Channel faders, crossfader with selectable curve | M1 |
| Per-channel crossfader assignment (A / thru / B) | M2 |
| 3-band EQ per channel, kill switches | M1 |
| Per-channel filter (low/high-pass sweep) | M1 |
| Gain + auto-gain from EBU R128 analysis | M2 |
| VU meters per channel and master | M1 |
| Headphone cue (PFL) with cue/master blend and split-cue | M1 |
| Booth output with independent level | M1 |
| Master limiter | M1 |
| Microphone / aux input with ducking | M5 — done |
| Microphone effects (reverb, echo, pitch) | M5 |
| Crossfader assign per channel (A / B / thru) | M1 |

### Waveforms and displays

| Feature | Milestone |
|---|---|
| Scrolling waveform, per deck | M1 |
| Stacked / parallel multi-deck waveform view | M2 |
| Overview waveform with position and cue markers | M2 |
| Beat grid overlay, editable (shift, scale, tap) | M2 |
| Saved loops, recalled per track | M3 |
| Cues, grids and loops kept with the track across sessions | M3 |
| Playlists, crates and folders in one tree | M3 |
| Play history, recorded when a track is actually played | M3 |
| Smart folders with a filter language, incl. harmonic matching | M3 |
| Import rekordbox XML, Traktor NML and iTunes XML with cues and grids | M3 |
| Import Serato crates and database (clean-room) | M3 |
| Read Serato hot cues and loops out of the audio files | M3 |
| Duplicate detection across copies of the same audio | M3 |
| Batch tag editing, colour coding and ratings | M3 |
| Session export as a set list | M3 |
| Spectral colouring (bass/mid/high energy) | M2 |
| Loop region and hot cue overlays | M2 |
| Phrase / structure markers | M8 |
| Waveform zoom, per-deck and global | M2 |
| Lyrics on the waveform | M8 |

### Cues, loops, pads

| Feature | Milestone |
|---|---|
| Hot cues (8+, named and colour-coded) | M2 |
| Manual loop in / out, loop adjust | M2 |
| Auto-loop by beat count | M2 |
| Loop roll (momentary, always slipping, down to 1/16 beat) | M5 — **done** |
| Loop move / halve / double | M2 |
| Saved loops | M2 |
| Slicer | M5 — **done** |
| Pad pages: Cues · Loops · Roll · Saved · Sampler · FX | M5 — **done** |
| Pad pages: Slicer · Stems | Slicer page M5 — **done**; Stems page M6 — started |
| Pad colour and LED feedback to hardware | M4 |

### Effects

| Feature | Milestone |
|---|---|
| FX rack, three chained slots per deck and on the master | M5 — **done** |
| Per-deck and master FX routing | M5 — **done** |
| Pre-fader / post-fader chain placement | M5 — **done** |
| Beat-synced FX timing (1/16 to 4 beats, following the pitch fader) | M5 — **done** |
| Core effect set: echo, delay, reverb, gate, crush, flanger, phaser, filter | M5 — **done** |
| Roll (as a loop roll, on its own pad page rather than in a slot) | M5 — **done** |
| Brake and backspin — transport, not signal: they live on the deck | M5 — **done** |
| CLAP plugin hosting (master insert; generic controls, no plugin window) | M5 — done |
| **Per-stem EQ and filter** | **shipped** — three bands and a sweep per stem, composed with the deck's own EQ rather than replacing it |
| Per-stem effects | M6 |
| Effect chain presets | M5 — **done** |

### Sampler

| Feature | Milestone |
|---|---|
| Sampler banks with pad grid | M5 — **done** |
| Trigger modes: one-shot, loop, hold, stutter | M5 — **done** |
| Sync samples to master tempo | M5 — **done** |
| Sample volume / output routing (mix or headphones) | M5 — **done** |
| Record from deck or master into a sample slot | M5 — **done** |

### Browser and library

| Feature | Milestone |
|---|---|
| Folder tree + song list, VirtualDJ layout | M3 |
| **SideView**: Sidelist · Sampler · Automix · Karaoke · Clone panels | M3 (Karaoke K1) |
| Search across the whole library, instant | M3 |
| Playlists / crates / smart folders | M3 |
| Columns: BPM, key, energy, rating, play count, last played, comment | M3 |
| Sort, filter, colour-coding | M3 |
| Track info editor, batch tag editing | M3 |
| Import: rekordbox, Serato, Traktor, iTunes, folders | M3 |
| Duplicate detection | M3 |
| Play history and session export | M3 |
| Harmonic (Camelot) key display and compatible-key filtering | M3 |
| Automix with configurable transition style | M5 — done |
| Search across online sources (Spotify, YouTube, Jamendo, Internet Archive) | **shipped** |
| Match an online result to a file you already own | **shipped** |
| Licensed streaming catalogue (Beatsource/Beatport/TIDAL/SoundCloud) | slots shipped; **needs a partnership** — see [SOURCES.md](SOURCES.md) |

### Hardware

| Feature | Milestone |
|---|---|
| Class-compliant MIDI controller support | **shipped** — 7-bit, 14-bit and all three encoder conventions |
| Data-file mappings (no recompile) | **shipped** — TOML, bundled and user files, checked when the file loads |
| Mapping editor (learn a control, bind an action) | **shipped** — press the control, pick the action, save; the file is proved to reload before it is written |
| LED / display feedback to controllers | **shipped** for LEDs and pad colours; segment displays still M4 |
| **Stem swapping across decks** — one deck's vocal over another's mix | **shipped** — `stem_swap vocal 1 2`, latching and undoable to what the DJ had |
| **Lua scripting in mappings** — a shift key, a mode-dependent jog, one knob doing two things | **shipped** — sandboxed: no filesystem, no process, every action through the parser, stopped after 100k instructions |
| HID controller support | **shipped** — 8- and 16-bit fields, both byte orders, level-to-edge conversion, and a learn mode that diffs two reports to name the control that moved |
| **Motorized platter support** — high-res absolute position, motor ramp, torque | **shipped** — absolute angle with wrap handling, motor driven by the transport |
| Controller-specific audio setup presets | **shipped** — an `[audio]` block per mapping; a master that overlaps the cue is refused when the file loads |
| Controllers panel — what is connected, on which mapping, with which outputs | **shipped** |
| **Keyboard as a controller** — same vocabulary and file format as a MIDI mapping, with a live sheet | **shipped** |
| Multi-device audio setup (4-channel, or two devices with drift correction) | M1 |
| **MIDI clock out** — djmanzo as clock master for a drum machine or light desk | **shipped** — 24 PPQN at the room's tempo, on its own thread |
| **MIDI clock in** — follow a drum machine or a second DJ | **shipped** — an external clock outranks every deck as the sync leader |
| **OSC** — TouchOSC, Lemur, QLab | **shipped** — the action grammar is the address space; loopback only, because UDP cannot carry a passphrase |
| Network control API — drive djmanzo from a script, a Stream Deck or a lighting desk | **shipped** — line-delimited JSON over TCP, off by default, loopback by default, passphrase required off-machine |
| **Pro DJ Link** (Pioneer CDJ/XDJ) | M7 |
| **StagelinQ** (Denon Prime) | M7 |
| Network tempo sync (Ableton Link or equivalent) | M7 |
| MIDI clock in/out | M7 |
| DVS timecode vinyl | *deferred — architecture leaves room; see ARCHITECTURE.md §10* |

### Output, recording, broadcast

| Feature | Milestone |
|---|---|
| Record the master to disk (WAV; FLAC/MP3 later) | M5 — **done** |
| **Network tempo sync between djmanzo instances** | **shipped** — announce and follow over UDP, no master, bounded corrections. Not Ableton Link; see ROADMAP |
| Broadcast to Icecast/Shoutcast | M5 |
| **Per-stem outputs for external processing** | **shipped** — one deck as four stereo pairs (vocals 1–2, drums 3–4, bass 5–6, other 7–8), pre-EQ and pre-fader, on an interface with eight outputs |
| **Per-deck outputs for external processing** | **shipped** — each deck on its own stereo pair, pre-fader, no master chain; exclusive with stem out |
| **Phrase detection** | **shipped** — phrase length and phrase anchor, from beat-synchronous novelty in four bands; markers on the waveform, `phrasejump` and `loop_phrase` on the keyboard and the deck panel, persisted across restarts. Verified against synthetic tracks whose structure is arithmetic; **not measured against a corpus of real records** |
| **Next-track suggestions** | **shipped** — harmonic, tempo, loudness and phrase, ranked with typed reasons shown as chips; lift/hold/ease. Deterministic and local. **Energy is approximated by loudness**, which is a proxy and not the same thing |
| **Transition planner** | **shipped** — where to start (phrase boundary with a tail margin), how long, and which style, with typed reasons. Proposes; does not act. **Cannot know where the outro is** — a phrase boundary near the end is a structural guess |
| **Set files and take diffing** | **shipped** — a set saved as readable, diffable text; two takes compared by move and drift |
| **Deterministic replay / re-render** | **shipped** — a set file rendered back to audio, faster than real time, byte-identical run to run. Live inputs (mic, aux, DVS) cannot be reproduced and are stated as such |
| **Timecode vinyl / DVS** | **shipped** — speed, direction and absolute position drive a deck, in relative or absolute mode, with an input picker and a live calibration reading in Settings. djmanzo writes its own control signal to a WAV, so any turntable, CD deck or phone works without a licensed record. **Not yet run against a pressed record** |
| Video mixing / VJ output | M8 |
| Karaoke | K1 / K2 — see [KARAOKE.md](KARAOKE.md) |

### Interface

| Feature | Milestone |
|---|---|
| Layout presets: Starter · Essentials · Pro · Performance | M3 |
| Skin system (CSS themes + JSON layouts) | M3 |
| Multi-monitor / detachable panels | M5 — done |
| Configurable waveform and jog appearance | M3 |
| Light and dark themes | M1 |

### Deliberately not planned

- **DRM'd content** of any kind.
- **Mixing Spotify or YouTube Music audio.** Not a scope decision — Spotify's policy forbids
  mixing their catalogue, and YouTube Music exposes no API that permits it at any tier. Both are
  integrated for search and planning instead. [SOURCES.md](SOURCES.md) has the detail.
- **Cloud library sync between machines.** VirtualDJ has it; it needs hosted infrastructure and
  an account system, which is a different kind of project. The session record is local and
  portable, so nothing precludes it later.

---

## 2. Beyond VirtualDJ

Parity is the floor. These are the reasons to switch.

### Stem engine done right

VirtualDJ separates on the GPU in realtime. We separate **outward from the playhead** into a
persistent cache, which means the audio path adds exactly zero latency, the second load of a
track is instant, and the quality ceiling is set by the best available model rather than by
what fits in a 5 ms budget. On top of that:

- per-stem EQ and filter, not just per-stem volume — **shipped**, and composed with the deck's channel strip rather than fighting it;
- **stem-aware transitions** — drop the incoming vocal over the outgoing instrumental, planned
  and executed as one action rather than four hands;
- **stem swapping across decks** — take deck 1's vocal onto deck 2's instrumental as a
  first-class operation;
- stem isolation as a mixing surface, with the beat grid and phrase markers to align it;
- **the stems as four physical outputs** — one deck's parts on four stereo
  pairs, ahead of djmanzo's own EQ and fader, so an external mixer or a DAW
  gets the separation rather than the separation-plus-our-opinion-of-it.

The best separation model is a **download rather than part of the package** --
it is tens of megabytes and carries its own licence, so it is the DJ's to
accept. But an application whose headline feature only works after finding and
installing a 60 MB file does not have that feature on the night, so djmanzo
ships a **built-in separator** that needs no model, no runtime and no download:
harmonic/percussive separation over an FFT, split by band and by how centred a
sound is. Stems work out of the box; the panel names which separator is running
and says what a model would improve.

Separated audio reaches the deck without the audio thread ever taking a lock:
the worker publishes an immutable table of chunks and swaps it in atomically.
That is not incidental tidiness — the earlier lock-based handoff meant a muted
stem came back every time the worker appended, which is exactly the moment a
DJ would notice.

djmanzo also does not start ONNX Runtime speculatively to find out whether it
is there, because a missing runtime takes the process down at exit rather than
returning an error.

### A platter that behaves like a platter

A jog wheel is the control a DJ touches most, so djmanzo is precise about what
it does. One turn moves one turn of a record. A hand on the top in vinyl mode
stops the music and drives it; the same hand in CDJ mode only nudges the tempo.
The rim always bends. A paused deck searches, with sound, because that is how a
cue point is found by ear.

The two halves are deliberately different kinds of control: a scratch is a
*position* and is applied the instant it arrives, while a bend is a *speed* and
is estimated over time. That is why a scratch feels attached to the hand and a
bend does not jump — and why neither changes when you pick a different audio
buffer size.

### Motorised platters, without the revolution

A motorised platter — a Rane Twelve, a Denon SC6000M — reports its **angle**,
not its movement, and that angle wraps at zero. Treating it like an ordinary
jog wheel plays a whole revolution of audio backwards every time the record
goes round, which is the kind of bug that sounds like the software is broken
because it is.

djmanzo treats it as its own kind of control, with the number of steps in a
revolution declared in the mapping because every device counts differently. It
takes the short way round two readings, which is not a guess: at playing speed
a platter covers three thousandths of a turn between reports, so the long way
is physically impossible. And when a reading is too far to believe — a dropped
packet, a cable knocked — it reports nothing rather than lurching the record,
because the truth is that nobody knows how far it went.

### A controller you can map yourself

Every DJ application claims that mappings are files you can edit. djmanzo means
it in both directions: the files are plain TOML with the same action grammar
the interface and the assistant use, *and* you can make one without reading a
manual — press a control, choose what it does, save.

Two things make that safe rather than merely convenient. Learning suppresses
whatever the control already does, so mapping a play button does not start the
deck. And a binding is checked against the engine's vocabulary the moment it is
made, so a mistake is a sentence while you are still looking at the control
rather than a pad that quietly does nothing an hour into a set.

### The universal hardware bridge

No other application speaks Pro DJ Link *and* StagelinQ *and* network tempo sync *and* MIDI
clock. djmanzo does: walk into a club with CDJs already running, join the link network, and be
in phase — as a peer, not as a replacement. This is the feature that makes djmanzo usable in
rooms it does not control.

### Deterministic set replay and offline re-render

The action bus is an ordered, timestamped log, so a performance is fully described by data,
not just by its audio. That gives:

- **replay** — watch or hear a set exactly as it was played;
- **offline re-render** — regenerate the master at studio quality with no realtime deadline, so
  the recording is better than the live output;
- **practice loops** — isolate a 30-second transition and rehearse it against the same
  starting state, repeatedly;
- **diffing takes** — see what you did differently the second time.

This falls out of the architecture rather than being built on top of it. See
[ARCHITECTURE.md §1.1](ARCHITECTURE.md#11-one-action-bus-one-parameter-registry).

### Structure-aware assistance

Phrase and section detection (intro / build / drop / breakdown / outro) drives:

- phrase-locked looping and beat jumping — jump to the *next 16 bars*, not the next 16 beats;
- transition planning that suggests *where* as well as *what*;
- next-track suggestions ranked by harmonic compatibility, energy trajectory and phrase fit —
  and which **explain their reasoning** rather than presenting an opaque score.

### First-class Linux

PipeWire and JACK are supported properly, not as PulseAudio with extra steps. Xubuntu is a
tested target in CI, and waveform performance under WebKitGTK is a benchmarked requirement from
M1 rather than a late discovery. Linux DJs are currently served by exactly one application; we
intend to be the second one that actually works.

### A DJ you can talk to

Voice control with a wake phrase and a push-to-talk fallback, understanding
Spanish and English and the mix of the two people actually speak. Session
planning in the terms a DJ thinks in -- "half an hour of warm-up bachata, then
build through merengue into dembow" -- with live steering that adjusts the
remaining plan instead of discarding it. A domain pack for Dominican and
Caribbean repertoire that knows what a bachata-to-merengue transition really
costs. Songs generated on request for the people in the room. Full design in
[ASSISTANT.md](ASSISTANT.md).

The assistant holds no privileged access: it emits the same action text a
controller does, so everything it does is visible, reversible and replayable.

### An open extension surface

- **CLAP** effect hosting — a genuinely permissive plugin standard with a real ecosystem.
- **WebSocket + OSC control API** speaking the same Actions and Parameters the UI uses, so
  lighting rigs, OBS overlays, stage automation and phone remotes are all first-class clients.
- **Art-Net / DMX** output driven by beat and structure data.
- **Community mappings** as data files — a new controller is a pull request, not a release.

---

## 3. Interface and workflow map

The layout follows VirtualDJ's, because that is the handling we are cloning. Our own assets,
our own code.

```
┌──────────────────────────────────────────────────────────────────────────┐
│ TOP BAR   master level · CPU/latency · recording · layout preset · config │
├──────────────────────────────────────────────────────────────────────────┤
│ WAVEFORM ZONE                                                            │
│   stacked or parallel scrolling waveforms · beat grid · cues · loops     │
│   phrase markers · playhead centred, tracks move                         │
├───────────────────────┬──────────────────────┬───────────────────────────┤
│ DECK A                │ MIXER                │ DECK B                    │
│  overview waveform    │  channel faders      │  overview waveform        │
│  title/artist/key/BPM │  3-band EQ + filter  │  title/artist/key/BPM     │
│  jog / platter        │  gain · VU · PFL     │  jog / platter            │
│  play · cue · sync    │  crossfader + curve  │  play · cue · sync        │
│  pitch fader          │  master · booth      │  pitch fader              │
│  loop controls        │                      │  loop controls            │
├───────────────────────┴──────────────────────┴───────────────────────────┤
│ PAD ZONE   page selector: Cues · Loops · Roll · Slicer · Sampler ·        │
│            Stems · FX          8 pads per deck, colour-coded              │
├──────────────────────────────────────────────────────────────────────────┤
│ FX ZONE    slots · beat-synced timing · routing · presets                 │
├───────────────────┬──────────────────────────────────────────────────────┤
│ FOLDER TREE       │ SONG LIST                         │ SIDEVIEW         │
│  local library    │  sortable columns, instant search │  Sidelist        │
│  playlists/crates │  BPM · key · energy · rating      │  Sampler         │
│  smart folders    │  colour-coded, drag to deck       │  Automix         │
│  imported sources │                                   │  Karaoke         │
│                   │                                   │  Clone           │
└───────────────────┴───────────────────────────────────┴──────────────────┘
```

**Layout presets** trade complexity for screen space, as VirtualDJ's do:

| Preset | Shows |
|---|---|
| Starter | 2 decks, 160 px waveforms, no pads, loops, filter or keylock — for learning |
| Essentials | 2 decks, 120 px waveforms, cues, loops and the EQ |
| Pro | 4 decks, everything on screen, browser open |
| Performance | 4 decks, 72 px waveforms, density 0.85 — for a controller-driven set |

**Skinning** is CSS themes plus JSON layout definitions. Layout presets are just built-in
skins, which keeps one mechanism instead of two: the four above are ordinary `Layout` values
that happen to ship, and a DJ's own is JSON read from `layouts/` in the config directory by the
same code. A layout says which components are on screen, how tall the waveform lane is, and
one overall density; **it cannot execute code**, reach a file, or change what any control does,
which is what makes one somebody sent you safe to load. Every field has a default, so a file
names only what it changes; out-of-range values are clamped rather than refused; a malformed
file is skipped with a warning rather than costing the DJ their other layouts mid-set. The
choice is stored by name and restored at start-up, so editing your own layout file takes
effect.

Moving and restyling individual components — as opposed to showing, hiding and resizing them —
is not in yet. It needs a component-addressing scheme that survives the interface changing
underneath it, which is a design problem rather than a coding one.
[ADR-0008](adr/0008-one-widget-vocabulary.md) is that design: a widget registry, and a layout as
a tree of addressed instances in named slots rather than a struct of feature flags. Decided, not
yet implemented.
