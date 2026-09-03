# The adaptive cockpit — audit and redesign analysis

**Status:** Phase 0 complete. No component has been redesigned yet.
**Scope:** the interface and the adaptive-assistant experience around it. The
realtime audio core, the action bus and the parameter registry are not in
scope except as things the interface must keep using correctly.

This document is the thing that has to exist before any `.svelte` file is
touched. It is an audit of what djmanzo's interface actually is — measured, not
recalled — followed by the model the overhaul is built on.

---

## 0. How the numbers here were obtained

Everything counted below was counted in the working tree at `v0.10.0`, not read
off the documentation. Where the documentation and the code disagreed, the code
won and the disagreement is noted.

Two limits on this audit, stated up front:

- **Nothing here has been heard.** There is no audio device, microphone, camera
  or phone in the machine this was written on. Claims about sound are absent
  rather than optimistic.
- **The layout figures in this document were floors when it was written, and
  are not any more.** `ui/e2e/` was not drawing the deck's pad zone, because
  the browser stub answered one command with `null` and the resulting throw
  ended the render pass early. Fixed in the same phase that made the deck
  render from the widget tree, which is how it was found. A deck measures
  **878 px**, not the 675 quoted below and in `ROADMAP.md`; the pad zone is
  197 of the difference. Where a figure in Part One still reads 675, it is the
  old measurement and the shape of the argument is unchanged -- the deck is
  too tall, by more than was thought.

---

# PART ONE — THE AUDIT

## 1. Component inventory

37 Svelte components, **17,614 lines**. The ten largest:

| Component | Lines | What it is |
|---|---:|---|
| `App.svelte` | 1,724 | shell, top bar, panel switch, layout, keyboard |
| `Library.svelte` | 1,451 | track table, search, filters, sidebar |
| `Settings.svelte` | 1,331 | every preference in the application |
| `Deck.svelte` | 1,320 | one deck, top to bottom |
| `Stems.svelte` | 655 | four stems, mutes, volumes, swap |
| `Requests.svelte` | 597 | the room's requests |
| `Memory.svelte` | 576 | find a record from what you remember |
| `Assistant.svelte` | 555 | the assistant panel |
| `Plan.svelte` | 540 | set planning |
| `SideView.svelte` | 511 | the sidelist |

Four files carry 5,826 lines — a third of the interface. Each is doing several
jobs at once, which is the first structural problem.

## 2. The current UI map — and the finding that drives the overhaul

`App.svelte` holds a single piece of state:

```ts
let panel = $state<
  | "none" | "browse" | "assistant" | "presets"
  | "sampler" | "settings" | "keyboard" | "mapping"
>("none");
```

**One panel at a time, out of eight.** Everything else the application can show
is nested inside one of those eight, and the nesting is deep:

```
App
├── Deck ×N ── Waveform, Overview, JogWheel, Pads, Stems, Fx
├── MasterMixer
├── Mic, Plugin, Automix, Watershed, ThemeSwitcher, Shortcuts
└── panel (exactly one of):
    ├── Browse ── Library ── Crates, Journal, Memory, Plan,
    │                        Requests, ShareSet, SideView      ← depth 3
    ├── Assistant ── Conduct ── Coach, RoomSense               ← depth 3
    ├── Settings ── Screens
    ├── Presets
    ├── Sampler
    ├── MappingEditor
    └── (keyboard sheet)
```

Read that as workflow rather than as a tree and it says:

- You **cannot watch the room and browse for the next track at the same time.**
  `RoomSense` is three levels inside the Assistant panel; the browser is a
  sibling panel. They are mutually exclusive.
- You **cannot see the set plan and the assistant together.** `Plan` is inside
  Library, inside Browse.
- You **cannot have the library open and the sampler open.**
- Audience **requests arrive while the DJ is looking at something else**, three
  levels down a panel that is closed during a mix.

The most recently built features — room sensing, requests, set planning,
find-from-memory — are the **deepest** ones. The capability graph grew; the
operating environment did not. That is the thesis of this overhaul stated as a
measurement rather than an opinion.

## 3. State inventory

| Layer | What | Size |
|---|---|---|
| Engine → UI | `Snapshot` over a Tauri event at 60 Hz | one object, decks + master + context |
| UI → Engine | `Action` text lines through the bus | one path, no bypass |
| Commands | `#[tauri::command]` | **148** registered in `generate_handler!` |
| Parameters | `ParamId` variants | **448** |
| Crates | Rust workspace | **20** |

