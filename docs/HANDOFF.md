# Picking djmanzo up cold

Written for a session that has never seen this project: a different machine, a
different account, no memory of any conversation. Everything needed to carry on
is in the repository, and this file is the index and the method.

**djmanzo** is a professional DJ application — a Rust workspace of twenty
crates behind a Svelte 5 / Tauri 2 interface. It plays records, beatmatches
them, analyses a library, hosts effects and plugins, drives controllers and
timecode vinyl, and carries an assistant that speaks only in actions. It is
built to compete with VirtualDJ, Serato and Engine DJ, with a Dominican and
Caribbean repertoire treated as first-class rather than as a genre pack.

## Read these four, in this order

| File | What it answers |
|---|---|
| [`DIRECTIVE.md`](DIRECTIVE.md) | **What the owner asked for.** 105 sections, verbatim. The governing document. |
| [`DIRECTIVE-STATUS.md`](DIRECTIVE-STATUS.md) | Where each of the 105 stands, counted by a script rather than by memory. |
| [`ROADMAP.md`](ROADMAP.md) | Why each milestone exists and what was learned building it. Long, and worth it. |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | How the crates fit together, and which rules are load-bearing. |

Then [`FEATURES.md`](FEATURES.md) for what exists, [`GUI-OVERHAUL.md`](GUI-OVERHAUL.md)
for the cockpit's nine phases, and [`adr/`](adr/) for the decisions that are not
open to casual revision.

## The rules that always hold

- **No GPL or AGPL code, ever** ([ADR-0002](adr/0002-clean-room-permissive-licensing.md)).
  Every dependency's licence is recorded in [`RESEARCH.md`](RESEARCH.md). This
  is why Mixxx's controller mappings cannot be used however convenient they
  would be.
- **The audio thread never allocates, locks, does I/O, or blocks.** A network
  server, a decoder, an analyser all live on their own threads. `rt_safety.rs`
  enforces it.
- **The assistant speaks only in actions** ([ADR-0005](adr/0005-assistant-speaks-only-actions.md)),
  and the tool schema it is given is generated from `dj_core::vocabulary` so it
  cannot be told about verbs the parser does not accept.
- **One widget vocabulary** ([ADR-0008](adr/0008-one-widget-vocabulary.md)) and
  **the living interface** ([ADR-0009](adr/0009-the-living-interface.md)) are
  the design language. Serve them; do not replace them.
- **No model identifier** in a commit message, a code comment, a document, or
  anything else that is pushed.

## How to work on it

### The gates, all of which must be green before a commit

```
cargo fmt --all
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
cargo test --workspace --all-targets
cd ui && npx svelte-check && npx vitest run && npx playwright test
```

### Mutation-test whatever the new test claims to cover

Break the code the test is about and confirm the test fails. If it still
passes, the test is wrong — strengthen it, and say so in the commit message.
This has caught several tests that were passing for the wrong reason, including
one that could not have failed because the fixture had nothing in it to press.

### Run the interface; do not only type-check it

This is the single most valuable habit in the project. Every serious defect
found in the last several sessions was invisible to `svelte-check` and obvious
within a minute of driving the application:

- a deck 22 px tall whose channel strip painted across the master strip;
- a result line rendered below the fold where it could never be read (twice, in
  two different panels);
- a command palette announcing "Nothing matches that" while its first answer
  was still in flight.

```
Xvfb :95 -screen 0 1400x900x24 &
cargo build --bin djmanzo
DISPLAY=:95 DJMANZO_NULL_AUDIO=1 DJMANZO_DEMO=/path/to/some/audio \
  WEBKIT_DISABLE_COMPOSITING_MODE=1 ./target/debug/djmanzo &
DISPLAY=:95 import -window root shot.png     # then look at it (ImageMagick)
DISPLAY=:95 xdotool mousemove X Y click 1    # and drive it (xdotool)
```

`DJMANZO_DEMO` points at any folder of audio and loads two decks, so the
application is in a usable state rather than empty. `DJMANZO_NULL_AUDIO=1`
makes it run without a sound card, and `WEBKIT_DISABLE_COMPOSITING_MODE=1` is
what stops WebKitGTK failing outright where there is no GPU. Both `import` and
`xdotool` need installing; they are the whole toolkit.

## Traps that have each cost real time

**`npm run build` fails silently in a pipeline.** It runs `svelte-check &&
vite build`, so a type error means `vite build` never runs and `dist/` keeps
the *previous* bundle. A Playwright run then tests the old code and passes.
This has produced a false "mutation killed" result more than once. Always look
for `✓ built` in the output; never pipe it to `tail -1` and assume.

