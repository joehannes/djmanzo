# Architecture

djmanzo is a **Rust realtime core** with a **Tauri 2 web UI**, targeting macOS (Apple Silicon
and Intel) and Linux/Xubuntu as equal first-class platforms.

The rest of this document is the reasoning and the shape. Decisions with alternatives worth
recording live in [`adr/`](adr/).

---

## 1. The three load-bearing ideas

Most of this architecture is conventional for an audio application. Three choices are not, and
everything else arranges itself around them.

### 1.1 One action bus, one parameter registry

Every user intent — a click in the UI, a MIDI note from a controller, a keyboard shortcut, a
line in a user script, a command over the network API — is converted into a **typed `Action`**
and pushed onto a single ordered bus. Nothing in the system acts on hardware input directly.

Every observable value — deck position, fader levels, BPM, loop state, VU — lives in a
**`ParameterRegistry`**: a fixed, pre-allocated table of atomics that the audio thread can read
and write without locking, and that everything else observes.

VirtualDJ's scripting model ("`deck 1 play`", "`loop 4`") and Mixxx's `ControlObject` registry
each solve half of this. Combining them, and making both halves type-safe and allocation-free
on the hot path, is what buys us:

- controller mapping, keyboard mapping and macros for free — they are all just Action producers;
- a remote-control API (WebSocket/OSC) that is not a bolt-on but the same door everyone uses;
- scripting with no privileged access to internals;
- and, because the bus is an **ordered, timestamped event log**, deterministic replay.

That last one is the differentiator. A recorded set is not only audio: it is the exact sequence
of actions that produced it. That means you can replay a performance, re-render it offline at
studio quality with no realtime constraint, diff two takes of the same transition, or loop a
30-second passage to practise it. No competitor ships this, and we get it as a side effect of
the control model rather than as a feature we have to build.

See [ADR-0003](adr/0003-action-bus-and-parameter-registry.md).

### 1.2 The UI shell is replaceable; the renderer is not

Waveform drawing is the single hardest UI problem in a DJ app: two to six lanes of scrolling
audio at 60 fps, under a webview, on a laptop that is also doing realtime DSP.

Tauri on Linux renders through WebKitGTK, and WebKitGTK's WebGL is a known hazard — contexts
can be created successfully and still be backed by a software rasteriser, with no reliable way
to detect it because the renderer string is masked for fingerprinting protection. Reports of
canvas animation dropping to single-digit fps on Linux are common. Xubuntu is a stated target
platform, so "it works on my Mac" is not an acceptable answer.

Therefore: **waveforms are rasterised in Rust** (`wgpu`, offscreen) into scrolling tiles. The
webview only *translates* those tiles with CSS transforms — pure compositor work, no JS
per-frame drawing. Beat grid, cues and loop regions are overlay layers transformed identically.

The pay-off is twofold. It is fast on WebKitGTK because we never ask WebKitGTK to draw
anything. And because the renderer lives in Rust and knows nothing about the shell, we can
replace the webview with a native `wgpu` window without touching it, if the webview ever proves
insufficient. The escape hatch is designed in from day one instead of being a rewrite.

See [ADR-0004](adr/0004-waveform-rendering-strategy.md).

### 1.3 Stems are separated ahead of the playhead

Neural source separation good enough to perform with is too expensive to run per audio
callback. Separation quality and realtime inference are in direct tension, and the low-latency
research models give up exactly the quality that makes live stem work useful.

So djmanzo never separates in the audio path. A background worker separates a **rolling window
ahead of the playhead** and writes results into a content-hashed on-disk cache. Playback is
then plain four-channel mixing: **zero added latency**, instant on any later load of the same
track, and a seek just re-primes the window.

---

## 2. Process and thread model

```
┌─ UI process — WKWebView (macOS) / WebKitGTK (Linux) ───────────────────┐
│  Svelte 5 + TypeScript · skin & layout system · DOM/CSS overlays       │
│  waveform tiles as textures, moved by CSS transform only               │
│                                                                        │
│      ↑ state snapshots @60Hz (Tauri Channel)    ↓ Actions (IPC)        │
└────────────────────────────────────────────────────────────────────────┘
┌─ Rust host process ────────────────────────────────────────────────────┐
│                                                                        │
│  control thread    MIDI/HID I/O · mapping engine · scripting           │
│  net thread(s)     Pro DJ Link · StagelinQ · tempo sync · WS/OSC       │
│  worker pool       decode & prefetch · analysis · stem separation      │
│  render thread     wgpu offscreen waveform tiles                       │
│                                                                        │
│  ══════════ AUDIO CALLBACK THREAD ═══════════════════════════════════  │
│   no allocation · no locks · no I/O · no logging · no panics           │
│   decks → per-deck DSP → stem mix → FX → mixer → limiter → device      │
│  ═════════════════════════════════════════════════════════════════════ │
└────────────────────────────────────────────────────────────────────────┘
```

