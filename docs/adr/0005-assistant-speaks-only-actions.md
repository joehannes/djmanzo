# ADR-0005 — The assistant speaks only actions

- **Status**: accepted
- **Date**: 2026-08-15

## Context

djmanzo is growing an AI layer: voice control, session planning, live steering
of energy and genre, and generated music. The obvious way to build this is to
give the assistant an API into the application — a handle on the deck objects, a
planner that mutates the library, a privileged channel into the engine.

That would be a mistake, and the reasons are worth writing down before anyone
reaches for the convenient version.

## Decision

**The assistant is a client, not a component. It can only emit action text onto
the existing bus, and it can only read the existing parameter snapshot.**

Concretely:

- The assistant's entire write surface is `dispatch("<action text>")` — the same
  call the UI makes and the same strings a controller mapping produces.
- Its entire read surface is the parameter snapshot and the library query API.
- It gets no reference to `Engine`, no reference to a `Deck`, no channel of its
  own into the realtime path.
- Model output is parsed by `Action::parse` at the edge. Anything that does not
  parse is rejected there and never travels further.

Tool-calling schemas exposed to the model are generated *from* the action
vocabulary, so a model can only be told about actions that actually exist.

## Alternatives considered

**A dedicated assistant API into the engine.** Faster to write, and it would
allow richer operations than the action vocabulary currently expresses.
Rejected: it creates a second control path with its own bugs, its own
concurrency story, and its own security surface, and it puts model output closer
to the audio thread than any model output should ever be.

**Let the model emit structured JSON commands instead of action text.** Slightly
more robust parsing. Rejected because it forks the vocabulary: we would maintain
a JSON schema *and* a text grammar that must stay in step, and the text grammar
has to exist anyway for scripting and the network API. One vocabulary, exercised
by everything, stays correct. (Tool-calling still uses JSON at the transport
level — but the *arguments* are action strings, so there is one grammar.)

**Let the assistant plan directly against library internals.** Rejected for the
same reason: the planner should compose actions a human could have taken.

## Consequences

**Good**

- **A hallucination is a parse error.** The worst a confused model can do is
  emit a string that fails validation. It cannot invent a deck, exceed a range
  (values clamp), or reach past the queue.
- **Everything is auditable.** AI-issued actions land in the session log
  alongside hand-played ones, so an assisted set replays and re-renders exactly
  like any other. You can read back precisely what the assistant did.
- **Everything is reversible by hand.** Anything it did, you can undo with the
  same controls.
- **No feature drift.** A new deck capability becomes assistant-controllable the
  moment it has an action, with no assistant-side work.
- **The realtime guarantee is untouched.** `dj-engine` still only ever sees
  typed enums arriving through `rtrb`; the assistant is several layers away and
  cannot allocate on the audio thread because it cannot reach it.
- **Local and cloud models are interchangeable**, since the contract is text in,
  text out.

**Costs**

- Some operations need an action to exist before the assistant can perform them.
  That is friction, and it is the point: it forces the capability to be a
  first-class, hand-usable feature rather than an AI-only backdoor.
- Round-tripping through text is marginally slower than a direct call. At the
  rate a human speaks, this does not register.
- Complex plans become *sequences* of actions, which the planner must schedule.
  That scheduling is ordinary application logic and belongs outside the engine
  anyway.

## Related

- [ADR-0003](0003-action-bus-and-parameter-registry.md) established the bus this
  relies on.
- [ADR-0006](0006-music-sources-and-licensing.md) covers where tracks may come
  from.
- [ASSISTANT.md](../ASSISTANT.md) is the feature design.