**"Rail" already meant something here.** §22's *Next* rail — which record comes
next — has `rail.spec.ts` and a `mod rail` in the command tests. §74's rail is
about which *controls* are under your hands, and building it as `rail` silently
overwrote §22's test file: eight tests gone, and the suite still green because
the replacement passed. It is `at_hand` throughout now. Before naming a module
after a word from the directive, grep for it: the 105 sections reuse words that
mean different things in different places.

**A card grid is a good way to discover a panel is too small.** The card view
shipped with `minmax(140px, 1fr)`, which in the browser docked along the bottom
gave one column the whole width — a single cover taller than the panel, cut
off, with nothing else reachable. A grid of things that should stay a readable
size needs a *ceiling* as well as a floor, and anything with `aspect-ratio`
inside one needs a `max-height` or its width decides the panel's height.

**The container's Rust can be older than CI's, and clippy gains lints.** CI
runs `dtolnay/rust-toolchain@stable`, so it is whatever stable is on the day.
This container was four releases behind it, and a clean local
`clippy -D warnings` was followed by a CI failure on `collapsible_match` — a
lint the local clippy did not have. Check `cargo clippy --version` against
CI's before trusting a green local run, and `rustup update stable` if they
disagree; it takes a few minutes and a full rebuild, and it is cheaper than a
red pipeline per commit.

**The Playwright stub runs in the browser, not in Node.** `openShell` is
serialised into the page by `addInitScript`, so anything it closes over from
`shell.ts`'s module scope — an import, a helper — is a `ReferenceError` in
the page. It does not look like one: the failing command's promise rejects and
the panel simply does not change, which reads as an adjustment that did
nothing. Everything the stub body needs has to reach it through the answers
table, which *is* serialised. Three previously-green tests failed this way in
one edit — and once, worse, a *new* test passed because of it: a branch meant
to be reached only when a save fails was reached on every save, so the test
that was supposed to prove the branch existed could not have failed either
way. A stub edit that touches an existing command deserves a run of the whole
file, not of the test you just wrote.

**A threshold written beside the thing it colours is a threshold nobody
agrees with.** The strip in the top bar grew one reading at a time — sample
rate, latency, load, dropouts, clock drift — and only one of them ever got a
rule: `class:hot={load > 0.7}`, written in the template. So the one figure on
screen in colour was a CPU percentage a DJ can do nothing about, while a
recording that had stopped writing and a limiter flattening the mix were both
plain grey text. §5's bar is that gathering moved into `dj_app::mission`: one
pass, one place that decides what is worth a colour, and a test that fails if a
healthy night lights anything up. When you add a reading to the bar, add it
there — the component is not allowed to decide, and a browser test asserts it
draws the level Rust gave rather than one of its own.

**A surface Rust knows about is not a surface the shell draws.**
`cockpit::surfaces()` lists twenty; `App.svelte`'s `DRAWN` list has seventeen,
and the other three — the room sensor, the stem controls, the FX rack — are
components mounted *inside* other panels. A workspace placing one of them
passes `resolve` (it is a real surface) and is then filtered out of
`placements` without a word, so the preset opens looking like it did nothing.
Four shipped presets did exactly that, "Read the room" among them, and nothing
said so until §7's picker made them pressable.
`every_preset_places_only_surfaces_the_shell_draws` reads `DRAWN` out of
`App.svelte` and is the guard. The general rule: **before placing a surface in
anything, check it is in that list**, and if it should be and is not, promoting
it is its own piece of work.

**Painting the theme is not choosing it.** §31 reads the night and adapts the
theme on a tick, so `theme.setPackage(id)` alone survives about four seconds:
the next tick takes it back. `themeChosen(id)` is how a choice is declared, and
the adaptation then stops deciding over it. §7's "High Contrast" preset shipped
in a green interface for exactly this reason, and neither `svelte-check` nor
the browser suite could see it — the harness has no night to read. It was
found by watching the running application for half a minute, which is the
argument for the habit above.

**A screenshot under software rendering can be a frozen region, not a
defect.** With no GPU the webview sometimes leaves a rectangle of the previous
frame — a black block, or old text — that survives mouse moves, scrolls and
even a window resize. Reading through one is how a session came to believe the
application was running last build's copy. Before concluding anything from a
screenshot that looks wrong, check the process against the binary:
`ps -o lstart= -p $(pgrep -f target/debug/djmanzo)` against
`ls -l target/debug/djmanzo`. A relaunch clears it.

**`pkill` returns 1 when nothing matched**, which aborts the rest of a compound
shell command — so `pkill djmanzo; sleep 3; ./djmanzo &` silently does nothing
at all when the application has already exited. Three attempts to restart it
went nowhere before that was noticed. Use `|| true`, or put the launch in its
own command.

