# Research — the DJ software landscape

Everything here is background for building **djmanzo**. It answers three questions:

1. What does the product we are cloning actually do?
2. What open-source work already exists, and what can we legally take from it?
3. Which building blocks can a permissively-licensed Rust project actually depend on?

---

## 1. The benchmark: VirtualDJ

VirtualDJ (Atomix Productions) is the product djmanzo mimics and intends to surpass — in
feature set, appearance, workflow and handling. The relevant shape of it, as of the 2026
generation:

- **Decks**: 2, 4 or 6 decks; per-deck EQ, filter, gain, pitch, keylock, sync, quantize.
- **Real-time stems**: tracks split live into vocals / instruments / bass / drums, with
  dedicated stem pads for live mash-ups. Their current engine runs on the local GPU and adds
  no latency to the audio path.
- **Effects**: a large built-in FX collection (120+), pre- and post-fader chains, per-deck and
  master slots.
- **Pads**: hot cues, loops, loop rolls, slicer, sampler, stems, FX — organised into swappable
  pad *pages*.
- **Browser**: folder tree + song list + **SideView**, a panel that carries Sidelist, Sampler,
  Automix, Karaoke and Clone views so the main list stays free.
- **Layouts and skins**: Starter / Essentials / Pro / Performance layout presets, plus a
  documented skin system (XML + bitmaps) with a large third-party skin scene.
- **Video and karaoke**: video mixing for VJ work, dedicated karaoke tooling.
- **Hardware**: plug-and-play mappings for 300+ MIDI/HID controllers, including motorized
  units, with predefined audio setups for controllers that carry their own interface.
- **Automation and AI**: automix, lyrics extraction shown on the waveform, track
  recommendation assistance.
- **Streaming, recording, broadcasting** and integration with the major streaming catalogues.

That list is the parity target. `FEATURES.md` turns it into a matrix with milestone
assignments.

### What we copy and what we never touch

| Do | Don't |
|---|---|
| Clone the **workflow**: what a control does, where it sits, how a DJ moves through a set | Copy any VirtualDJ code, skin graphics, icons, fonts, sound assets |
| Clone the **feature set** and the ergonomics that make it fast under pressure | Use their trademarks, or imply endorsement/compatibility |
| Read public manuals and docs to understand behaviour | Decompile, or ship anything derived from their binaries |
| Support the same *hardware* they support | Reuse their mapping files verbatim |

Features and layout conventions are not protectable the way code and artwork are. We stay
firmly on the safe side of that line: **our own code, our own art, their good ideas.**

---

## 2. Open-source prior art

### Mixxx — the reference architecture

