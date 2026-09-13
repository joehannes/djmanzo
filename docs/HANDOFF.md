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

**Anything that rearranges the cockpit on its own must ask `Attention::reflow`
first.** §18 states it without exceptions — *when actively mixing: no major
layout reflow* — and `reflow` is false during every mix for that reason. §17's
phase priorities are gated on it, and a phase that turns over mid-mix is
**skipped rather than queued**: by the time the mix ends the phase is either
still the same one, and the next snapshot applies it because the "last applied"
marker was never advanced, or it has moved on and the stale one was never worth
applying. The same gate belongs on anything else that moves a panel by itself.
And it only ever opens: the DJ's own arrangement outranks a reading, and
closing a panel the phase opened is how the override in §17's last sentence
works without a dialog.

**A token nothing defines fails in two different silent ways, and both were
in the tree.** Without a fallback the declaration is invalid and the property
inherits — forty-seven pieces of text asked to be `--muted` and came out the
ordinary colour. With one, `var(--ok, #6a9955)` draws that green on every
palette, which is exactly what `theme-tokens.test.ts` exists to stop, escaping
through the single exemption that test grants. Neither shows up as an error
anywhere. The vitest `every token a component asks for is defined somewhere`
now catches both; a token one component sets for another (`--jog-size`) counts
as provided, because the rule is that *somebody* answers, not that the palette
does.

**The place to name a colour's meaning is `cockpit::Role`, and the place to
give it a value is `app.css`.** Each role aliases a token the palettes already
define, so all seven packages get all fourteen for free. Before adding a
fifteenth, read `Role::must_differ_from`: it names the pairs a DJ has to tell
apart, and roles outside a pair are deliberately allowed to share a colour —
forcing fourteen hues is the neon-everything failure §30 opens by warning
about.

**A wall-clock assertion on this machine is a coin toss, and the fix is
always the same shape.** Three tests failed under the full parallel workspace
run and passed alone, about one run in three each, and every one of them was
asserting that a thread got scheduled inside a fixed sleep or a fixed budget:

- `dj-render`'s `zoomed_out_rendering_does_not_walk_the_whole_track` took a
  single wall-clock sample. `fastest_ms_per` was written in that same file for
  exactly this and the other three tests already used it; this one was the last
  holdout.
- `dj-net`'s `an_endless_body_is_refused_rather_than_read_forever` bounded the
  client by a count of writes, so the client could finish pushing before the
  server thread reached its cap check. It waits on a ten-second deadline now.
- `dj-audio`'s `stream_only_calls_back_while_playing` slept 50 ms and asserted
  the callback had run. It polls for the callback now, and samples after the
  pause has settled rather than allowing "one in-flight block", which was the
  same coin toss in the other direction.

**Wait for the condition; never sleep for it.** And when a budget really is the
point, take the best of a few attempts rather than one sample — a genuine
regression is slow in every attempt. Each fix was checked to still fail for the
right reason before being kept.

**The scrolling lane is a moving target, and a browser test cannot chase it.**
`Waveform.svelte` interpolates between snapshots at sixty frames a second, so a
marker on a *playing* deck moves about seven pixels between `boundingBox()` and
`mouse.down()` — and the press lands on the tile behind it, with no error and
no dispatch. That is not a defect: a hand tracks a moving surface and a
measure-then-click test cannot, and it is also the wrong case to design for,
since cues get edited while a record is prepared. `cues.spec.ts` emits a
**stopped** deck through `__emit` before dragging, and the arrow-key nudge is
what covers the deck that is playing out to a room. If a drag test on a lane
mysteriously dispatches nothing, this is why — check `deck.playing` in the
state you emitted before looking anywhere else.

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

**Three fields have now been found stored and read by nobody**: `frozen`
(§78), `focus` (§77) and the `last_played` / `phrase_beats` the library row
carried into a DTO that dropped them (§20). The shape is always the same — a
field added when the type was designed, for a consumer written later that never
arrived — and it is invisible to every gate, because a field that is serialised
round-trips perfectly. When a section's row says something "is modelled",
`grep` for a reader before believing it.