**A `$effect` that reads the snapshot runs at 60 Hz.** `Deck.svelte` is handed
a fresh `deck` object sixty times a second, so an effect touching `deck.number`
re-ran at that rate — refetching a fixed table, reassigning its state to a new
object, and remounting every knob beneath it. The visible symptom was a
contextual menu that opened and vanished within the same frame. Anything that
should happen *once per deck* belongs in `onMount`; a deck component's number
does not change for the life of the instance.

**Svelte state set during a `pointerup` that releases pointer capture can be
lost.** Setting `menu = true` in the handler was reliably discarded: the flag
went true, the handler read it back as true, and the template never rendered.
Deferring the write by one tick (`setTimeout(fn, 0)`) puts it outside that
release and it holds. Found by driving it — a type-check cannot see a state
write that does not survive.

**The icon test only saw `icon="literal"`.** An icon chosen by a ternary —
`icon={locked ? "fa-solid fa-lock" : "fa-solid fa-lock-open"}` — was invisible
to its regex, so a button shipped rendering a fallback letter with every test
green. It now also matches any `"fa-solid fa-…"` string anywhere in a
component, which is what catches a name however it is assembled. If you add a
third way to name an icon, check the guard still sees it.

**`.deck` means three things.** It is the deck section, *and* a label in the
Next rail, *and* a label in §74's At Hand panel — "Deck 1", saying which deck a
row is about. A test counting `.deck` read four decks as five. Use
`section.deck[data-deck]`, which is on the deck itself and means nothing else.

**Playwright's browser.** CI installs its own. A container that pre-installs
one is pointed at it with `DJMANZO_CHROMIUM=/opt/pw-browsers/chromium`,
otherwise every test fails with "Executable doesn't exist".

**Two copies of the density band table.** Rust owns it in
`dj_app::cockpit::BANDS`; `ui/e2e/shell.ts` carries a copy because a Playwright
stub cannot call into Rust. They drifted once, and the sweep that exists to
catch a clipped deck spent a session measuring an application that no longer
existed. A Rust test now reads the TypeScript file and fails when they
disagree — do not delete it.

**`toBeVisible()` is not "can be seen".** Playwright asks whether an element
has a box, not whether anything is painted over it. A waveform layer shipped
underneath the opaque tiles — `z-index: 0` against tiles at `z-index: auto` that
come later in the DOM — and the browser test passed while the application showed
nothing. Where paint order is the thing that matters, assert computed
`z-index`, and look at the running application.

**The browser harness answers only what somebody remembered to stub.** An
unstubbed command resolves to `null`, and a component that reads a field off it
throws inside its own subtree — quietly, while every assertion around it stays
green. `waveform_info` was unstubbed for as long as the pair view has existed,
so its waveform lanes had never once rendered in a test. If a panel looks empty
in a browser test and full in the application, look at `ANSWERS` first.

**A result that appears below the fold reads as a button that did nothing.**
Recorded a fourth time, because it happened a fourth time: §27's ghost opened
a panel under a row of the Next rail, and in a docked list a hundred pixels
tall every word of it was below the fold — the overlay drew perfectly and
nothing explaining it could be reached. The fix is one call, scroll the answer
into view when it arrives, and it is worth doing wherever a control's result
appears *inside* a panel rather than replacing it.

The sharper half, learned on the second pass: **`block: "nearest"` is not
enough for a panel taller than a line.** It brought the top of the ghost into
view and left its last line — what djmanzo *cannot* see, which is the one that
must not be missable — still clipped. Use `block: "end"` when the important
line is the last one. A browser test of any of this would be vacuous at the
harness's window size, where nothing overflows; it was found, and both fixes
were checked, by driving the application.

**Flex children shrink to nothing, so a dock squeezes instead of scrolling.**
A third surface in a side dock rendered as one row cut through the middle of
its letters — the dock has `overflow: auto` and never reached the height that
would use it, because every surface in it shrank to make room. `.dock.side >
.surface` has a `min-height` floor now. Worth remembering as a shape rather
than as one fix: a scrollable flex column only scrolls if its children refuse
to shrink.

**The harness can be *fuller* than the application, not only emptier.** The
`waveform_info` note above is the emptier direction. The other direction cost a
session's confidence: a new field was added to that command, the Playwright stub
was given a plausible value for it, four browser tests passed — and Rust never
produced the value at all, because the producer asked the library for a grid
that a freshly loaded deck only has in the engine. Nothing on any lane. When you
add a field to a stubbed command, the browser test proves the *drawing*; write a
Rust test for the *producer* in the same commit, and look at the running
application before believing either.

