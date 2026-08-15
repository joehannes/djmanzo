# ADR-0003 — One action bus and one parameter registry

- **Status**: accepted
- **Date**: 2026-08-14

## Context

A DJ application takes input from many sources — the UI, MIDI controllers, HID controllers,
the keyboard, user scripts, network peers, automation — and they all want to do the same
things. It also has to expose a large amount of continuously-changing state to the interface at
60 Hz, while an audio callback runs every few milliseconds and must never block.

The naive approach wires each input source to the engine separately. That produces N different
paths to "play deck 2", each with its own bugs, and makes scripting and remote control into
retrofits that need privileged access to internals.

Two established designs solve half of this each:

- **VirtualDJ** exposes a script language where every control is a verb-and-parameter string
  (`deck 1 play`, `loop 4`). Uniform and scriptable, but stringly-typed.
- **Mixxx** has `ControlObject`: a global registry of named parameters that any thread can
  reach through a `ControlProxy`. Excellent for state, but name lookups and a general-purpose
  registry are not what you want on a realtime hot path.

## Decision

**Combine them, typed.**

### The Action bus

Every user intent becomes a typed `Action` value and goes onto one ordered, timestamped bus.
No input source ever touches the engine directly.

```
MIDI / HID ─┐
keyboard   ─┤
UI         ─┼─→ mapping engine ─→ Action bus ─→ ┬─→ engine (via rtrb SPSC)
script     ─┤                                   ├─→ library
network    ─┘                                   ├─→ UI state
                                                └─→ session log
```

Actions are an enum — `Deck(2).Play`, `Deck(1).SetLoop(Beats(4))`, `Mixer.Crossfader(0.35)`,
`Pad(3, HotCue(2)).Press`. Text forms exist for scripting and the network API, in the spirit of
VirtualDJ script, but they **parse to the same enum**. There is exactly one execution path and
no stringly-typed logic at runtime.

### The Parameter registry

Every observable value lives in a fixed table of atomics, established at startup. No dynamic
registration, no string lookup on the hot path, no allocation. Each entry declares its type,
range and default. The audio thread reads and writes atomics directly; the snapshot pump reads
the whole table at 60 Hz and ships a diff to the UI.

### Crossing into the audio thread

Only two mechanisms, both pre-allocated: `rtrb` SPSC ring buffers for Actions in and telemetry
out, and atomics in the registry for continuous values that tolerate being one frame stale.

## Alternatives considered

**Direct calls from each input source into the engine.** Simplest to start, and it is how
prototypes usually begin. Rejected: it multiplies code paths, makes locking discipline
everyone's problem, and leaves scripting and remote control with no clean surface.

**A message bus with dynamic string keys (closer to `ControlObject`).** Flexible and easy to
extend at runtime. Rejected for the hot path — hashing strings and chasing pointers is exactly
the work the audio callback cannot afford. The fixed table gives the same reach with none of
the cost; extensibility is served by the text form of Actions, which is parsed once at the
edge, not per frame.

**An ECS or full actor system.** Rejected as far more machinery than the problem requires; the
realtime constraint is better served by a small, auditable amount of code.

## Consequences

**Good**

- Controller mapping, keyboard mapping, macros and scripting are all just Action producers —
  one implementation serves all of them.
- The network control API is not a bolt-on: it is the same door the UI uses.
- The audio thread stays allocation- and lock-free by construction, not by vigilance.
- Every feature is automatically remotely controllable and automatically scriptable.

**The big one: the bus is a log**

Because Actions are ordered and timestamped, a performance is completely described by data. A
recorded set is not just audio — it is the exact sequence that produced it. That yields, with
no additional architecture:

- **replay** of a set exactly as played;
- **offline re-render** of the master at studio quality, free of realtime deadlines, so the
  recording can be *better* than the live output;
- **practice loops** — replay to a given state and rehearse a transition repeatedly;
- **diffing two takes** of the same mix.

No competing product ships this. It is the clearest single argument for the architecture.

**Costs**

- Adding a feature means adding an Action variant and a Parameter entry — slightly more
  ceremony than calling a method.
- The Parameter table being fixed at startup means plugins cannot register arbitrary new
  parameters; they get a defined slice of namespace instead.
- Determinism for replay must be actively maintained: anything that reaches the engine
  *without* going through the bus breaks it. That is a standing review rule, and it is also
  exactly the rule that keeps the realtime path clean, so the two constraints reinforce each
  other.
