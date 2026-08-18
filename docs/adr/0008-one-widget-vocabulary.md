# ADR-0008 — One widget vocabulary

- **Status**: accepted — not yet implemented
- **Date**: 2026-08-18

## Context

[ADR-0003](0003-action-bus-and-parameter-registry.md) established that every *behaviour* in
djmanzo goes through one named vocabulary, so the UI, controllers, scripts, the network API and
the assistant all speak the same language and there is exactly one execution path. It worked:
by M3 the assistant could drive the whole application without a single privileged call, because
there was nothing to privilege — it emits the same action text a MIDI pad does.

**Nothing equivalent exists for what is on screen.** The interface is roughly 7,300 lines of
Svelte behind two paragraphs of architecture, and the one mechanism it does have — the layout
system shipped in M3 — is a flat struct of hardcoded feature flags:

```rust
pub struct Layout {
    decks: u8, waveform_height: u16, overview: bool,
    pads: bool, loops: bool, beat_jump: bool,
    eq: bool, filter: bool, keylock: bool,
    browser: bool, density: f32, …
}
```

Each field is matched by a `$derived` in `Deck.svelte` and an `{#if}` around the markup. That
is fine for four presets and it is already the wrong shape:

- **A skin can only hide, resize and scale a fixed set.** It cannot move a component, reorder
  one, put two of something on screen, or restyle anything. `FEATURES.md` says so out loud.
- **A DJ's layout file can only name widgets the shipped struct already knows.** The format is
  closed: extending it means shipping a new binary, which defeats the point of a file format.
- **Multi-monitor and detachable panels (M5) have no mechanism waiting for them.** There is no
  way to say "this component, on that screen", because components have no names.
- **The assistant can drive every action in the application and cannot touch the interface**,
  which is a strange asymmetry once you notice it: the layer users look at all night is the one
  layer nothing else can address.
- **Every new widget costs a field, a derived value and a conditional**, in a struct that
  already has thirteen fields and is the wrong place for the fourteenth.

The underlying mistake is that the layout describes *the interface djmanzo happens to have*
rather than *a vocabulary of things an interface can be made of*. That is the same mistake
ADR-0003 rejected for behaviour, and it has the same fix.

## Decision

**A widget registry, mirroring the action bus. A layout is a tree of addressed widget instances
placed into named slots, not a struct of booleans.**

### The vocabulary

Every component that can appear on screen has a stable dotted name, in the same spirit as
action names:

```
deck.waveform      deck.overview     deck.transport    deck.pads
deck.loops         deck.beat_jump    deck.eq           deck.filter
deck.pitch         deck.keylock      deck.grid         deck.meter
mixer.crossfader   mixer.master      mixer.cue         mixer.limiter
browser.crates     browser.tracks    browser.sideview  browser.search
shell.topbar       shell.status      shell.log
panel.assistant    panel.presets     panel.settings    panel.sources
```

The registry is the single source of truth for that list. For each widget it declares:

- the name;
- which **slots** it may be placed in, and which slots it offers to its own children;
- its **props**, each with a type, a range and a default — the same discipline the
  `ParameterRegistry` applies to observable values;
- what it needs from the snapshot, so a widget that is not on screen is not paid for.

### The layout

```json
{
  "name": "Booth",
  "tokens": { "accent": "#22d3aa", "radius": "4px" },
  "slots": {
    "deck.*": [
      { "widget": "deck.waveform", "height": 160 },
      { "widget": "deck.transport" },
      { "widget": "deck.pads", "rows": 2 },
      { "widget": "deck.eq" }
    ],
    "stage": [{ "widget": "deck", "number": 1 }, { "widget": "deck", "number": 2 }],
    "window.2": [{ "widget": "browser.tracks" }]
  }
}
```

Three rules make this safe and durable:

1. **Slots, never pixel coordinates.** A widget is placed in a named container and ordered
   within it; it is never given an `x`, `y`, `width` and `height` in device pixels. Layouts
   therefore survive window resizing, density changes, a different deck count, and the
   interface being redesigned underneath them.