**A test whose loop is bounded by the constant it is testing proves nothing.**
Recorded twice now, in §81 and again in §37: `for count in 0..ENOUGH_NIGHTS`
looks like it walks every case below the threshold, and lowering the constant
to 1 shortens the loop to a single empty case and the test stays green. Write
the literals — 0, 1, 2 — and the mutation dies. It is the same shape as any
test that derives its expected value from the code under test.

**A median is robust to the window being wrong, which makes a test of the
window's *value* useless.** §37 reads the room twelve to thirty seconds after
a mix. Widening that to start at the mix's end moves six readings into
fifteen and the median does not move at all, so the test that checked the
number passed with the window in the wrong place. What separates them is
whether a reading counts: a room watched *only* in the twelve seconds djmanzo
is meant to ignore must produce nothing. State a window as an exclusion, not
as an average.

**A panel that asks once asks before the answer exists.** The Next rail came
up with no deltas and no transition on every row while two analysed records
sat on the decks, and stayed that way all night. It asks once, on open — and
at start-up that is *before* the interface has loaded the decks, so
`current_track` answered `None`, djmanzo honestly ranked against silence, and
nothing ever asked again. Confirmed rather than guessed: a temporary
`tracing::info!` in `suggest_next` printed `on_deck=None` at start-up and
`on_deck=Some(..)` one refresh later.

