# djmanzo

A VirtualDJ-class DJ application for **macOS** and **Linux/Xubuntu** — built to match
VirtualDJ's feature set, appearance, workflow and handling, and then go past it.

> **Status: beta (v0.46.0, October 2026).** A complete DJ application that
> has not yet been through a real gig: two to six decks, the mixer, keylock,
> sync, cues, loops, pads, effects, a sampler, live stems, timecode vinyl,
> controllers, CLAP plugins, video and visuals, broadcasting, a night of
> karaoke, an assistant that suggests, explains, teaches and can run the mix,
> and a sense of the room. Twenty-two Rust crates behind a Svelte interface,
> on a realtime engine a test proves allocation-free. The keyboard is a
> first-class controller, so a laptop with nothing plugged into it is a
> playable instrument.
>
> Start at [QUICKSTART.md](docs/QUICKSTART.md). What is built, part-built and
> planned is in [FEATURES.md](docs/FEATURES.md); what comes next is
> [below](#whats-coming).
>
> **Continuing the work?** Read [docs/HANDOFF.md](docs/HANDOFF.md) first. It is
> written for someone arriving with no context: what to read, how the project is
> worked on, and the traps that have each cost a session.

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

A native desktop DJ application with a Rust realtime audio engine and a skinnable web UI.

**For mixing**
- **2–6 decks**, full mixer, keylock, sync, hot cues, loops, pads, an FX rack, a
  sampler, slip, reverse, brake and backspin, a scrolling waveform rendered in Rust
- **Live stems**: vocals, drums, bass and the rest apart, separated ahead of the
  playhead into a cache, so the audio path adds no latency
- **A practice deck** only the headphones hear, to try a record, a loop or an
  effect while the set goes on

**For the hands**
- **The keyboard as a controller**, not a pile of shortcuts: the same vocabulary and
  file format as a MIDI mapping. Press **Keys** to see the layout
- **MIDI and HID controllers**, with lights, a learn mode, Lua for what a table
  cannot say, and shipped mappings (Pioneer DDJ-200, DDJ-SR, CDJ-3000 in HID mode,
  generic 2-deck)
- **Motorised platters** and **timecode vinyl** (DVS), with a control signal you can
  write to a file and play from any deck

**For the night**
- **An assistant** that says what to play next and why, whispers what cannot wait,
  explains, teaches, and **runs the mix on autopilot**, giving way the moment a
  hand touches a control. Bring your own AI provider, or none
- **Knowledge packs** for a genre's or a night's shape, a set plan, events kept step
  by step, a press kit
- **Reading the room**: the floor by microphone and camera, the crowd's reactions
  from their phones, and what each mix did, stated with how sure it is
- **Karaoke**: the singers' screen with words in time, a queue and a sign-up page,
  vocals out, the singers' microphones with their own effects, duets, a guide voice,
  finding the song a singer cannot name, a build for Macs back to 10.13
- **Video and visuals**: video on a deck, layered WebGL looks, MilkDrop presets,
  cameras; **going live** to a stream, with OBS driven from djmanzo
- **Themes and skins**: palettes, Caribbean themes, artwork layers over them, panels
  you move, zoom and pop out into windows of their own

**Under it**
- **Deterministic set replay**: every action is a timestamped event, so a set can be
  replayed, re-rendered offline at studio quality, or looped to practise a transition
- **Linux as a first-class target**, with PipeWire and JACK supported properly
- **An open surface**: CLAP plugins, a WebSocket/OSC control API, MIDI clock,
  djmanzo-to-djmanzo tempo sync, signed packs to make and share

## What makes it different

| | |
|---|---|
| **Stem engine done right** | Look-ahead separation into a persistent cache: no added latency, instant on reload, per-stem EQ/FX, stem swapping across decks |
| **An assistant that speaks only actions** | Everything it does is a command a DJ could have pressed, so it can explain, be undone, and never fight a hand on a control |
| **Deterministic set replay** | A set is data: replay it, re-render it, loop a transition to practise it, diff two takes |
| **Structure-aware** | Phrase detection driving phrase-locked loops and transition planning, with suggestions that explain their reasoning |
| **Karaoke and DJing in one** | The same decks, effects and room sense, so a karaoke night can become a dance floor without changing application |
| **Open** | CLAP hosting, a documented control API, mappings and packs as data files anyone can share |

## What's coming

| | |
|---|---|
| **Editions, one download** | Auto-Pilot, Hobbyist, Apprentice, Karaoke, Pro DJ, Full and Custom, all one application, unlocked by signing in with Google, Apple, GitHub, Microsoft or Discord. A **free 7-day trial** of any edition, Full included. Priced at half of what the professional competitors charge on average, with regional and student discounts. [EDITIONS.md](docs/EDITIONS.md) |
| **A free marketplace** | Themes, skins, visuals, controller mappings, knowledge packs, lessons, audio plugins, and **DJ techniques the autopilot can perform** and the teacher can teach. Everything in it free. [MARKETPLACE.md](docs/MARKETPLACE.md) |
| **More hardware** | Mappings for the controllers people own (Pioneer DDJ-FLX4 and DDJ-400, Numark Mixtrack and Party Mix, Hercules Inpulse, Traktor Kontrol S2/S4, Denon and Rane, Reloop), Pro DJ Link and StagelinQ for club players, Ableton Link, DJ mixers as sound cards, MIDI 2.0, lighting through DMX, phones and tablets as remotes. [FEATURES.md §4](docs/FEATURES.md#4-outlook) |
| **The Auto-Pilot surface** | Style the night (genres, energy over time, tempo, era), fill the playlist, start it, take over |
| **The Apprentice curriculum** | A course from the first beatmatch to the first gig, with exercises the teacher checks |
| **A real gig** | Everything above has been tested in this repository, none of it in front of a crowd. That is the step that matters most |

## Stack

Rust realtime core (`cpal` · `symphonia` · Signalsmith Stretch · `wgpu` · ONNX Runtime) with a
Tauri 2 + Svelte 5 interface. Waveforms are rendered in Rust and scrolled by the compositor, so
they stay fast under WebKitGTK on Linux. Permissively licensed throughout (MIT OR Apache-2.0).

## Documentation

| | |
|---|---|
| [**QUICKSTART.md**](docs/QUICKSTART.md) | Five minutes with djmanzo, for somebody with a laptop and no controller |
| [**FEATURES.md**](docs/FEATURES.md) | Everything VirtualDJ does and where djmanzo stands on each, what goes beyond it, and the outlook |
| [**EDITIONS.md**](docs/EDITIONS.md) | The editions, their prices, the licence and the trial |
| [**MARKETPLACE.md**](docs/MARKETPLACE.md) | The free marketplace that is planned |
| [**KARAOKE-NIGHT.md**](docs/KARAOKE-NIGHT.md) | For the person hosting a karaoke night |
| [**CONTROLLERS.md**](docs/CONTROLLERS.md) | The keyboard, MIDI and HID mappings, and why a mapping from a stranger is safe to open |
| [**BUILDING.md**](docs/BUILDING.md) | Building it yourself on either platform, and what CI does |
| [**ARCHITECTURE.md**](docs/ARCHITECTURE.md) | Threading model, signal flow, crate map, control model, stem engine, extension points |
| [**ROADMAP.md**](docs/ROADMAP.md) | The milestones, with acceptance criteria and where each actually stands |
| [**DIRECTIVE-STATUS.md**](docs/DIRECTIVE-STATUS.md) | Every section the owner asked for, and where it stands |
| [**RESEARCH.md**](docs/RESEARCH.md) | The OSS landscape, licences, and the clean-room rules |
| [**screenshots/**](docs/screenshots/) | The screens, in their themes, skins and states |
| [**adr/**](docs/adr/) | The decisions, with the alternatives that were rejected and why |
| [**WEBSITE.md**](docs/WEBSITE.md) | The website — Isla Manzo — that presents and sells djmanzo, and the account and licences it connects to |

## A note on originals

djmanzo clones VirtualDJ's *workflow and feature set* — what a control does, where it sits, how
a DJ moves through a set. It contains no VirtualDJ code, graphics, or assets, and claims no
affiliation with or endorsement by Atomix Productions, Pioneer DJ, Denon DJ, Serato or Native
Instruments. Open-source prior art such as Mixxx and xwax is studied for technique and
implemented independently; see [ADR-0002](docs/adr/0002-clean-room-permissive-licensing.md).