The audio callback is the only hard-realtime context in the system, and it is the *only* place
subject to those restrictions. Crossing into it is allowed exactly two ways:

1. **`rtrb` SPSC ring buffers** — Actions in, telemetry out. Bounded, lock-free, pre-allocated.
2. **Atomics in the `ParameterRegistry`** — for continuous values that can tolerate being read
   slightly stale (fader positions, gains).

All audio buffers are allocated when the device is opened and never again. Enforcement is a CI
concern, not a discipline concern: `dj-engine` is built with an allocation-denying allocator
under test, and any xrun in the integration suite fails the build.

---

## 3. Signal flow

### Per deck

```
  file
    │
    ▼
  CachingReader ──────── decoded frames, kept ahead of the playhead by a worker;
    │                    the callback never touches the filesystem
    ▼
  position / rate engine ── scratch, jog, pitch fader, sync, quantize
    │
    ▼
  resample ──────────────── always. One playhead, one step, one arithmetic,
    │                       whatever keylock is doing
    ▼
  keylock (insert)
    ├─ ON  → transpose by 1/tempo, undoing the pitch the speed introduced.
    │        Reads ahead by the shifter's group delay so engaging it does not
    │        move the music in time. See ADR-0007.
    └─ OFF → straight through (pitch follows speed — the correct behaviour
             for scratching and for turntable feel)
    │
    ▼
  stem split ──────────── 4 buffers from the stem cache, or 1× passthrough
    │
    ▼
  per-stem gain / EQ / FX
    │
    ▼
  deck EQ (3-band) + filter → deck gain → VU
    │
    ▼
  pre-fader FX chain
```

### Mixer

```
  decks 1..N ─┐
  samplers  ──┤→ channel faders → crossfader (curve-configurable)
  mic/aux   ──┘        │
                       ├──→ master FX → limiter → MAIN OUT
                       ├──→ booth (independent gain)         → BOOTH OUT
                       └──→ PFL/cue mix (cue/master blend)   → HEADPHONE OUT
```

Pull-based, like Mixxx: the mixer asks each active channel to render into its buffer, then
applies gain, crossfader orientation and routing flags. Inactive channels are skipped entirely,
so idle decks cost nothing.

---

## 4. Audio I/O

`cpal` is the starting backend, but it sits behind our own `AudioBackend` trait, because a DJ
application needs things generic audio libraries handle poorly:

- **Four channels on one device** — main + headphone cue from a single controller interface.
  This is the common case and the easy one.
- **Two devices with independent clocks** — e.g. built-in output for the master and a USB
  interface for cueing. Their clocks drift. We run one device as the clock master and
  asynchronously resample the other with a slowly-adapting ratio derived from a
  buffer-fill error signal. Getting this right is what separates a usable app from a clicking one.
- **macOS aggregate devices** — created and inspected via CoreAudio directly.
- **PipeWire and JACK as first-class on Linux**, not as a PulseAudio afterthought. `cpal`
  prefers PipeWire at runtime when available; for JACK-based pro setups we expose our
  ports properly rather than presenting as a single stereo sink.

Target latency: **256 frames @ 48 kHz ≈ 5.3 ms**, configurable down for capable interfaces and
up for troublesome ones. Reported honestly in the UI as round-trip, not just buffer size.

---

## 5. Analysis

Runs on the worker pool at import time and on demand, never in the audio path. Results are
cached in the library database and keyed by content hash, so re-analysis is never repeated and
moving a file does not lose its grid.

| Product | Method | Notes |
|---|---|---|
| Waveform data | multi-resolution min/max/RMS + spectral colouring | feeds the tile renderer |
| Onsets | spectral flux over log-magnitude STFT, adaptive peak picking | |
| BPM + beat grid | comb-filter tempo estimate → dynamic-programming beat tracking | emits a **confidence** value; low confidence disables auto-sync rather than lying |
| Key | HPCP chroma → key-profile correlation | shown in standard *and* Camelot notation |
| Loudness | EBU R128 via `ebur128` | drives auto-gain |
| Structure (M8) | self-similarity + novelty over chroma/timbre | phrase and section boundaries for phrase-locked looping and transition planning |

