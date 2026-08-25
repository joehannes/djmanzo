# djmanzo

A VirtualDJ-class DJ application for **macOS** and **Linux/Xubuntu** — built to match
VirtualDJ's feature set, appearance, workflow and handling, and then go past it.

> **Status: beta (v0.1.0).** Two to six decks, mixer, headphone cue, isolator EQ,
> filter, keylock, hot cues, loops, an FX rack, a sampler, an SQLite collection
> with importers, and a Rust-rendered scrolling waveform — on a realtime engine
> proven allocation-free by test. The controller layer has arrived, which is
> what turns a set of panels into something a DJ can play: the keyboard is a
> first-class controller, so a laptop with nothing plugged into it is a playable
> instrument.
>
> Start at [QUICKSTART.md](docs/QUICKSTART.md). Nothing here has been through a
> real gig yet — that is what a beta is.

---

## Building

```sh
npm --prefix ui ci && npm --prefix ui run build   # tauri-build needs the bundle first
cargo test --workspace --all-targets

# run it. `--features dj-app/custom-protocol` is not optional: without it Tauri
# loads the interface from the dev server instead of the bundle, and the window
# opens on "Connection refused". See docs/BUILDING.md.
cargo run --release --bin djmanzo --features dj-app/custom-protocol

# installable bundles from the repository root
npm install                # once; installs the root tauri wrapper
npx tauri build:deb        # Debian package on Linux
npx tauri build:dmg        # macOS dmg on macOS
```

**Prerequisites.** A Rust toolchain (1.90+), Node 22, and — because keylock builds
[Signalsmith Stretch](https://github.com/Signalsmith-Audio/signalsmith-stretch) from C++ and
generates its bindings with `bindgen` — a C++ compiler and **libclang**.

- **macOS**: the Xcode command-line tools supply both. `xcode-select --install`.
- **Debian/Ubuntu/Xubuntu**:
  ```sh
  sudo apt install build-essential libclang-dev libasound2-dev \
      libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libsoup-3.0-dev \
      libssl-dev libxdo-dev libayatana-appindicator3-dev
  ```

If `cargo build` fails with *"Unable to find libclang"*, that package is what is missing.

For installable bundles — `.dmg`, `.deb`, `.AppImage` — and what CI does with
them, see [BUILDING.md](docs/BUILDING.md).

---

## What it is

A native desktop DJ application with a Rust realtime audio engine and a skinnable web UI:

- **2–6 decks**, full mixer, keylock, sync, hot cues, loops, pads, FX rack, sampler
- **Real hardware** — class-compliant MIDI controllers, HID controllers, and **motorized
  turntable controllers** (Rane Twelve, Hercules Inpulse T7, Denon SC5000M class)
- **The keyboard as a controller**, not a pile of shortcuts — the same vocabulary and the
  same file format as a MIDI mapping, so a laptop with nothing plugged into it is a playable
  instrument. Press **Keys** to see the layout
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
| [**QUICKSTART.md**](docs/QUICKSTART.md) | Five minutes with djmanzo, for somebody with a laptop and no controller |
| [**BUILDING.md**](docs/BUILDING.md) | Building it yourself on either platform, and what CI does |
| [**CONTROLLERS.md**](docs/CONTROLLERS.md) | The keyboard, MIDI mappings, and why a mapping from a stranger is safe to open |
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