The action-bus discipline holds. Every UI control resolves to an action line;
the assistant, controllers, keyboard, network API and scripts all emit the same
vocabulary. **This is the single most valuable thing in the codebase and the
overhaul must not weaken it.**

## 4. Existing layout capability

Two systems, one shipped and one half-built:

- **The flat `Layout`** — thirteen booleans plus a density float. Four presets.
  Can hide, resize and scale a fixed set. Cannot move, reorder, duplicate or
  name anything the binary does not know.
- **The widget tree** (ADR-0008, shipped at `v0.10.0`) — **33 named widgets**,
  **9 slots**, **23 design tokens**, a JSON tree format, a loader that reads
  both formats out of one directory, and a resolver that skips what it does not
  know and *counts* what it skipped. Flat layouts upconvert on load.

**W1 and W2 are built. W3 is not.** `App.svelte` and `Deck.svelte` still
*contain* the layout as markup with an `{#if}` per feature, rather than
rendering the resolved tree. So today a layout file can recolour and can be
validated, but **cannot actually move a widget**. Closing that gap is the
single highest-leverage piece of work in this whole programme, because the
surface/dock model in Part Two is a consumer of it rather than a replacement
for it.

## 5. Visual token inventory

23 skinnable tokens, already used throughout the interface:

`bg, panel, panel-raised, panel-hover, chip, text, text-dim, muted, accent,
accent-2, accent-warm, accent-soft, on-accent, border, border-strong, edge,
line, scrim, warn, danger, radius, radius-s, density`

Plus runtime-driven properties a skin deliberately **cannot** set —
`--audio-*`, `--stem-*` — because a layout that could pin them would be a
layout lying about the mix.

The tokens are named for *appearance* (`accent`, `panel-raised`), not for
*meaning* (`incoming`, `outgoing`, `uncertain`). That is the gap §30 of the
directive names, and it is a rename-and-add job rather than a rewrite: the
mechanism is already there and already enforced.

## 6. The world model

`dj-world` is **2,489 lines** of Rust: entities with form, vitality, bonds and
readings, plus a palette that maps musical key to hue and enforces a greyscale
rule so hue is never the sole carrier of meaning. `ui/src/world.ts` renders it.

ADR-0009's measurements are on record and are unusual enough to repeat, because
they constrain everything visual:

| shapes | DOM transform | Canvas 2D | WebGL |
|---:|---:|---:|---:|
| 60 | 59.8 fps | 60.0 | 59.5 |
| 240 | **27.0** | 57.8 | 60.0 |
| 960 | **18.6** | 45.4 | 59.7 |

…but **inside the real application**, drawing the same scene: Canvas 2D 12 fps,
WebGL 8 fps. The isolated benchmark and the embedded one disagree, and the
embedded one is the one that decides. Compositing a GL surface into a live
document costs more than the drawing saves on software GL.

The rule this yields: **measure the thing you are going to ship, in the place
you are going to ship it.** It applies to every visual proposal in Part Two.

## 7. Current UX problems, measured