The rail was right not to poll — its own comment says a rail that reshuffled
every time a deck moved would be unreadable — so the fix is to re-ask when
**the one input the answer depends on** changes: the record on the deck it
follows. As a `$derived` string, not by reading the `decks` prop inside the
effect, because that prop is a fresh array sixty times a second (§29's trap).

Two things about testing it, both of which cost a round. The deck **picker**
was already refreshing on change, so the obvious test measured that and passed
with the fix removed. And no browser test could stage the real case at all,
because the harness delivered exactly one snapshot — a component that asks
once looked identical to one that keeps up. `shell.ts` now exposes `__emit`
and `__lastState` so a test can deliver a second frame with one field
changed; reach for it whenever a component has to react to state *changing*
rather than to state *being*.

**A timing test made patient can become a test of the timeout.** A 60 Hz
pump test slept 80 ms and asserted a snapshot had arrived; it failed once on a
loaded macOS runner and nowhere else. Waiting five seconds instead fixed the
flake and broke the test: the pump's own one-second heartbeat emits anyway, so
a mutation that stopped it noticing state changes at all still passed. The fix
that works is to remove the *other* source first — `SnapshotPump::with_heartbeat`
pushes it a minute out — and then wait as long as you like, because anything
arriving is now the thing being tested. **Whenever you make a timing assertion
more patient, mutate the behaviour it covers and watch it fail again**; a
longer wait tests less, not more.

**A layout test can be vacuous at the harness's window size, and look green.**
§35's baseline table has five columns in a side dock, which looked like a
table that would push its own panel sideways — so wrapping headers and
`table-layout: fixed` went in, with a test to prove it. The test passed with
every fix removed, at 1280 and again at 900: the side dock's own floor is
wider than the table's natural width, so the overflow it guarded against
cannot happen. All of it came out again, fixes and test both. The general
form: **before writing a layout budget, break the layout and watch the budget
fail.** A budget that has never failed is a budget measuring something that
was never at risk, and it will be quoted later as evidence.

**A width assertion in pixels can be satisfied by the element's own borders.**
The ghost band's first test asked for a bounding box wider than one pixel, and
a mutation setting the band's width to *zero* left it green: the element has a
two-pixel dashed border on each side, so its box is four pixels wide with no
band in it at all. Assert against something the drawing cannot fake — here the
band's left edge and width as **fractions of the record**, checked against the
frame numbers the stub answers with, which pins where the mix is as well as
that it has a width. A geometry test that cannot say *where* is usually not
saying *whether*, either.

**The `npm run build` trap is real and it caught this project again.** Under a
mutation test the mutated file failed `svelte-check`, so `vite build` never
ran, `dist/` kept the previous bundle, and Playwright reported the mutation
*killed nothing* — two green tests against code that was never built. Grepping
the output for `✓ built|ERROR` is not enough either: the ERROR line matches and
the `&&` succeeds. Gate on `✓ built` alone:

```
npm run build 2>&1 | grep -q "✓ built" || { echo "not built"; exit 1; }
```

**Recapturing `ui/e2e/snapshot.json` can quietly weaken the budget.**
`DJMANZO_SNAPSHOT_OUT` writes the moment two decks are loaded, which is *before*
the analyser has finished — so a fresh capture can land with `analysis: null` on
both decks, and a deck with no tempo, key or phrase row is shorter than the one
the budget exists to measure. Adding a field to `Snapshot` does not need a new
capture: the fixture test compares *keys*, so adding the new key to the
committed file with the value djmanzo really produces for that state preserves
everything the geometry was calibrated against. Recapture only when the values
themselves need to change, and check the analysis came with them.

**The band floors are a fact about the top bar, not only about the deck.**
Adding one destination button pushed that row onto another wrapped line and
took 40 px out of every stage at every window height. Do not guess the
correction: `ui/e2e/density.spec.ts` prints, for each window height, how much
the deck needs and how much it has, and the floor is where that reaches zero.

**Disk.** The writable allowance is fixed, and `df` misleads. `rm -rf
target/debug/incremental` recovers several gigabytes and cargo rebuilds it.

**`pkill -f "something"` matches its own shell.** It kills the command that
ran it. Use `pkill -x <name>`.

## What this container cannot prove

There is **no audio device, no microphone, no camera and no phone**. The tests
prove pitch, timing, level, and that the audio thread never allocates. They
cannot prove any of it *sounds* right, and nothing here should claim otherwise.

The [ADR-0004](adr/0004-waveform-rendering-strategy.md) rendering gate is still
open for the same reason: the benchmark needs a real Xubuntu machine with a
GPU. Under Xvfb there is no accelerated compositing to measure, and the
numbers are a floor rather than a verdict. That is recorded in the ADR.

## The environment, as it behaves in practice

- **Pushing a tag returns HTTP 403** from an agent session. Branch pushes work.
  A release is cut by dispatching `.github/workflows/release.yml` with a `tag`
  input — the runner's own token creates the tag and the release. With no
  `tag`, it builds without publishing.
- **Release builds run on minor and major tags only**: the workflow triggers on
  `v[0-9]+.[0-9]+.0`, so `v0.15.0` builds installers and `v0.14.1` stays quiet.
  Bump the version in `Cargo.toml`, `crates/dj-app/tauri.conf.json` and
  `ui/package.json` together; the workflow refuses a tag that disagrees with
  them.
- **Manufacturer documentation hosts return 403** through the egress gateway,
  which is why controller mappings exist for Pioneer only. The owner supplies
  vendor MIDI tables when they can. Do not re-litigate this.
- **Work happens on `main`.** No pull request unless one is asked for.

## Where the work stands, and what is next

`DIRECTIVE-STATUS.md` holds the live count; re-run its script rather than
trusting a number written anywhere else, including here.

The recently shipped cockpit work followed one shape four times: a valuable
thing was mounted *inside* another panel, so it could only exist where that
panel did. **Prepare**, the **Next** rail, the **set plan** and §39's **room**
each became dockable surfaces of their own, and each time the thing they
replaced was removed rather than duplicated — because two places that do the
same job eventually disagree.

The room is the one to read if you are about to do the fifth. Three things had
to move together and only the first is obvious: the component gets a surface
and a `DRAWN` entry; the tests that reached it through its old parent have to
be rerouted (four of `baseline.spec.ts`'s did, and they failed loudly, which is
the good case); and **it needs a way in that does not depend on the thing it
was nested under**. The panel row is thirteen buttons and already wraps on a
laptop — a fourteenth cost the deck eleven pixels at the relaxed density and
`density.spec.ts` caught it — so the way in is §39's own indicator in the
status strip, which draws even with nothing to report precisely so that it can
be that way in.

The context engine (§11) now exists and is the shape the rest of phase 5 should
follow: one judgement, made in one place, published on the snapshot, with a
stated certainty and a stated basis — and consumers that read it rather than
each working the same thing out. `dj_core::ContextEngine` is the engine,
`dj_app::night` feeds it, `cockpit::Attention::for_context` and the autopilot
are the first two consumers, and the **Night** surface shows its working.
Three of `DJContext`'s eight fields are still ungathered (`musicContext`,
`hardwareContext`, `djBehaviorContext`) and they are absent rather than empty,
because a field that is always null is a promise.

Phase 5's gate is met. §44's transactions, §72's matrix and §47's emergency
ship alongside the context engine and §9's warrant, and the three gates an
assistant passes — a hand on the control, the posture's matrix, the night's
certainty — are three separate questions in three separate places, on purpose.

**One limitation to know about the transaction.** Its "next record" comes from
`Conduct::setlist` only, because that is where `autopilot::Situation::next`
comes from. With no set built there is nothing to load and the plan is empty,
and it says so. Wiring the Next rail's suggester in as a fallback is a small
piece of work and is part of §68's remaining half rather than of §44.

**Phase 5 is closed.** The context engine, §9's warrant, §44's transactions,
§72's matrix, §47's emergency and §41's typed interface vocabulary all ship.
The shape to copy from it: one judgement made in one place, published, with a
stated certainty — and consumers that read it rather than each working the same
thing out. Two duplicated tables were removed on the way (the histogram
`dj_assistant::room` kept, and the dock table `App.svelte` kept), both found
because a second consumer appeared and disagreed with the first.