**And the mirror image: a preference read by a component and stored by
nobody.** §8's Level 1 turned up three, all the same shape — `let sortBy =
$state("artist")`, `let ascending = $state(true)`, `let page = $state("cues")`.
Each is a perfectly good initialiser and each resets on every **mount**, which
is every time a panel closes and reopens, not only every launch. Nothing is
wrong on either side of the join; it is only wrong across it, so
`svelte-check`, `vitest` and every Rust test pass. The tell is a `$state`
holding something a DJ *chose* rather than something the application is
currently doing. `remembered::Remembered` is the list of what is supposed to
survive, and its test reads `state.rs` and fails when a row claims a file
nothing writes — so the next one of these is a failing test rather than a
discovery.

**A filter whose predicate is the sort key does nothing when you move it.**
The palette ranks by §58's tier and then cuts to twelve; §18's budget cuts it
again to what a hand needs. The first version of that said the order mattered —
filter before the cut to twelve, or a DJ gets four rows — and a mutation moving
the filter after the cut left every test green. It has to: "everything at or
above this tier" is a *prefix* of a list sorted by tier, so the two operations
commute. The claim was wrong, not the test. What did turn out to be
load-bearing was smaller and had no test at all: the note saying the list was
shortened must be computed over the twelve the DJ would have seen, not over
every match, or djmanzo announces a cut it did not make on almost every query.
Mutate the thing the comment claims, and when the mutation survives, suspect
the comment first.

**A Playwright drag on the waveform needs a stopped deck — and sometimes not
even that.** The strip is translated under a clipping lane, so a handle can
have a layout box, be off-screen, and still report one; `boundingBox` does not
know. `cues.spec.ts` stops deck 1 first and drives `page.mouse`, which works for
the cue markers. For the phrase handles even that raced, so `phrase.spec.ts`
dispatches the real handler chain on the element instead —
`pointerdown` on the handle, then `pointermove`/`pointerup` on the window,
which is how `grab` listens on purpose. That tests the wiring without a
coordinate race; whether the handle is *reachable* is a CSS question and the
§75 rule in `theme-tokens.test.ts` answers it. A new overlay handle needs
`pointer-events: auto`, a `z-index` and `touch-action: none`, copied from
`.cue-marker.grabbable` — without them it is invisible to a pointer and the
drag test fails in a way that looks like the handler being wrong.

**`DJMANZO_DEMO` does not override decks the application remembers.** Restarting
with a different demo folder leaves whatever `persist` restored on the decks, so
a drive that needs a *particular* record cannot get one that way. The library
database at `~/.config/app.djmanzo.desktop/library.db` is the quickest way to
find out what the decks actually hold — and what analysis those records have,
which is often the reason a feature appears not to work here.

**A guessed verb is a rule that silently covers nothing.** §53's pad count
listed `hotcue_set`, `sample_play` and `loop_recall` from memory; the
vocabulary's actual spellings are `hotcue`, `sampler`, `slice` and `roll`, so a
DDJ-SR covered in fifty-six pads read as having none — and §53 would have given
the on-screen pad zone room on the one controller that least needs it. The fix
is the rule `dj_app::tiers` already had: a test walks both exception lists and
fails on a verb `dj_core::vocabulary` does not have. Any list of verb strings
outside the vocabulary needs one.

**A fourth table has now been found read by nobody**: `dj_hid::feedback`'s
`FeedbackMap`. It parses `[[feedback]]` blocks, several shipped mappings have
them, and no other module consults it — so djmanzo never lights a controller.
§53 reports the count *and* says nothing sends them, rather than letting a
number imply otherwise. If you wire it, `out.rs` is the sender and it needs
hardware to verify.

**A confirmation can be destroyed by the thing it confirms.** §54's presets
reported what they had just applied, in the Settings block the DJ pressed them
in — and applying one opens its arrangement, which does not include Settings.
The line was gone before it could be read. It passed in the browser harness
because the stub's cockpit closes nothing, which is the shape of test that lies:
green, specific, and about a state the application never reaches. The fix was to
delete it; the list shown *before* the press is the whole promise and nothing
takes that away. Before writing an "it worked" message, ask what the action does
to the surface the message is on.

**A token can exist, be correct, and be asked for by nobody.** §30's fourteen
roles were defined, tested against `app.css`, and almost unused: thirty CSS
rules painted a state with `--accent` directly. Nothing was visibly wrong,
because `--active` *is* the accent — the meaning was simply not expressed, so a
palette could not move one role without moving the brand colour with it. The
tell is a token that only ever appears in the file that defines it. When you
add a semantic layer, add the rule that makes something ask for it in the same
commit.

**A rule that cannot fire is worse than no rule.** §58's row named a gap —
the tiers do not gate what a phase may promote — and building it would have
been a rule about nothing: every budget whose `room_for` is narrower than
tier 4 also has `reflow: false`, so promotion is already forbidden there.
Before implementing a gap a status row names, find the input that reaches it.

**A test of an ordering needs an input the old order gets wrong.** §58's
palette ranking was first tested with the query `e`, and deleting the sort
entirely left it green: the passes that generate the list already run verbs,
then surfaces, then interface operations, which is roughly §58's order anyway.
Four queries do interleave — `ss`, `sa`, `st`, `rs` — because a preparation
*verb* (`grid_*`) is generated before a performable one (`crossfader`) and a
performable *surface* (`stems`) after both. Before trusting a sort, find the
input that is wrong without it; and note that an ordering test cannot see a
single mis-ranked item that still happens to land in order, which is why the
tiers are also asserted directly.

**`e2e_fixture.rs` compares keys one level down, and `attention` was not one
of the levels.** The shape guard walks the snapshot, one deck and the master —
deliberately, because going deeper compares `Option` fields that are present or
absent by state. That left every nested always-present object unguarded, and a
field added to `Attention` reached the browser fixture-less with every gate
green. It is checked now, on the same criterion the other three meet: never an
`Option`, never absent. If you add a field to one of the other nested types,
ask which side of that line it falls on.

**A feature that crosses both halves of the renderer needs a test that says
so.** §25's layers are drawn in two places — seven as elements in the
interface, three rasterised in Rust and carried in the tile URL — so a layer
can be offered in the picker and gated in neither, and the box would tick and
nothing would happen. A Rust test reads `Waveform.svelte` and `Overview.svelte`
and fails when a choosable layer has no `showing("…")` guard, plus a separate
one over `gridSlug`. Without them the picker could go fully decorative with
every gate green.

**Anything in a tile URL is part of a one-year immutable cache key.** The
theme was there for that reason and §25's grid layers are there now for the
same one: a setting that changes the pixels and not the URL is a setting a DJ
will report as broken, because the browser keeps serving what it already has.

**Do not recapture `snapshot.json` on different audio to add one field.** The
file is the baseline every layout budget is measured against, and a recapture
moves every number in it — track lengths, loudness, which budget the engine was
in. Adding `room_for` by hand was right; what makes it safe is a test that the
fixture's attention block equals one of the four budgets djmanzo can actually
be in, so a hand-written value cannot be a guess. Recapture when the *shape* of
a deck or the master changes; hand-add when one constant gained a field.

**A default answer added to `e2e/shell.ts` overrides the one already there.**
`ANSWERS` is one object literal, so a second `library_search:` key silently
replaces the first — and the fixture that had two records became one, which
failed five specs that had nothing to do with the change. Search the table for
the command before adding an answer for it; the right move is usually to extend
the rows that are there with the fields djmanzo now sends, because the harness
is supposed to send the shape the application really sends.

**A test that reads a source file must normalise line endings.** CI runs the
suite on Windows, where git checks the repository out with CRLF, so a scan for
`"\n}\n"` — a closing brace at column zero — finds nothing in a file whose lines
end `\r\n`. §87's guard failed there and only there, reporting that `put_on_deck`
had moved when it had not. The house-pattern tests that came before it all
happen to search for `"\n}"`, which *is* a substring of `"\r\n}"`, which is why
this took until the twelfth one to bite. Every source-reading test in `dj-app`
now does `.replace("\r\n", "\n")` on the way in; the next one should too.

**`setViewportSize` returning is not the resize handler having run.** The
density follows the window on the window's own `resize` event, so a test that
sets the size and reads `--density` on the next line passes alone and fails
about one run in ten under the full parallel suite. `expect.poll` for the change
you expect; a plain sleep is right only where the assertion is that *nothing*
happens, because then there is no condition to wait for.

**Naming a thing is not the same as being in it.** §7's save writes the
arrangement into the DJ's own collection *and* stores it as the current
workspace, and the second half was missing at first: the picker showed the new
name, and after a restart it read the shipped name it had been saved from. A DJ
being told they are in an arrangement they are not. Every test was green — it
was found by restarting the application, which is the check a browser test
cannot make.

**A socket that reaches the engine has not reached the application.** The
network protocol dispatched straight at `ActionBus`, which is *not* where an
action finishes: `deck 1 eject` has to clear the deck's name and its analysis,
which live in `dj_app`, and `record on` has to open a file, which the engine
cannot do at all. So a socket ejected the audio and left the header showing the
record, and answered "accepted" to a recording it had not started. Anything that
takes actions from outside goes through `commands::perform`, which is why
`dj_net::Carry` is a required argument rather than an optional one: the way this
breaks is a caller forgetting it.

**A seam tested only in the type it expects cannot see a verb that is not one.**
djmanzo's grammar is wider than `Action` — `load deck 1 <track-id>` is in every
session file and `Action::parse` has never accepted it. `dj_net::Carry` first
took an `Action`, so dj-net parsed each line before the host saw it and refused,
at its own door, the one verb that had just been added for every origin to
share. Both sides of the seam were green, because every test on both sides was
asking about actions. It was found by opening the port and sending the line.
When a boundary converts, test it with something that does not convert.

**A polled panel is not a stream of offers.** Every panel here refreshes on a
timer — the Next rail, the mission bar, the assistant. Anything that counts what
the DJ was *shown* has to count the thing rather than the answers: §43's fatigue
treats re-offering the same records as one offer, because counting each poll
would reach its twenty in about a minute of nobody doing anything, and the
assistant would go quiet at a DJ who had never looked at it. The same shape is
waiting in anything that counts notices, warnings or suggestions.

**A machine that goes quiet without saying so reads as broken.** §43's
instruction is *do not spam*, and the obvious implementation — offer less, say
nothing — gets reported as a bug, because a DJ whose rail has thinned cannot
tell whether djmanzo took the hint, crashed, or ran out of library. Anything
that reduces itself in response to the DJ needs a sentence saying what it
counted and how to undo it. `Fatigue::says` is the pattern.

**A gate in the interface is not a gate.** §78's locks are honoured in
`App.svelte` for the four things the shell does to itself — the phase promotion,
the density fitting, the theme adaptation, the audio-reactive properties — and
that is the right place for those, because the shell is what does them. The
assistant's path is not one of them: `ui_request` applies the arrangement in
Rust, stores it to `workspace.json`, and *then* emits `cockpit` to the window.
A permit consulted when that event arrives is consulted after the DJ's screen
has moved and the change has been written. If a rule has to hold against the
assistant, it goes where the assistant acts.

**Do not hand-write a parameter the engine owns while the engine is running.**
`Engine` republishes a block of deck state into the `ParameterRegistry` on every
callback — `LoopActive`, `LoopStart`, `LoopEnd` among them, derived from
`deck.active_loop()`. A test that opens a device and then writes those slots is
writing to a mirror: a deck with no decoded track has no loop, so the next tick
puts zeros back. `commands::persistence_tests` did exactly that, passed on every
platform for months, and then failed once on the Windows runner with a loop
stored as starting at frame 0 — because `save_loop` checks `LoopActive` before
it reads `LoopStart`, and the tick landed between the two. The fix is not a
sleep or a retry: those tests exercise persistence and never needed a device, so
they do not open one. If a test needs the engine to have a loop, give the deck a
loop; if it needs a figure in the registry, do not also start the writer that
owns it. The probe that settles it in ten seconds:

```rust
registry.set(ParamId::Deck(deck, DeckParam::LoopStart), 480_000.0);
let now = registry.get(...);          // 480000
std::thread::sleep(Duration::from_millis(120));
let later = registry.get(...);        // 0, with a device open
```

**A component that restyles `.active` inherits half of a pair.** `app.css` sets
`button.active { background: var(--accent-2); color: var(--on-accent) }` — a
fill and a foreground written together, as §30 asks. A scoped rule in a
component outranks it, so `.ladder button { color: inherit }` and `.arcs
button.active { color: var(--accent) }` each kept the fill and replaced the
text: 2.4:1 and 1.47:1, on the chosen posture and the chosen arc. Both were
invisible to `svelte-check`, to the hex-literal test (they name tokens) and to
the role test (the roles are all defined). If you restate one half of a colour
pair in a component, restate the other.

**A hover rule outranks a state rule.** `button:hover:not(:disabled)` is two
pseudo-classes; `.active` is one class. The sheet already knew this for plain
buttons and says so — but `IconButton` had its own `:hover` and no `.active:hover`,
so putting the pointer on the *open panel's* button repainted it in the dark
hover fill and left the near-black `--on-accent` text on it. 1.23:1, on the one
control whose state you are checking. Any component with both a `:hover` and an
`.active` needs the third rule.

**The theme's palette is inline, so a media query cannot beat it.** §31's
packages set their tokens as inline custom properties on the root element, which
outrank every normal rule in every sheet. A `@media (prefers-contrast: more)`
block that simply redeclares `--text-dim` matches, applies, and is overwritten a
layer up on all seven packages — green tests, nothing on screen. `!important`
on the declaration is the only lever a stylesheet has there, and it is what
`app.css`'s high-contrast block uses.

**An accessibility audit only sees what it can open.** `ui/e2e/access.spec.ts`
runs axe-core over the shell and the fourteen panels the rail can open in a
container. It cannot open the ones that need a MIDI device, a `.clap` on disk or
a sound card — so the six nameless `select`s it found were six of twenty-four.
`ui/src/access.test.ts` reads the source instead: it knows nothing about
contrast or computed roles, and it reaches every component. Both, not either.

**A fifth table was found read by nobody, and a fourth copy was found written
by hand.** The pattern is now the single most reliable source of defects here.
`Night.svelte` carried all eighteen strings of §81's six occasions — slug, title
and blurb — beside `dj_app::setting::Setting`, which already owned them and
which every other part of djmanzo files a night under. Nothing was wrong on
screen, and nothing would have been wrong on screen after a seventh occasion was
added either: the panel would simply have gone on offering six, and a list that
is merely short looks exactly like a list that is right. It is read off Rust
now, through a `night_settings` command, with a golden fixture so the browser
stub cannot become the copy one file further out. **Look for this shape
first in any new work: a table in Rust and a list in Svelte that agree today.**

**A test that scans the interface for hard-coded knowledge has to be narrow to
survive.** §16 ends *do not hard-code this logic into UI components*, and the
obvious test — no Svelte file names a genre family or a technique — is wrong in
both directions. Technique names are ordinary control labels (`SYNC`, `Censor`,
`Brake`, a `cut` transition) and a scan for them fails on four files that are
doing nothing wrong. Genre families are safe, but only as *exact string
literals*: a comment explaining that bachata turns over on the phrase is
documentation, and a search placeholder reading "bachata with a piano hook" is
an example of what to type. A test that failed on those would be turned off
within a week, and a test that is turned off guards nothing. The one in
`pack.rs` scans for `"family"`, `'family'` and `` `family` `` only.

**`rnb` renders as `mb`.** The rn/m collision, in every proportional face the
interface wears. It sat in `dj_core::genre` for a long time and reached the
screen the first time a picker printed a family list — a genre nobody plays,
in an interface nobody had looked at closely. These names are shown to people,
not just matched against tags, so the family is `r&b` now with `rnb` kept as an
alias, and `genre.rs` has a test that bans a bare `rn` in any family name.
`normalise` reads `&` as *and*, so both spellings still match a ripper's tag.

**The container gets slow enough to look like a bug.** With three app instances
and a `cargo build` running, djmanzo showed *Interface running at 4 fps* and
every list in the Settings surface was empty — setups, packs, screens, locks —
while the polled panels kept updating. It read exactly like a broken `invoke`
bridge. It was the promises not having resolved yet; ten seconds later every
list was there. Check `uptime` and `pgrep -f target/debug/djmanzo` before
diagnosing an empty list, and kill stray instances: `pkill` returns exit 144
and aborts a compound command, so run it alone, then confirm with `pgrep`.

**Three copies of one guard were each asking half the question.** §31's mood
table, §7's arrangements and §54's setups all name theme ids, and each had its
own test that read `packages.ts` and grepped for `id: "…"`. Every one of them
asked *does this id exist* and none could ask *is every theme there is supposed
to be here* — so six of §32's sixteen shipped and nothing anywhere said which
ten were missing, and a package added to the interface that Rust never named
was invisible. `dj_app::theme` is the table, the three greps are one check, and
it runs in **both directions**. If you find two tests reading the same foreign
file, that is the shape: the duplication is the symptom, and the missing
direction is the defect.

**Copy written in a doc-comment idiom reaches the screen as asterisks.** A
`why_not` string in `theme.rs` said *built \*around\* them*, three lines below
a doc comment where that emphasis is correct, and the picker drew the asterisks
because a picker draws text. Nothing could catch it: no type-check and no
browser test reads copy, and the fixture scan that would have caught it did not
exist. It does now — `nothing_a_dj_reads_is_written_in_markup` in `theme.rs`
holds that table, and every fixture under `ui/e2e/` was scanned for the same
thing and is clean. **When a table's strings are shown rather than documented,
test for the notation**, because the surrounding file will teach you to write
it wrong.

**An empty list in a browser test can be a missing stub rather than a bug.**
§32 made the watershed reachable from the theme picker, and the first browser
run of that threw `Cannot read properties of null (reading 'entities')` — the
harness had no `world` answer, because until then nothing in a test had a way
to open the watershed. That was a harness gap, but it exposed a real one too:
`App` and `Detached` both read `world.entities` from a value they trusted
`getWorld()` never to answer `null` for, with a `catch` on the very next line
showing the author already expected that read to be able to fail. Both take
`?? emptyWorld()` now.

**When the running application disagrees with a green browser test, instrument
before theorising.** §8's level axis stored correctly in Rust — `level.txt` and
`workspace.json` both held the press — and the picker went on showing *You have
not set one*, while the Playwright test over the same gesture passed. Half an
hour went into hypotheses about remounts, `$state` and prop plumbing. What
settled it in two minutes was one temporary line in the markup printing the
component's own state:

```svelte
<p class="hint">DEBUG stand={JSON.stringify(stand)} levels={levels.length}</p>
```

It showed the state was correct all along, and a clean rebuild-and-relaunch
reproduced nothing. I could not establish which bundle that earlier window was
serving — the working directory was correct and the hashes should have differed
— so treat it as unexplained rather than as a fixed defect. The lesson that does
generalise: **a screenshot cannot tell you what a component holds**, and a
readout that can costs one line and one rebuild. Remove it before committing;
`grep -c DEBUG` on the file is the check.

**A `pkill -f` in this container returns exit 144 and kills the shell**, so a
compound command stops there and the rest never runs — including the relaunch.
Run it alone, or use `pgrep -f … | xargs -r kill -9`, and confirm with `pgrep`
before launching, or you end up driving an old process and reading its window.

**A house-pattern test that greps for an identifier is satisfied by the
import.** §48's guard read `source.contains("roomPollMs")` against
`RoomSense.svelte`, and a mutation replacing the actual call with a hard-coded
`Promise.resolve(2000)` survived it — the import line and the doc comment still
spelled the name. It now looks for `roomPollMs(tier)` and for
`performance.resolved`, and both mutations die. **When you scan a foreign file
for a call, scan for the call**: the name alone appears in at least two places
that are not it, and a guard a dead call site satisfies is not a guard. This is
the first of these tests in the repo that a mutation got past, which is also the
argument for running the mutation every time rather than only when the test
looks weak.

**This container runs the interface at Eco.** The frame-rate governor really
does step down here — *Currently rendering at Eco mode* is what the Performance
panel says under Xvfb with no GPU — so §48's savings are exercised rather than
merely configured when you drive it. That is a genuinely useful accident: a
laptop-mode change can be seen working here in a way most adaptive behaviour
cannot. It also means any measurement taken from a driven session is a
worst-case one, and should be reported that way.

**Generated copy gets its grammar wrong where nothing automated can see it.**
§80's first sentence came out as *"You like a Ultra Dense layout."* — the
density names are the interface's own and capitalised, so the article has to be
chosen per name — and its evidence line read *"every kind of night djmanzo has
enough of … 1 of them"*, which is true and reads like a machine counting. Both
were found in the first minute of driving it, and neither a type-check nor a
browser test could have: **nothing automated reads copy**. Two lessons that
generalise: shape a generated sentence so no article is needed rather than
choosing one, and **test the n=1 case**, because a profile needs three nights of
a kind before that kind exists at all, so every DJ's first persona claim rests
on exactly one kind of night and the plural wording is what they see first.

**Adding to a panel can push a neighbour into failing its accessibility audit.**
§80's block made the Assistant panel tall enough that its conversation region
started overflowing, and axe's `scrollable-region-focusable` fired — correctly,
and about a defect that was always there. Two `.log` elements exist (`App.svelte`
and `Assistant.svelte`) and the first fix went to the wrong one. Both now carry
`tabindex="0"`, a `role="region"` and a name.

That fix needs `<!-- svelte-ignore a11y_no_noninteractive_tabindex -->`, which is
two rules disagreeing rather than a shortcut: Svelte's heuristic is right in
general and has no way to know the element scrolls, and a focusable scroll
container is the documented fix for the axe rule. The `role` and the name are
what stop it being a bare focusable div. **Note also that `svelte-check` exits 0
on warnings**, so a warning does not fail the gate and has to be looked at.

**The MCP servers can drop mid-session.** Both the GitHub and Claude Code Remote
servers timed out and reconnected during this session. Plain `git push` is
unaffected — it does not go through MCP — so if CI cannot be dispatched, push
anyway and dispatch when the server is back rather than holding the work.

**A counting allocator shared across parallel tests measures the wrong thing.**
§90's snapshot ratchet copied `dj-engine/tests/rt_safety.rs`'s harness, where
the counter is a process-wide `AtomicUsize` and only the *watching* flag is
thread-local. Cargo runs the tests in a file on separate threads at once, so
each measurement included whatever the others were allocating: the same capture
read 52 alone and 86 in parallel. **The counter has to be thread-local too**,
and it is in `snapshot_budget.rs`. `rt_safety.rs` has the same shape and has not
been seen to misreport — it asserts *zero*, so cross-talk would make it fail
rather than pass quietly — but if it ever flakes, this is why.

**Write the determinism test first; it earns its place immediately.** The flake
above was found by the test that asserts the figure is the same every time, and
found again as a too-short warm-up (`[94, 52, 52, …]` — something behind the
registry initialises lazily and not all in one call). Both would otherwise have
shipped as a ratchet that failed at random, which is the kind of test that gets
turned off. The shape that works: **warm, measure a tail, assert the tail is
flat** — and make the failure message distinguish the two causes, because a
short warm-up shows one high figure at the front and something growing per call
shows a rising tail.

**Set a ratchet against a measured regression, not against a feeling.** The
first draft of `SNAPSHOT_ALLOCATIONS` was 600 against a real 52; the slack test
caught that. The second was 65, and the mutation it exists to catch — one
`format!` per deck and a `Vec`, thirteen allocations — slid underneath. A
ratchet is only worth the headroom it denies: mutate first, then pick the
number.

**"djmanzo cannot see this" is a claim, and it needs checking like any other.**
§80's fourth trait shipped with a `why_not` saying the log recorded that a DJ
reached for the stems and never which stem. That was wrong — `DeckAction::Stem`
carries the stem and always has. What collapses all four into one is §14's
*gesture* vocabulary, deliberately (*six EQ verbs are one gesture a DJ would
name*), and that rule is right for §14 and wrong for §80. The correct move was
not to change §14 but to **read the actions directly**, which is what
`persona::stems_for` does. Before writing a row that says djmanzo cannot see
something, go and look at the action rather than at the derived gesture: the
derivation is where information gets thrown away, and it throws it away on
purpose.

**A "what is left" clause can be wrong, and building it would have done harm.**
§79's row said the waveform lock should pin the layers a DJ chose. Nothing
automatic changes the layers — the DJ's own picker and their own preset press,
and nothing else — and §78's four bullets are each about an *automatic* or
*surprise* change, while §79's own sentence is that the AI may use the
underlying state *without rearranging presentation*. Building the claimed
behaviour would have made a preset refuse a change the DJ deliberately pressed,
breaking the contract §7 and §54 both state. **Before building a named gap, read
the section rather than the row**: the row is a summary somebody wrote, and this
one had drifted into asking for the opposite of what the directive says.

**Two tables that each carry a number will fight over it.** §5B lets an
arrangement name a deck composition, and a `Layout` carries its own deck count
and density — so applying one silently overwrote both. `Laptop Compact` states
Ultra Dense and the `Performance` composition it names states 0.85. The rule
that settles it: a composition is *what a deck is made of*; how many there are
and how tightly they are packed stay the arrangement's. An existing browser test
caught it, which is the argument for running the whole suite rather than the new
file.

**`npm run build` failing makes a mutation test lie, and it is easy to miss.**
A mutation that removed a call left `densityOf` unused, `svelte-check` failed,
`vite build` never ran, and the browser suite passed against the previous
bundle — reported as "the mutation survived". `CLAUDE.md` warns about exactly
this and it still happened, because the failure is one line above a wall of
passing tests. **Confirm `✓ built` in the mutation run itself**, not only in the
gate run; a mutation that does not compile has to be rewritten so it does.

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

11. **§8 is closed, and the shape of how is worth copying.** Level 1 was the
   nine things djmanzo remembers; Levels 2 to 6 were each already real under
   another section's name, and what was missing was the *level* — one control
   that says how far djmanzo may go. `dj_app::level` is it, and three decisions
   in it are the ones to reuse elsewhere:

   **It maps onto §10 rather than beside it.** §10 says its six postures are
   the main autonomy axis and asks that they not be replaced, so a level *sets*
   a posture. Two pairs of levels share one, and that is stated rather than
   smoothed over — a test fails if any step becomes indistinguishable from its
   neighbour, because an axis with a dead step has fewer notches than it claims.

   **It sets and does not own.** One press writes the posture and all six of
   §79's locks and then leaves them alone, and `standing` reports what has since
   drifted. An axis that owned six switches would be the axis arguing with the
   switches. Any future "one control for many" should copy that pair: write
   them, then be able to say what changed.

   **§9 is enforced rather than assumed.** A test asserts that every question a
   level answers takes no reading, so there is no certainty it could consult.
   The tempting version of that table is one where a confident djmanzo quietly
   acts a level higher, which is §9's *low confidence + high autonomy =
   invalid/unsafe* arriving through the back door.

   **The waveform row closed itself.** §8 asks djmanzo to remember the
   *preferred waveform display*, and the row said "not yet, and here is what
   has to happen first" until §25's layer inventory became a picker. That is
   the posture to copy: a list of eight would have read as the whole of §8, and
   naming the blocker on screen is what made it obvious which section to do
   next.

12. **§5B's compositions ship, and the trap is the one a golden file cannot
   see.** An arrangement can name a deck composition and six of the twenty-three
   do; `Layout` gained `jog` (pixels, 48..=320) and `stems_open`, which
   `from_layout` puts on `deck.jog`'s `size` prop and `deck.stems`'s `open`
   prop, and `Scratch` and `Stem Performance` ship as compositions.

   **A prop is the one thing in this format that can be set, resolved, stored
   in a golden file and still reach nothing.** The tree would be right, the
   snapshot would be right, `compositions.json` would be right, and the deck
   would look exactly as it did before — every test in the workspace agreeing
   with a defect. So `widgets::tests::a_prop_the_upconversion_sets_is_a_prop_the_deck_reads`
   reads `Deck.svelte` and fails when a prop `from_layout` sets is not read
   there in one of two spellings. **If you add a prop to a deck placement, that
   test is the one that will stop you**, and the fix is a line in the renderer,
   not an arm in the test.

   **The default was 5rem, which is 70 and not 80.** `.jog-row` said
   `--jog-size: 5rem` and this interface's root font size is
   `calc(14px * var(--density))`, so the wheel a DJ has been looking at is 70 px
   at density 1. Declaring 80 as "the default" was a ten-pixel growth on every
   deck in the application, and it was `density.spec.ts` — a deck no longer
   fitting its window — that said so, three gates after the change looked
   finished. A number lifted out of a stylesheet has to be *measured*, not
   rounded: `5rem` is only 80 if the rem base is 16.

   **The supervisory mode was a mapping, not a feature.** §5B's autopilot mode
   names six things to watch -- current, next, transition, room response,
   automation state, emergency takeover -- and every one of them already
   existed. The work was deciding which panel carries which, writing that
   decision where it can be argued with (`cockpit::tests::the_autopilot_
   arrangement_shows_what_5b_asks_a_supervisor_to_watch`) rather than leaving
   it implicit in a list of three surface names, and taking the booth *out*:
   its own `about` says "set up once a night rather than reached for during a
   mix", which is the opposite of what a supervisor has open. When a directive
   section reads as a feature, check first whether it is a mapping; the ones
   that are cost a table and a test, not a panel.

   **A surface can be declared and drawn by nobody.** `cockpit::surfaces()`
   declares `transition` and `App.svelte`'s `DRAWN` does not list it, so a
   preset placing it would be filtered out in silence --
   `every_preset_places_only_surfaces_the_shell_draws` is why no preset does.
   That is why §5B's *transition* is carried by the assistant here. If you
   build that panel, the supervisory arrangement is where it belongs.

12b. **The assistant had no idea what was happening, and nothing said so.**
   `commands::ask` built a system prompt out of the action vocabulary, added
   the DJ's sentence, and sent that. No deck, no tempo, no key, no position.
   Every test passed, every answer parsed, and the panel looked exactly as it
   does now — because what was missing was *context*, and nothing in a test
   suite notices an answer that was merely uninformed.

   **The shape of the fix is the part to copy.** `dj_app::sight::ALL` is §40's
   own list of twenty-six, in §40's order and words, each with a JSON pointer
   into the snapshot or a reason it is not gathered, and `brief` turns a
   snapshot into the lines the model is handed. Three tests hold it together:
   every gathered pointer resolves against a real capture (`ui/e2e/snapshot.json`,
   not a fresh registry — a fresh one has nothing loaded, so every
   `/analysis/...` pointer would resolve to nothing and the test would pass by
   never looking); every item the table calls gathered appears in the briefing;
   and every absence carries more than thirty characters of reason.

   **The unseen half is on screen on purpose.** Seven of the twenty-six are
   not gathered, and the assistant panel lists them with the reason beside
   each. A DJ deciding whether to trust an answer needs "it could not see your
   history" far more than it needs the reassuring list. Gathering one is a
   `Held` variant and a match arm -- the compiler asks for the arm -- plus a
   read in `dj_app::assistant::context_lines`; the panel, the fixture and the
   browser test then follow by themselves, which is how four of the original
   eleven were closed in the commit after the first.

   **Absence is said out loud rather than left out.** `loop state none` costs
   two tokens and "there is no loop" and "nobody told me about the loop" are
   different answers to "get out of the loop". A list that was *cut* says so too
   -- `(the last 12 of 40)` -- because a briefing silently truncated reads as a
   night with twelve actions in it.

12c. **Two things can differ by name and not by sight, and a test that
   checks the name will say they differ.** `Role::must_differ_from` names the
   pairs a DJ has to tell apart -- the mix pair, the four stems, who-did-it,
   the severity ladder, selected against active -- and
   `every_role_has_a_colour_and_the_pairs_that_must_differ_do` checks that each
   pair points at *different tokens*. It passed for months while the organic
   palette's light variant defined `--accent` as `#0f7b57` and `--accent-2` as
   `#057a5f`: two names, one green. Selected and active were one colour. The
   assistant and the room were one colour. The vocal stem and the other stem
   were one colour.

   **The fix is to measure the thing the DJ sees.**
   `the_pairs_that_must_differ_differ_to_the_eye_in_every_palette` resolves
   role -> token -> the hex each palette gives it, and computes CIE76 ΔE in
   Lab. A WCAG contrast ratio is the wrong instrument: it is about legibility
   of text, and pure red and pure blue have nearly the same luminance. The
   floor is 20, which is about "obviously a different colour in a glance at a
   small swatch" and which the palettes that were right already cleared
   comfortably. Five themes needed repairing.

   The general form: when a rule is about what somebody perceives, a test on
   the representation is a proxy, and a proxy passes in exactly the cases worth
   catching. Ask what instrument measures the actual claim.

