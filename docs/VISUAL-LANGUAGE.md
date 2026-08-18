# The visual language

How djmanzo looks, moves and says things, and why each choice carries information rather than
decorating. The decision behind it is [ADR-0009](adr/0009-the-living-interface.md); this is the
system it commits to.

---

## 1. One world, not a collection of imagery

The metaphor is a **watershed**: a single system of water from the highlands to the sea. That is
a constraint, not a theme. A metaphor teaches only when it is coherent — the moment the interface
borrows a tree here and a flame there because each looked good alone, it stops being something a
DJ can reason inside and becomes decoration with a nature palette.

So there is one world, and everything in the application has a place in it.

```
   highland            the library: springs not yet flowing
      │
   springs             tracks, each waiting to be opened
      │
   rivers              the decks — one river per deck
      │
   riverbed            the waveform: the terrain ahead of you
      │
   confluence          the crossfader, where rivers meet
      │
   estuary             the master output
      │
   the sea             the room
```

The direction is fixed and it matters: **downstream is the future.** The stretch of river ahead
of the playhead is what is about to happen. Every DJ interface already scrolls the waveform this
way; the world simply takes it seriously everywhere else too.

---

## 2. What each thing is

Every row below is state djmanzo already publishes in its 60 Hz snapshot. Nothing here requires
new analysis — that is the main practical argument that this design is buildable rather than
aspirational.

### The river — a deck

| State | In the world |
|---|---|
| Track loaded | a spring opens; the river exists |
| Playing / paused | flowing / still water |
| Tempo (BPM) | rate of the current |
| Beat phase | the travelling crest; the beat is where the crest is |
| Position in track | where you are along the river |
| Time remaining | **distance to the mouth** — the end is visible from far off |
| Volume | the volume of water |
| Peak level | agitation of the surface |
| Pitch fader | the gradient the river runs down |
| Keylock | whether steepening the gradient changes the water's colour, or only its speed |
| Grid confidence | **clarity of the water** — an untrustworthy grid is turbid |
| Not yet analysed | mist over an unsurveyed stretch |
| Failed to decode | the spring is dry, and says why |

Two of these are worth dwelling on because they are better than what they replace.

**Time remaining as distance to the mouth.** Today this is a number that turns red. In the world
it is a thing you can see coming for minutes, in peripheral vision, without looking away from
whatever you were doing. That is exactly how a DJ actually tracks it.

**Grid confidence as water clarity.** `dj-analysis` already refuses auto-sync below a confidence
threshold, and the reason is currently a tooltip. As clarity it needs no explanation: *you do not
navigate water you cannot see through.* The rule and the appearance are the same fact.

### The riverbed — the waveform

The waveform is not a picture beside the river; it **is** the riverbed. Loud is deep and broad;
quiet is shallow. A breakdown is a still pool, a drop is a narrows. Phrase boundaries are bends.

This layer is produced by `dj-render` in Rust and composited as tiles, exactly as it is today
([ADR-0004](adr/0004-waveform-rendering-strategy.md)) — the most expensive imagery in the
application stays out of the webview, and the living layer draws over it.

### The water column — the EQ

Three strata, which is what an isolator EQ physically is:

| Band | Stratum |
|---|---|
| Low | the deep current — mass, slow, dark |
| Mid | the body of the water |
| High | the surface — light, spray, glitter |

Killing a band dries that stratum. A DJ swapping lows on a transition sees the deep current pass
from one river to the other, which is precisely what they are doing.

**Filter** narrows the channel: low-pass cuts the surface away and leaves something deep and
slow; high-pass cuts the depth away and leaves something thin and bright and fast.

### Eddies, stones and stepping stones

| Feature | In the world |
|---|---|
| Loop | **an eddy** — water circulating instead of passing |
| Loop halve / double | the eddy tightens or widens |
| Saved loop | an eddy that stays on the map when you leave it |
| Hot cue | a stone in the river, a place you can return to |
| Beat jump | stepping stones, spaced one beat apart |
| Quantize | the stones snap to the crests |

A loop is the clearest case in the whole system: an eddy is *literally* what a loop is, and
nobody needs it explained.

### The confluence — the mixer

Two rivers meet. The crossfader is **where** they meet and which one dominates downstream.

- **Sync** is two rivers running in step: crests aligning. Out of sync, the crests interfere
  visibly, and beating against each other is what being out of time actually looks like.
