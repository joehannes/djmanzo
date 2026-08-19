# Visual tech stack

How the world described in [VISUAL-LANGUAGE.md](VISUAL-LANGUAGE.md) is actually drawn, and with what.
Every section traces back to a design requirement from that document or from
[ADR-0009](adr/0009-the-living-interface.md); nothing here is aspirational
technology looking for a home.

---

## 1. The rendering architecture in one picture

```
 ┌─ Rust host ────────────────────────────────────────────────────────────┐
 │                                                                        │
 │  dj-engine  ─60 Hz→  ParameterRegistry  ─snapshot→  dj-app            │
 │                                                        │               │
 │                                          ┌─────────────┤               │
 │                                          ▼             ▼               │
 │                                    WorldModel     Tauri Channel        │
 │                                    (Rust ECS)     → webview            │
 │                                          │                             │
 │                                  serialised scene                      │
 │                                  (flat buffer)                         │
 └──────────────────────────────────────────┼─────────────────────────────┘
                                            │
                         ┌──────────────────┼──────────────────┐
                         ▼                  ▼                  ▼
                    ┌─────────┐     ┌──────────────┐    ┌───────────┐
                    │  DOM    │     │  Canvas 2D   │    │  WebGL    │
                    │  layer  │     │  (Tier 2)    │    │  (Tier 3) │
                    │         │     │              │    │           │
                    │ trunk:  │     │ foliage:     │    │ foliage:  │
                    │ hit     │     │ the living   │    │ same      │
                    │ targets,│     │ world drawn  │    │ world,    │
                    │ text,   │     │ per frame    │    │ GPU path  │
                    │ ARIA,   │     │              │    │           │
                    │ focus   │     └──────────────┘    └───────────┘
                    └─────────┘
```

**Three layers, always.** The DOM is not a fallback — it is the permanent
interaction and accessibility surface. The canvas (2D or WebGL, selected by
measurement) draws the living forms over it. The waveform tiles produced by
`dj-render` continue to be served and scrolled exactly as
[ADR-0004](adr/0004-waveform-rendering-strategy.md) specifies; the living
layer draws *around* and *over* them, never replacing them.

---

## 2. The world model

### 2.1 Not an ECS library — an ECS shape

ADR-0009 specifies the world as entities with components. The instinct is to
reach for an ECS library (bitECS, miniplex, ecs-lib). Rejected: every one of
them is either too heavy for what is ultimately a few hundred entities with
fixed component sets, or too opinionated about storage and iteration order. The
world model is a flat, pre-allocated table — closer to the `ParameterRegistry`
than to a game engine — because it has the same constraints: bounded, no
dynamic allocation, serialisable in one pass.

**Owned in Rust, in `dj-app`.** The world needs the registry, the engine
snapshot, and the widget tree from [ADR-0008](adr/0008-one-widget-vocabulary.md).
All three live in Rust. Building the world in TypeScript would mean duplicating
all of them over IPC, and the world's own simulation step (pulse, drift,
settling) needs to run even when the webview is blocked on a long paint.

### 2.2 Entity and component schema

```rust
/// One thing in the world. Fixed-size, pre-allocated, no heap.
struct Entity {
    id:        WidgetName,       // ADR-0008 identity — `deck.1.waveform`
    slot:      SlotAddress,      // where in the layout tree
    kind:      EntityKind,       // River | Confluence | Stone | Eddy | Terrain |
                                 // Current | Sluice | Dam | Pool | Basin |
                                 // Highland | Weather
    form:      Form,             // shape family, bounding rect, path data index
    vitality:  Vitality,         // pulse_phase, pulse_amplitude, agitation, growth
    colour:    Colour,           // hue (OKLCH), chroma, lightness, opacity
    reading:   Reading,          // the numbers: BPM, time remaining, key text, gain dB
    strata:    Strata,           // EQ bands: lo/mid/hi gain, filter position, resonance
    currents:  Currents,         // stem levels: vocal, drums, bass, other (0..1 each)
    terrain:   Terrain,          // FX: kind (Canyon|Cavern|Narrows|Sluice|Cascade|…),
                                 //     depth (wet/dry), timing (beat division), active
    bond:      [BondTarget; 4],  // what it connects to (e.g. river → confluence)
    flags:     EntityFlags,      // visible, interactive, trunk, reduced_motion,
                                 //   slip_active, recording, reverse
}
```

**Why OKLCH rather than HSL.** VISUAL-LANGUAGE.md §4 maps hue to musical key on
the Camelot wheel — a circle for a circle. That mapping needs perceptual
uniformity: a 30° rotation must look like the same magnitude of change
everywhere round the circle, or adjacent keys will appear more or less
different depending on which part of the wheel they sit on. HSL fails this —
its yellow is visibly brighter than its blue at the same L. OKLCH's lightness
channel tracks perceived brightness, so the colour axis carries consistent
information as the key changes. The shader converts to sRGB at the last step;
every colour computation upstream stays in OKLCH.

### 2.3 Simulation step

The world ticks at the snapshot rate (60 Hz), driven by the engine's own clock.
One function, no threads:

```
fn tick(world: &mut World, snapshot: &Snapshot, dt: f32) {
    for entity in world.entities_mut() {
        // 1. Read state from snapshot → entity.reading, entity.colour
        // 2. Derive vitality: pulse_phase from beat_phase, agitation from peak level
        // 3. Step spring physics: drift, scale, opacity → settle toward rest
        // 4. Compute form: path control points, width, depth
        // 5. Mark dirty flag if anything changed
    }
}
```

**Spring physics, not keyframe animation.** Every continuous property (drift,
scale, opacity, agitation) is a critically-damped spring converging to its
target. Springs have no duration — they arrive when the physics says so — which
means:
- a fast tempo change settles quickly because the displacement is small;
- a slow, dramatic fader move settles slowly because it is;
- interrupting a transition mid-flight costs nothing: the spring already knows
  where it is and where it is going.

The damping constants are derived from VISUAL-LANGUAGE.md §5's excursion
limits: a control must be hittable by aiming where it was, which bounds the
overshoot, which bounds the damping ratio. Concretely:

