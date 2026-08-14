# djmanzo

A VirtualDJ-class DJ application for **macOS** and **Linux/Xubuntu** — built to match
VirtualDJ's feature set, appearance, workflow and handling, and then go past it.

> **Status: design phase.** The architecture and feature plan are written; no application code
> exists yet. [M0](docs/ROADMAP.md#m0--foundations-and-walking-skeleton) is the first build.

---

## What it is

A native desktop DJ application with a Rust realtime audio engine and a skinnable web UI:

- **2–6 decks**, full mixer, keylock, sync, hot cues, loops, pads, FX rack, sampler
- **Real hardware** — class-compliant MIDI controllers, HID controllers, and **motorized
  turntable controllers** (Rane Twelve, Hercules Inpulse T7, Denon SC5000M class)
- **Club interop** — Pro DJ Link (Pioneer CDJ/XDJ) and StagelinQ (Denon Prime) over the network
- **Neural stems** — separated ahead of the playhead, so the audio path adds zero latency
- **Linux as a first-class target**, with PipeWire and JACK supported properly

## What makes it different

| | |
|---|---|
| **Stem engine done right** | Look-ahead separation into a persistent cache — no added latency, instant on reload, per-stem EQ/FX, stem swapping across decks |
| **Universal hardware bridge** | Pro DJ Link *and* StagelinQ *and* network tempo sync *and* MIDI clock, in one app. Join a running club setup as a peer. |
| **Deterministic set replay** | Every action is a timestamped event, so a set is data: replay it, re-render it offline at studio quality, loop a transition to practise it, diff two takes |
| **Structure-aware assistance** | Phrase detection driving phrase-locked loops and transition planning, with suggestions that explain their reasoning |
| **Open extension surface** | CLAP plugin hosting, a documented WebSocket/OSC control API, Art-Net/DMX out, community controller mappings as data files |

## Stack

Rust realtime core (`cpal` · `symphonia` · Signalsmith Stretch · `wgpu` · ONNX Runtime) with a
Tauri 2 + Svelte 5 interface. Waveforms are rendered in Rust and scrolled by the compositor, so
they stay fast under WebKitGTK on Linux. Permissively licensed throughout (MIT OR Apache-2.0).

## Documentation

| | |
|---|---|
| [**ARCHITECTURE.md**](docs/ARCHITECTURE.md) | Threading model, signal flow, crate map, control model, stem engine, extension points |
| [**FEATURES.md**](docs/FEATURES.md) | VirtualDJ parity matrix with milestones, differentiators, UI/workflow map |
| [**ROADMAP.md**](docs/ROADMAP.md) | M0–M8 with acceptance criteria |
| [**RESEARCH.md**](docs/RESEARCH.md) | The OSS landscape, licences, and the clean-room rules |
| [**adr/**](docs/adr/) | The decisions, with the alternatives that were rejected and why |

## A note on originals

djmanzo clones VirtualDJ's *workflow and feature set* — what a control does, where it sits, how
a DJ moves through a set. It contains no VirtualDJ code, graphics, or assets, and claims no
affiliation with or endorsement by Atomix Productions, Pioneer DJ, Denon DJ, Serato or Native
Instruments. Open-source prior art such as Mixxx and xwax is studied for technique and
implemented independently; see [ADR-0002](docs/adr/0002-clean-room-permissive-licensing.md).
