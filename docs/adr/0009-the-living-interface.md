# ADR-0009 — The living interface

- **Status**: accepted — foundation being built
- **Date**: 2026-08-18

## Context

Every DJ application converges on the same interface: rectangles, sliders, tabs, dropdowns and
property inspectors. That vocabulary was invented for filing cabinets, and it is what a DJ has
to *learn* before they can play. Meanwhile a DJ already understands, without having learned any
of it, gravity, friction, flow, pressure, growth, tension, resonance, rhythm and balance.

The proposal this ADR records is to build the interface out of the second vocabulary instead of
the first: **don't make the user learn the interface; make the interface behave like things
they already understand.** Concretely, an audio ecosystem — decks as sources, audio as water,
routing as a river system, connections as roots, sync as coupled rhythm, groups as organisms.

Three things make this a decision rather than a mood board.

### 1. The performance question is genuinely open, and the obvious answer is wrong

[ADR-0004](0004-waveform-rendering-strategy.md) measured something specific on a machine with no
GPU: when WebKitGTK has no accelerated compositing, animating *anything* costs a **whole-page
repaint**, and the cost tracks page area rather than animated area. That is why four scrolling
waveform lanes cost barely more than one, and why the failure is silent.

A continuously animating interface is exactly the workload that hits that wall hardest. So the
rendering strategy could not be chosen by taste. But ADR-0004's own finding suggested the answer
might be counter-intuitive: if the cost is *document invalidation* rather than fill rate, then
replacing N animating DOM layers with **one self-repainting surface** should be *cheaper* on the
bad path, not dearer — a canvas dirties its own rectangle; it does not invalidate layout.

That was measured, in the same worst-case environment (Xvfb, 1920×1080, **no GPU**, WebKitGTK),
with identical motion and identical shape counts across three strategies —
`ui/src/renderbench.ts`, run with `DJMANZO_RENDERBENCH=<count>`:

| shapes | DOM (`transform`) | Canvas 2D | WebGL |
|---|---|---|---|
| 60 | 59.8 fps | 60.0 fps | 59.5 fps |
| 240 | **27.0 fps** | 57.8 fps | 60.0 fps |
| 960 | **18.6 fps** | 45.4 fps | 59.7 fps |

The hypothesis holds, and more strongly than expected. **The interface's current DOM approach is
the worst of the three for this workload, by a wide margin**, and it is the only one that
collapses. Canvas 2D degrades gracefully. WebGL is flat across a sixteen-fold range.

This is a floor, not a ceiling: every number improves on a machine with a GPU.

### 2. The driver string lies

The WebGL run reports its renderer as **"Apple GPU" — on a headless Linux container with no GPU
at all.** WebKitGTK masks the real renderer for fingerprinting resistance.

This settles a question ADR-0004 left as a fear: **`WEBGL_debug_renderer_info` cannot be used to
detect a software fallback.** Feature detection is not merely unreliable here, it is actively
misleading. The only honest detector is measuring frame times, which the shell already does
(`ui/src/framerate.ts`).

### 3. A DJ has to hit a cue in a dark room in 200 ms

This is the strongest objection to the whole idea and it has to be answered in the design rather
than apologised for later. An interface whose controls grow, drift and breathe is an interface
whose targets move, and a target that moves cannot be hit from muscle memory. Beautiful and
unusable is a real failure mode, and it is the usual one for interfaces like this.

The answer is in the metaphor itself, which is what makes it an answer rather than a compromise.
**A tree's trunk is rigid and bears weight; its foliage moves and carries the light.** Nothing in
nature asks you to stand on something that is swaying.

## Decision

**Build a living interface as a world model rendered in measured tiers, with load-bearing
elements rigid and informational elements alive.**

### The world model is not the renderer

A world of entities and components, owned in Rust, knowing nothing about canvases, WebGL or the
DOM:

```
Entity ── Identity      the ADR-0008 widget name; what this is
       ── Place         which slot it belongs to, and where within it
       ── Form          the shape family: flow, organism, field, marker
       ── Vitality      pulse, growth, agitation — driven by the audio clock
       ── Bond          what it is connected to, and how strongly
       ── Reading       the numbers it stands for, so a still frame is legible
```

This is the same separation [ADR-0003](0003-action-bus-and-parameter-registry.md) makes for
behaviour. The renderer is a client of the world, exactly as the interface is a client of the
action bus. Swapping Canvas 2D for WebGL, or the webview for a native `wgpu` window, must not
require the world to know.

It also composes with [ADR-0008](0008-one-widget-vocabulary.md) rather than replacing it: the
widget registry supplies **identity and slots**, and the world supplies **form, vitality and
bonds**. ADR-0008's rule that a layout file never contains coordinates is refined rather than
broken — *a layout file never contains coordinates; the running world always has them*, because
they are simulated rather than authored, and simulated geometry does not go stale when the window
resizes or the interface is redesigned.

### Canvas paints, DOM listens

**The DOM does not go away, and it is not a fallback.** It is the interaction and accessibility
layer, sitting over the canvas that draws the living form:

- a control's **appearance** is drawn in the canvas as part of the world;
- a control's **hit target, focus ring, keyboard handling and ARIA role** stay a real DOM
  element, positioned by the world and otherwise unstyled.

