# ADR-0004 — Waveforms rendered in Rust, scrolled by the compositor

- **Status**: accepted, amended 2026-08-15 (see *Amendment*)
- **Date**: 2026-08-14

## Context

Waveform display is the hardest UI problem in a DJ application. Two to six lanes of audio
scroll continuously at 60 fps, with beat grid, hot cues, loop regions and phrase markers
overlaid, while the same machine runs realtime DSP with a ~5 ms budget. During a scratch the
display must track the platter with no perceptible lag — this is the feature people judge the
app's *feel* by.

[ADR-0001](0001-rust-core-tauri-ui.md) puts the UI in a webview. On macOS that is WKWebView and
generally fine. On Linux it is **WebKitGTK**, and WebKitGTK is a known hazard for this exact
workload:

- WebGL2 contexts are created successfully even when backed by a software rasteriser or a slow
  presentation path, so the failure is silent;
- `WEBGL_debug_renderer_info` is masked for fingerprinting protection, reporting the same
  string on every Linux machine, so the application cannot detect the bad case at runtime;
- canvas-heavy UIs have been reported dropping to single-digit frame rates, with the same code
  running fine in a normal browser;
- the usual workarounds are environment flags (`WEBKIT_DISABLE_DMABUF_RENDERER`,
  `WEBKIT_DISABLE_COMPOSITING_MODE`) that trade the problem for a different one.

Xubuntu is a stated target platform. "Works on my Mac" is not an acceptable answer.

## Decision

**Rasterise waveforms in Rust; let the webview only translate the result.**

1. `dj-render` draws waveform **tiles** offscreen with `wgpu`, from the multi-resolution
   waveform data produced by `dj-analysis`.
2. Tiles reach the UI through a Tauri custom protocol — binary, no base64 IPC.
3. The UI positions tiles in a strip and scrolls them with **CSS transforms only**. That is
   compositor work: no JavaScript drawing per frame, no canvas, no WebGL.
4. Beat grid, cues, loop regions and phrase markers are overlay layers, transformed with the
   identical transform so they stay locked to the audio.
5. The playhead is fixed; the strip moves under it.
6. Position comes from the engine at 60–120 Hz over a Tauri Channel; the UI interpolates
   between updates using the last known position and rate, so scratching stays smooth even if
   an update is late.

## Alternatives considered

**Canvas 2D drawing in the webview.** Simplest, and the obvious first instinct. Rejected: it is
precisely the pattern that collapses on WebKitGTK, and it puts per-frame work on the JS thread
where a garbage collection pause becomes a visible stutter.

**WebGL/WebGPU in the webview.** Would be fast where it works. Rejected because on Linux we
cannot even *tell* whether it works — the silent software fallback plus the masked renderer
string means we would be shipping a coin flip. WebGPU support in WKWebView and WebKitGTK is
also uneven.

**Native `wgpu` window composited over the webview.** Best possible performance, and no webview
involvement at all. Rejected as the default because compositing a native surface with a webview
is fiddly on both platforms — z-ordering, input routing, resize synchronisation and
transparency all become per-platform work. It stays the escape hatch (below).

**A native UI toolkit for the whole application** (egui/iced). Considered and rejected in
ADR-0001; skinning and layout are worth the webview.

## Consequences

**Good**

- Fast on WebKitGTK because we never ask WebKitGTK to draw anything demanding.
- Rendering quality, spectral colouring and pixel-exactness are controlled in Rust and are
  identical on both platforms.
- CSS-transform scrolling is the cheapest path a webview has; it is what the compositor exists
  to do.
- Tiles are cacheable and reusable across zoom steps and across decks showing the same track.

**Costs**

- Tile management is real work: sizing, prefetching ahead of the playhead, invalidation on
  zoom or on beat grid edits, and eviction.
- A tile boundary crossing must not be visible, which means overlap and careful seams.
- Non-waveform UI (browser lists, meters, pads) still runs in the webview and still has to be
  built with WebKitGTK's limits in mind — virtualised lists, transform-based animation,
  no per-frame layout thrash.

**The escape hatch, designed in from day one**

`dj-render` produces textures and knows nothing about its host. If the webview proves
insufficient, replacing the waveform layer with a native `wgpu` window — or replacing the whole
shell — does not touch the renderer, the engine or the control layer. Keeping `dj-render`
host-agnostic is a standing constraint on every change to it.

**This is gated, not hoped for.** Waveform performance is benchmarked on real Xubuntu hardware
at **M1**, with 60 fps on four decks as the acceptance criterion. If it fails there, we take
the escape hatch immediately — while the UI is small — rather than discovering the problem at
M7 when it would be a rewrite.


---

## Amendment, 2026-08-15 — the rasteriser is CPU, not `wgpu`

The decision above said tiles would be rasterised offscreen with `wgpu`.
`dj-render` rasterises them on the CPU instead. Recording the change rather than
quietly diverging from the document.

**The architectural requirement is unchanged and still met.** What this ADR
exists to protect is *the webview never draws the waveform*, so that WebKitGTK's
silent software-rasteriser fallback cannot become a performance cliff. Tiles are
still produced in Rust, still handed over as finished pixels, still scrolled by
CSS transform alone. `dj-render` still knows nothing about its host, so the
escape hatch — swap the shell for a native window — remains exactly as available.

**What changed is the reasoning about cost.** A tile turned out to be a
column-fill: per pixel column, look up one bucket in a multi-resolution summary
and paint a vertical run. It is memory-bandwidth-bound, and tiles are cached per
track per zoom level, so the work happens once and never again while scrolling.

Measured on this machine, five minutes of stereo at 48 kHz
(`crates/dj-render/tests/throughput.rs`):

| Operation | Time |
|---|---|
| Summarise the whole track | 79 ms |
| 32 tiles — a 4K-wide lane on four decks, uncached | 15.8 ms (0.49 ms/tile) |
| One newly-exposed tile while scrolling | 0.46 ms |
| Full-track overview, 2000 px wide | 6.35 ms |

An entire screen of waveform for four decks, regenerated from scratch, fits
inside a single 60 fps frame — and that is the pathological case, since scrolling
only ever costs the newly-exposed edge at 0.46 ms.

Against those numbers, a GPU path would buy nothing and cost a device and queue
to manage, shaders to compile, async surface handling, adapter-selection failure
modes on headless Linux, and roughly a hundred crates of dependency.

**This is reversible.** `render_tile` takes a summary and returns pixels. A
`wgpu` implementation drops in behind the same signature the moment profiling
says it is needed — and the throughput test above is what would tell us.

**Unchanged: the Xubuntu benchmark still gates the UI strategy.** These numbers
say tile *generation* is cheap. They say nothing about whether WebKitGTK can
composite a CSS-transformed strip at 60 fps on four decks, which is a different
question and still the one that decides whether the webview survives as the
shell. That benchmark remains outstanding.
