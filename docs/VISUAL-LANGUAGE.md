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
   rapids & gorges     effects: terrain that transforms the flow
      │
   confluence          the mixer, where rivers meet
      │
   estuary             the master output
      │
   the sea             the room
```

The direction is fixed and it matters: **downstream is the future.** The stretch of river ahead
of the playhead is what is about to happen. Every DJ interface already scrolls the waveform this
way; the world simply takes it seriously everywhere else too.

**Upstream is the past.** What you already played is behind you, growing dimmer. A DJ glances
downstream to plan, upstream to remember.

---

## 2. What each thing is

Every row below is state djmanzo already publishes in its 60 Hz snapshot. Nothing here requires
new analysis — that is the main practical argument that this design is buildable rather than
aspirational.

### The river — a deck

A deck is a river. One deck, one river. The river comes into existence when a track is loaded
and ceases when it is ejected.

| What the DJ knows | What happens on screen | In DJ terms |
|---|---|---|
| Track loaded | a spring opens; the river appears | a waveform appears; the deck is armed |
| No track | the riverbed is dry and empty | an empty deck |
| Playing | the water flows; crests travel downstream | the play button is active; audio is moving |
| Paused | the water is still; the surface reflects but nothing moves | the deck is paused; the playhead is frozen |
| Tempo (BPM) | how fast the current runs — a 140 BPM track is a fast stream, a 90 BPM track is a slow, wide river | the BPM readout |
| Beat phase | **the crest** — a bright travelling wave that IS the beat; each crest is one beat passing | the beat counter / the metronome flash |
| Position in track | where you are along the river's length | the playhead position |
| Time remaining | **the distance to the mouth** — the river's end is visible, and it gets closer. What was a number that turns red is now a thing you see shrinking in your peripheral vision | the time-remaining counter |
| Volume (channel fader) | **how much water is in the channel** — full fader is a full river, fader at zero is dry | the channel fader position |
| Peak level / loudness | **the agitation of the surface** — a loud passage makes the water churn and spray; a quiet passage is glassy calm | the VU meter / peak indicator |
| Gain | the depth of the channel at rest — more gain means the riverbed is cut deeper, so the same amount of water sits higher against the banks | the gain knob |
| Pitch fader | **the gradient** — steepening the slope makes the water run faster (higher pitch), flattening it makes the water pool and slow (lower pitch) | the pitch / tempo slider |
| Pitch bend (nudge) | a gust across the water — the current briefly speeds or slows and then returns to its natural pace | the jog wheel nudge / pitch bend buttons |
| Keylock | when the gradient changes, the water speeds up but its **colour stays** — steepening changes speed without changing character. Keylock off: speed AND colour shift together | the keylock / master-tempo toggle |
| Key shift | the water's hue rotates directly — a deliberate change of character without touching the gradient | the key adjust / transpose control |
| Grid confidence | **clarity of the water** — confident grid means crystal-clear water; uncertain grid means turbid, murky water you cannot see through | the grid confidence indicator |
| Not yet analysed | **mist** over the stretch of river — you know the river is there but you cannot see its features | the "analysing…" spinner |
| Failed to decode | the spring is dry; cracked earth where water should be, and text saying why | the error message |

Three of these are worth dwelling on because they are better than what they replace.

**Time remaining as distance to the mouth.** Today this is a number that turns red when it
drops below thirty seconds. In the world it is a thing you can see coming for minutes, in
peripheral vision, without looking away from whatever you were doing. That is exactly how a DJ
actually tracks it — and the shape of the remaining river (is it dense or sparse? does it have
a breakdown coming?) carries information the number never could.

**Grid confidence as water clarity.** `dj-analysis` already refuses auto-sync below a confidence
threshold, and the reason is currently a tooltip. As clarity it needs no explanation: *you do not
navigate water you cannot see through.* The rule and the appearance are the same fact. A DJ who
has never read a manual will instinctively distrust an opaque river, which is exactly the
response the confidence score is trying to produce.

**Volume as the amount of water.** In every other DJ application, the channel fader is a slider
and the VU meter is a bar chart and the level is a number, and they are three separate widgets a
DJ must learn to correlate. Here the fader position, the level, and the peak are all visible in
the same thing: the river. Close the fader and the river runs dry. Push a loud track and the
surface churns. The three readings are three aspects of one physical object.

### The riverbed — the waveform

The waveform is not a picture beside the river; it **is** the riverbed. The terrain the water
flows over.

| What the waveform shows | In the world |
|---|---|
| Loud passage | the riverbed is deep and wide — a gorge |
| Quiet passage | shallow, almost a sandbar — the water barely covers the stones |
| A breakdown | a wide, still pool — the gorge opens out and the water calms |
| A drop / build-up | a narrows — the riverbed tightens and the water accelerates |
| Phrase boundaries | bends in the river — the direction changes |
| Spectral colouring | the colour of the rock: bass-heavy passages are dark stone, treble-heavy passages are light sand |

A DJ who has ever looked at a waveform already reads these shapes. The world names them instead
of leaving them as abstract coloured bars.

This layer is produced by `dj-render` in Rust and composited as tiles, exactly as it is today
([ADR-0004](adr/0004-waveform-rendering-strategy.md)) — the most expensive imagery in the
application stays out of the webview, and the living layer draws over it.

### The water column — the EQ

A real river has three strata, and so does an isolator EQ:

| EQ band | Stratum | What it looks and feels like |
|---|---|---|
| **Low** | the deep current — the mass at the bottom | dark, slow, heavy; the bulk of the river's weight |
| **Mid** | the body of the water | the visible middle; where most of the colour lives |
| **High** | the surface — glitter, spray, reflected light | bright, fast, thin; the sparkle on top |

**Killing a band dries that stratum.** Cut the lows and the deep current vanishes — the river
becomes thin and bright and has no weight. Cut the highs and the surface goes flat and dark — the
sparkle is gone but the mass remains. A DJ swapping lows on a transition sees the deep current
literally pass from one river to the other, which is *precisely what they are doing.*

**The EQ kill switch** is drought at one stratum: instantaneous and total. The band's layer
snaps to zero, a visible discontinuity. This is deliberate — a kill is not a gentle turn of a
knob, and it should not look like one.

**The filter** narrows the channel from one side:

| Filter position | What happens |
|---|---|
| Low-pass (fully clockwise) | the surface is sheared away from the top: what remains is deep and slow and dark |
| High-pass (fully counter-clockwise) | the depth is cut from the bottom: what remains is thin and bright and fast |
| Centre (noon) | no filtering; the full river flows |
| Resonance | the edge where the cut happens **ripples** — a standing wave at the filter frequency, visible as an intensified line between the remaining and removed strata |

A DJ sweeping the filter sees the river physically narrow, which is exactly the audible
experience of a filter sweep.

### Eddies, stones and stepping stones — loops, cues, jumps

| What the DJ does | What it is | In the world |
|---|---|---|
| Set a loop | trap the water | **an eddy** — water circulating instead of passing. The loop region becomes a visible whirl. |
| Loop halve | the eddy tightens | the whirl contracts — faster rotation, smaller circle |
| Loop double | the eddy widens | the whirl opens — slower, broader circle |
| Loop move | the eddy slides downstream or upstream | the whirl drifts along the riverbed |
| Exit the loop | release the water | the eddy opens and the water resumes flowing downstream |
| Saved loop | a permanent feature of this river's map | a carved-out eddy that stays visible even when not active — a known whirlpool, dimmed |
| Loop roll (momentary) | hold and release | an eddy that exists only while the pad is held; release and the water snaps forward to where it would have been (slip position) |
| Set a hot cue | mark a point | **a stone in the river** — a fixed, named, coloured landmark. The DJ can see it from upstream and jump to it. |
| Jump to a hot cue | return to a point | the playhead snaps to the stone; the river view centres on it |
| Beat jump forward | skip downstream | **stepping stones** — flat rocks spaced one beat apart along the river. Jump takes you to the next one. |
| Beat jump backward | skip upstream | step back to the previous stone |
| Quantize on | stones lock to crests | the stepping stones snap precisely to beat positions — you land on the beat, guaranteed |
| Quantize off | stones are free | stones at arbitrary positions; you land wherever you jump |

A loop is the clearest case in the whole system: an eddy is *literally* what a loop is — water
going around instead of going forward — and nobody needs it explained. A DJ who has never seen
this interface will recognise a loop region as a whirl in the water without reading a word.

### The rapids and gorges — effects

Effects are **terrain features** that the river flows through. Different terrain transforms the
character of the water in different ways. This is physically accurate — a real river's sound and
behaviour are shaped by the geology it crosses — and it is immediately recognisable: each effect
is a named kind of terrain, and its visual signature tells the DJ both what it is and how much
of it there is.

| Effect | Terrain | What the DJ sees |
|---|---|---|
| **Echo / delay** | a **canyon** with walls that reflect — the water strikes the walls and bounces back at timed intervals | visible reflections downstream of the playhead, repeating and fading; the number of walls = the number of repeats |
| **Reverb** | a **cavern** — the river enters a wide enclosed space and the reflections multiply | the river temporarily widens into a diffuse, shimmering pool; long reverb = deeper cavern |
| **Flanger / phaser** | **interference ripples** — two copies of the surface wave, slightly offset, creating visible striping | a moiré pattern on the water surface; the stripe spacing shifts with the rate |
| **Filter** (as FX) | a **narrows** — the same as the channel filter, but applied as a terrain feature in the FX chain | the river physically constricts |
| **Gate** | a **sluice** — a gate that opens and closes at timed intervals, chopping the flow | the river is interrupted by periodic bars of dry riverbed |
| **Bitcrush** | a **stepped cascade** — the smooth flow breaks into discrete ledges | the water's surface becomes jagged and stepped, like a staircase rapid |
| **Roll** | a **series of small eddies** — the water is caught and released in rapid succession | visible rapid-fire whirls, spaced at the beat division |
| **Brake** | a **shallows** — the river hits flat ground and slows dramatically to a stop | the gradient visibly flattens; the water pools and stalls |
| **Backspin** | the current **reverses** — water flowing briefly upstream | the flow direction visibly inverts; crests travel upstream |

**FX depth (wet/dry mix)** is the length of the terrain feature: a short canyon produces a light
echo; a long canyon produces a deep one. The dry/wet knob stretches or compresses the terrain.

**Beat-synced FX timing** is the spacing of the terrain's features — canyon walls spaced at 1/4
beat versus 4 beats. The DJ can see the timing in the spacing of the reflections.

**FX routing** determines which river passes through the terrain: per-deck FX means that river
alone hits the rapids; master FX means the combined flow after the confluence passes through.

### The four currents — stems

A river is not one uniform body of water. Stand at a riverbank and you can see currents within
the flow — different temperatures carry different colours, sediment layers are visible, the
surface shimmer is distinct from the deep pull.

djmanzo's stem engine separates a track into four currents:

| Stem | Current | Visual character |
|---|---|---|
| **Vocals** | the surface shimmer — bright, reflective, the part you notice first | the highest visible layer; it catches the light |
| **Drums** | the rhythmic pulse — visible as the beat crests themselves, the percussive heartbeat of the flow | the crests are *in* this layer; muting drums smooths the surface flat |
| **Bass** | the deep undertow — dark, slow, heavy, the weight that gives the river its momentum | the lowest visible layer; the mass at the bottom |
| **Other** (instruments) | the body — everything between the shimmer and the depth | the middle layer; the harmonic colour of the water |

**Muting a stem** is drying that current: the layer goes transparent, the water that remains is
thinner, and the character changes the way it does when you hear it — remove the vocal and the
surface goes calm; remove the bass and the river loses its weight.

**Soloing a stem** is the inverse: the other three layers go transparent and only the selected
current remains visible. The river narrows to just that stratum.

**Per-stem volume** is the relative thickness of that current's layer within the river cross-
section. The four always sum to the full river width.

**Stem-aware transitions** are visible as one current passing from one river to the other at the
confluence — the incoming vocal shimmer blends into the outgoing river while the outgoing
instrumental body continues. This is exactly what a stem-aware transition is, and the DJ can
*see the handoff happen.*

### The confluence — the mixer

Two rivers meet. The confluence is the mixer — the place where separate flows become one. Every
control on the mixer has a physical meaning at the meeting point.

| Mixer control | In the world |
|---|---|
| **Crossfader** | **where the rivers meet and which one dominates.** Hard left: only river A flows through. Hard right: only river B. Centre: both merge equally. The crossfader curve shapes how the merge happens — a sharp curve is a sudden diversion; a smooth curve is a gradual blending. |
| **Channel fader** | the **sluice gate** on each river — how much water each one contributes to the confluence. Fader down = the sluice is closed, no water passes. Fader up = full flow. |
| **Crossfader assign (A / thru / B)** | which bank the river enters on. A-assigned rivers approach from the left; B from the right; *thru* rivers bypass the confluence entirely and flow straight to the estuary at full volume. |
| **Sync** | two rivers running in step: **crests align.** Beat sync means the travelling waves arrive at the confluence together. Out of sync, the crests interfere visibly — they beat against each other, creating a visible interference pattern that IS what being out of time sounds like. |
| **Phase alignment** | the crests from both rivers arriving at exactly the same position at exactly the same moment. Drifting phase shows as one crest sliding ahead of the other. |
| **Harmonic compatibility** | Adjacent keys (within 1–2 steps on the Camelot wheel) **blend** — the two hues merge into one body of water at the confluence. Clashing keys (3+ steps) **refuse** — a visible seam runs down the middle where the two colours will not mix. |
| **VU meter** | the **water level against the banks** — the meter IS how high the water stands. Per-channel VU is the level in each tributary; master VU is the level downstream of the confluence. |
| **Gain** | depth of the channel — more gain cuts the riverbed deeper, so the water stands higher for the same flow. The banks (the maximum before clipping) stay fixed. |
| **The limiter** | the **estuary's banks** — fixed, rigid, the absolute boundary. When the combined flow hits the banks, it is visibly constricted. Gain reduction is the water being squeezed through a narrower opening. More limiting = more constriction. The DJ can *see the mix being crushed* instead of reading a GR number. |
| **Headphone cue (PFL)** | a **side channel** — a smaller stream branching off before the confluence, carrying one river's water to the DJ's ear without affecting the main flow. The cue/master blend knob is how much main-flow water mixes into the side channel. |
| **Split cue** | the side channel splits in two — one ear gets the cued river, the other gets the master flow |
| **Booth output** | a **second estuary** — an independent channel branching off downstream of the confluence, with its own sluice gate (booth level) |
| **Microphone** | a spring that opens directly into the confluence — not from the highland, not from a track; a new water source right at the mixer |
| **Ducking** | the main rivers recede (their level drops) when the mic spring opens, making room; they return to full when it closes |

### The hand in the water — the jog wheel and platter

The jog wheel is **the DJ's hand in the river.**

| Jog action | In the world |
|---|---|
| **Vinyl mode touch** | a hand placed in the water — the flow stops, held | the platter touch stops playback |
| **Vinyl mode drag** | the hand moves through the water, pushing it forward or pulling it back | scratch / scrub |
| **CDJ mode nudge** | a finger trailing in the current — it speeds or slows the flow slightly but does not stop it | pitch bend / nudge |
| **Search** | skimming across the surface at speed — the DJ sweeps through the river without the water flowing normally | fast-forward / rewind scrub |
| **Release (vinyl mode)** | the hand lifts — the water resumes its natural flow | release-to-play |

**Motorized platters** are the inverse: the river drives the hand. The physical platter spins
under the DJ's fingers at the rate of the current, and resisting it slows the water.

### Slip, reverse and censor

| Feature | In the world |
|---|---|
| **Slip mode** | the visible river continues flowing underneath, transparent and ghostly, while the DJ's action (a scratch, a loop roll, a backspin) happens on the surface. When the action ends, the surface snaps to where the underlying current has reached. The DJ sees both: what they are doing, and where they will land. |
| **Reverse** | the current reverses. The water flows upstream. The crests travel backward. Simple and total. |
| **Censor** | identical to reverse, but the DJ holds a button — release and the river resumes forward. It is reverse as a momentary gesture. |

### Transport — play, pause and cue

These are the most fundamental gestures and they must be the most immediately recognisable.

| Control | In the world |
|---|---|
| **Play** | the water begins to flow. Springs open, the current starts, crests begin travelling downstream. |
| **Pause** | the water stops. The surface goes still. Everything freezes in place — the crest where it was, the position where it was. |
| **CDJ-style cue** | a **dam** at the cue point. Press cue: the water snaps back to the dam, and flows only while the button is held. Release: the water stops at the dam. Press play from the cue: the dam opens and the river flows freely. |
| **Cue point** | the dam's position — settable, visible as a strong vertical marker across the river |

CDJ-style cue is worth getting right because it is the gesture DJs use most often without
thinking about it: *preview from the cue, release, listen, adjust, preview again.* The dam
metaphor matches exactly: the water always returns to this point when you are not holding it
forward.

### Pools and vessels — sampler

Samples are **pools** — captured water stored in vessels beside the river, ready to be poured
back in.

| Sampler feature | In the world |
|---|---|
| Sample bank | a row of vessels — small pools above the confluence, each holding captured water |
| Trigger (one-shot) | a vessel tips and pours its water into the flow in one motion |
| Trigger (loop) | a self-filling vessel — the water circulates like a small eddy |
| Trigger (hold) | the vessel pours only while the DJ's hand tilts it |
| Trigger (stutter) | the vessel tips and resets rapidly, pouring in bursts |
| Record to sample | a vessel dips into the river or the estuary and fills itself |
| Sample volume | how much water the vessel releases |
| Sample sync | the vessel empties at the tempo of the main current |

### The highland — library and browser

The highland is the high country where the springs originate. It is above the rivers — upstream
of everything — and it is where the DJ goes to choose what will flow next.

| Library feature | In the world |
|---|---|
| The collection | the highland — all the springs in the landscape |
| A track | a **spring** — a source of water, waiting to be opened |
| A crate or playlist | a **basin** — a natural collection of springs sharing a catchment. A DJ-made grouping. |
| A smart folder | a basin defined by what flows into it rather than by what was placed there — it fills itself based on the terrain |
| A folder | a **ridge** — a geographic feature that groups basins. Folders contain playlists the way ridges contain basins. |
| The browser tree | the highland's geography — ridges, basins, springs; the map of the collection |
| Search | surveying the highland — searching narrows the visible landscape to matching springs |
| Play history | the **delta** — the alluvial plain downstream, where you can see which rivers have already run. Behind you, fading. |
| Duplicate detection | two springs feeding from the same underground source — which is literally what content-hash identity means |
| Background scan/analysis | **surveying** — the mist retreats as springs are identified, which makes a long scan legible instead of a progress bar |
| Drag to deck | opening a spring — choosing it and letting it become a river |
| Track info (tags, artwork) | the geology of the spring — what kind of water it carries, where it comes from, its character |
| Star ratings | the spring's reputation — how valued this source is |
| Colour coding | a flag planted beside the spring — visible from a distance |
| BPM / key / energy columns | the spring's measured properties — flow rate, mineral content, temperature — shown as text, always (§7) |

### Recording — the dam at the estuary

| Feature | In the world |
|---|---|
| Recording to disk | a **dam** closes at the estuary, capturing the outflow. The water accumulates — the recording grows. |
| Recording active | the dam is visible; a pool forms behind it |
| Recording stopped | the dam opens; the pool is sealed — the recording is a file |
| Broadcast | the estuary feeds a **canal** that carries the water beyond the sea — to a remote audience |

### Automix — rivers that sequence themselves

When automix is active, the next spring opens automatically as the current river approaches its
mouth. The confluence times itself — the transition happens without the DJ's hand. The rivers
still flow through the same world; they are just scheduled rather than chosen live.

### Weather, light and season

| State | In the world |
|---|---|
| CPU load, xruns | **weather** — turbulence when the machine is struggling. Light load is clear air. Heavy load is gusting wind that roughens all the water surfaces. An xrun is a sharp gust. |
| Clock drift between two sound cards | two rivers' currents pulling against each other — visibly fighting |
| Session phase (M9) | **the light**: warm-up is dawn (amber, long shadows), peak is high sun (bright, saturated), cool-down is dusk (blue, dimming) |
| Assistant proposal | a **fork appearing ahead** — the river branches, and the suggested channel is faintly lit. The water does not take it until the DJ steers. |

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
| "Will these two keys work together?" | whether the waters blend or seam at the confluence |
| "Can I trust this grid?" | clarity of the water |
| "Am I crushing the mix?" | constriction at the estuary |
| "Which deck is louder?" | width and depth of each river |
| "Where's the breakdown?" | the riverbed ahead — the pool is visible |
| "Is it still analysing?" | how much mist is left |
| "Is the machine coping?" | turbulence |
| "What is it suggesting?" | which fork is lit |
| "Where am I in the loop?" | the eddy's rotation — where in the circle the water is |
| "Is that cue coming up?" | the stone is visible downstream, approaching |
| "What does this effect sound like right now?" | the kind of terrain the river is passing through |
| "How much effect am I applying?" | how long the terrain feature stretches |
| "Where are the vocals in this mix?" | the surface shimmer — present or absent |
| "Is the incoming track's bass going to collide?" | whether the deep currents merge cleanly at the confluence |
| "Am I recording?" | whether the dam at the estuary is closed |
| "What time of night is it — musically?" | the light (dawn / noon / dusk) |
| "Is slip mode on?" | whether a ghostly river is flowing underneath the surface |
| "Is the filter engaged?" | whether the river is narrowed from one side |

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
- level is shown by width as well as lightness;
- each EQ band has a position (top/middle/bottom), not only a shade;
- each stem current has a position in the water column, not only a colour.

The test: **switch the display to greyscale and the interface must still work.** If it does not,
a channel is over-loaded.

---

## 5. Motion

### Where the eye reads it

The design's hardest requirement is not beauty, it is this: **a DJ must be able to take the state
of their mix without moving their primary focus.** For most of a set that focus is the crowd, the
controller, or the other deck — not the screen. So the question for every channel is not only
*what does it mean* but **which vision reads it**, and the two answers are very different.

Foveal vision — the sharp part — covers about two degrees, which is a thumbnail at arm's length.
Everything else is periphery, and periphery is a different instrument:

| Channel | Read by | Fit for |
|---|---|---|
| **Motion onset** — something starts moving | periphery, involuntarily | the one thing that must reach you now |
| **Luminance change** — something brightens | periphery, reliably | magnitude that must arrive unlooked-at |
| **Large-scale shape and area** | periphery, coarsely | gross state: is the river wide, is the end near |
| **Hue** | fovea | things you *look at* in order to decide — key, compatibility |
| **Saturation** | fovea, mostly | certainty |
| **Fine detail and text** | fovea only | precision, when you have chosen to check |

From which one rule follows, and it is worth stating as flatly as possible:

> **If a fact must reach a DJ who is not looking at the screen, it may not be carried by hue, by
> saturation, or by text.**

That rule reorganises §2's channels by *where they are read*, and the assignment is not the one a
purely aesthetic design would reach:

| Fact | Must reach an unlooking DJ? | Therefore carried by |
|---|---|---|
| Audio is dropping out | yes — the room can hear it | motion onset: the whole surface breaks up |
| A playing deck is about to run out | yes | the mouth: growing area and rising luminance |
| The limiter is squashing the mix | yes | visible constriction at the estuary — shape, not colour |
| A deck stopped unexpectedly | yes | motion *ceasing*, which the periphery catches as readily as motion starting |
| These two keys will clash | no — you look when choosing | hue, plus the seam at the confluence |
| This grid is not trustworthy | only when you reach for sync | clarity, plus the sync control refusing |
| The BPM is 128.4 | no — you look to check | text |

The last two rows are the useful ones. Key compatibility genuinely *is* a foveal question: it is
asked while choosing the next track, with your eyes on the browser. Spending a peripheral channel
on it would waste the scarcest resource the interface has on the one question the DJ is already
looking at.

### Onset, not state

Peripheral attention is captured by **change**, not by condition. A thing that has been red for
five minutes is not a warning any more, it is wallpaper — and worse, it is wallpaper that hides
the next warning behind it.

So the interface signals **transitions with motion and states with form**:

- when something crosses a threshold, it *moves* — briefly, and then stops;
- once it has been seen, it settles into a static shape that still says the same thing;
- if the condition worsens, that is a new transition and earns a new onset.

A warning that never stops moving is a warning nobody can act on, because it has taken the
channel a genuinely new event needed.

### One alarm at a time

Peripheral attention is close to a single channel. Three things demanding it at once means none of
them arrive. So the world ranks what may take it, and **only the highest active claim gets
motion** — everything below it degrades to static form, which is still legible when looked at:

| Priority | Claim | Why it outranks the rest |
|---|---|---|
| 1 | Audio dropouts | the audience is hearing it right now |
| 2 | A playing deck about to end with nothing cued | the only unrecoverable one |
| 3 | The limiter working hard | the mix is being damaged while it plays |
| 4 | End of track approaching, next track ready | expected, and handled |
| 5 | Analysis finished, a track arrived, an assistant proposal | information, not urgency |

Ranking is the point. Without it, an interface this alive becomes an interface a DJ learns to
ignore, which is worse than a still one.

### The screen must agree with the room

The pulse has to coincide with **what the room hears**, not with what the engine computed. Those
are not the same instant: the output chain has latency, and the interface draws from a snapshot
that is already at least a frame old.

djmanzo knows the number — `output_latency_ms` is published in the master snapshot precisely so
it is stated rather than discovered — so the visual pulse is **delayed by it**. At 128 BPM one
beat is 469 ms; a twenty-millisecond error is four percent of a beat, which is small but visible
as a crest sitting slightly ahead of the kick. It costs one subtraction to be right, and an
interface whose pulse is visibly early is one a DJ stops trusting for phase.

The same rule at the next level up: **two decks that sound in sync must draw one crest.** If the
screen shows them apart while the room hears them together, the DJ will believe the room and stop
reading the screen — and every other channel loses its credibility with it.

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
| Biome (theme) | palette, texture, edges, ambient motion | what any channel *means* — see below |
| Layout preset | which components exist at all ([ADR-0008](adr/0008-one-widget-vocabulary.md)) | the meaning of the ones that do |
| Frame budget | rendering tier | what is communicated — every tier says the same things |

The last row is the load-bearing one. **Tier 3 and Tier 0 must convey the same state.** Tier 3
says it more beautifully; it must never say it *more completely*, or the design has made beauty
load-bearing — which [KARAOKE.md](KARAOKE.md) already forbids for the lyrics and which
generalises to everything.

### Biomes

A watershed exists somewhere. The same river in different country looks entirely different and
behaves by the same physics, which is exactly what a theme should be — **a change of terrain, not
a change of language.**

| Biome | The country | What it changes |
|---|---|---|
| **Mountain stream** | cold, high, narrow | tight channels, high contrast, quick settling, pale rock |
| **Lowland delta** | warm, wide, slow | broad braided channels, soft edges, long settling, silt tones |
| **Rainforest** | dense, humid, saturated | deep greens, heavy ambient texture, water everywhere |
| **High desert wadi** | sparse, sharp, dry | hard shadows, bare rock, water that matters because it is rare |
| **Winter** | monochrome, iced | very low saturation, slow motion, still edges |

The hard rule, and the reason a biome is safe:

> **A biome may change palette, texture, edge treatment and ambient motion. It may never change
> what a channel means.**

Hue still means key. Clarity still means grid confidence. Width still means level. A theme that
reassigned those would not be a skin, it would be a *dialect* — and a DJ who changed themes would
have to relearn the instrument, which is the one thing the whole design exists to avoid.

Three axes, deliberately orthogonal, and none of them can reach into another:

| Axis | Decides | Defined by |
|---|---|---|
| **Layout preset** | which components exist at all | [ADR-0008](adr/0008-one-widget-vocabulary.md) |
| **Biome** | what the country looks like | a token set — the same bounded vocabulary a skin restyles through |
| **Session phase** (M9) | the light in it: dawn → noon → dusk | the hour of the night |

A biome and a session phase compose without conflicting because they touch different things: the
biome says *what the rock and water are*, the phase says *what the light is doing to them*. A
winter dawn and a rainforest noon are both coherent, and neither needed a special case.

Like a layout, a biome ships **as data** and cannot execute anything — so one somebody sent you is
safe to open, and a DJ can make their own.

**Every biome must pass the same two tests**: greyscale (§4) and still frame (§5). That is what
stops a beautiful theme from quietly breaking the language — a biome that only reads because of
its colours has moved information into a channel one man in twelve cannot see, and a biome that
only reads while it is moving has made motion load-bearing. Neither ships, however good it
looks.

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
- **Not a game.** There is no scoring, no reward, no progression mechanic. The world carries
  information; it does not gamify it.

---

## 10. The rosetta stone

A DJ coming from VirtualDJ, rekordbox, Serato or Traktor already knows what every control does.
This table translates every familiar thing into its place in the world. If a control is not here,
it does not have a visual representation in the world (and §3 says it should not).

### Deck and transport

| You know it as | In the world it is |
|---|---|
| Deck | a river |
| Waveform | the riverbed — the terrain the water flows over |
| Scrolling waveform | the river flowing past the playhead |
| Overview waveform | a map of the whole river from spring to mouth |
| Play button | the water begins to flow |
| Pause button | the water stops |
| Cue button (CDJ-style) | a dam — the water returns to it on press, flows while held |
| Cue point marker | the dam's position on the river |
| Pitch fader / tempo slider | the gradient — steeper = faster |
| Pitch bend buttons | a gust — temporary push on the current |
| Jog wheel (vinyl touch) | a hand in the water |
| Jog wheel (CDJ nudge) | a finger trailing in the current |
| Keylock / master tempo toggle | gradient changes speed but not the water's colour |
| Key shift | the water's hue changes directly |
| Sync button | crests align between two rivers |
| Beat counter / phase meter | the travelling crest — the bright wave IS the beat |
| BPM display | the current speed (always shown as a number too — §7) |
| Time remaining | the distance to the mouth |
| End-of-track warning | the mouth is close — visible in peripheral vision |

### Mixer

| You know it as | In the world it is |
|---|---|
| Channel fader | the sluice gate — controls how much water passes |
| Crossfader | where the rivers meet and which one dominates |
| Crossfader curve | the shape of the merge — sharp diversion or gradual blend |
| EQ low knob | the deep current's volume — more = heavier, darker |
| EQ mid knob | the body of the water — more = fuller |
| EQ high knob | the surface shimmer — more = brighter, sparklier |
| EQ kill switch | drought at one stratum — instant, total |
| Filter knob | the channel narrows from one side |
| Gain knob | the depth of the riverbed — deeper channel, higher water |
| VU meter | the water level against the banks |
| Peak indicator | the surface agitation — churning, splashing |
| Master level meter | the water level at the estuary |
| Limiter GR meter | constriction at the estuary — the banks squeezing |
| Headphone cue / PFL button | a side channel branches off for the DJ |
| Cue/master blend | how much main flow mixes into the side channel |
| Booth level | the sluice on the second estuary |
| Mic input | a spring that opens directly at the confluence |

### Loops, cues and pads

| You know it as | In the world it is |
|---|---|
| Hot cue pad | a stone in the river |
| Auto-loop button (e.g. "4 beats") | an eddy of that size forms |
| Loop in / loop out | the eddy's boundaries — where the whirl starts and ends |
| Loop active indicator | the eddy is spinning |
| Loop halve | the eddy tightens |
| Loop double | the eddy widens |
| Loop roll pad | a momentary eddy — exists while held, snaps to slip position on release |
| Saved loop | a permanent eddy on the map — visible even when not active |
| Beat jump buttons | stepping stones along the river |
| Slicer pads | a section of river divided into stepping-stone slices |

### Effects

| You know it as | In the world it is |
|---|---|
| FX slot / rack | a stretch of terrain the river passes through |
| Echo / delay effect | a canyon — walls that reflect the flow |
| Reverb effect | a cavern — an enclosed space that multiplies reflections |
| Flanger / phaser | interference ripples on the surface |
| Filter (as FX) | a narrows — the channel constricts |
| Gate effect | a sluice — periodic bars chopping the flow |
| Bitcrush | a stepped cascade — the smooth flow breaks into ledges |
| Roll effect | rapid small eddies |
| Brake effect | shallows — the gradient flattens, water pools |
| Backspin | the current reverses |
| FX wet/dry knob | the length of the terrain feature |
| FX beat-sync timing | the spacing of the terrain's repeating features |

### Stems

| You know it as | In the world it is |
|---|---|
| Vocal stem | the surface shimmer |
| Drum stem | the rhythmic crests — the beat pulse |
| Bass stem | the deep undertow |
| Other / instrumental stem | the body of the water |
| Stem mute | that current dries up |
| Stem solo | only that current remains; the others go transparent |
| Per-stem volume | the thickness of that current's layer |

### Library and browser

| You know it as | In the world it is |
|---|---|
| Your music collection | the highland |
| A track (in the browser) | a spring |
| A crate / playlist | a basin |
| A smart folder | a basin that fills itself |
| A folder | a ridge |
| The sidebar tree | the highland's geography |
| Search box | surveying — narrowing the visible landscape |
| "Analysing…" | the mist retreating |
| Play history | the delta downstream — what already flowed |
| Drag to deck / load | opening a spring |
| Duplicate tracks | two springs from one underground source |

### System and session

| You know it as | In the world it is |
|---|---|
| CPU load indicator | the weather — turbulence |
| Audio dropout / xrun | a sharp gust |
| Recording active | the dam at the estuary is closed |
| Automix active | rivers sequencing themselves |
| Slip mode | a ghostly river flowing underneath |
| Session phase | the light — dawn, noon, dusk |
| Assistant suggestion | a fork in the river, faintly lit |

---

## 11. Building it

Order, and why.

| Step | What | Why here |
|---|---|---|
| **V1** | The world model — entities, components, the tier selector, the DOM-listens/canvas-paints split | Nothing can be drawn before there is something to draw. Renderer-agnostic from the first line, or the abstraction will be fiction. |
| **V2** | Canvas 2D renderer, one river, one deck — flow, pulse, riverbed, clarity, mouth, stones, eddies | One river proves the whole vocabulary. |
| **V3** | The confluence — two rivers, crossfader, sync, harmonic blending, the limiter as estuary | The first thing that says something no rectangle could. |
| **V4** | Strata and terrain — EQ, filter, effects as terrain features, stems as currents | The full control surface, once the world it lives in is proven. |
| **V5** | WebGL renderer behind the same world | The second renderer is what proves V1 was real. |
| **V6** | Highland and weather — library, analysis mist, sampler pools, recording dam, session light | The periphery, once the centre is right. |

Each step must leave the application usable, and each must pass the two tests: **greyscale** (§4)
and **still frame** (§5).