| Property       | Stiffness | Damping ratio | Max excursion        |
|----------------|-----------|---------------|----------------------|
| Drift (xy)     | 180       | 1.0 (critical)| element radius × 0.08|
| Scale          | 220       | 1.0           | 0.9× – 1.15×        |
| Opacity        | 300       | 1.0           | 0 – 1               |
| Agitation      | 120       | 0.85 (slight) | amplitude 0 – 0.4   |
| Pulse phase    | —         | —             | follows beat phase   |

Pulse phase is not a spring — it is copied from the snapshot's beat phase
directly, which is what makes the interface locked to the music rather than
chasing it.

---

## 3. From world to pixel — the rendering tiers

### 3.1 Tier 0 — Still (DOM + CSS)

**When:** `prefers-reduced-motion`, or the frame probe reports sustained
degradation below 40 fps.

Everything communicates through form, position, width and colour. No animation.
The DOM layer is all there is, and it must say everything — this is the
greyscale-and-still-frame test from VISUAL-LANGUAGE.md §4 and §5.

**Tech:**
- Standard Svelte components with CSS custom properties.
- River shapes as SVG `<path>` elements clipped and coloured by CSS.
- Confluence as an SVG union of two paths with a gradient fill.
- Width, depth, hue, saturation driven by CSS variables updated from the
  snapshot at 60 Hz.
- No `requestAnimationFrame` loop. State changes arrive as reactive updates.

**Cost:** Effectively zero beyond the snapshot diff Svelte already does.

### 3.2 Tier 1 — Breathing (DOM + compositor-only motion)

**When:** Always available on top of any other tier.

Transitions that cost nothing because they are `transform` and `opacity` only —
the compositor handles them without a repaint. Used for:
- Fader settling after a move.
- Cue arming glow.
- Deck-select highlight transition.
- Beat-synced `scale` pulse on the deck badge (within the 0.9–1.15× bound).

**Tech:**
- CSS `transition` on `transform` and `opacity` with durations derived from the
  beat period: `transition-duration: calc(var(--beat-ms) * 1ms)`.
- `will-change: transform, opacity` on the pulsing elements only.
- Beat phase arrives as `--beat-phase: 0.0 .. 1.0` on the deck's container.

**Cost:** One CSS variable update per deck per frame; the compositor
interpolates. ADR-0004's benchmark shows this is free.

### 3.3 Tier 2 — Living (Canvas 2D)

**When:** The default tier. Proves the world model with a renderer that cannot
silently fall back — Canvas 2D already *is* software.

One `<canvas>` element covering the foliage region. Draws the world every frame
from the serialised scene buffer. The DOM layer sits on top with
`pointer-events: auto`; the canvas has `pointer-events: none`.

**Rendering vocabulary:**

| Metaphor             | Canvas 2D technique                                                |
|----------------------|--------------------------------------------------------------------|
| River (flow)         | Quadratic Bézier paths, width from volume, filled with OKLCH-derived gradient along the path direction |
| Beat crest           | A brighter band travelling along the path at beat phase, drawn as a clipped gradient offset |
| Water clarity        | Path fill opacity: 1.0 for certain, fading toward 0.3 for turbid  |
| Riverbed (waveform)  | Composited *under* the existing `dj-render` tile strip via `globalCompositeOperation: 'destination-over'` — the tiles are the terrain |
| Confluence           | Two paths merging via smooth-minimum blending of their widths at the crossfader position |
| Eddy (loop)          | A circular arc with animated dash-offset for rotation, radius from loop length |
| Stone (hot cue)      | A filled circle at the cue's position on the river path             |
| Stepping stones      | Circles spaced at beat intervals along the path, opacity from quantize state |
| EQ strata (water column) | Three horizontal bands within the river width, each with independent opacity from EQ gain |
| Filter               | River width narrowing: low-pass removes top stratum, high-pass removes bottom |
| Mist (unanalysed)    | Radial gradient fill with noise-perturbed alpha, clearing as analysis progresses |
| Weather (CPU load)   | Agitation applied as jitter to all path control points              |
| Session light (M9)   | Background gradient hue shift: warm (dawn) → neutral (noon) → cool (dusk) |
| Assistant fork       | A second, fainter path branching from the river at the playhead     |

**Noise.** Agitation, mist and surface texture use a 2D simplex noise function
evaluated per frame on the CPU. For Tier 2's shape count (< 200 noise samples
per frame at typical zoom), this is faster than uploading a noise texture to a
canvas. The noise function is a port of Stefan Gustavson's `webgl-noise` to
plain TypeScript — MIT-licensed, no dependencies, and already pure arithmetic.

**Text stays in the DOM.** VISUAL-LANGUAGE.md §7 and ADR-0009 both require it:
canvas text ignores the system font renderer and the user's size preference.
BPM, time, key, gain are DOM elements positioned by the world model.

### 3.4 Tier 3 — Flowing (WebGL)

**When:** The frame probe measures Tier 2 struggling and WebGL is measured
faster — not by feature detection (ADR-0009 §2: the driver string lies) but by
a timed probe identical to the renderbench, run once at startup.

Same world, same scene buffer, different renderer. One `<canvas>` with a WebGL
context, same DOM overlay.

**Why WebGL rather than WebGPU:** WebKitGTK does not ship WebGPU. WKWebView on
macOS does, but the floor is WebKitGTK and the floor decides. WebGL 1 is the
target, with WebGL 2 features used opportunistically where available.

**Rendering vocabulary (GPU equivalents):**

