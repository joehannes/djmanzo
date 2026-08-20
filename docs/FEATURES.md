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
| Slicer | M5 |
| Pad pages: Cues · Loops · Roll · Saved · Sampler · FX | M5 — **done** |
| Pad pages: Slicer · Stems | M5 (Stems page M6) |
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
| Per-stem effects | M6 |
| Effect chain presets | M5 |

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
| Mapping editor (learn a control, bind an action) | M4 |
| LED / display feedback to controllers | M4 |
| HID controller support | M4 |
| **Motorized platter support** — high-res absolute position, motor ramp, torque | M4 |
| Controller-specific audio setup presets | M4 |
| **Keyboard as a controller** — same vocabulary and file format as a MIDI mapping, with a live sheet | **shipped** |
| Multi-device audio setup (4-channel, or two devices with drift correction) | M1 |
| **Pro DJ Link** (Pioneer CDJ/XDJ) | M7 |
| **StagelinQ** (Denon Prime) | M7 |
| Network tempo sync (Ableton Link or equivalent) | M7 |
| MIDI clock in/out | M7 |
| DVS timecode vinyl | *deferred — architecture leaves room; see ARCHITECTURE.md §10* |

### Output, recording, broadcast

| Feature | Milestone |
|---|---|
| Record the master to disk (WAV; FLAC/MP3 later) | M5 — **done** |
| Broadcast to Icecast/Shoutcast | M5 |
| Per-deck and stem-bus outputs for external processing | M6 |
| Video mixing / VJ output | M8 |
| Karaoke | K1 / K2 — see [KARAOKE.md](KARAOKE.md) |

### Interface

| Feature | Milestone |
|---|---|
| Layout presets: Starter · Essentials · Pro · Performance | M3 |
| Skin system (CSS themes + JSON layouts) | M3 |
| Multi-monitor / detachable panels | M5 |
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

VirtualDJ separates on the GPU in realtime. We separate **ahead of the playhead** into a
persistent cache, which means the audio path adds exactly zero latency, the second load of a
track is instant, and the quality ceiling is set by the best available model rather than by
what fits in a 5 ms budget. On top of that:

- per-stem EQ and effects, not just per-stem volume;
- **stem-aware transitions** — drop the incoming vocal over the outgoing instrumental, planned
  and executed as one action rather than four hands;
- **stem swapping across decks** — take deck 1's vocal onto deck 2's instrumental as a
  first-class operation;
- stem isolation as a mixing surface, with the beat grid and phrase markers to align it.

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
