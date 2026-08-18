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
| [xwax](https://xwax.org/) | C, GPL-2.0 | Timecode/DVS decoding. Mixxx embeds its decoder. Deferred for djmanzo, but the design constraints matter. |
| Mixxx's DVS write-ups ([pt1](https://mixxx.org/news/2021-11-21-dvs-internals-pt1/), [pt2](https://mixxx.org/news/2021-12-22-dvs-internals-pt2/), [pt3](https://mixxx.org/news/2025-08-27-dvs-internals-pt3/)) | Articles | How timecode vinyl actually works: Serato 1 kHz / Final Scratch 1.2 kHz / Traktor MK2 2 kHz carriers, zero-crossing pitch detection, Nyquist-bounded max scratch speed (~22× for Serato, ~11× for Traktor MK2). **Prose, not code — safe to learn from.** |
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
| MIDI | `midir` | MIT | Cross-platform realtime MIDI, RtMidi-inspired. |
| HID | `hidapi` | MIT | Direct HID for high-res jogs and NI-class devices. |
| Tags | `lofty` | MIT/Apache-2.0 | Reading/writing metadata across formats. |
| `roxmltree` | MIT OR Apache-2.0 | Read-only XML tree for the library importers. Random access over a document already in memory, which is what rekordbox's id references and plist's key/value pairs need. |
| Library DB | `rusqlite` | MIT | SQLite; boring and correct for a music library. |
| Loudness | `ebur128` | MIT | EBU R128 / ReplayGain for auto-gain. |
| Neural inference | `ort` (ONNX Runtime) | Apache-2.0/MIT | One runtime for stems *and* optional beat tracking. Backends: CoreML (macOS), CUDA/DirectML, CPU. |
| Effects hosting | **CLAP** | MIT | Truly permissive plugin standard from u-he/Bitwig, with a real ecosystem (~15 hosts, ~400 plugins). Preferred over VST3, whose SDK only moved to GPLv2 in late 2025 and remains Steinberg-controlled. |
| rekordbox import | `rekordcrate` | MPL-2.0 | Compatible; file-level copyleft only. |
| UI shell | Tauri 2 | MIT/Apache-2.0 | See ADR-0001. |

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