| Metaphor             | WebGL technique                                                    |
|----------------------|--------------------------------------------------------------------|
| River (flow)         | **SDF capsule chain** — the river path as a sequence of capsule SDFs evaluated in the fragment shader, with `smoothstep` anti-aliasing and width modulated by volume. Smooth-minimum (`smin`) blending where tributaries merge. |
| Beat crest           | A `sin()` wave travelling along the path's parametric `t`, mixed into the river's lightness channel in OKLCH. Phase from the snapshot's beat phase uniform. |
| Water clarity        | Chroma channel in the SDF fill: high chroma = clear, desaturated = turbid. Uses OKLCH's perceptual uniformity. |
| Confluence           | `smin()` of two river SDFs with `k` controlled by the crossfader value — hard split at 0/1, smooth merge at 0.5. |
| Eddy (loop)          | **Annular SDF** with animated rotation via UV offset. Radius from loop length; line thickness from a uniform. |
| Surface texture      | **Fractal Brownian Motion** (fBm) from 3 octaves of simplex noise in GLSL (Gustavson's `webgl-noise`), sampled with a time-varying offset along the flow direction — flow-map scrolling without a texture. |
| EQ strata            | Three horizontal bands in the SDF's cross-section, each with independent chroma and lightness driven by EQ gain uniforms. |
| Filter               | The river SDF's width function is multiplied by a ramp: low-pass tapers the top, high-pass tapers the bottom. |
| Mist                 | Fragment alpha modulated by `fbm(uv + time * drift) * (1.0 - analysis_progress)`. |
| Weather              | Noise amplitude on the river path's control-point uniforms. |
| Harmonic blend/seam  | At the confluence, when keys are compatible: `smin` with large `k` (smooth). When keys clash: `max()` union with a visible seam — a stripe of achromatic pixels where the two SDFs overlap. Compatibility is a uniform derived from the Camelot distance. |

**Shader architecture:**

```
world.vert (shared)
  ├── river.frag          — the deck rivers, beat crests, EQ strata
  ├── confluence.frag     — the mixer merge, harmonic blend/seam
  ├── markers.frag        — eddies, stones, stepping stones
  ├── atmosphere.frag     — mist, weather, session light
  └── include/
        ├── sdf.glsl      — capsule, circle, annulus, smin, smooth unions
        ├── noise.glsl    — simplex 2D/3D, fbm (Gustavson, MIT)
        └── oklch.glsl    — OKLCH ↔ linear sRGB conversion
```

Each shader receives the same flat uniform buffer: entity data packed as
`vec4` arrays, plus global uniforms (beat phase, time, crossfader, tier,
viewport). No per-entity draw calls — all rivers in one draw, all markers in
one draw.

**Instanced rendering.** River segments, stones and eddies are drawn with
`ANGLE_instanced_arrays` (WebGL 1) or native instancing (WebGL 2). The
renderbench already proved this path at 960 shapes / 60 fps on a no-GPU floor.

### 3.5 Tier selection — the probe

```typescript
async function selectTier(): Promise<Tier> {
  if (prefersReducedMotion()) return Tier.Still;

  const canvas2dFps = await probeCanvas2d(PROBE_SHAPES, PROBE_MS);
  if (canvas2dFps >= 55) return Tier.Living;

  const webglFps = await probeWebgl(PROBE_SHAPES, PROBE_MS);
  if (webglFps >= 55 && webglFps > canvas2dFps * 1.3) return Tier.Flowing;

  if (canvas2dFps >= 30) return Tier.Living;
  return Tier.Breathing;
}
```

**Never by feature detection.** ADR-0009 established that
`WEBGL_debug_renderer_info` lies ("Apple GPU" on headless Linux). The only
honest detector is measuring frame times. The probe runs once at startup with
the same infrastructure as `renderbench.ts`, and the result is cached for the
session.

**Demotion is automatic, promotion is not.** If the frame-rate monitor
(`framerate.ts`) reports sustained degradation, the tier steps down silently.
It never steps back up without a restart, because the cause of degradation
(thermal throttling, a background process) may recur.

---

## 4. The colour pipeline

### 4.1 OKLCH everywhere, sRGB only at output

The entire colour pipeline works in OKLCH:

```
  Musical key (Camelot number 1–12)
       │
       ▼
  Hue = (camelot - 1) × 30°          ← a circle mapped to a circle
       │
  Chroma = f(certainty)              ← pale = unsure, vivid = known
       │
  Lightness = f(energy, level)       ← dark = quiet, bright = loud
       │
       ▼
  OKLCH → Oklab → LMS → linear sRGB → sRGB
```

**The Camelot-to-hue mapping.** 12 keys, 360° of hue, 30° per step. Minor (A)
and major (B) at the same number share a hue but differ in chroma — minor is
slightly desaturated, which is perceptually correct (minor keys sound "darker")
and provides a second channel besides the `A`/`B` text label.

**The conversion in GLSL** (for Tier 3):

```glsl
// oklch.glsl — OKLCH polar → linear sRGB
// Based on Björn Ottosson's Oklab specification.
// Conversion chain: OKLCH → Oklab → LMS → linear RGB

vec3 oklch_to_linear_rgb(float L, float C, float h) {
    float a = C * cos(h);
    float b = C * sin(h);
    // Oklab → LMS (cube root space)
    float l_ = L + 0.3963377774 * a + 0.2158037573 * b;
    float m_ = L - 0.1055613458 * a - 0.0638541728 * b;
    float s_ = L - 0.0894841775 * a - 1.2914855480 * b;
    // LMS → linear sRGB
    float l = l_ * l_ * l_;
    float m = m_ * m_ * m_;
    float s = s_ * s_ * s_;
    return vec3(
        +4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s
    );
}
```

**Greyscale test** (VISUAL-LANGUAGE.md §4): dropping the chroma channel to 0
produces the greyscale view. The lightness channel alone must still convey
level, energy, and the trunk/foliage distinction. This is a CI-automatable
screenshot test once the renderer exists.

### 4.2 Accessibility: never hue alone

Every hue channel carries a redundant one (VISUAL-LANGUAGE.md §4):

| Information      | Hue channel    | Redundant channel                      |
|------------------|----------------|----------------------------------------|
| Musical key      | OKLCH hue      | Text label (`8A`)                      |
| Key compatibility| Blend vs seam  | Behaviour: smooth merge vs hard seam   |
| Level / energy   | Lightness      | Width of the river                     |
| Certainty        | Chroma (saturation) | Text + icon (⚠ low confidence)    |
| EQ band          | Vertical position in strata | Label (Lo / Mid / Hi)       |

---

## 5. The audio clock

### 5.1 Beat-locked animation

VISUAL-LANGUAGE.md §5: *everything that pulses pulses on the beat.* The
snapshot already carries `beat_phase: f32` per deck (0.0 at the downbeat, 1.0
at the next). The world copies it directly into `entity.vitality.pulse_phase`.

The renderer uses it as follows:

**Canvas 2D (Tier 2):**
```typescript
// Beat crest: a bright band at the beat phase position along the river path
const t = entity.vitality.pulsePhase;
const [cx, cy] = pathPointAt(entity.form.path, t);
ctx.save();
ctx.globalAlpha = 0.6 + 0.4 * Math.cos(t * Math.PI * 2);
ctx.beginPath();
ctx.arc(cx, cy, entity.form.width * 0.8, 0, Math.PI * 2);
ctx.fillStyle = oklchToCSS(entity.colour.L + 0.15, entity.colour.C, entity.colour.h);
ctx.fill();
ctx.restore();
```

**WebGL (Tier 3):**
```glsl
// In river.frag
uniform float u_beat_phase;  // per-deck, 0..1
float crest = smoothstep(0.03, 0.0, abs(pathT - u_beat_phase));
L += crest * 0.15;  // brighten at the crest
```

### 5.1b Aligning the pulse with the room

The crest has to land when the **room** hears the beat, not when the engine
computed it. Those differ by the output chain's latency plus however old the
snapshot is, and both numbers are known:

```typescript
/**
 * Where in the beat the room is, now.
 *
 * `beat_phase` is where the *engine* was when the snapshot was taken. Two
 * corrections turn that into where the room is: the age of the snapshot, and
 * the output latency the audio has yet to travel through. Without the second
 * the crest sits visibly early — at 128 BPM a 20 ms error is 4% of a beat.
 */
function roomPhase(deck: DeckSnapshot, master: MasterSnapshot, takenAt: number): number {
    if (!(deck.bpm > 0)) return 0;
    const beatsPerSecond = deck.bpm / 60;
    const ageSeconds = (performance.now() - takenAt) / 1000;
    // Subtracted, not added: the room is *behind* the engine by the latency,
    // so the visual pulse must be too.
    const lagSeconds = master.output_latency_ms / 1000;
    const phase = deck.beat_phase + (ageSeconds - lagSeconds) * beatsPerSecond;
    // `rem_euclid`, not `%`: a negative phase is early in the previous beat,
    // not a negative beat.
    return ((phase % 1) + 1) % 1;
}
```

`output_latency_ms` is already in `MasterSnapshot`, stated rather than left to
be discovered, and constant whether the limiter is engaged or bypassed.

**The test that matters** is not that the number is right but that **two decks
in sync draw one crest.** If the screen shows them apart while the room hears
them together, the DJ believes the room, stops reading the screen, and every
other channel loses its credibility at the same time. Since both decks take the
same correction, sync is preserved by construction — which is the argument for
applying it once, here, rather than per-renderer.

### 5.1c The alarm channel

VISUAL-LANGUAGE.md §5 gives peripheral motion to **one** claim at a time.
Ranking is a pure function of the snapshot and belongs in `dj-world` beside the
rest of the world's rules, not in a renderer:

```rust
/// What currently owns the peripheral channel, if anything.
///
/// Exactly one, and the highest-ranked. Peripheral attention is close to a
/// single channel: three things claiming it means none of them arrive, and an
/// interface this alive becomes one a DJ learns to ignore -- which is worse
/// than a still one.
pub enum Alarm {
    /// The audience is hearing it right now.
    Dropouts,
    /// A playing deck about to end with nothing cued. The unrecoverable one.
    RunningOut { deck: u8 },
    /// The mix is being damaged while it plays.
    Limiting,
    /// Expected, and handled.
    EndingSoon { deck: u8 },
    /// Information, not urgency.
    Arrived,
}
```

Everything not holding the channel still shows its state — as **static form**,
which stays legible when looked at. Losing the alarm is losing the *motion*, not
losing the information.

Two rules the renderer must honour, both from §5's "onset, not state":

1. **Motion is spent on the transition**, then settles. A claim that has held
   the channel for a while stops moving and rests as a shape.
2. **A worsening condition is a new transition** and earns a fresh onset. That
   is the only way a second event is distinguishable from the first.

### 5.1d What the first implementation actually cost

Measured on the no-GPU floor, with the watershed drawing two rivers over the
existing deck panels:

| State | fps | What it says |
|---|---|---|
| Nothing loaded, dry beds | 32 | |
| Two decks playing, waveforms scrolling | 13 | |

**The watershed is not what costs.** ADR-0004's own table measured 15.1 fps for
two scrolling waveform lanes on this machine before any of this existed, so 13
is that same known cost — the rivers are close to free beside it. The first
reading of these numbers blamed the new code, wrongly; the comparison against
the older table is what settles it.

Two things the implementation did get wrong, both caught by running rather than
reading:

**The renderer animated when nothing was moving.** A dry bed, a paused deck and
a reduced-motion preference all have nothing to animate, and the component was
still driving `requestAnimationFrame` at 60 Hz to repaint identical pixels.
"Stillness is the default" is a rule about what the *machine* does as much as
what the DJ sees. The loop now starts and stops with the world.

**Gradients are far more expensive than the benchmark suggested.**
`ui/src/renderbench.ts` measures flat-filled discs, which under-counts
`createLinearGradient` plus `addColorStop` work considerably. The gradient is
now cached against the tint and the two y coordinates, none of which change on
most frames. Worth remembering when reading that table: it bounds shape *count*,
not fill cost.

### 5.2 Snapshot transport

The snapshot reaches the webview over the existing Tauri Channel at 60 Hz.
For the living world, the relevant fields are:

```typescript
interface WorldSnapshot {
    decks: DeckSnapshot[];     // per deck: beat_phase, bpm, volume, peak,
                               //           position, remaining, key, grid_confidence,
                               //           eq_lo/mid/hi, filter, filter_resonance,
                               //           pitch, keylock, key_shift,
                               //           loop_active, loop_start, loop_end,
                               //           cues: [f32; 8], playing: bool,
                               //           slip_active, slip_position, reverse,
                               //           stems: { vocal, drums, bass, other },
                               //           fx_slots: [{ kind, depth, timing, active }],
                               //           jog_touch, jog_velocity, cue_point
    mixer: MixerSnapshot;      // crossfader, crossfader_curve,
                               // channel_faders: [f32], channel_gains: [f32],
                               // master_level, limiter_gr,
                               // pfl_cue: [bool], cue_master_blend, split_cue,
                               // booth_level, mic_active, mic_ducking
    sampler: SamplerSnapshot;  // slots: [{ active, mode, level, position }]
    recording: RecordingSnap;  // active, duration, broadcast_active
    system: SystemSnapshot;    // cpu_load, xruns, session_phase
    assistant: AssistantSnap;  // proposed_action, proposed_fork
}
```

This is already produced. The world model consumes it; no new data path is
needed.

---

## 6. Metaphor-to-technique reference

The complete mapping from VISUAL-LANGUAGE.md's world to concrete rendering
decisions. Every row names the section of VISUAL-LANGUAGE.md it implements.

### 6.1 The river — a deck (§2, "The river")

| State               | Visual                | Tier 2 (Canvas 2D)              | Tier 3 (WebGL)                     |
|----------------------|-----------------------|----------------------------------|-------------------------------------|
| Track loaded         | Spring opens          | Path appears, alpha fade-in      | SDF fades from 0 to full width     |
| No track             | Dry riverbed          | Empty path outline, muted colour | SDF at zero width; riverbed visible|
| Playing              | Flowing water         | Beat crest animates along path   | `sin(pathT - u_beat_phase)` in L   |
| Paused               | Still water           | No crest animation; static fill  | Crest amplitude → 0                |
| Tempo (BPM)          | Current speed         | Crest travel speed = 1/BPM       | Uniform `u_bpm`                    |
| Beat phase           | Travelling crest      | Bright band at `beat_phase`      | Lightness peak at `u_beat_phase`   |
| Position             | Location on river     | Playhead marker on path          | Playhead uniform                   |
| Time remaining       | Distance to mouth     | Visible river length shrinking   | Path length shortens toward end    |
| Volume (ch. fader)   | Amount of water       | Path width ∝ volume              | SDF radius ∝ `u_volume`           |
| Peak level           | Surface agitation     | Noise amplitude on path edges    | fBm amplitude ∝ `u_peak`         |
| Gain                 | Channel depth         | Riverbed cut deeper; water sits higher | SDF baseline offset ∝ `u_gain`  |
| Pitch fader          | Gradient (slope)      | Path angle tilts slightly        | Path Y offset ∝ `u_pitch`        |
| Pitch bend (nudge)   | Gust across water     | Brief speed pulse, spring-settling| Temporary crest-speed multiplier   |
| Keylock              | Colour vs speed       | When on: hue stays, speed changes| Uniform flag; hue decoupled        |
| Key shift            | Hue rotation          | Hue rotates directly             | `u_hue += key_shift * 30°`        |
| Grid confidence      | Water clarity         | Fill opacity ∝ confidence        | Chroma ∝ `u_grid_confidence`      |
| Unanalysed           | Mist                  | Noise-alpha gradient overlay     | fBm in fragment alpha              |
| Failed to decode     | Dry spring            | Empty path outline, error text   | Wireframe SDF + DOM error          |
| Slip mode active     | Ghost river           | Second path at 20% opacity flowing beneath | Second SDF pass, low alpha    |
| Reverse              | Current reversal      | Crests travel upstream           | Negate crest phase direction       |
| Jog touch (vinyl)    | Hand in water         | Flow stops; position tracks drag | Crest animation pauses             |
| Jog nudge (CDJ)      | Finger trailing       | Temporary speed offset, settling | Brief velocity impulse uniform     |

### 6.2 The riverbed — the waveform (§2, "The riverbed")

The waveform tiles from `dj-render` *are* the riverbed. No change to tile
generation. The living layer composites around them:

- **Canvas 2D:** The river path is drawn *on a layer below* the tile strip
  (using a second canvas or `globalCompositeOperation`) so that tiles appear
  *on top of* the river body — the waveform terrain showing through the water.
- **WebGL:** The tile strip remains CSS-transformed DOM; the WebGL canvas draws
  behind it with a transparent clear colour, and the river SDF leaves a
  transparent hole where tiles should show through.

### 6.3 The water column — the EQ (§2, "The water column")

Three horizontal strata within the river's cross-section:

```
  ┌─────────────── river width ────────────────┐
  │ ░░░░ high (surface: light, spray)    ░░░░ │  ← hi EQ gain
  │ ████ mid  (body of the water)        ████ │  ← mid EQ gain
  │ ▓▓▓▓ low  (deep current: dark, slow) ▓▓▓▓│  ← lo EQ gain
  └────────────────────────────────────────────┘
```

Killing a band sets that stratum's opacity to 0, which visually "dries" it.
The lightness of each stratum encodes its gain.

**EQ kill switch** is instantaneous drought: the stratum snaps to zero alpha
in one frame — no spring transition. This matches the audible discontinuity.

**Filter** narrows the river width asymmetrically: a low-pass filter removes
the top (high) stratum and tapers the river from above; a high-pass removes
the bottom (low) stratum and tapers from below. The filter position (0–1)
controls how much of the cross-section remains.

**Filter resonance** is a standing wave at the cut edge: a brighter, thinner
line at the boundary between remaining and removed strata. Higher resonance
= brighter line, slight oscillation.

### 6.3b The four currents — stems (§2, "The four currents")

When stems are active, each river splits into four visible layers within its
cross-section. These are the same three strata as EQ (low/mid/high) plus a
fourth surface layer, but assigned by *source* rather than by *frequency*:

| Stem     | Position in cross-section | Canvas 2D                         | WebGL                              |
|----------|---------------------------|------------------------------------|------------------------------------||
| Vocals   | Top shimmer               | Bright overlay strip at river surface | Additional lightness band at top  |
| Drums    | Beat crests               | Crest amplitude modulated by stem level | Crest uniform `u_stem_drums`   |
| Bass     | Deep undertow             | Dark band at river bottom           | Low-stratum opacity ∝ `u_stem_bass`|
| Other    | Body                      | Main fill opacity                   | Body alpha ∝ `u_stem_other`       |

**Muting a stem** sets its layer to zero opacity — that current dries up.
**Soloing a stem** sets the other three layers to low opacity (~0.15) — the
river narrows to a single visible current.

### 6.3c The rapids and gorges — effects (§2, "The rapids and gorges")

Effects are terrain features that the river flows through. Each FX slot is a
stretch of terrain drawn along the river path downstream of the playhead.

| Effect         | Terrain kind | Canvas 2D technique                          | WebGL technique                          |
|----------------|--------------|----------------------------------------------|------------------------------------------|
| Echo / delay   | Canyon       | Repeated fading copies of the crest, spaced at beat division, drawn downstream | Instanced crest copies with diminishing alpha |
| Reverb         | Cavern       | River path widens into a diffuse pool, fill opacity drops | SDF width multiplied, chroma reduced   |
| Flanger/phaser | Interference | Sinusoidal stripe pattern across river surface | `sin(uv.y * freq + u_time)` modulating L |
| Filter (FX)    | Narrows      | Same as channel filter; the river constricts  | Same SDF width reduction                 |
| Gate           | Sluice       | Periodic gaps in the river fill (dry bars)    | Periodic alpha mask: `step(fract(t * divisions), u_gate)` |
| Bitcrush       | Cascade      | River surface quantised to visible steps      | `floor(L * steps) / steps` — posterise L |
| Roll           | Rapid eddies | Small whirl arcs at beat-division spacing      | Instanced annular SDFs along the path    |
| Brake          | Shallows     | Path gradient flattens toward horizontal; water pools | Crest speed → 0 via uniform ramp      |
| Backspin       | Reversal     | Crests travel upstream                         | Negate crest phase direction             |

**FX depth (wet/dry)** controls the length of the terrain feature along the
river path. Dry = zero-length terrain (not visible). Fully wet = terrain
stretches across the visible path.

**Beat-synced timing** controls the spacing of repeating features: canyon wall
spacing = beat division. Visible as the gap between echo reflections or gate
bars.

### 6.3d Transport — play, pause, cue (§2, "Transport")

| Control      | Rendering                                                   |
|--------------|-------------------------------------------------------------|
| Play         | Spring-animated flow start: crest amplitude ramps from 0 to 1 over ~200ms |
| Pause        | Spring-animated flow stop: crest amplitude decays to 0; surface goes glassy |
| Cue point    | A **dam marker** — a strong vertical line across the river at the cue position, rendered as a DOM element (trunk) |
| CDJ cue hold | While held: water flows from the dam; on release: snaps back to the dam position; the river view recentres |

### 6.3e Pools and vessels — sampler (§2, "Pools and vessels")

Sampler slots are drawn as small pools above the confluence, in a row:

| State          | Rendering                                                  |
|----------------|------------------------------------------------------------|
| Slot filled    | A small circular pool, hue from sample content key          |
| Slot empty     | A dry basin outline                                         |
| Triggered      | The pool tips — a stream of water pours into the confluence; one-shot = empties, loop = circulates |
| Level          | Pool size ∝ sample volume                                  |
| Sync           | Pool's internal ripple locked to master beat phase          |

Lower priority; Canvas 2D circles suffice — no SDF pipeline needed.

### 6.3f Recording — the dam at the estuary (§2, "Recording")

| State              | Rendering                                              |
|--------------------|--------------------------------------------------------|
| Recording active   | A visible dam closes at the estuary; water accumulates behind it as a growing pool (the recording grows) |
| Recording stopped  | The dam opens; the pool is sealed                       |
| Broadcast active   | A canal branches from the estuary downstream — a second outflow path |

### 6.4 Eddies, stones and stepping stones (§2, "Eddies, stones…")

| Feature       | Shape                | Animation                                  |
|---------------|----------------------|--------------------------------------------|
| Loop          | Annular arc          | Dash-offset rotation at beat rate           |
| Loop halve    | Arc radius halves    | Spring-animated radius transition           |
| Loop double   | Arc radius doubles   | Spring-animated radius transition           |
| Saved loop    | Persistent arc       | Drawn even when loop is not active; dimmed  |
| Hot cue       | Filled circle        | On the river path at the cue's position     |
| Beat jump     | Row of circles       | Spaced one beat apart along the path        |
| Quantize on   | Circles snap to crests| Stepping stones lock to beat grid positions |

### 6.5 The confluence — the mixer (§2, "The confluence")

Two river SDFs meet. The crossfader value controls the blend:

```glsl
// confluence.frag — smooth minimum of two river SDFs
float river_a = sdRiver(uv, u_path_a, u_width_a);
float river_b = sdRiver(uv, u_path_b, u_width_b);

// k is the blend radius, controlled by crossfader
float k = mix(0.001, u_blend_max, u_crossfader_blend);
float d = smin(river_a, river_b, k);

// Harmonic compatibility → blend or seam
float camelot_distance = u_camelot_distance;  // 0 = same key, 6 = opposite
if (camelot_distance <= 2.0) {
    // Compatible: smooth merge, colours blend
    vec3 col = mix(col_a, col_b, smin_factor);
} else {
    // Clashing: hard seam — achromatic stripe at the meeting line
    float seam = smoothstep(0.01, 0.0, abs(river_a - river_b));
    col = mix(mix(col_a, col_b, 0.5), vec3(L, L, L), seam * clamp(camelot_distance / 6.0, 0.0, 1.0));
}
```

**Channel faders as sluice gates.** Each river's width before the confluence is
multiplied by the channel fader value (0..1). Fader at zero = the river is dry
before it reaches the merge — no water passes. The sluice is a DOM hit target
(trunk); its visual effect is the river narrowing.

**Crossfader curve.** The `smin` blending factor `k` is shaped by the curve
setting: a sharp curve makes `k` transition abruptly (hard cut), a smooth
curve makes it transition gradually (long blend). The DJ sees a sudden
diversion vs a gradual merge.

**Crossfader assign.** Rivers assigned to A enter from the left of the
confluence; B from the right. A river assigned to *thru* bypasses the
confluence entirely — its SDF is drawn directly into the estuary at full
width, unaffected by the crossfader uniform.

**Sync visibility.** When two decks are in sync, their beat crests align at the
confluence — the crests arrive at the merge point simultaneously. Out of sync,
the crests visibly interfere, producing a moiré-like beating pattern. This is
not a special effect; it is the natural consequence of drawing two
phase-offset sine waves with the same spatial frequency.

**VU meters as water level.** The water level against the river banks IS the
VU meter. Per-channel: the height of the water in each tributary. Master: the
height at the estuary. Peak: the surface agitation — churning, splashing over
the bank edge.

**Headphone cue (PFL).** A side channel branches off upstream of the confluence.
When PFL is active for a deck, a smaller stream visually forks from that
river's path. The cue/master blend controls how much confluence water mixes in.
Split cue: the side channel visibly divides into two sub-channels (L/R).

**Microphone.** A new spring opens directly at the confluence — not from the
highland, not from a track. When mic ducking is active, the main rivers' widths
are multiplied by `(1.0 - ducking_amount)`, visibly receding.

**The limiter as the estuary's banks.** The master output section's SDF has a
maximum width. Gain reduction narrows it: `width *= (1.0 - limiter_gr)`. The
constriction is visible.

### 6.6 The highland — library and browser (§2, "The highland")

The highland is drawn above the deck rivers in the layout, as a calmer, more
static region. Its living elements are:

| Element              | Rendering                                                |
|----------------------|----------------------------------------------------------|
| Unscanned folders    | Mist overlay (fBm alpha) that retreats as scanning progresses |
| Track springs        | Small circles in a grid, hue from key, chroma from certainty |
| Basins (crates)      | Grouped springs sharing a background fill — a catchment  |
| Ridges (folders)     | Background partitions grouping basins                    |
| Play history delta   | A faded-out river fragment below the current position    |
| Duplicates           | Two springs sharing a visible connecting line (same underground source) |
| Star ratings         | Spring brightness / prominence                           |
| Colour coding        | A flag element (DOM, trunk) beside the spring            |

This is lower priority (V6 in VISUAL-LANGUAGE.md §11) and does not need the
full SDF pipeline — Canvas 2D primitives suffice.

### 6.7 Weather, light and season (§2, "Weather, light and season")

| State              | Visual technique                                          |
|--------------------|-----------------------------------------------------------|
| CPU load / xruns   | Noise amplitude multiplier on all path control points. Normal: 0. High load: up to 0.3, causing visible jitter. An xrun is a sharp gust — a single-frame spike in noise amplitude. |
| Clock drift        | The two rivers' crest frequencies visibly diverge, pulling against each other. |
| Session phase (M9) | Background gradient: hue rotates from warm (30° amber, dawn) through neutral (180°, noon) to cool (240° blue, dusk). Controlled by a `u_session_phase` uniform, 0..1. |
| Assistant proposal | A second river path, drawn at 30% opacity, branching from the playhead. Fades in when a proposal arrives, fades out when dismissed. |
| Automix active     | Rivers sequence themselves — the next spring opens automatically as the current river approaches its mouth. A subtle connecting flow line between the current river's mouth and the next spring. |

---

## 7. The DOM layer — trunk

### 7.1 What stays in the DOM, always

VISUAL-LANGUAGE.md §6: *Canvas paints, DOM listens.*

Every interactive or textual element is a real DOM node:

- **Hit targets:** Invisible `<button>`, `<input[range]>`, `<div[role=slider]>`
  elements positioned by the world model over the canvas form.
- **Text:** BPM, time, key, gain, track title — all DOM `<span>` elements,
  never canvas `fillText`.
- **Focus rings:** CSS `:focus-visible` outlines on the DOM elements.
- **ARIA:** `role`, `aria-label`, `aria-valuenow` on every control.
- **Keyboard navigation:** Standard tab order; `Enter` and `Space` on buttons;
  arrow keys on sliders.

### 7.2 Positioning

The world model computes layout positions for every entity. These reach the
webview as an array of `{ id, x, y, w, h }` records. A thin Svelte layer
positions the DOM hit-target elements with `transform: translate(x, y)` and
`width`/`height` to match. The canvas renderer draws the living form at the
same coordinates.

**The two must not drift apart.** This is the standing maintenance cost
ADR-0009 acknowledges. The world is the single source of both positions — the
DOM layer does not compute its own.

---

## 8. Dependencies and licensing

Every dependency must be MIT or Apache-2.0 compatible per
[ADR-0002](adr/0002-clean-room-permissive-licensing.md).

### 8.1 What is needed from outside

| Need                       | Source                         | Licence         | Notes |
|----------------------------|--------------------------------|-----------------|-------|
| Simplex noise (GLSL)       | `stegu/webgl-noise`            | MIT             | 2D/3D simplex, drop-in `.glsl` files. No runtime dependency. |
| Simplex noise (TypeScript) | Port of the above              | MIT             | Same algorithm, for Tier 2. ~80 lines. |
| OKLCH ↔ sRGB conversion   | Björn Ottosson's specification | Public domain   | ~30 lines of matrix math. No library. |
| SDF primitives (GLSL)      | Inigo Quilez's published formulas | MIT           | `sdCapsule`, `sdCircle`, `smin`. ~60 lines. |
| Spring physics             | Hand-rolled                    | —               | ~40 lines of damped harmonic oscillator. |

### 8.2 What is explicitly not used

| Rejected              | Why                                                            |
|-----------------------|----------------------------------------------------------------|
| three.js              | ADR-0009: 3D scene graph for a 2.5D problem; hundreds of kB; wants to own scene semantics. |
| PixiJS                | 2D renderer, but still 150+ kB for a fraction of its features. Our world model is the scene graph. |
| regl / twgl           | Thin WebGL wrappers. Useful, but the shader surface is small enough that raw `gl.*` calls in a single file are clearer and debuggable. No abstraction earns its keep for 5 shaders and 3 draw calls. |
| Any ECS library       | ADR-0009 rejected a full toolkit. The entity table is 200 entries with a fixed schema; a library adds indirection without solving a problem we have. |
| Any animation library | Spring physics is 40 lines. A library (Motion, anime.js) would impose its own timing model, and we need the audio clock, not wall time. |

### 8.3 Total added weight

| Item                    | Size (minified, gzipped) |
|-------------------------|--------------------------|
| Noise (TS, Tier 2)      | ~1.5 kB                  |
| OKLCH conversion (TS)   | ~0.8 kB                  |
| Spring stepper (TS)     | ~0.5 kB                  |
| World scene deserialiser| ~2 kB                    |
| Canvas 2D renderer      | ~6 kB                    |
| WebGL renderer + shaders| ~10 kB                   |
| **Total JS added**      | **~21 kB**               |

The shader source (GLSL) is compiled at context creation and does not count
toward the JS bundle.

---

## 9. Build order

Mapped to [VISUAL-LANGUAGE.md §10](VISUAL-LANGUAGE.md#10-building-it) and the
existing milestones.

### V1 — The world model

**What:** Entity schema, component types, the simulation step (spring physics,
pulse phase, colour derivation), scene serialisation, tier selector.

**Where:**
- `dj-app/src/world/` — Rust: `Entity`, `World`, `tick()`, `serialise()`.
- `ui/src/world/` — TypeScript: scene deserialiser, tier probe, DOM positioner.

**Depends on:** ADR-0008 widget names (for entity identity). Does not require
ADR-0008's full layout tree — entity `id` strings suffice.

**Tests:**
- Greyscale test: serialise a world state, render it with chroma=0, assert all
  information is distinguishable.
- Still-frame test: render one frame, no animation, assert all DJ questions
  from VISUAL-LANGUAGE.md §3 are answerable.
- Spring convergence: step a spring 1000 times, assert it settles within the
  excursion bound.

---

### V2 — Canvas 2D renderer, one river, one deck

**What:** The Canvas 2D renderer drawing one deck's river, with flow, beat
crest, riverbed composition, clarity, and the first markers (one cue, one
loop). The DOM hit-target layer positioned over the canvas.

**Where:**
- `ui/src/world/canvas2d.ts` — the draw loop.
- `ui/src/world/noise.ts` — simplex noise (MIT port).
- `ui/src/world/oklch.ts` — OKLCH ↔ sRGB.
- `ui/src/world/spring.ts` — damped harmonic oscillator.

**The two tests that gate it:**
1. Greyscale: `canvas2d.ts` with chroma forced to 0. Every state visible.
2. Still frame: one rendered frame of a loaded, paused deck. A DJ can tell:
   track loaded, paused, BPM, key, time remaining, grid confidence.

---

### V3 — The confluence, two rivers

**What:** Two rivers meeting at the crossfader. Sync/phase alignment. Harmonic
blend vs seam. The limiter as estuary constriction.

**New Canvas 2D techniques:** `smin` as a JS function for path-width blending
at the merge point. Dual-crest drawing.

---

### V4 — Strata, terrain and currents

**What:** EQ strata, filter narrowing, effects-as-terrain, stems-as-currents,
transport controls (dam/cue). The full control surface drawn in the world.

The step is larger than V2–V3 because it covers three new entity families
(Terrain, Current, Dam), but each is simpler than the river or confluence
it attaches to.

---

### V5 — WebGL renderer

**What:** The same world drawn by WebGL. This is where the SDF pipeline,
the GLSL noise, the OKLCH shader, and instanced rendering land.

**The test that proves V1:** The Canvas 2D and WebGL renderers must produce
visually indistinguishable output from the same scene buffer. If they diverge,
the world model was leaking renderer assumptions.

**Where:**
- `ui/src/world/webgl/` — renderer, shader sources, uniform packing.
- `ui/src/world/webgl/shaders/` — `.vert` and `.frag` files.

---

### V6 — Highland, periphery and session

**What:** Library (springs, basins, ridges, mist), sampler pools, recording
dam, session light, CPU weather, automix, assistant fork. The periphery.

---

## 10. Standing constraints

These survive every step and are the tests that enforce the design rules
from VISUAL-LANGUAGE.md:

1. **Greyscale.** Switch the display to greyscale (drop chroma to 0) and the
   interface must still work. §4.
2. **Still frame.** Freeze the world (stop the `tick()` loop) and every DJ
   question from §3 must be answerable from the rendered frame. §5.
3. **Reduced motion.** `prefers-reduced-motion` holds the world still; Tier 0
   alone must communicate the full state. §5.
4. **Trunk never occluded.** No canvas-drawn foliage may cover a DOM hit
   target. §6.
5. **Centre never moves.** A control's `transform` origin stays fixed; drift
   is excursion around it. §5.
6. **Numbers always legible.** BPM, time, key, gain are DOM text, always. §7.
7. **Every channel answers a question.** If a visual channel cannot name its
   row in VISUAL-LANGUAGE.md §3's table, it is cut. §3.
8. **Tier 3 and Tier 0 convey the same state.** Higher tiers say it more
   beautifully; they must never say it more completely. §8.

---

## 11. What this document is not

- **Not a schedule.** V1–V6 is an ordering, not a timeline. Each step is sized
  by what must be true before the next can start.
- **Not a renderer specification.** The shader source above is illustrative. The
  real shaders will be written against measured frame budgets on real hardware.
- **Not a replacement for ADR-0009.** That ADR records the *decision*; this
  records the *tech*. If they disagree, the ADR wins.