**The directive's open list is now empty.** Every one of its 86 deliverable
sections has something real behind it: 42 finished, 44 partly there, and none
untouched. That is a different claim from the directive being done, and the
two must not be confused — a 🟡 in `DIRECTIVE-STATUS.md` names exactly what is
missing from that section, and those named gaps are the work. Read them before
starting anything; "what is left" is answerable from that file rather than
from a list here that will go stale.

The largest of them, in the order they are worth doing:

1. **§25–§27, the waveform as instrumentation.** The architecture and §57's
   colour rule ship; `dj_render::layer` is the inventory and the count is
   checked in both directions. **Twelve of twenty** exist. The eight that do
   not are the ones djmanzo cannot yet compute: vocal and stem presence,
   breakdowns, drops, saved loops, energy trajectory, the mix-*in* region and
   crowd response. Each needs analysis or history that does not exist, so the
   next one built is a research question rather than a drawing one —
   **mix-in** is the cheapest of them, and even it needs something that can say
   where a record's intro ends.

   Nothing here can be checked against a real room. **This container has no
   camera and no microphone**, so §34–§37 are tested against synthetic series
   in Rust and against a fixture in the browser, and the running application
   shows the panel in its empty state and nothing else. Do not let a green
   suite there become a claim about how a room reads.

   §37 is also **the one place djmanzo writes down something it could not
   otherwise derive**. Room readings live twenty minutes and a night's log
   does not outlive the run, so "this has happened on previous nights" has no
   source; `mix_responses` is that exception, taken as narrowly as the claim
   allows — one row per mix per sense, four values, no time series. If you are
   about to add a second such table, the bar is that one: say what cannot be
   derived, and store the *finding* rather than the feed.

   §27's ghost has already paid for itself twice: `ghost::look` is what §22's
   rail estimates a transition with, so the line beside a candidate, the band
   drawn on the record and the mix the automix performs are one plan rather
   than three opinions. If something else needs to know what a mix *would* be,
   call it rather than planning again.

   §27's ghost is the twelfth, and it went in without any of that because it
   is arithmetic over two records rather than a new reading of one: the mix is
   `plan::plan`'s, and all `dj_app::ghost` adds is where the candidate's first
   full phrase lands on the outgoing grid. Its two missing marks — the vocal
   entry and the drop — are **derived** from this same layer table rather than
   written down beside it, so the day either analysis ships the panel stops
   claiming it cannot see them. That is the pattern to copy for the other six.

   **Where a layer is drawn matters as much as whether it is.** The mix-out
   band was written for the scrolling lane first and was almost never on
   screen: a lane runs at a couple of hundred frames per pixel, which is two
   seconds of record, so anything twenty beats from the end is invisible until
   you are inside it. The overview is where a whole-record fact belongs. Ask
   which of the two views a new layer is a fact *about* before drawing it.

   §26 has its first handle and the rest of its list — cue markers, phrase
   markers, loop edges — uses the same `onMoveMark` shape: it is mostly a
   matter of giving each mark an owner that knows what moving it means. Its
   last item, *AI suggestion: show as a ghost layer*, ships with §27.

   **§27 was mis-filed for several sessions as "needs a preview player".** It
   does not: it says *display*, *overlay*, *ghost* and "make the future
   visible", which is a drawing, not an audition. Auditioning a candidate into
   headphones is a real thing djmanzo lacks, and it is not this section. Read
   the section before believing a status row about it — this one had been
   copied forward unexamined.
2. **§68 is closed; what it was going to drive is not.** The automix and the
   autopilot perform the held mix, `dj_app::mixes` derives the *performed* one
   back out of the action log, and **replay reads it**: `replay::Window`
   renders one handover back to a WAV with its run-up, and *hear it again* on
   any row does it.

   The last field arrived as `dj_app::shape` — **one table of what a style
   does beyond the two channel faders**, which is §68's `outgoingStems`,
   `incomingStems`, `eqPlan` and `fxPlan`. Read it before adding a transition
   style, because the automix now *performs* that table rather than branching
   on the style itself: a new style is a row there and nothing in `automix`
   changes. The plans are derived from the style on every read, never stored
   on the transition — a stored copy is a second answer, and the one that goes
   stale is the one a panel is reading.

   **This is the shape of the whole class of bug it fixed.** The styles were
   listed once in Rust and again in TypeScript, and the descriptions a third
   time; `vocal drop` was in the vocabulary and performed by the automix for
   months while no panel offered it, because nothing made the interface's copy
   wrong when a style was added. The list is served from Rust now
   (`transition_styles`) and the browser harness stubs it from a golden file
   the Rust test blesses. If you find yourself typing a list that Rust already
   has, that is the bug arriving again.

   Re-*planning* — changing a mix's length or style and hearing the
   alternative — is now `dj_app::practice`, below. Seeing what a mix would do
   before anything is loaded is `dj_app::ghost`, §27: it takes the same
   `plan::plan` and draws it over the outgoing record's overview, so a
   candidate can be understood while it is still a library row.

   **A replay window is not a seek**, and anything built on it inherits that:
   the engine's state at a moment is the whole set up to it, so rendering the
   third hour costs three hours. Do not try to make it cheaper by skipping the
   run-up — a test asserts the window is the same audio the whole set produces
   there, and it fails under exactly that change.

   **What `mixes` cannot see**, so nobody rediscovers it as a bug: a mix made
   entirely with EQ and no fader movement at all. The outgoing record never
   becomes inaudible, so nothing crosses. That is a real gap and a small one —
   a mix that never takes the outgoing record out is a mix that has not
   finished.