| Problem | Evidence |
|---|---|
| One surface at a time | `panel` is a union of 8, exclusive |
| Newest features are deepest | RoomSense, Requests, Plan, Memory at depth 3 |
| The crossfader is below the fold | y 877 at 1280×800 with records loaded — a floor |
| The deck column is 675 px | against ~527 available — a floor |
| Layout cannot move a widget | ADR-0008 W3 not built |
| Tokens are cosmetic, not semantic | 23 tokens, none named for meaning |
| Blue outlines read as focus rings | visible on transport, knobs and faders |
| Settings is 1,331 lines | one panel for every preference in the app |
| No command surface | no palette; everything is a click path |
| ~~The harness under-measures~~ | fixed: pad zone absent (#100), a `null` answer in the stub |

Three of these have already recurred: a performing control has gone below the
fold **three times**, and each time it was found by a human with a screenshot.

## 8. Competitor comparison — as design input, not skins

Researched at the time of writing rather than recalled.

**djay Pro** has moved AI *into the performance interaction model* rather than
into a side panel: Neural Mix crossfaders blend and swap individual stems, with
crossfader FX per stem and mute FX that add echo tails when muting or soloing.
That is the clearest signal in the field — stems are a *performance primitive*,
not four toggle buttons.
([Algoriddim](https://help.algoriddim.com/user-manual/djay-pro-windows/neural-mix/crossfaders),
[Crossfader](https://wearecrossfader.co.uk/blog/djay-5-3-update/))

**Serato** binds stems to eight performance pads per deck — top four toggle
vocal, melody, bass, drums. Hardware-shaped, immediate, no menu.
([Crossfader](https://wearecrossfader.co.uk/blog/serato-dj-3-stems/))

**Traktor Pro 4** generates stems as a *background* process — right-click,
carry on working, load the stemmed version later. Preparation that does not
block performance. ([DJ.Studio](https://dj.studio/blog/dj-software-stem-separation-compatibility))

**rekordbox** is described as powerful but "not always elegant" in the browser,
with preparation pain concentrated in: waiting on analysis, fixing beatgrids
through fiddly dialogs, tagging one right-click at a time, and **scrolling flat
playlists that give no sense of set structure**. DJs leave for hardware
flexibility, creative limitation, or workflow mismatch.
([Ora DJ](https://www.ora-dj.com/blog/best-rekordbox-alternatives),
[Vibes](https://vibesdj.io/learn/gear/rekordbox-dj))

**Engine DJ** is criticised as "not a full DJ app" — a library tool rather than
a performance environment. ([DJ.Studio](https://dj.studio/blog/dj-mixing-software-index-real-users))

The finding that matters most across all of them:

> satisfaction depends less on feature availability and more on whether a tool
> reliably supports a specific role, workflow, and performance context.
> ([DJ.Studio](https://dj.studio/blog/dj-mixing-software-index-real-users))

djmanzo already has the features. It does not yet have the roles.

**"Scrolling flat playlists that give no sense of set structure"** is the single
most actionable competitor finding, and it maps directly onto the Set Flow view
in §20 of the directive.

---

# PART TWO — THE MODEL

## 9. The DJ workflow model

Every surface must serve a step of this loop, or justify itself:

```
Observe → Select → Preview → Prepare → Load → Cue → Align
   → Shape → Transition → Observe response → Adapt → repeat
```

Mapped against what exists today:

| Step | Served by | State |
|---|---|---|
| Observe | Waveform, MasterMixer, RoomSense | room sensing is 3 deep |
| Select | Library, Memory, more-like-this | good, buried |
| Preview | cue monitor | no ghost/preview overlay |
| Prepare | Plan, SideView | two systems,不 one gesture |
| Load | Library → deck | fine |
| Cue / Align | Deck, Waveform | fine |
| Shape | EQ, filter, FX, Stems | fine |
| Transition | transition planner | exists in Rust, thin in UI |
| Observe response | RoomSense | 3 deep, off during mixing |
| Adapt | assistant | panel-exclusive with the browser |

The loop is **broken at "observe response"** — the one surface that closes it is
the one that cannot be open while mixing.

## 10. New information architecture

Four persistent regions, replacing "shell + one panel":

- **Mission Bar** — always present, compact, HUD not toolbar. Phase, occasion,
  AI posture, room, output health, recording, tempo, set duration, device
  status, alerts.
- **Performance Zone** — decks, mixer, waveforms. The only region that is
  *never* demoted.
- **Surface Docks** — left / right / bottom / detached. Many surfaces at once.
- **Context Rail** — 4–8 controls that matter *now*, promoted by context.

Tier discipline (directive §58) governs what may appear where:

| Tier | Content | May be promoted |
|---|---|---|
| 1 glanceable | play state, track, BPM, position, phase, level, next | always visible |
| 2 performable | cue, loop, EQ, filter, pitch, stems, FX, crossfader | always reachable |
| 3 contextual | suggestions, explanations, transition plan, room | rail / dock |
| 4 preparation | metadata, library, analysis, tags, history, settings | dock only |

**Tier 4 must never displace Tier 1 or 2.** That is the rule the current panel
model has no way to express.

## 11. Surface / dock model

A surface is data, not a component conditional:

```ts
type SurfaceDefinition = {
  id: string;
  title: string;
  category: "performance" | "library" | "planning" | "assistant" | "utility";
  minSize: { width: number; height: number };
  preferredSize: { width: number; height: number };
  priority: number;
  performanceCritical: boolean;
  detachable: boolean;
  stackable: boolean;
  collapsible: boolean;
  contextual: boolean;
};
```

**This is deliberately the same shape as `dj_app::widgets::Widget`.** A surface
is a widget with placement metadata, and the registry that already validates
widget trees is the registry that should validate surface placements. Building
a second parallel system would be the mistake ADR-0008 exists to prevent.

24 surfaces to define, all of which already exist as components: Library,
Browser, Prepare, Playlist, Set Plan, Suggestions, Assistant, Conduct, Coach,
Automix, Sampler, FX, Stems, History, Requests, Journal, Audience, Controllers,
Mapping, Settings, Session, Practice, Track Analysis, Transition Lab.

Sixteen of those are components today. Prepare, Suggestions, Practice, Track
Analysis, Transition Lab and Session are new — and five of the six are *views
over data the engine already produces*.

## 12. Workspace model

```ts
type Workspace = {
  name: string;
  surfaces: SurfacePlacement[];
  density: DensityProfile;
  focus: WorkspaceFocus;
  theme: ThemeProfile;
};
```

Persisted as data beside the existing layout trees, in the same directory, read
by the same loader. 23 presets to ship (§7 of the directive), all editable.

## 13. Adaptive context model

One context object, computed in Rust, published on the existing snapshot, and
consumed by *everything*:

```ts
type DJContext = {
  sessionPhase: "setup" | "warmup" | "build" | "peak"
              | "release" | "cooldown" | "closing" | "emergency";
  occasion: OccasionProfile;
  musicContext: MusicContext;
  hardwareContext: HardwareContext;
  audienceContext: AudienceContext;
  djBehaviorContext: BehaviorContext;
  attentionBudget: AttentionBudget;
  performanceHealth: PerformanceHealth;
};
```

**`SessionContext` already exists on the snapshot** (`Snapshot.context`) and
already drives contextual expression. This is an expansion of a field that
ships, not a new subsystem — which is what makes it affordable.

Autonomy and confidence stay **orthogonal** (§9 of the directive):

| | low autonomy | high autonomy |
|---|---|---|
| **high confidence** | "sure, but only suggesting" | may execute |
| **low confidence** | show uncertainty / do nothing | **invalid — never allowed** |

That bottom-right cell being unrepresentable is a type-level obligation, not a
runtime check.

## 14. AI integration model

The posture axis already documented stays: **Off, Watch, Suggest, Prepare,
Assist, Autopilot.** No new switches.

Three rules the implementation is held to:

1. **The AI is a context engine under the GUI, not a panel in it.** Its normal
   output is a marker, a ghost, a one-line reason, a staged transaction — never
   a chat window occupying half the screen.
2. **Non-trivial actions are transactions.** Stage load + cue + gain + loop
   together; show Accept / Modify / Reject; on accept it becomes ordinary
   actions on the ordinary bus and is ordinarily replayable.
3. **Human touch wins instantly, at parameter granularity.** No confirm, no
   mode exit. The AI retreats from *that control* and stays active elsewhere.
   `dj-app` already has takeover-and-resume (#79); this extends it rather than
   inventing it.

The AI may request **typed UI operations** (`show Prepare`, `focus Deck 2`,
`expand next-track rail`) — never arbitrary JavaScript, never DOM mutation.
That vocabulary is the GUI's equivalent of the action bus and must be as
closed.

## 15. Library redesign

Four views over one collection:

1. **Performance Table** — dense, configurable columns, the current table done
   properly.
2. **Compact Cards** — artwork where it earns its place; every card operational.
3. **Set Flow** — tracks as a *sequence* with energy, BPM, key and genre
   trajectories, transition links, anchors and alternates. This is the direct
   answer to the strongest competitor finding: *flat playlists give no sense of
   set structure.*
4. **Pair / Transition view** — two tracks side by side with predicted
   compatibility, vocal conflict and candidate techniques.

Plus a **Next-Track Rail** (3–8 candidates, each with BPM delta, key relation,
energy delta, confidence, one-line reason) and **function tags** (opener,
builder, peak, floor-reset, singalong, transition-tool, closer, safe, risky,
emergency) which are more useful to a working DJ than genre.

And **learned track relationships** — "A → B works", "A → C needs an 8-beat
loop", confidence-weighted. This addresses a real, repeatedly-voiced pain:
remembering combinations that worked.

## 16. Waveform redesign

Keep the Rust renderer. Add semantic layers, and make them *manipulable*:
amplitude, spectral balance, beat grid, phrase, downbeats, cues, loops, vocal
presence, stem presence, mix-in and mix-out regions, breakdowns, drops, energy,
AI recommendation as a ghost layer, confidence, and the runway to the end.

The waveform stops being a picture and becomes **instrumentation**: what is
happening, what is about to happen, what could I do, what happens if I do it.

Direct manipulation — drag a cue, resize a loop, drag a transition boundary —
replaces editing numbers in a settings panel.

## 17. Theme architecture

```
Visual Language
  ├── Semantic Tokens   ← rename/extend the 23 that exist
  ├── Theme             ← palette per token
  ├── World             ← Watershed becomes one of several
  ├── Density
  ├── Motion
  ├── Waveform Language
  ├── Iconography
  └── Surface Styling
```

Semantic roles to add: `incoming`, `outgoing`, `uncertain`, `assistant`,
`audience`, `selected`, `active`, `success`, and the four stem roles.

Watershed becomes `World = Watershed` — kept, not deleted, and no longer the
only possible identity.

Theme adapts slowly: hysteresis, minimum duration, manual lock. **Never
flicker per track.**

## 18. Audience model

```ts
type AudienceContext = {
  activity: number; movement: number; cohesion: number;
  vocalResponse: number; roomNoise: number; visualChange: number;
  brightness: number; confidence: number;
  trend: "rising" | "stable" | "falling";
};
```

Evidence signals, not measurements. **No single fake-precision "crowd score" as
the primary UI.** Normal presentation is a compact `ROOM ↑ / ↓ / STABLE` with a
confidence marker; the dashboard is one click away and nobody has to stare at
it.

The existing baseline discipline is kept and is the right one: compare the room
against *its own earlier state tonight*, never against an absolute threshold,
because every room and every microphone is different.

Causal analysis is where the value is: correlate room change with *DJ actions*
12–30 seconds prior, across sessions. "This kind of transition has historically
improved response in this room" beats any raw reading.

Privacy is non-negotiable and the toggles stay separate: camera, microphone,
observe, store aggregates, use for suggestions, send to cloud. Aggregate
metrics, local processing, no identity recognition, no individual profiling.

---

# PART THREE — GETTING THERE

## 19. What is buildable now vs what needs backend

The directive is explicit: *do not invent backend capabilities merely because a
UI would like them.* This is that audit.

### Buildable today on existing state and actions

| Work | Rests on |
|---|---|
| Surface/dock manager | ADR-0008 registry + widget tree |
| Workspace presets | layout loader, already reads a directory |
| Mission Bar | `Snapshot` — all fields already published |
| Multiple simultaneous surfaces | pure UI restructuring |
| Command palette | 148 commands + the action vocabulary |
| Semantic tokens | 23 tokens, validated, already enforced |
| Theme packs | theme system ships |
| Next-track rail | more-like-this + transition planner exist |
| Library views 1, 2 and 4 | library data ships |
| Function tags | tag system ships |
| Attention budget | occasion + `mistakes_are_costly` ship |
| Instant takeover per parameter | takeover-and-resume ships (#79) |
| Room HUD | RoomSense ships — it needs *promoting*, not building |

That is most of it. **The overhaul is mostly an integration and information-
architecture problem, exactly as the directive's §100 predicts.**

### Needs backend work

| Work | Why |
|---|---|
| ADR-0008 W3 renderer | the tree exists; nothing renders from it |
| `DJContext` expansion | `SessionContext` exists but is narrower |
| Learned track relationships | new storage + confidence decay |
| `TransitionPlan` as a first-class object | planner exists, object does not |
| Waveform semantic layers | renderer must emit more than amplitude |
| Set Flow trajectories | needs per-track energy/phrase served together |
| Causal audience correlation | needs action↔room time-series storage |
| Behavioural learning signals | needs an events table with decay |

### Explicitly out of reach here

Anything requiring audio output, a microphone, a camera or a phone cannot be
verified in this container. It can be built and unit-tested; it cannot be
demonstrated. That will be stated per feature rather than glossed.

## 20. Implementation dependency graph

```
ADR-0008 W3 (render the tree)
   └─> Surface/Dock manager
          ├─> Workspaces + presets
          ├─> Mission Bar
          ├─> Context Rail
          └─> Specialist workspaces

Semantic tokens
   └─> Theme packs ─> World packs (Watershed as one)

DJContext expansion
   ├─> Adaptive promotion (needs dock manager)
   ├─> Attention budget
   ├─> Audience integration
   └─> Suggestion weighting

TransitionPlan object
   ├─> Waveform ghost layer
   ├─> Next-track rail reasons
   ├─> Practice / Transition Lab
   └─> Autopilot staging
```

**Everything downstream of the dock manager is blocked on ADR-0008 W3.** That
makes W3 (#98) the critical path, not an optional tidy-up.

## 20a. Where a deck's 878 pixels go, and why density does not move them

Measured in the browser harness at 1280x800 with two records loaded, after
#100 was fixed. This section exists because Phase 3's gate is a height and the
decision needs numbers rather than impressions.

| Block | px | Block | px |
|---|---|---|---|
| pad zone (tabs 37 + grid 155) | **197** | transport | 40 |
| channel strip (fader 154) | **154** | header | 36 |
| waveform lane | **96** | jump row | 36 |
| overview | 30 | loop row | 36 |
| stems fold | 16 | playhead row | 36 |
| effects fold | 16 | channel foot | 36 |
| times | 14 | meter | 4 |
| progress | 8 | | |

755 in children, 88 in the fourteen gaps between them, 25 in padding.

**The density control barely touches it.** Driving `--density` from 1 to its
floor of 0.8 moves the deck from 878 px to 810 -- 68 px, against the 280 that
would put the crossfader back on screen:

| `--density` | deck |
|---|---|
| 1.0 | 878 px |
| 0.9 | 844 px |
| 0.8 | 810 px |

The reason is that the three largest blocks do not answer to it:

- **The waveform lane is a pixel height from the layout** (96 by default),
  passed to the renderer as a number.
- **The faders and knobs are pixel sizes in `Deck.svelte`** -- `height={140}`
  twice, `size={46}` and `size={56}` -- handed to an SVG that is then drawn at
  exactly that many device pixels.
- **The pad grid's height is set by the deck's *width*.** A pad is an SVG with
  a fixed aspect ratio stretched to its grid cell, so two rows of four are tall
  because the deck is wide. `min-block-size: 3.1rem` is a floor it never
  reaches. This also means the pad zone shrinks when four decks are on screen
  and grows when two are, which is the opposite of what a DJ wants from a
  performance surface.

So `--density` scaled the type and the padding and left the furniture where it
was, which is why the interface "getting denser" never bought the room it
looked like it should.

**All three are fixed.** The pad grid takes a row height in `rem` instead of
its cell's width; the SVG controls multiply their pixel dimensions by
`--density`; and djmanzo picks a density band from the window it was given
(`cockpit::BANDS`, applied unless a layout or workspace names one). Density now
moves a deck 122 px across its range instead of 68, and a deck at djmanzo's own
default window is **685 px, down from 878**.

That is not enough on its own and the arithmetic says so: the stage has 559 px,
so a deck would need about a 0.64 scale against a 0.80 floor. Scaling was never
going to close it, and what did was **pinning, twice**. The master strip came
out of the scrolling stage — it had been inside it, under decks taller than the
stage, so it scrolled away with them. Then the deck did the same one level
down: its body scrolls and its channel strip is pinned, so what goes below the
fold on a short window is the waveform's tail and the loop rows rather than the
volume fader and the filter.

Three things had to change before a deck could pin anything at all. A grid
row's height is its content's, so the deck grid's rows are `minmax(0, 1fr)` —
setting `max-height: 100%` on the deck moved it by zero pixels, because 100% of
an `auto` row is the content again. The stage stopped scrolling, because a
child cannot pin itself inside a parent that grows to fit it. And the booth
panel that sat under the decks became a dock surface, which is what it always
was.

What actually has to change, in descending order of what it is worth: the pad
grid's sizing rule, the fader and knob dimensions, and whether the master strip
stays in the vertical stack at all. The third is the one with several
defensible answers and it is recorded in §21 as the owner's call.

## 21. Migration plan

Vertical slices, each shippable, each green before the next:

| Phase | Deliverable | Gate |
|---|---|---|
| ~~**0**~~ | this document | **done** |
| ~~**1**~~ | semantic tokens, density, motion, surface/workspace schemas | **done** — `crates/dj-app/src/cockpit.rs` |
| ~~**2**~~ | ADR-0008 W3 + dock manager. *New shell, old functionality.* | **done** — the deck renders from the widget tree; surfaces dock; `ui/e2e/docks.spec.ts` enforces reachability |
| 🟡 **3** | performance cockpit: decks, waveform, mixer, rail, mission bar | **the gate is met** — every performing control is on screen at 1280×800, master strip and deck channel strip both pinned, enforced by `ui/e2e/budget.spec.ts` with no `test.fail()` left. The context rail and the mission bar are still to come. |
| **4** | library: table, cards, set flow, pair view, prepare, rail | one gesture Library→Prepare→Deck |
| **5** | intelligence: context engine, transactions, takeover, promotion | posture matrix enforced by test |
| **6** | room: baseline, causal, Room HUD | HUD visible while browsing |
| **7** | theming: packs, adaptation, Watershed as a world | no flicker; hysteresis tested |
| **8** | specialist workspaces | each opens and is usable |

Phase 2's gate is the important one. *New shell, old functionality* means no
feature may become unreachable during the migration — which is exactly the
failure mode a 1,724-line `App.svelte` invites.

## 22. Testing plan

Extending what exists (2,331 Rust tests, 82 vitest, 5 Playwright):

- **Muscle memory** — Play, Cue, deck identity, crossfader, EQ order and jog
  behaviour must be positionally stable across every workspace. A test per
  preset asserting the semantic identity and relative order of Tier 1 and 2
  controls.
- **Action-bus purity** — no component may mutate engine state directly; a
  test that every new UI action resolves to an action line.
- **State consistency** — loading from browser, assistant, preset, controller,
  keyboard, network and drag-drop must produce identical UI state.
- **Deterministic adaptation** — given a context change, the layout response
  must be reproducible. Adaptive must not mean random.
- **Geometry** — the existing budget, extended per workspace. #100 is fixed,
  so it measures the deck djmanzo draws.
- **Nothing may throw while being measured** — the guard that would have caught
  #100 on its first run. Geometry cannot notice a missing zone; a shorter deck
  is not a taller one.
- **Visual regression** — the ten workspace configurations named in §89.
- **Sizes** — 1280×720 through 4K, and constrained height especially.

## 23. Performance plan

The order is fixed and non-negotiable: **audio > control > visual effects.**

- Degrade visuals, never audio. Reduce animation, layers, polling and preview
  cost on constrained machines.
- The waveform stays in Rust. No expensive per-frame DOM loop.
- No AI, vision, network, filesystem, logging or analysis on the audio
  callback. A network server keeps its own thread.
- Re-measure in the application, not in a microbenchmark — ADR-0009's two
  tables disagree by 4× and the embedded one is the one that governs.
- Track: UI frame rate, xruns, memory, CPU, worker utilisation.

---

## 24. What this document commits to

1. No component redesign begins until Phase 1's schemas exist.
2. ADR-0008 W3 is the critical path and is done before the dock manager.
3. Semantic control identity is invariant. Presentation priority is the only
   thing adaptation may change.
4. The AI is never the largest thing on screen.
5. Human touch wins instantly, at parameter granularity.
6. Every pixel claim comes with the harness limitation attached. #100 is
   fixed, and the numbers moved 203 px when it was.
7. Nothing is reported as verified that has not been run.

## Sources

- [2026 DJ Mixing Software Index — DJ.Studio](https://dj.studio/blog/dj-mixing-software-index-real-users)
- [2026 DJ Software Guide for Stem Separation and Workflow Fit — DJ.Studio](https://dj.studio/blog/dj-software-stem-separation-compatibility)
- [Neural Mix crossfaders — Algoriddim](https://help.algoriddim.com/user-manual/djay-pro-windows/neural-mix/crossfaders)
- [djay Pro 5.3 update — Crossfader](https://wearecrossfader.co.uk/blog/djay-5-3-update/)
- [Serato DJ 3.0 stems — Crossfader](https://wearecrossfader.co.uk/blog/serato-dj-3-stems/)
- [Best rekordbox alternatives — Ora DJ](https://www.ora-dj.com/blog/best-rekordbox-alternatives)
- [rekordbox club workflow — Vibes](https://vibesdj.io/learn/gear/rekordbox-dj)
- [DJ Software: Who's Leading The Way In 2026? — Digital DJ Tips](https://www.digitaldjtips.com/best-dj-software-2026/)
