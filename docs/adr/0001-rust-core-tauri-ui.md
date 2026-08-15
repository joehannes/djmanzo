# ADR-0001 — Rust realtime core with a Tauri 2 web UI

- **Status**: accepted
- **Date**: 2026-08-14

## Context

djmanzo needs a hard-realtime audio engine and a dense, skinnable, VirtualDJ-style interface,
on macOS and Linux/Xubuntu equally. Those two halves have opposite requirements: the engine
wants determinism, tight control over memory and threads, and no runtime surprises; the
interface wants fast iteration, rich layout, and theming.

Nothing in the repository constrained the choice — it was greenfield.

## Decision

**Rust for everything below the UI; Tauri 2 with a TypeScript/Svelte 5 front end for the UI.**

- Audio, analysis, hardware I/O, library and control logic are Rust crates.
- The UI is a webview: WKWebView on macOS, WebKitGTK on Linux.
- Audio I/O goes through `cpal` (CoreAudio / PipeWire / JACK / ALSA), wrapped behind our own
  `AudioBackend` trait.
- Waveforms are **not** drawn by the webview — see [ADR-0004](0004-waveform-rendering-strategy.md).

## Alternatives considered

**C++ with JUCE.** The industry-standard framework for this exact job: mature device I/O,
plugin hosting, decades of audio-specific tooling. Rejected on two counts. JUCE is
GPL-or-commercial, which conflicts with [ADR-0002](0002-clean-room-permissive-licensing.md)
unless we pay. And a realtime audio application is precisely where C++'s memory-safety
failures become audible crashes in front of an audience — Rust's guarantees are worth more here
than in ordinary software.

**C++/Qt, mirroring Mixxx.** Would let Mixxx's code and mappings port over easily. Rejected
because we are not forking Mixxx (its GPL licence rules it out as a base), so the portability
argument evaporates — and Qt's licensing brings its own constraints.

**Rust core with a native GPU UI (egui / iced on wgpu).** No webview, lowest possible UI
latency, one binary. Genuinely tempting, and it remains the fallback. Rejected as the default
because building a dense, skinnable, VirtualDJ-grade interface with layout presets and
community themes is dramatically faster in HTML/CSS, and skinning is a real product requirement
rather than a nicety.

## Consequences

**Good**

- One memory-safe language for all the hard parts; no FFI boundary in the audio path.
- Permissive licensing throughout the stack.
- Small binaries, fast startup, no bundled browser runtime.
- The UI can be iterated on without recompiling the engine.
- Skinning via CSS is a solved problem rather than a framework to write.

**Costs and risks**

- **WebKitGTK on Linux is the weak link.** Its WebGL can silently fall back to software
  rasterisation, and canvas-heavy UIs have been reported dropping to single-digit fps. This is
  mitigated architecturally in [ADR-0004](0004-waveform-rendering-strategy.md) and gated by a
  benchmark on real Xubuntu hardware at M1.
- Two languages at the boundary; state must be marshalled across IPC. Handled by a 60 Hz
  snapshot pump with diffing rather than chatty per-event messaging.
- Rust's audio ecosystem is thinner than C++'s — notably no permissive beat/key detection
  library, so we implement our own ([RESEARCH.md](../RESEARCH.md#the-analysis-gap)).

## The escape hatch

If the webview proves insufficient, the UI shell is replaceable without touching the engine or
the renderer: `dj-render` produces textures and knows nothing about its host, and the UI
communicates only through Actions and Parameters. Swapping to a native `wgpu` window is a
front-end project, not a rewrite. Keeping that true is a standing constraint on every UI change.