Every one of these is implemented in-house because the mature libraries are all copyleft — see
[RESEARCH.md § the analysis gap](RESEARCH.md#the-analysis-gap). A labelled regression set of
tracks with hand-verified grids is part of the M2 deliverable, and beat-tracking accuracy is
scored on every change from then on.

---

## 6. Stem engine

```
track loaded
    │
    ├─→ cache hit?  ──yes──→ mmap 4 stem streams, done
    │
    └─→ no: queue separation job
              │
              ├─ separate [playhead, playhead + N s] first (N ≈ 30)
              ├─ hand that window to the engine as soon as it's ready
              ├─ continue separating forward in chunks, in the background
              └─ write each chunk to the content-hashed cache as it completes
```

- **Model**: HT-Demucs exported to ONNX, run through `ort` — CoreML on macOS, CUDA/DirectML
  where available, CPU otherwise.
- **Cache**: keyed by content hash, so the same file at a different path is a hit, and a
  re-encoded file correctly is not. Stored as compressed streams; the cache is bounded and
  evicts least-recently-used.
- **Before the window is ready**: the deck plays the original mix and stem controls are shown
  as pending. Never a silence, never a stall.
- **On seek**: the window re-primes from the new position; already-cached regions are instant.
- **Chunk boundaries**: overlap-add with crossfaded margins, so no seam is audible when two
  separately-processed chunks meet.

The engine sees only four buffers. Everything above is invisible to the audio path.

---

## 7. Control: actions, parameters, mapping

```
  MIDI / HID ─┐
  keyboard   ─┤
  UI         ─┼─→ Mapping engine ─→ Action bus ─→ ┬─→ engine (via rtrb)
  script     ─┤    (declarative       (ordered,   ├─→ library
  network    ─┘     TOML + Lua)       timestamped)├─→ UI state
                                                  └─→ session log ─→ replay / re-render
```

**Actions** are a typed enum — `Deck(2).Play`, `Deck(1).SetLoop(Beats(4))`,
`Mixer.Crossfader(0.35)`, `Pad(3, HotCue(2)).Press`. Text forms exist for scripting and the
network API (`deck 2 play`, in the spirit of VirtualDJ script) but parse to the same enum, so
there is exactly one code path and no stringly-typed logic at runtime.

**Parameters** are a fixed table established at startup — no dynamic registration, no string
lookup on the hot path. Each entry is an atomic with a declared type, range and default. The UI
snapshot pump reads the whole table at 60 Hz and ships a diff to the webview.

**Controller mappings** are data, not code: a TOML file per device describing controls,
resolution (7-bit, 14-bit, absolute/relative), and the Action each produces, plus outbound
feedback (LED, display, motor). Devices that need real logic get an optional Lua script,
executed on the control thread — never anywhere near the audio callback. Mappings ship without
a recompile, so a new controller is a pull request against `mappings/`, not a release.

Motorized platters are treated as a first-class control kind, not a special case bolted onto a
jog wheel: absolute position at high resolution inbound, and motor start/stop ramp and torque
outbound.

---

## 8. Library

SQLite (`rusqlite`), one database, content-hash-keyed tracks. Holds tags, analysis results,
cue points, loops, play history, playlists/crates, and user ratings.

Importers, so a user is not asked to abandon their history:

| Source | Route |
|---|---|
| rekordbox | `rekordcrate` (MPL-2.0) — `export.pdb` + ANLZ files: grids, cues, waveforms |
| Serato | **clean-room** implementation from published format documentation — `triseratops` is AGPL and must not be linked ([RESEARCH.md](RESEARCH.md#2-open-source-prior-art)) |
| Traktor | NML collection XML |
| iTunes / Music | library XML |
| Folders | plain filesystem scan with tag reading |

---

## 9. Interface

The interface is a Svelte 5 + TypeScript application in a webview, and it is a **client of the
action bus like any other** ([ADR-0003](adr/0003-action-bus-and-parameter-registry.md)). It
sends action text in and receives 60 Hz state snapshots back over a Tauri channel; it holds no
authority over the engine and no private path into it. Every control on screen could equally be
a MIDI pad, a script line or a sentence to the assistant, because all four end up in the same
place.

**Nothing on the hot rendering path is drawn in the webview.** Waveform tiles are produced by
`dj-render` in Rust, served over a `wave://` custom protocol as binary PNG, and scrolled by
`translate3d` on a fixed strip — the webview moves a layer and does not repaint. This is
[ADR-0004](adr/0004-waveform-rendering-strategy.md), and it exists because WebKitGTK on Linux
will silently fall back to software compositing. The shell measures its own frame rate and says
so when it does: a webview that quietly loses acceleration otherwise just looks broken, and the
audio engine is unaffected either way.

**State comes from one place.** Track titles, analysis, cue positions and deck state all arrive
in the snapshot rather than being remembered by whichever component triggered the load — a deck
loaded by the browser, the assistant, a preset or a controller must look identical to one loaded
by its own Load button, and the only way to guarantee that is to have no second source.

**Themes** are CSS custom properties on `:root`, light and dark. **Density** is a single
`--density` multiplier on the root font size, which works because every other measurement in the
interface is in `em`.

### Layouts and skins

A layout preset and a skin are the same thing — a description of what is on screen and how
densely — so they are one mechanism rather than two that eventually disagree. The four
built-ins (Starter, Essentials, Pro, Performance) are ordinary `Layout` values that happen to
ship; a DJ's own is JSON read from `layouts/` in the config directory by the same code. Every
field has a default so a file names only what it changes, out-of-range values are clamped rather
than refused, and a malformed file is skipped with a warning rather than costing a DJ their
other layouts at the start of a set. The choice is stored by name, so editing your own file
takes effect.

**A layout is data.** It says what to show and how big; it cannot execute code, reach a file, or
change what any control does. That is what makes one somebody sent you safe to open.

The shipped form is a flat struct of feature flags, which can show, hide, resize and scale a
fixed set of components. It cannot move, reorder or restyle them, a DJ's file cannot name a
widget the binary does not already know, and there is no mechanism waiting for M5's detachable
panels. [ADR-0008](adr/0008-one-widget-vocabulary.md) is the decided replacement: a widget
registry and a layout as a tree of addressed instances in named slots — the same move ADR-0003
made for behaviour, applied to what is on screen. It is not implemented yet.

---

## 10. Crate map

```
crates/
  dj-core       domain types: TrackId, TimePos, Beatgrid, CuePoint, MusicalKey, Action
                — no I/O, no dependencies beyond std + serde; everything else depends on it
  dj-audio      AudioBackend trait, device enumeration, clock/drift correction, cpal backend
  dj-engine     THE realtime crate: decks, scaler, mixer, sampler, sync. RT rules apply here
                and nowhere else.
  dj-dsp        EQ, filters, stretch wrapper, resampler, meters, limiter, effect slots —
                pure, testable, allocation-free
  dj-decode     symphonia + platform fallbacks, CachingReader, prefetch pool
  dj-analysis   beatgrid, BPM, key, loudness, waveform data, structure
  dj-stems      ONNX look-ahead separation + content-hashed cache
  dj-library    SQLite schema, tags, playlists, importers
  dj-control    Action bus, ParameterRegistry, mapping engine, Lua scripting, session log
  dj-hid        midir + hidapi, device profiles, LED/screen/motor feedback
  dj-net        Pro DJ Link, StagelinQ, tempo sync, WebSocket/OSC API, Art-Net
  dj-render     wgpu offscreen waveform tile renderer
  dj-app        Tauri host: wiring, commands, channels, the 60 Hz snapshot pump
ui/             TypeScript + Svelte 5, skin and layout system
mappings/       controller definitions (TOML + optional Lua)
docs/           this directory
```

Dependency direction is strictly downward: `dj-core` at the bottom, `dj-app` at the top,
no cycles. `dj-engine` depends on `dj-dsp` and `dj-core` only — it must stay auditable for
realtime safety, which means keeping its dependency surface tiny.

---

## 11. Extension points

Designed in, not retrofitted:

- **Effects** — CLAP hosting (MIT-licensed standard, real plugin ecosystem). Built-in effects
  use the same internal interface as hosted ones, so there is no second-class path.
- **Controller mappings** — data files, community-contributable.
- **Scripts** — Lua against the Action/Parameter API; the same surface the UI uses.
- **Network API** — WebSocket + OSC, speaking Actions and Parameters. Enables lighting rigs,
  OBS overlays, stage automation, phone remotes, and anything not yet imagined.
- **Skins** — CSS themes plus JSON layout definitions, following VirtualDJ's layout-preset
  concept (Starter / Essentials / Pro / Performance) with our own assets. Shipped in M3 as a
  flat set of feature flags; [ADR-0008](adr/0008-one-widget-vocabulary.md) replaces that with a
  widget registry so a skin can move and restyle rather than only show and hide.
- **DVS** — deliberately deferred, but the position/rate engine already takes its rate from an
  abstract source. A timecode decoder becomes one more rate source rather than a redesign.

---

## 12. Known risks

| Risk | Mitigation |
|---|---|
| WebKitGTK waveform performance on Xubuntu | Rust tile renderer + CSS-transform scrolling from day one; native `wgpu` window as a pre-planned escape hatch. **Benchmarked on real Xubuntu hardware at M1**, not discovered at M7. |
| Realtime-safety violations causing dropouts | RT rules confined to `dj-engine`; allocation-denying allocator in tests; xruns fail CI. |
| Beat/key quality without the copyleft libraries | Published algorithms, plus an optional ONNX model; labelled regression set scored on every change from M2. |
| Undocumented HID / motorized platter behaviour | Start with MIDI-class devices; USB capture for HID units; mappings as data so fixes ship without a release. |
| Pro DJ Link / StagelinQ are unofficial protocols | Optional modules, clearly labelled, clean-room from published analyses, never required for playback. |
| Two-clock audio drift | Explicit async-resampling drift correction with a soak test in CI, not a hope. |
| Scope: this is a multi-year product | Every milestone is independently usable; M0–M2 is already a playable two-deck mixer. See [ROADMAP.md](ROADMAP.md). |