- **Harmonic compatibility** shows at the confluence. Adjacent keys blend into one body of water;
  clashing keys visibly refuse, with a seam down the middle. This is real information — the
  Camelot wheel is a circle and hue is a circle, so the mapping is exact, not fanciful.
- **The limiter** is the estuary's banks. Gain reduction is visible constriction: you can see the
  mix being squeezed rather than reading a number that says so.
- **Headphone cue** is a side channel you can look down without diverting the main flow.

### The highland — library and browser

Tracks are springs in the highland, grouped into **groves** (crates and playlists). A smart
folder is a grove defined by what grows there rather than by what was planted. Play history is
the delta already behind you. Duplicates are two springs feeding from one source, which is what
content-hash identity means.

The background identifier surveying the collection is exactly *surveying*: the mist retreats as
tracks are identified, which makes a long scan legible instead of a progress bar.

### Weather, light and season

| State | In the world |
|---|---|
| CPU load, xruns | weather — turbulence when the machine is struggling |
| Clock drift between two sound cards | two rivers' currents pulling against each other |
| Session phase (M9) | **the light**: warm-up is dawn, peak is high sun, close is dusk |
| Assistant proposal | a fork appearing ahead, the suggested channel faintly lit |

The assistant one matters. [ADR-0005](adr/0005-assistant-speaks-only-actions.md) says the
assistant proposes and never acts. A fork in the river is that constraint made visible: the
channel is *shown*, the water does not take it until the DJ steers.

---

## 3. Every channel answers a question

This is the discipline that separates the system from a screensaver. A visual channel earns its
place by answering a question a DJ actually asks mid-set. If it answers none, it is cut.

| The question, as a DJ would ask it | Answered by |
|---|---|
| "Are these two in time?" | crest alignment at the confluence |
| "How long have I got?" | distance to the mouth |
| "Will these two keys work together?" | whether the waters blend or seam |
| "Can I trust this grid?" | clarity of the water |
| "Am I crushing the mix?" | constriction at the estuary |
| "Which deck is louder?" | width and depth of the channel |
| "Where's the breakdown?" | the riverbed ahead |
| "Is it still analysing?" | how much mist is left |
| "Is the machine coping?" | turbulence |
| "What is it suggesting?" | which fork is lit |

**A channel with no row here does not ship.** Including one that took a week.

---

## 4. Colour

One meaning per axis. Colour becomes noise the moment two things use the same channel.

| Axis | Means | Range |
|---|---|---|
| **Hue** | musical key, on the Camelot wheel | the full circle; a circle for a circle |
| **Saturation** | certainty | pale = unsure, saturated = known |
| **Lightness** | energy and level | dark = quiet, light = loud |
| **Achromatic** | structure — trunk, chrome, anything not music | greys only |

Two consequences fall out and both are wanted:

**Uncertainty looks like one thing everywhere.** A weak beat grid, an unanalysed track and a
low-confidence key detection are all pale. A DJ learns that once.

**Colour belongs to music.** If a control is grey, it is furniture. If it has hue, it is telling
you something about the sound. That rule alone removes most of the visual noise a conventional
interface carries.

### Colour is never alone

Hue-based key coding fails for roughly one man in twelve. Every hue channel therefore carries a
redundant one:

- key is also written as text, as it is today (`8A`);
- harmonic compatibility is shown by **behaviour** — blending versus seaming — not only by hue;
- level is shown by width as well as lightness.

The test: **switch the display to greyscale and the interface must still work.** If it does not,
a channel is over-loaded.

---

## 5. Motion

### Stillness is the default

Nature is mostly still. A forest that thrashed constantly would tell you nothing about the wind.
So: **a paused deck is still water; motion means something is happening.** An idle djmanzo is
almost motionless, and that is what makes movement worth looking at.

### The clock is the music

Everything that pulses pulses on the **beat**, from the engine's own snapshot — not on wall time,
and not on `requestAnimationFrame` alone. This is the single thing this design offers that a
conventional interface structurally cannot: the room, the music and the screen in time together.

It also costs nothing extra. Beat phase is already in the snapshot at 60 Hz.

### Bounded excursion

Controls may scale, drift and breathe — within limits that keep them hittable:

| Property | Limit |
|---|---|
| **Centre of mass** | never moves. Muscle memory is aimed at the centre. |
| Excursion | at most a small fraction of the element's own radius |
| Scale | 0.9×–1.15× of its resting size |
| Rate | never faster than the beat; nothing flickers |
| Settling | motion always resolves to rest, never oscillates indefinitely |

