# Roadmap

Nine core milestones, plus a six-milestone assistant track. The ordering rule is simple: **every milestone ends with something you can
actually use.** M0 plays a file. M2 is a playable two-deck mixer. Nothing is a six-month
foundation with no output.

Feature-to-milestone assignments are in [FEATURES.md](FEATURES.md). Architecture is in
[ARCHITECTURE.md](ARCHITECTURE.md).

---

## M0 — Foundations and walking skeleton

Prove the stack end to end before building anything on it.

- Cargo workspace with the crate skeleton from [ARCHITECTURE.md §9](ARCHITECTURE.md#9-crate-map).
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