13. **§90 has one ratchet and four honest refusals, and the split is the
   lesson.** A ratchet is only worth having where the number means the same
   thing on two machines. Frame rate, memory and CPU do not — the argument §89
   already makes about screenshot baselines — and an xrun count measured against
   a null device is always nought, so a ratchet on it would pass for the wrong
   reason. Allocation counts are the exception, and that is where the one real
   ratchet went: the 60 Hz snapshot, held at 56 over six decks.

   **If you add a field to the snapshot, that test is the one that will stop
   you**, and it is meant to: it runs sixty times a second on the thread that
   also serves the engine's controls. Raise `regress::SNAPSHOT_ALLOCATIONS`
   deliberately and say in the commit what the field is worth, or take the cost
   back out. Lower it whenever the real figure drops.

   What is left: memory and worker utilization are not measured at all.
   Instrumenting the decoder and analyser threads is its own piece of work, and
   a resident-set figure on a container sharing a page cache with a build is
   noise — which is why the allocation count is the part of "memory" that
   means the same thing twice.

14. **§80 is the template for "learned, and therefore arguable".** §13's
   tendencies and §81's profiles both learn and both are constructor-enforced;
   what neither did was let a DJ disagree, and that is half of what §80 asks
   for. `persona::Verdict` is the answer the DJ owns, and a rejection has to be
   both halves — not acted on **and** not raised again. Either alone is a
   rejection that did not take.

   The register is the other half worth copying. §13 deliberately writes
   observations ("you often sweep the filter when the night is peaking") and
   never preferences, because a preference learned from four gestures is a
   claim djmanzo cannot support. §80 asks for the preference register, and it
   is only honest with an answer attached — which is why the label on screen is
   §80's own words, *Learned preference*. **If you find yourself writing a
   sentence about a DJ in the second register, the answer buttons are part of
   the feature, not a follow-up.**

   All four traits ship. The fourth was briefly written off as underivable —
   see the note above about checking that kind of claim — and reads the actions
   rather than §14's gestures, which stay coarse on purpose.