2. **Restyling is a bounded token set, not CSS.** A layout may set values for declared design
   tokens — colours, radii, spacing, the font stack, the density multiplier. It may not supply
   arbitrary CSS. Arbitrary CSS is a code-execution surface wearing a costume (`url()`,
   `@import`, and selector-driven side effects), and it would make every interface change a
   breaking change for every skin.
3. **An unknown name is skipped with a warning, never fatal** — the rule the current loader
   already follows for malformed files, extended to unknown widgets. A layout written for a
   newer djmanzo opens on an older one, minus the parts it cannot draw. A DJ opening their
   laptop before a set gets the interface, not a dialog.

### Where the registry lives

**In Rust, in `dj-app`, not in TypeScript.** The interface is the largest consumer but not the
only one: the network API, controller mappings and the assistant all need to enumerate widgets
and validate a layout without a webview running. The UI receives the resolved tree over the
existing command surface and renders it; it does not own the vocabulary.

### Migration

The four presets become tree literals. The current `Layout` struct is kept as a **reader**: an
existing layout file, and the `layout.txt` choice beside it, are upconverted into a tree on
load. Nobody's file breaks, and the flat form can be dropped a release later once no one is
writing it.

## Alternatives considered

**Pixel-positioned skins, as VirtualDJ and Serato do.** The most expressive option, and the one
DJs coming from those products would recognise. Rejected: pixel geometry binds a skin to one
window size and one version of the interface. Their skins break on redesign and need per-
resolution variants; ours would too. Slots give most of the reach and survive change.

**Keep the flat struct and add fields as widgets appear.** Cheapest today. Rejected: it is
already at thirteen fields, and it structurally cannot let a DJ's file name anything the binary
does not — which means the file format is decoration, not an extension point.

**Let the UI own the vocabulary in TypeScript.** Natural, since the UI is what renders. Rejected
for the same reason ADR-0003 rejected per-source paths into the engine: the assistant, the
network API and controller mappings would each need their own view of what exists, and they
would drift.

**A full retained-mode widget toolkit or an ECS for the interface.** Rejected as far more
machinery than the problem needs. Svelte already does retained-mode rendering well; what is
missing is a *naming* layer over it, and that is a registry, not a framework.

**Arbitrary CSS in skins.** Rejected above, and worth restating: it is the difference between a
layout being data and a layout being a program. ADR-0002's licensing discipline and ADR-0005's
assistant discipline both rest on that line, and this is the third place it matters.

## Consequences

**Good**

- Skins can move, reorder, duplicate and restyle within bounds — the gap `FEATURES.md`
  currently admits to.
- Multi-monitor and detachable panels (M5) become *a subtree with a window*, not a redesign.
- Controllers and the network API can address components by the same kind of name they already
  address actions by, so an OBS overlay or a phone remote asks for a component instead of
  scraping the DOM.
- A widget is added once, in one place, instead of as a field plus a derived value plus a
  conditional.

**The big one: the interface becomes addressable the same way the engine already is**

ADR-0003's payoff was that every feature is automatically scriptable and remotely controllable.
This is the other half of that sentence. In particular the assistant can **compose a layout**,
and it is safe for exactly ADR-0005's reason: a layout in a named vocabulary is data, and data
cannot execute. *"Four decks, big waveforms, hide the FX for this venue"* becomes a proposal the
DJ accepts or rejects — identical in kind to an action proposal, going through the same review
gesture, with the same guarantee that the worst case is an interface you did not want rather
than something you did not authorise.

It also makes the interface **inspectable**: a resolved layout tree is a complete description of
what is on screen, which is a bug report a user can send and a state a test can assert against.
Neither is possible today.

**Costs**

- It is a refactor of the whole UI layer — `Deck.svelte` and `App.svelte` stop being layouts and
  become renderers. This is the reason the ADR is written before the code.
- **Widget names become a compatibility surface.** They leak into every DJ's saved layout file
  the moment the feature ships, so renaming one later costs an upconversion path. Names must be
  chosen as carefully as action names were, and the registry needs the same review discipline.
- A widget can no longer quietly read whatever it likes from the snapshot; it declares what it
  needs. That is more ceremony, and it is also what makes "do not pay for a widget that is not
  on screen" enforceable rather than aspirational.
- The token set bounds what a skin can restyle, so some skin somebody wants will not be
  expressible. That is the accepted price of layouts staying data.