3. **The practice lab renders; it does not play.** `dj_app::practice` builds a
   set file for a mix that was never played and hands it to `replay`, so §69's
   "without altering the live master" is a property of the mechanism rather
   than a promise: there is no second engine and no live output path, and
   nothing it does can reach the decks.

   **The automix writes the rehearsal.** The real state machine is stepped
   against a simulated playhead and its actions *are* the file. Do not be
   tempted to write the crossfade out in `practice` — that would be a second
   implementation, and the day the automix changed the lab would keep
   rehearsing the old one. `what_it_rehearses_is_what_the_automix_performs`
   fails under exactly that change.

   **Two defects in it were invisible to every test and obvious on playing the
   file**, which is the third time this project has learnt the same thing. The
   tail is not made of events — a replay stops at the last action, and the last
   thing a transition does is eject the outgoing record — so the file ended the
   instant the mix landed, four seconds short of what the panel claimed. And
   `mix_to` came from the plan's geometry rather than from where the rehearsal
   actually finished, so a cut was marked as a fifteen-second mix inside a
   twelve-second file. Both are pinned now, including a Rust test that renders
   a rehearsal and compares the file's length with the number beside it.

   What §69 asks for and is **not** here: stems, FX and loops explored *live*.
   Those need a second engine feeding the headphone output. That is a real
   piece of work and the offline half does not pretend to be it.

4. **The learning cluster, now that §13 and §14 exist.** `dj_app::signals`
   names the gestures and `Tendency` is the only thing that generalises from
   them — constructible only through `tendencies()`, never without a phase,
   never on fewer than four in that phase. §12 (learn the DJ), §24 (pairs and
   relationships) and §81 (profile by context) all want to be built *on that
   type* rather than beside it: anything that counts gestures its own way is a
   second learner with its own idea of what is enough, which is the failure
   §13 is about.

   **§24's first half exists**: `kept_pairs` stores the transitions a DJ
   *kept*, directionally and weighted by how many times, and the Next rail
   reads them. What it deliberately does not store is what merely happened —
   that is derivable from the log, and a second copy would disagree with it.
   Of §24's remaining examples, "A vocal → B instrumental" is now derivable —
   the style is in the log and `dj_app::shape` says what each style does to
   the stems, so a kept vocal drop *is* a record of one. "Works only with an
   8-beat loop" is not: nothing records the loop that was running under a
   transition.
5. **§81's profiles are read and not yet acted on.** `dj_app::profile` builds
   a conditional profile per kind of night — §81's six settings, told rather
   than inferred, because nothing in the signal says *wedding*. It carries the
   §13 discipline up a level: private fields, constructible only through
   `profiles()`, nothing at all under three nights of a setting, and each field
   silent until half the nights that spoke agree.

   **Nothing consults one.** The suggester, the layout and the posture all
   still behave the same at a wedding as at a club, which is the half that
   would make the profiles worth having. Whatever reads them first should read
   them *as a tilt* rather than as an override, the way `kept_pairs` is
   weighted below a key match and a tempo match together — a DJ whose night is
   going differently wants the machine to notice.

   **The split to keep**: the setting is stored because nothing can derive it;
   genre weights are derived because `history` already holds them; the other
   four figures are stored only because the action log does not outlive the run
   that made it. That last one is the exception to "derive, never record" and
   it is worth stating rather than discovering.

