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

**The window under Xvfb is 1280×800 whatever the screen is,** and there is no
window manager to drag it. That is the right size to *judge* the interface at,
and the wrong one for looking at a docked surface's contents — a card grid in
the bottom dock gets about 230 px. `xdotool search --name djmanzo` returns
**two** window ids; sizing the first does nothing, because the one carrying the
webview is the second. `xdotool windowsize <second id> 1560 1150` on a
1600×1200 screen gives the dock room to be read. Do not conclude a surface is
broken because it is cut off at 800 px; check what the window actually is.

**`pkill -f` matches the shell running it.** `pkill -f "target/debug/djmanzo"`
kills your own tool call, because the pattern is in that process's command
line. Bracket a character — `pkill -f "[d]ebug/djmanzo"` — or kill by pid.

**Playwright's browser.** CI installs its own. A container that pre-installs
one is pointed at it with `DJMANZO_CHROMIUM=/opt/pw-browsers/chromium`,
otherwise every test fails with "Executable doesn't exist".

**Two copies of the density band table.** Rust owns it in
`dj_app::cockpit::BANDS`; `ui/e2e/shell.ts` carries a copy because a Playwright
stub cannot call into Rust. They drifted once, and the sweep that exists to
catch a clipped deck spent a session measuring an application that no longer
existed. A Rust test now reads the TypeScript file and fails when they
disagree — do not delete it.

**The band floors are a fact about the top bar, not only about the deck.**
Adding one destination button pushed that row onto another wrapped line and
took 40 px out of every stage at every window height. Do not guess the
correction: `ui/e2e/density.spec.ts` prints, for each window height, how much
the deck needs and how much it has, and the floor is where that reaches zero.

**A deck's frames are the record's, not the device's.** `DeckParam::Position`
and `LengthFrames` are counted in the *file's* frames — the engine resamples on
the way out — so anything turning them into seconds divides by
`DeckParam::SourceRate`. Dividing by the device rate showed a 2:30 record at
44.1 kHz as 2:17 on a 48 kHz device, for as long as the snapshot has existed.
Nothing could see it: every test used one rate for both.

**A fixture the application cannot produce tests nothing.** The autopilot's
mixing tests were green for months against a `Situation` with an idle deck and
a staged record — a pair the function that assembles it could not return, so
the autopilot had never mixed in the running application. When a test builds
its own input, check that the thing which builds it in production can produce
that shape.

**Driving a gesture: measure and act in one motion.** A waveform lane scrolls
while the record plays, so a mark's screen position is stale seconds after the
screenshot it was measured in. Two drags "failed" that way before the cause was
found — and the first guess was a WebKitGTK bug in Svelte's event delegation,
which would have been a wrong fix written into a comment for the next reader.
Find the target and grab it in the same script.

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

The largest open sections, in the order they are worth doing:

1. **§11, the context engine.** The night half ships: `dj_app::context` reads
   the phase and the energy off the records that have been played, publishes
   them on every snapshot, and says how sure it is and why. What remains is the
   *unification* §11 is really asking for — the occasion, the hardware, the
   audience, the DJ's own behaviour, the attention budget and the performance
   health are still six separate types nobody assembles, so §9, §12, §14 and
   §17 still each reach for their own. Phase 5 of `GUI-OVERHAUL.md`.
2. **§25–§27, the waveform as instrumentation.** The transition is drawn on
   the outgoing lane and can be **dragged** there — §26's first two examples.
   What is left is the rest of §25's semantic layers (vocal and stem presence,
   breakdowns, drops, the energy trajectory), the other things §26 wants
   grabbable (cue markers, phrase markers, loops), and §27's ghost track, which
   needs a second render of a track that is not loaded.
3. **§68's last reader.** The object ships; the pair view draws it, the automix
   performs it and the autopilot defers to it. Replay still reconstructs a
   transition from the action log rather than from the object. Its stem, EQ and
   FX plans are absent because nothing yet decides them; a field that is always
   empty is a promise.
4. **§20's remaining half.** Three of the four views ship — the performance
   table, Set Flow, the pair view and now the compact cards, with the sleeve
   read out of the file's own tags and served over a `cover://` scheme. What is
   left is the table's wider column set (§20 lists twenty columns and offers
   instant custom configuration; the browser draws six and cannot be
   reconfigured), and the two card actions nothing behind them exists for:
   **preview** needs somewhere to listen that is not a deck, and **queue**
   needs a play queue this application does not have — the Sidelist is a
   different statement. Both are features, not wiring.
5. **§74, the contextual rail.**

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