[mixxxdj/mixxx](https://github.com/mixxxdj/mixxx) is the only mature open-source DJ
application. C++/Qt, cross-platform, GPL-2.0-or-later, very large (400+ source files in `src/`
alone), 100+ built-in controller mappings, working DVS, and since 2.6 (beta May 2025) playback
of Native Instruments STEM files.

Its architecture is the best-documented example of how this kind of app is shaped, and it
matches what we independently need:

- `EngineMixer` orchestrates and **pulls** audio: it asks each active `EngineChannel` to
  `process()` into its buffer, then applies volume, crossfader orientation and PFL routing.
- Decks and samplers are the same thing to the engine — both are `EngineDeck`, an
  `EngineChannel` that chains `EngineBuffer` → pregain → pre-fader FX → VU meter.
- `EngineBuffer` is where the player logic lives: decode, resample, loops, hot cues, sync.
- A `CachingReader` keeps decoded audio ahead of the playhead so the callback never touches
  the filesystem.
- `ControlObject` is a global, name-addressed registry of parameters; any thread can look one
  up by `ConfigKey` through a `ControlProxy`. It is the cross-thread nervous system.
- The audio callback thread runs hundreds of times a second and **must not** do I/O or take
  mutexes; everything else is asynchronous around it.

**We cannot use it as a base.** GPL-2.0-or-later would force djmanzo to be GPL too, which
conflicts with the licensing decision in [ADR-0002](adr/0002-clean-room-permissive-licensing.md).
It is also a two-decade-old C++ codebase whose UI and workflow are explicitly *not* what we
want — we want VirtualDJ's. So Mixxx is a **teacher, not a parent**: read it and its wiki to
understand the problem shape, then write our own in Rust.

### The rest of the field

| Project | Language / license | What it teaches us |
|---|---|---|
| [xwax](https://xwax.org/) | C, GPL-2.0 | Timecode/DVS decoding. Mixxx embeds its decoder. ⚠️ **GPL — not read, not linked, not transcribed.** `dj-dvs` is written from the prose descriptions below. |
| Mixxx's DVS write-ups ([pt1](https://mixxx.org/news/2021-11-21-dvs-internals-pt1/), [pt2](https://mixxx.org/news/2021-12-22-dvs-internals-pt2/), [pt3](https://mixxx.org/news/2025-08-27-dvs-internals-pt3/)) | Articles | How timecode vinyl actually works: Serato 1 kHz / Final Scratch 1.2 kHz / Traktor MK2 2 kHz carriers, zero-crossing pitch detection, Nyquist-bounded max scratch speed (~22× for Serato, ~11× for Traktor MK2). **Prose, not code — safe to learn from.** This is what `dj-dvs` was built from. |

### A note on the published timecode parameters

The tap value that circulates in prose for the Serato record — a 20-bit
register with seed `0x59017` and taps `0x361e4` — **could not be reproduced as
a working register here.** Walked in Galois, Fibonacci-right, Fibonacci-left
and reversed-tap conventions, it cycles after 43,307 states rather than the
1,048,575 a maximal 20-bit register gives. At a 1 kHz carrier that is a
position that repeats every forty-three seconds.

Either the figure is garbled in transmission or the convention behind it is one
this project has not reproduced. Without a real record and a turntable there is
no way to tell, so djmanzo **does not ship it**: `TimecodeFormat::is_usable`
walks a candidate register and refuses one that is not maximal, and a test pins
that these particular numbers are refused so nobody pastes them back in.

What djmanzo ships instead is its own timecode, whose parameters are verified
maximal — and a synthesiser, so a DJ can generate the signal and use any
turntable or CD deck without buying a licensed record.
| [Deep-Symmetry/dysentery](https://github.com/Deep-Symmetry/dysentery) + [beat-link](https://github.com/Deep-Symmetry/beat-link) | Java | Pro DJ Link: how CDJs announce themselves, share beat/tempo/on-air state and track metadata over Ethernet. Deep Symmetry publishes a protocol analysis document — that document is our input, not their Java. |
| [icedream/go-stagelinq](https://github.com/icedream/go-stagelinq), [Jaxc/PyStageLinQ](https://github.com/Jaxc/PyStageLinQ) | Go / Python | Denon Prime StagelinQ: discovery handshake and the device state map. Also community-reverse-engineered, also documented in prose. |
| [Holzhaus/rekordcrate](https://github.com/Holzhaus/rekordcrate) | Rust, **MPL-2.0** | rekordbox device exports: the `export.pdb` database and ANLZ analysis files (beat grids, cues, waveforms). MPL-2.0 is file-level copyleft — **we can depend on this crate directly** and keep djmanzo permissive. |
| [Holzhaus/triseratops](https://github.com/Holzhaus/triseratops) | Rust, **AGPL-3.0-or-later** | Serato tag/database formats. ⚠️ **AGPL — we must not link it.** Serato import gets a clean-room implementation from the published format documentation instead. |
| [Ableton/link](https://github.com/Ableton/link) | C++, GPLv2+ **or** proprietary | Network tempo/phase/start-stop sync, already in Serato DJ, Reason and Max. Dual-licensed: a proprietary licence is available from Ableton on request. Decision deferred to when we build `dj-net` — either take their licence or implement the documented protocol ourselves. |

### Hardware protocol notes

- Most DJ controllers are **class-compliant USB-MIDI**; a plain MIDI in/out plus a mapping
  layer gets us the majority of the market. This is the M4 starting point.
- **HID** is used where MIDI's 7-bit resolution isn't enough — notably Native Instruments'
  newer controllers, which ship OS drivers that translate HID→MIDI on macOS and Windows but
  *not* on Linux. Cross-platform support therefore means speaking HID directly.
- **Motorized platters** (Rane Twelve, Hercules Inpulse T7, Denon SC5000M, Rane Performer)
  need high-resolution absolute platter position, plus *outbound* control for motor start/stop
  ramps and torque. Our mapping format must handle 14-bit/high-res values and output feedback
  from the start, not as a bolt-on.
- Manufacturer documentation is routinely incomplete. The established community method is to
  capture USB traffic while the vendor's own software drives the device, then decode it. Mixxx
  maintains a wiki page on exactly this, and it is how mappings for the Denon MC4000 and
  similar were produced.
- **Network protocols** (Pro DJ Link, StagelinQ) are unofficial. They stay optional modules
  that never gate core playback.

---

## 3. Dependency shortlist

Chosen to keep djmanzo MIT-OR-Apache-2.0. See
[ADR-0002](adr/0002-clean-room-permissive-licensing.md) for the rule.

| Need | Choice | Licence | Note |
|---|---|---|---|
| Audio I/O | `cpal` | Apache-2.0 | CoreAudio, PipeWire, PulseAudio, JACK, ALSA. PipeWire and PulseAudio became first-class Linux backends in 0.18, with runtime selection preferring PipeWire. Raises the callback thread to RT priority (rtkit via D-Bus on Linux). Wrapped behind our own trait — see ADR-0001. |
| Decoding | `symphonia` | MPL-2.0 | Pure Rust, wide container/codec coverage. Platform decoders (AVFoundation on macOS) as fallback for gaps. |
| Resampling | `rubato` | MIT | Async + sync resampling, used both for pitch and for clock-drift correction between devices. |
| Time-stretch / keylock | **Signalsmith Stretch** | **MIT** | Quality comparable to Rubber Band's R3 engine at a usable CPU cost. |
| ~~Rubber Band~~ | — | GPL / commercial | Excellent quality, but GPL-or-pay and historically heavy. **Rejected.** |
| RT-safe queues | `rtrb` | MIT | Lock-free SPSC ring buffer; the only channel type allowed to touch the audio thread. |
| MIDI | `midir` | MIT | Cross-platform realtime MIDI, RtMidi-inspired. **In use** — `dj-hid`. Pulls `coremidi`/`coremidi-sys` (MIT) on macOS, `alsa`/`alsa-sys` (Apache-2.0 OR MIT, MIT) on Linux, `windows` (MIT OR Apache-2.0) on Windows. A thin wrapper over each platform's own MIDI API with no runtime of its own, which is what a callback that has to be cheap wants. |
| HID | `hidapi` | MIT (wrapper) | Direct HID for high-res jogs and NI-class devices. **In use** — `dj-hid`. The backend is chosen per target. **Linux: `linux-native-basic-udev`**, which is pure Rust (`basic-udev` MIT OR Apache-2.0, `nix` MIT) — no C hidapi is compiled and no `libudev-dev` has to be installed first, which is what keeps the `.deb` build buildable on a plain machine. **macOS/Windows:** the crate's vendored C hidapi over IOKit / the native API. That C library is triple-licensed and says so in its own `LICENSE.txt`: *"HIDAPI can be used under one of three licenses ... The license chosen is at the discretion of the user of HIDAPI."* djmanzo elects the **BSD-style** licence (`LICENSE-bsd.txt`), which ADR-0002 permits; the GPL-3.0 option is expressly **not** taken. |
| Lua | `mlua` | MIT | Sandboxed scripting in controller mappings. **In use** — `dj-hid`. `vendored` builds Lua 5.4 from source, also MIT, about 200 KB of C with no system dependencies; a C compiler is already required because keylock builds Signalsmith Stretch from C++. The `send` feature is needed because a script runs on the MIDI thread. **Note for anyone auditing the sandbox:** `StdLib::NONE` does not mean no base library — mlua installs it regardless, so `dofile`, `loadfile` and `load` are removed by name in `crates/dj-hid/src/script.rs` and a test enumerates them. |
| Tags | `lofty` | MIT/Apache-2.0 | Reading/writing metadata across formats. |
| `roxmltree` | MIT OR Apache-2.0 | Read-only XML tree for the library importers. Random access over a document already in memory, which is what rekordbox's id references and plist's key/value pairs need. |
| Library DB | `rusqlite` | MIT | SQLite; boring and correct for a music library. |
| Loudness | `ebur128` | MIT | EBU R128 / ReplayGain for auto-gain. |
| Neural inference | `ort` (ONNX Runtime) | MIT OR Apache-2.0 | One runtime for stems *and* optional beat tracking. Backends: CoreML (macOS), CUDA/DirectML, CPU. Built with `load-dynamic`, so ONNX Runtime itself (MIT, Microsoft) is opened at runtime rather than linked. **It panics when the library is absent**, and the panic poisons a mutex its own `atexit` handler then locks, aborting the process at exit — so `dj-stems` opens the library itself first and refuses to call `ort` at all when that fails. See `crates/dj-stems/src/availability.rs`. |
| Dynamic loading | `libloading` | ISC | Used by `dj-stems` to answer "is ONNX Runtime here?" without touching `ort`. ISC is permissive and ADR-0002-compatible. Already in the tree as `ort`'s own loader. |
| Tensors | `ndarray` | MIT OR Apache-2.0 | Shapes for the separation model's input and output. |
| Lock-free publish | `arc-swap` | MIT OR Apache-2.0 | How a separated chunk reaches the audio thread. The separation worker publishes an immutable table; the deck loads it without waiting. Replaces an `RwLock` the audio thread read with `try_read` -- see ADR note in `dj_decode::StemTable`. |
| Built-in separation | `rustfft` | MIT OR Apache-2.0 | The fallback separator is Fitzgerald's 2010 median-filter harmonic/percussive method, implemented in `dj-stems::hpss` from the published description — arithmetic over an FFT, no weights and no runtime. It is what makes stems work on a machine with no model. |
| Separation model | HTDemucs (ONNX export) | MIT (Demucs, Meta) | **Not bundled.** Tens of megabytes with its own licence, so it is a download the DJ chooses; the application looks for it under its data directory and says so plainly when it is not there. |
| Effects hosting | **CLAP** | MIT | Truly permissive plugin standard from u-he/Bitwig, with a real ecosystem (~15 hosts, ~400 plugins). Preferred over VST3, whose SDK only moved to GPLv2 in late 2025 and remains Steinberg-controlled. |
| CLAP host bindings | `clack-host` | MIT OR Apache-2.0 | Safe Rust wrappers over the CLAP C API. Its thread split — a `!Send` instance on the main thread, a `Send` audio processor — is the same one djmanzo already has, so a plugin's processor crosses on the command queue like a track buffer. Brings `clack-common` and `clap-sys`, both MIT OR Apache-2.0. |
| CLAP extensions | `clack-extensions` | MIT OR Apache-2.0 | Parameters, audio ports, latency, state. |
| CLAP plugin bindings | `clack-plugin` | MIT OR Apache-2.0 | **Test-only**, behind `dj-clap`'s `test-plugin` feature. Used to compile a real CLAP plugin into the test binary, because there are no `.clap` bundles on a CI machine and a host tested against nothing is a host tested nowhere. Not in a shipped build. |
| rekordbox import | `rekordcrate` | MPL-2.0 | Compatible; file-level copyleft only. |
| UI shell | Tauri 2 | MIT/Apache-2.0 | See ADR-0001. |
| Opening a link | `tauri-plugin-opener` | MIT OR Apache-2.0 | Hands a URL to the operating system. A webview cannot reach a browser by itself — `target="_blank"` inside a Tauri window opens nothing at all on Linux — so every external link needs this, including the WhatsApp handoff in A6. The plugin's own IPC command is deliberately **not** granted to the webview: djmanzo opens URLs from Rust, having first checked them against its own catalogs. See `commands::open_signup_link`. |
| Lyrics | LRCLIB (no crate) | service, no key | `https://lrclib.net/api/get`, called through the crate's own `HttpClient` trait. The only lyrics service djmanzo can use for this: every other one wants a key, a contract, or both, and most **forbid storing what they return** — which is exactly what "search the words you remember" has to do, since searching means having the text before the question is asked. LRCLIB has no key, no account, a community database meant to be used this way, and MIT client software. It asks callers to identify themselves in a `User-Agent`, which djmanzo does. **It is looked up by artist and title, not searched by lyric text**, so this fetches words for records djmanzo already knows about and the searching happens locally — a real limit, and the honest shape of the feature. The duration is sent because the seven-minute mix has different words in different places than the single. **The live call is unverified from this container**: the egress proxy denies `lrclib.net`, so the parser is tested against the documented response shape through `StubClient` and has never seen a real one. The sweep, the storage and the miss-remembering *have* been driven end to end in the running application. |
| Serving a room | `tiny_http` | MIT OR Apache-2.0 | The HTTP server behind the audience page — song requests from a printed URL, and a phone reporting what the room looks and sounds like. Chosen for what it is *not*: no async runtime, no framework, no middleware stack, in a crate that has none of those already. Brings `ascii` (Apache-2.0 OR MIT), `chunked_transfer` (Apache-2.0 OR MIT) and `httpdate` (MIT OR Apache-2.0). Request parsing is written by somebody else on purpose — header handling, chunked bodies and request smuggling are the whole attack surface of a socket that faces strangers, and the one part djmanzo keeps for itself is the size limit. See `dj_net::web`. |
| QR codes | `qrcode` | MIT OR Apache-2.0 | The square on the sticker and on the screen. Built with `default-features = false, features = ["svg"]`, which brings **no transitive dependencies at all** — the default features pull an image encoder djmanzo does not want, because a QR going onto paper and into a page has to stay sharp at whatever size a printer picks. Its renderer emits a standalone XML document; `dj_net::sticker::qr_svg` cuts the prolog, since inside HTML a prolog is a bogus comment rather than a declaration. |
| Answering to a name | `mdns-sd` | Apache-2.0 OR MIT | Multicast DNS, so `http://djmanzo.local:7331/` resolves on the venue's network and a sticker can be printed before anybody knows what the venue's router hands out. Pure Rust, its own thread, nothing near the audio path. Brings `flume` (Apache-2.0/MIT), `if-addrs` (MIT OR BSD-3-Clause), `socket-pktinfo` (MIT) and `spin` (MIT). Chosen over doing without because the alternative is a sticker carrying an address that is only true in one building. **It does not make every phone resolve the name** — Apple devices always, Android since 12 and not on every build — which is why `dj_net::sticker` offers the plain address beside it and prints the caveat instead of hiding it. |
| Layout budget | `@playwright/test` | Apache-2.0 | **Development only**, never in a shipped build. Drives a real browser to measure *where the controls actually land* at djmanzo's own 1280x800, which is the only kind of test that can catch the failure that has now happened three times: a control a DJ performs with sitting below the fold. A template assertion passes while the crossfader is 900 px off the screen, and jsdom does no layout at all, so nothing cheaper answers the question. It drives **Chromium**, which is *not* the engine djmanzo ships on — the application runs in WebKitGTK — so every assertion carries slack and the limitation is written down beside the number in `ui/e2e/shell.ts`. Brings `playwright` and `playwright-core` (both Apache-2.0) and nothing else. |
| URL escaping | `urlencoding` | MIT | Percent-encoding, for the source APIs in `dj-sources` and the shared tracklist in `dj-app::share`. Small enough to have written by hand and exactly the kind of thing that is wrong when written by hand — the failure is a set list truncated at the first `&` in an artist name. |

### The phone as a room sensor, and why it is not one yet

A phone can sit on a speaker stack facing the floor while the laptop faces the
DJ, which makes it the obviously right instrument for measuring what a room is
doing. It is not the instrument djmanzo uses, and the reason is worth writing
down so it is not rediscovered.

**`getUserMedia` requires a secure context.** Chrome, Firefox and Safari all
refuse camera and microphone access on an `http://` origin that is not
`localhost` — and `DeviceMotion` on iOS 13+ needs both a secure context and a
gesture-initiated `requestPermission()`. `AmbientLightSensor` is Chrome-only
behind a flag, so light has to come from the camera anyway. A page served over
plain HTTP from a laptop on a venue's wifi can therefore measure **nothing**.

So the sensor page needs HTTPS, and djmanzo would have to serve it. The obvious
route is `tiny_http`'s `ssl-rustls` feature. It was tried and **refused**:

| What it pulls | Version resolved | Why that is a problem |
|---|---|---|
| `rustls` | **0.20.9** | Pinned by `tiny_http` 0.12, which is its latest release (2023). |
| `ring` | **0.16.20** | Forced by that rustls. Unmaintained. |

Nineteen packages, and the two that matter are a 2021-era TLS stack and an
unmaintained cryptography crate, on a socket facing a club's wifi with a
self-signed certificate that every phone would warn about anyway. That is not a
trade worth making for a convenience, so it was not made — and the whole point
of ADR-0002's dependency discipline is that this decision gets recorded rather
than quietly taken.

**What djmanzo does instead:** its own window is a secure context, because a
Tauri webview is served from `localhost`. So the camera and microphone are
opened there, in `ui/src/RoomSense.svelte`, and the same three numbers reach
`dj_assistant::room`. A USB webcam on a long cable puts the lens where a phone
would have gone.

**What would unblock the phone:** a `tiny_http` release on a current `rustls`,
or a different embedded server with one. Nothing else about the design changes —
the page, the readings and the model are all in place and none of them care
where the numbers came from.

### The analysis gap

Every mature beat/key detection library is copyleft: `aubio` (GPL), `libKeyFinder` (GPL),
`Essentia` (AGPL), `BTrack` (GPL). None can be linked into a permissive djmanzo. The
algorithms themselves, however, are published research. `dj-analysis` therefore implements:

- **Onsets** — spectral flux over a log-magnitude STFT, adaptive-threshold peak picking.
- **Tempo** — autocorrelation / comb-filter bank over the onset envelope, with octave-error
  correction biased to DJ-relevant ranges.
- **Beats** — dynamic-programming beat tracking over the onset envelope (Ellis's published
  method), yielding a beat grid with a confidence score.
- **Key** — HPCP/chroma → correlation against key profiles (Krumhansl-Schmuckler / Temperley),
  reported in both standard and Camelot notation.
- **Optional** — a small ONNX beat-tracking model as a second opinion for difficult material,
  reusing the runtime we already ship for stems.

This is real work, and it is a genuine quality risk. Mitigation: a labelled regression set of
tracks with hand-checked grids, scored on every change, from M2 onward.

### Stems: why look-ahead, not per-callback

HT-Demucs is the strongest open separator (`#1` on MUSDB18-HQ) and now exports to ONNX cleanly,
running on CoreML/CUDA/CPU without PyTorch at inference. But it is a large model: real-time,
per-buffer inference on a laptop is not realistic, and the low-latency research models trade
away the quality that makes stems worth using live.

So djmanzo separates **ahead of the playhead** into a content-hashed disk cache, and playback
is ordinary 4-channel mixing. Zero added audio latency, instant on any subsequent load, and a
seek simply re-primes the window. Details in [ARCHITECTURE.md § stem engine](ARCHITECTURE.md#6-stem-engine).

---

## Controller mappings: where the numbers come from

A mapping file is a table of facts about a piece of hardware — this button
sends that note. The facts are not anybody's expression, but the *compilations*
of them are, and the largest freely available compilation is Mixxx's, which is
GPL-2.0 and therefore off limits under ADR-0002. So each bundled mapping records
its own source, and none of them came from Mixxx.

| Mapping | Source | Status |
|---|---|---|
| `pioneer-ddj-sr` | Pioneer's own *DDJ-SR List of MIDI Messages* | Vendor documentation, read as reference. Not redistributed. |
| `pioneer-cdj-3000` | Pioneer's own *CDJ-3000X MIDI Message List* | As above. |
| `pioneer-ddj-200` | The shared DDJ controls confirmed against Pioneer's DDJ-SR list; the filter and jog numbers from `laksateef/vdj-ddj200-linux` (MIT) | Two sources that agree. Attributed in the file. |
| `pioneer-ddj-2deck` | The same two sources, for the family: DDJ-400, DDJ-FLX4, DDJ-FLX2, DDJ-SB3 and near relatives | The most widely sold controllers there are. The filter knob is left out because its placement is **not** a family constant — the DDJ-SR puts it on the global channel and the DDJ-200 on the deck's own. |
| `generic-2-deck`, `generic-hid`, `motorised-platter`, `scripted-shift` | Written here from the conventions common to class-compliant controllers | Meant to be edited, and they say so. |

**None of these have been run against the hardware.** Every number matches the
vendor table line for line and no hand in this project has touched a DDJ-SR or a
CDJ. That is a well-sourced starting point, not a tested one, and the files say
so at the top rather than burying it here.

### Two facts about Pioneer hardware worth knowing before reading a table

- **One controller, seven MIDI channels.** Decks 1–4 on channels 1–4, effect
  units on 5–6, browser and crossfader on 7, and the performance pads on 8–11 —
  so a deck's play button and its own pads are on *different channels*.
- **Faders arrive in two halves.** Every fader and knob is 14-bit, split across a
  high-byte control change and a low-byte one 32 controllers above it. This is
  ordinary MIDI, not a Pioneer invention: controllers 0–31 are high bytes and
  32–63 are their partners, and Denon and Native Instruments do the same.
  djmanzo's `cc14` names the high byte and pairs them; binding the high byte
  alone with `cc` would work and would throw away half the resolution.

### What was checked, and what could not be

The vendor PDFs are hosted on `pioneerdj.com` and `support.alphatheta.com`, and
**both are blocked by this project's network egress policy** — the gateway
answers 403 to the CONNECT, as it does for every other manufacturer's
documentation host tried (Native Instruments, Denon, Novation, Akai, Numark,
Reloop, Hercules, and the Internet Archive). GitHub and the package registries
are reachable; nothing else is. Where a table below was obtained, it was because
the document had been vendored into a repository that *is* reachable, or
supplied directly.

### Permissively licensed mapping repositories

Licences below were **verified by cloning each repository and reading its own
LICENSE file**, not taken from any summary:

| Repository | Licence | Verified |
|---|---|---|
| `flesniak/python-prodj-link` | Apache-2.0 | Yes — Apache License 2.0 text |
| `pestrela/dj_maps` | MIT | Yes — "Copyright (c) 2020 Pedro Estrela" |
| `rbax/mixxx-mappings` | MIT | Yes — "Copyright (c) 2021 Ryan Baxter" |
| `laksateef/vdj-ddj200-linux` | MIT | Yes — MIT |
| `marcan/Mixxx-Pioneer-DDJ-SX2` | MIT | Yes — "Copyright (c) 2014 Hilton Rudham" |

A permissive licence on a repository covers **that author's** work. Where one of
these vendors a manufacturer's PDF, the PDF remains the manufacturer's and is
treated as reference only. Nothing from `mixxxdj/mixxx` itself is used.

### One tempting source that was refused

A GitHub document titled *DDJ-400 MIDI map* gives a complete byte table for the
DDJ-400 and DDJ-FLX4 — the two most popular controllers in the world — and it is
tempting. It states its own source in its second paragraph: **the Mixxx DDJ-400
mapping**, which is GPL-2.0. The repository carries no licence of its own. It was
not used.

It did serve one purpose, read and not copied: it independently describes the
same family layout — decks on channels 1 and 2, mixer on 7, pads on 8 and 10,
faders 14-bit with the low byte 32 controllers up — which djmanzo already had
from Pioneer's own DDJ-SR list and from the MIT DDJ-200 repository. Two
independent non-GPL sources agreeing is what `pioneer-ddj-2deck` rests on;
nothing in it comes from that document.

### Why there are no mappings for other brands yet

Not for want of looking. For Denon, Numark, Hercules, Reloop and Roland, the
only substantial compilation of MIDI numbers in public circulation is **Mixxx's
own**, which is GPL-2.0 and therefore excluded — and every manufacturer's
documentation host is blocked by the egress policy above.

Native Instruments is a different case and worth stating separately: on a
Traktor Kontrol the MIDI notes and control numbers are **not fixed**. They are
whatever the owner has set in NI's Controller Editor, so there is no vendor
table to ship and a bundled mapping would be guessing at one machine's
settings. Those controllers want the mapping editor's learn mode, not a file.

The way to add a brand is the way Pioneer was added: someone supplies the
manufacturer's MIDI message list, and it gets transcribed.

---

## Sources

- [VirtualDJ](https://virtualdj.com/) — product, manual, hardware list, skin SDK
- [Mixxx](https://mixxx.org/) — [source](https://github.com/mixxxdj/mixxx), [developer wiki](https://github.com/mixxxdj/mixxx/wiki/Developer-Guide-Engine), [DVS internals series](https://mixxx.org/news/2025-08-27-dvs-internals-pt3/), [STEM support](https://mixxx.org/news/2024-08-26-stem-mixing/)
- [xwax](https://xwax.org/)
- [Deep Symmetry — dysentery](https://github.com/Deep-Symmetry/dysentery) / [beat-link](https://github.com/Deep-Symmetry/beat-link)
- [go-stagelinq](https://github.com/icedream/go-stagelinq) · [PyStageLinQ protocol notes](https://github.com/Jaxc/PyStageLinQ/blob/main/StageLinQ_protocol.md)
- [rekordcrate](https://github.com/Holzhaus/rekordcrate) · [triseratops](https://github.com/Holzhaus/triseratops)
- [Ableton Link](https://github.com/Ableton/link) · [documentation](https://ableton.github.io/link/)
- [cpal](https://github.com/RustAudio/cpal) · [Symphonia](https://lib.rs/crates/symphonia) · [rubato](https://lib.rs/crates/rubato) · [rtrb](https://crates.io/crates/rtrb) · [midir](https://lib.rs/crates/midir)
- [Rubber Band](https://breakfastquay.com/rubberband/) · [Signalsmith Stretch](https://github.com/Signalsmith-Audio/signalsmith-stretch)
- [CLAP](https://cleveraudio.org/) · [Tauri Linux graphics issues](https://v2.tauri.app/develop/debug/linux-graphics/)
- [HT-Demucs ONNX export](https://huggingface.co/StemSplitio/htdemucs-ft-onnx)
