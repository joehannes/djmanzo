# djmanzo

A VirtualDJ-class DJ application: a Rust workspace of twenty crates behind a
Svelte 5 / Tauri 2 interface. Work happens on `main`; no pull request unless
one is asked for.

**Read [`docs/HANDOFF.md`](docs/HANDOFF.md) before doing anything else.** It is
written for a session arriving with no context and names what to read, how the
project is worked on, and the traps that have each cost a session. The rules
below are the ones that must be in mind from the first turn; the reasoning for
all of them is in that file and in `docs/adr/`.

## Non-negotiable

- **No GPL or AGPL code** (ADR-0002). Every dependency's licence goes in
  `docs/RESEARCH.md`. This is why Mixxx's controller mappings are unusable.
- **The audio thread never allocates, locks, does I/O or blocks.** Servers,
  decoders and analysers live on their own threads.
- **No model identifier** in a commit message, code comment, document, or
  anything else that is pushed.
- **`docs/DIRECTIVE.md` is what the owner asked for.** Where an implementation
  and that file disagree, that file wins. Its framing governs: keep the
  application and enhance it, do not coldly rewrite it.

## Before every commit

```
cargo fmt --all
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
cargo test --workspace --all-targets
cd ui && npx svelte-check && npx vitest run && npx playwright test
```

Then **mutation-test the load-bearing new test**: break the code it claims to
cover and confirm it fails. If it still passes the test is wrong — strengthen
it, and say so.

## Run the interface; do not only type-check it

Every serious defect found recently was invisible to `svelte-check` and obvious
within a minute of driving the application. Under Xvfb:

```
Xvfb :95 -screen 0 1400x900x24 &
DISPLAY=:95 DJMANZO_NULL_AUDIO=1 DJMANZO_DEMO=/path/to/some/audio \
  WEBKIT_DISABLE_COMPOSITING_MODE=1 ./target/debug/djmanzo &
DISPLAY=:95 import -window root shot.png     # then actually look at it
```

**`npm run build` fails silently in a pipeline.** It is `svelte-check && vite
build`, so a type error leaves the *previous* bundle in `dist/` and a browser
test then passes against stale code. Always confirm `✓ built`.

Playwright needs `DJMANZO_CHROMIUM=/opt/pw-browsers/chromium` where a browser
is pre-installed rather than downloaded.

## Reporting

Report measurements honestly, and revert changes whose benefit cannot be
demonstrated. This container has no audio device, microphone, camera or phone:
nothing here can prove any of it *sounds* right, and nothing should claim to.
