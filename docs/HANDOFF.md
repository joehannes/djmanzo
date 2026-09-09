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
Recorded a third time, because it happened a third time: pressing *hear it
again* added a line to its row, and in a docked panel that line was exactly the
one pushed out of sight. The fix is one call — scroll the answer into view when
it arrives — and it is worth doing wherever a control's result appears *inside*
a panel rather than replacing it. A browser test of it would be vacuous at the
harness's window size, where nothing overflows; this was found and checked by
driving the application.

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

The recently shipped cockpit work followed one shape three times: a valuable
thing was mounted *inside* another panel, so it could only exist where that
panel did. **Prepare**, the **Next** rail and the **set plan** each became
dockable surfaces of their own, and each time the thing they replaced was
removed rather than duplicated — because two places that do the same job
eventually disagree.

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

The largest open sections, in the order they are worth doing:

1. **§25–§27, the waveform as instrumentation.** The architecture and §57's
   colour rule ship; `dj_render::layer` is the inventory and the count is
   checked in both directions. **Eleven of twenty** exist. The nine that do not
   are the ones djmanzo cannot yet compute: vocal and stem presence,
   breakdowns, drops, saved loops, energy trajectory, the mix-*in* region, the
   AI's own suggestion and crowd response. Each needs analysis or history that
   does not exist, so the next one built is a research question rather than a
   drawing one — **mix-in** is the cheapest of them, and even it needs
   something that can say where a record's intro ends.

   **Where a layer is drawn matters as much as whether it is.** The mix-out
   band was written for the scrolling lane first and was almost never on
   screen: a lane runs at a couple of hundred frames per pixel, which is two
   seconds of record, so anything twenty beats from the end is invisible until
   you are inside it. The overview is where a whole-record fact belongs. Ask
   which of the two views a new layer is a fact *about* before drawing it.

   §26 has its first handle and the rest of its list — cue markers, phrase
   markers, loop edges — uses the same `onMoveMark` shape: it is mostly a
   matter of giving each mark an owner that knows what moving it means. §27's
   ghost track still needs a preview player djmanzo does not have.
2. **§68's last quarter.** The automix and the autopilot perform the held mix,
   and `dj_app::mixes` now derives the *performed* one back out of the action
   log — the night's own list of what went into what, in beats, with the style
   named from what was actually done. **Replay reads it now**: `replay::Window`
   renders one handover back to a WAV with its run-up, and *hear it again* on
   any row does it. What is still missing is re-*planning* — changing a
   recorded mix's length or style and hearing the alternative, which is §69's
   practice lab rather than §68's object. The stem, EQ and FX plans are still
   absent because nothing decides them — the automix's own style handling
   (`begin`) is the closest thing to an FX plan that exists.

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
3. **The learning cluster, now that §13 and §14 exist.** `dj_app::signals`
   names the gestures and `Tendency` is the only thing that generalises from
   them — constructible only through `tendencies()`, never without a phase,
   never on fewer than four in that phase. §12 (learn the DJ), §24 (pairs and
   relationships) and §81 (profile by context) all want to be built *on that
   type* rather than beside it: anything that counts gestures its own way is a
   second learner with its own idea of what is enough, which is the failure
   §13 is about. §24 in particular is now reachable — `dj_app::mixes` already
   derives the pairs a night actually contained.
4. **§20's performance table** is the browser at fewer columns than §20 lists —
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