6. **§76's lens adds; it is built so it cannot replace.** `library_lens` takes
   the ids the table is already showing and answers about *those*. It never
   queries, filters or orders the collection — which is how "this must never
   replace the standard library view" stays true under later edits rather than
   depending on someone remembering it. If a future column needs the lens to
   choose which rows appear, that is a different feature and §76 says not to
   build it here.

   **A blank is not a zero**, and the distinction is the whole design: an empty
   cell means djmanzo has no opinion, a `0.00` means it thinks the record is
   bad. Filling blanks would rank an unanalysed record below a merely
   unsuitable one while looking like it had considered both.

   **Crowd suitability is one of §76's eight and is not here**, because it
   needs to know what the room is doing. Novelty and familiarity are likewise
   this DJ's own history and not the room's ears — the crowd's familiarity with
   a record is not knowable from a laptop. Both are named in the interface
   rather than quietly dropped, so a DJ counting the columns they were promised
   knows which is missing and why.

7. **§29's gestures are a table in Rust, and that is the point.** Every
   `SvgKnob` used to carry its own `ondblclick` naming its own reset value —
   three EQ bands each spelling `1`, the filter spelling `0`, and nothing
   making a fourth call site agree. `dj_app::handle` owns it now, and answers
   in **action text** so a drag, a double-click, a menu entry and a MIDI CC are
   the same action. Adding a control is a row there.

   **The menu stays short by rule.** §29's own warning is "do not turn every
   knob into a huge widget", and a contextual menu is exactly where one grows
   into a widget. A test refuses more than four entries.

   **The AI hover is the bullet that is missing**, and it is missing for a
   reason worth keeping: the assistant stages whole moves — load this, cue
   there, set the fader — rather than opinions about single parameter values,
   so there is nothing for a hover to read yet. Whatever provides it should
   come from the assistant's own staging rather than a second scorer invented
   for the tooltip.

8. **§31 is mostly brakes, and the brakes are the feature.** `dj_app::mood`
   holds a four-minute minimum, forty seconds of hysteresis, a fade, and a lock
   that beats all of them. Choosing a theme from the menu tells djmanzo to stop
   deciding — an interface that overrode a deliberate choice four minutes later
   would be worse than one that never adapted.

   **Venue ambience is §81's setting**, told rather than sensed. That is why
   §81 came first: §31's own examples are *beach*, *club* and *daylight*, which
   is the axis §81 already asks the DJ about. The one thing this cannot do is
   *daylight as a measurement* — whether the sun is on the screen now.

   **Do not feed it the genre of the record on deck 1.** That changes every
   four minutes, which is precisely the flicker §31 forbids. The phase is the
   musical context, already smoothed over minutes by `dj_core::context`.

9. **§89 regresses geometry, not pixels, and the reason matters.** CI installs
   its own Chromium and this container has a different build, so a screenshot
   baseline captured in either place fails in the other on font rasterisation
   alone. A suite that has to be re-blessed every run has stopped being a test,
   and one re-blessed automatically never was one. What runs instead is
   `budget.spec.ts`'s rules against all ten of §89's configurations — controls
   on the first screen, decks inside the window, surfaces actually drawn, no
   sideways scroll. Those fail for the right reason.

   **It does not compare appearance**, and that gap is real: a theme that
   turned every panel the same colour would pass all fifty of these. If pixel
   diffing is ever wanted, pin the browser build in both places first.

10. **§20's performance table** is the browser at fewer columns than §20 lists —
   the other three views ship. Adding the missing columns (energy, vocal and
   stem availability, transition suitability, request count, AI confidence)
   mostly waits on analysis that does not exist, which is the same wall §25's
   remaining layers are behind.

Three older items are open and are not part of the 105:

- **Visual effects and audio-visualisation.** WebGL over the master output, and
  the interface motion the living-interface ADR describes but the code does not
  yet perform. Gated behind the same rendering question as ADR-0004.
- **Effects in the world model.** The visual language treats the set as a
  watershed; the effect rack is not yet part of that weather, so an effect
  changes the sound and nothing on screen.
- **The polish pass for a beta demo.** "Every feature reachable and worth
  reaching." The keyboard half is done and enforced by a test in `dj-hid`'s
  `bundled.rs`. The interface half is done by *using* the application under
  Xvfb — scraping the Svelte source for unreachable controls was tried and
  rejected, because it produces both false positives and false negatives.

Two smaller ones are open and honest about why: **#97**, a blue focus ring the
owner reported, could not be reproduced in the default theme — the only blue in
the palette is `pkg-daylight`'s deliberate high-contrast cyan, and it needs the
owner to name the control they mean. **#23** is the ADR-0004 gate above.

## If you are running unattended

The four-hourly trigger that drove this work is bound to the account that
created it and does not travel. Recreate one that says: pull `main`, read
`ROADMAP.md` and this file, build one complete tested slice — vocabulary,
engine, snapshot, interface, tests — verify every gate, mutation-test the
load-bearing test, drive the interface under Xvfb, update the docs so they
match the code, commit and push to `main`.

**Build one slice at a time and finish it.** The half-built feature is the one
that gets forgotten and later mistaken for a bug.