A control that has drifted must still be hit by aiming where it was. That is the whole
constraint, and it is why the ranges are as narrow as they are.

### Reduced motion

`prefers-reduced-motion` holds the world still. Everything then communicates through form,
position, width and colour — Tier 0 in ADR-0009.

This is a hard requirement and also the best test in the system: **a still frame must tell a DJ
the state of their mix.** If it cannot, the design is leaning on animation to say something that
should have been said by shape.

---

## 6. Trunk and foliage

The rule that makes the whole thing usable, and it comes from the metaphor rather than fighting
it: *a trunk is rigid and bears weight; foliage moves and carries the light.*

| | Trunk | Foliage |
|---|---|---|
| What | anything a DJ clicks, drags or aims at | anything that reports state |
| Behaviour | rigid, stable, exactly where it was last time | grows, flows, pulses, responds |
| Rendered as | a real DOM element — focusable, keyboard-reachable, named to a screen reader | drawn into the canvas world |
| May occlude | never occluded by foliage | may be occluded by other foliage |

**Canvas paints, DOM listens.** A control's *appearance* is drawn as part of the world; its hit
target, focus ring, keyboard handling and ARIA role stay a real element positioned by the world.
Both, always, for every control.

Text is trunk too — all of it stays in the DOM, because canvas text ignores the system's font
rendering and the user's size preference, and a DJ who has set their system font to 20 px meant
it.

---

## 7. Nature carries the gestalt; digits carry the precision

The world tells you at a glance. The numbers tell you exactly. Neither replaces the other, and
removing the numbers would be the single fastest way to make this design fail in a real booth.

Always legible as text, always: **BPM · time elapsed and remaining · key · gain in dB · pitch
percent · loop length in beats.**

A DJ decides *"that one, next"* from the world and *"128.0 against 127.9"* from the digits, often
within the same second.

---

## 8. Adaptation, and its limits

The world adapts to context — but within declared bounds, because an interface that reorganises
itself is one you cannot learn.

| Adapts to | What changes | What never changes |
|---|---|---|
| The music | pulse rate, agitation, hue | where any control is |
| Session phase (M9) | the light: dawn → noon → dusk | the layout |
| Theme | palette and contrast | shape and meaning |
| Layout preset | which components exist at all ([ADR-0008](adr/0008-one-widget-vocabulary.md)) | the meaning of the ones that do |
| Frame budget | rendering tier | what is communicated — every tier says the same things |

The last row is the load-bearing one. **Tier 3 and Tier 0 must convey the same state.** Tier 3
says it more beautifully; it must never say it *more completely*, or the design has made beauty
load-bearing — which [KARAOKE.md](KARAOKE.md) already forbids for the lyrics and which
generalises to everything.

---

## 9. What this is not

Written down so it stays true.

- **Not a visualiser.** A visualiser reacts to audio. This reports state, and every channel has a
  question in §3 that it answers.
- **Not 3D.** Depth is used as a stratum of the water column, not as a camera. Perspective would
  make distant controls smaller and harder to hit, which trades usability for nothing.
- **Not skeuomorphic.** No wood-grain decks or brushed-metal faders. The metaphor is *physical
  behaviour*, not photographs of objects.
- **Not a replacement for numbers.** See §7.
- **Not mandatory.** Tier 0 is a complete, working, still interface, and a DJ who wants that can
  have it permanently.

---

## 10. Building it

Order, and why.

| Step | What | Why here |
|---|---|---|
| **V1** | The world model — entities, components, the tier selector, the DOM-listens/canvas-paints split | Nothing can be drawn before there is something to draw. Renderer-agnostic from the first line, or the abstraction will be fiction. |
| **V2** | Canvas 2D renderer, one river, one deck | One river proves the whole vocabulary: flow, pulse, riverbed, clarity, mouth. |
| **V3** | The confluence — two rivers, crossfader, sync, harmonic blending | The first thing that says something no rectangle could. |
| **V4** | Eddies, stones, strata — loops, cues, EQ, filter | The control surface, once the world it lives in is proven. |
| **V5** | WebGL renderer behind the same world | The second renderer is what proves V1 was real. |
| **V6** | Highland and weather — library, analysis mist, load, drift | The periphery, once the centre is right. |

Each step must leave the application usable, and each must pass the two tests: **greyscale** (§4)
and **still frame** (§5).