15. **§48 is closed, and it is the template for "a priority nobody can see".**
   The frame rate had been measured for a long time and the tier was consulted
   by the theme pipeline alone, so §48's closing sentence — AUDIO > CONTROL >
   VISUAL EFFECTS, never the reverse — existed only as prose. `thrift::Band` is
   that sentence as a type whose derived ordering *is* the priority, and three
   tests hold the table to it.

   Two things there generalise. **Put the ordering in the type**, so the test is
   a comparison rather than a list somebody maintains. And **show it**: the
   panel lists all seven with the three djmanzo keeps carrying their reasons,
   because a DJ who sees a tier name and nothing about what it cost them has no
   way to trust the claim. A priority that is enforced and invisible is one
   nobody will believe the first time their laptop stutters.

16. **§32's remaining palettes are content, not format.** The architecture was
   always there — a package is a palette, a geometry generator, behaviours and
   effects, through one pipeline — and what shipped is the table that knows
   which themes there are supposed to be. Eight of §32's sixteen are palettes
   somebody has to design, each named on screen with its reason, and two more
   are deliberate refusals: *High Contrast* is an override the stylesheet
   already applies over every theme from the operating system's own setting,
   and *Minimal* is a density §5 already fits to the window. Adding one of the
   eight is a palette in `colors.ts`, a package in `packages.ts` and a row in
   `dj_app::theme`; the tests will tell you if you miss any of the three.

   **The watershed is a theme now, and the shape of that is worth copying.**
   §32 asks for the metaphor as one identity among others and, in the same
   sentence, that it must not constrain a DJ who does not want it. So choosing
   Watershed Living opens the world and nothing else touches the switch: the
   theme is a starting point, not a mode. Where a feature has an *and it must
   not* clause, build the prohibition as its own assertion — the browser test
   here presses Booth first, precisely to prove that half.

17. **§16's remaining packs are content, not format.** The format ships:
   `dj_assistant::pack::Pack` selects from the genre map, the technique
   catalogue and §81's occasions rather than restating any of them, and
   `coach::next_lesson` teaches inside the chosen one. Eight of §16's thirty
   are written, and the twenty-two that are not are deliberately absent: a
   *Karaoke / MC* pack asserting a technique list nobody checked is a
   curriculum that teaches the wrong thing with confidence, which is worse than
   an honest gap. Adding one is cheap and is the right work for somebody who
   actually does that kind of night; inventing one from the directive's section
   heading is not.

   Seven of §16's fifteen fields still have no table to select from — energy
   curves, allowed BPM relationships, suggestion weighting — and the rule that
   got this section right applies to each: build the table where it belongs
   first, then let a pack *name* it. A pack that grew its own copy of a curve
   would be the fifth duplicated table this project has had to remove.

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