This is what makes the interface reachable by a screen reader, navigable by keyboard, and
testable — and it is the same trunk-and-foliage split, expressed in the platform. It costs one
DOM element per control, which the benchmark above shows is free as long as those elements are
not what is being animated.

### Tiers, selected by measurement

| Tier | What | When |
|---|---|---|
| **0 — Still** | DOM and CSS, no animation. Every state legible from form, position and colour alone. | `prefers-reduced-motion`, or the frame probe reporting distress |
| **1 — Breathing** | Tier 0 plus compositor-only motion: transform and opacity, nothing that repaints | always available |
| **2 — Living** | One Canvas 2D surface drawing the world | the default |
| **3 — Flowing** | The same world through WebGL | when measurement says it is faster |
| **escape** | Native `wgpu` window | only if the webview fails on real hardware; forfeits the DOM layer, so it is a last resort |

Selection is by the running frame probe, never by feature detection, for the reason in §2 above.
Demotion is automatic and silent to the user's workflow but *stated* in the interface, the way the
frame-rate warning already is.

**Tier 0 is a hard requirement, not a courtesy.** If a still frame of the world does not tell a
DJ the state of their mix, the design has failed — and that is also the cheapest test of whether a
visual channel is carrying information or decorating.

### Four rules the design is held to

1. **Trunk and foliage.** Load-bearing elements — anything a DJ clicks, drags or aims at — are
   rigid, stable and predictable. Informational elements grow, flow and respond. Foliage never
   occludes trunk, and a control's centre never moves.
2. **Stillness is the default; motion is information.** Nature is mostly still. If everything
   moves all the time, nothing communicates. A paused deck is still water.
3. **Every channel answers a question a DJ actually asks.** If a visual channel does not answer
   one, it is decoration and it is cut. The questions are enumerated in
   [VISUAL-LANGUAGE.md](../VISUAL-LANGUAGE.md).
4. **Nature carries the gestalt; digits carry the precision.** BPM, time remaining, key and gain
   stay as legible numbers. The world tells you *at a glance*; the numbers tell you *exactly*.
   Both, never either.

### Motion runs on the audio clock

Everything that pulses pulses on the beat, from the same snapshot the engine already publishes at
60 Hz — not on wall time. An interface visibly in time with the music is the one thing this
design can offer that a conventional one structurally cannot, and it costs nothing extra because
the data is already there.

## Alternatives considered

**three.js.** MIT, so [ADR-0002](0002-clean-room-permissive-licensing.md) permits it, and the
ecosystem is excellent. Rejected on architecture rather than licence: it is a 3D scene graph for
a 2.5D problem, it is several hundred kilobytes for the fraction of it we would use, and — the
deciding reason — **a scene graph wants to own scene semantics**, which is precisely what must
stay in our world model. A thin renderer behind our own world is smaller and correct.

**Straight to WebGL, skipping Canvas 2D.** The benchmark makes it tempting: WebGL is flat where
Canvas 2D bends. Rejected because Canvas 2D has a property WebGL does not — **it cannot silently
fall back, because it is already software.** It is the tier that is always honest, and building
it first means the world model gets a second renderer early, which is what proves the abstraction
is real rather than aspirational.

**Native `wgpu` window now**, as ADR-0004's escape hatch and the user's own suggested stack.
Rejected *for now*: it forfeits the DOM interaction layer entirely, taking accessibility,
keyboard navigation and text rendering with it, and the measurement above shows the webview is
not the bottleneck the fear assumed. It stays available and unchanged.

**Pure canvas, no DOM.** Simplest to reason about and what most creative tools of this kind do.
Rejected: a canvas is a single opaque node to assistive technology, and rebuilding focus
management, hit testing and an accessibility tree by hand is both large and the sort of work
that is never finished.

**Keeping the current DOM interface and adding decoration.** Rejected by the benchmark: animated
DOM is the *worst* of the three strategies, and gets worse as the interface gets richer.

## Consequences

**Good**

- The interface can carry far more state than a rectangle-based one, because a shape has many
  more channels than a number: width, hue, saturation, pulse rate, phase, turbulence, direction.
- Motion locked to the audio clock makes the interface legibly *in time*, which no conventional
  DJ interface is.
- It is measurably faster than what exists, on the hardware where it matters most.
- The renderer being replaceable means ADR-0004's escape hatch stays open and gets cheaper, not
  dearer: a native window would need a new renderer, not a new application.
- The world is inspectable — a serialised world is a complete description of what was on screen,
  which is a bug report a user can send and a state a test can assert.

**Costs**

- Two representations of every control — a drawn form and a DOM hit target — that must not drift
  apart. This is the standing maintenance cost, and the reason the world positions both.
- Text in a canvas is expensive and does not inherit the system's font rendering or the user's
  size preferences, so **all text stays in the DOM layer**. That is a constraint on the design,
  not a detail.
- Colour-coding by hue fails for roughly one man in twelve, so every hue channel needs a
  redundant one — form, position or text.
- A world that is beautiful and says nothing is the failure mode. Rule 3 exists to catch it, and
  it has to be applied ruthlessly, including to things that took work.
