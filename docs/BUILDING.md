# Building djmanzo

Two ways: let CI do it, or do it on the machine in front of you.

## What you need

| | |
|---|---|
| **Rust** | stable, via [rustup](https://rustup.rs) |
| **Node** | 22 or later |
| **macOS** | Xcode command line tools: `xcode-select --install` |
| **Debian / Ubuntu** | the GTK and WebKit stack, below |

```sh
# Debian, Ubuntu, Xubuntu
sudo apt-get install -y \
  libasound2-dev libclang-dev libwebkit2gtk-4.1-dev libgtk-3-dev \
  librsvg2-dev libsoup-3.0-dev libssl-dev libxdo-dev \
  libayatana-appindicator3-dev
```

`libclang` is for bindgen, which builds the keylock bindings. `libasound2` is
ALSA. The rest is what Tauri links against.

## In CI

Push a tag whose patch component is zero and the release workflow builds every
target — macOS on both architectures, Linux and Windows — and opens a draft
release:

```sh
git tag -a v0.1.0 -m "..." && git push origin v0.1.0
```

To get installable packages **without** cutting a release, run the workflow by
hand: *Actions → Release → Run workflow*, with **platforms: all**. The bundles
land on that run's own page as artifacts. `platforms: linux` skips the two
macOS builds when a Debian package is all you want.

> **Note on cost.** GitHub-hosted runners are free and unlimited for *public*
> repositories. For private ones they are metered, and macOS bills at ten times
> the Linux rate — a 2,000-minute allowance is 200 macOS minutes. A private
> repository with the allowance spent fails every job in about two seconds with
> no runner assigned, which looks like a broken workflow and is not one.

## On the machine in front of you

```sh
cargo install tauri-cli --version '^2'   # once
npm install                              # once; installs the root tauri wrapper

npx tauri build:deb       # Linux: target/release/bundle/deb/*.deb
npx tauri build:appimage  # Linux: target/release/bundle/appimage/*.AppImage
npx tauri build:dmg       # macOS: target/release/bundle/dmg/*.dmg

# The wrapper runs `cargo tauri` from crates/dj-app, so the paths in
# tauri.conf.json stay correct while commands are launched from the root.
```

The bundles appear under `target/release/bundle/`:

| Platform | What you get |
|---|---|
| macOS | `macos/djmanzo.app` and `dmg/djmanzo_<version>_<arch>.dmg` |
| Debian / Ubuntu | `deb/djmanzo_<version>_amd64.deb` and `appimage/…AppImage` |
| Windows | `msi/djmanzo_<version>_x64_en-US.msi` and `nsis/…-setup.exe` |

**Windows is built and tested, but has never been run.** CI compiles it and the
test suite passes there; nobody has opened the application on a Windows machine,
and the audio backend has had no attention beyond what `cpal` gives for free.
Treat those bundles as untried. macOS and Debian are the two djmanzo claims.

The root wrapper expands those aliases to `cargo tauri build --bundles ...`.
Any extra arguments pass through, for example `npx tauri build:deb --debug`.

### Building for the other Mac architecture

An Apple Silicon machine builds an arm64 bundle by default. For an Intel one:

```sh
rustup target add x86_64-apple-darwin
cargo tauri build --target x86_64-apple-darwin
```

There is no universal binary in the release workflow. Two separate builds keep
each download half the size, and a DJ knows which Mac they have.

## The one thing a plain `cargo build --release` gets wrong

**`cargo build --release` alone does not produce a shippable binary.**

Tauri computes `dev = !custom-protocol`. Without that feature the interface is
loaded from `devUrl` — `http://localhost:5173` — instead of from the bundle in
`ui/dist`, so a packaged application opens on

    Could not connect to localhost: Connection refused

The feature is declared in `crates/dj-app/Cargo.toml` and is deliberately kept
out of `default`, because `tauri dev` wants the dev server and its hot reload.
`cargo tauri build` turns it on itself, which is why the commands above and CI
are fine — a hand-rolled `cargo build` is not:

```sh
( cd ui && npm ci && npm run build )     # tauri-build needs the bundle first
cargo build --release --bin djmanzo --features dj-app/custom-protocol
```

To check a binary you already have, **run it**: a working build shows the
mixer, a broken one shows the connection error. There is no way to tell from
`strings`, because Tauri stores the embedded interface compressed and keeps
`devUrl` in the embedded config either way.

## Signing

The builds are **unsigned and un-notarised**. macOS refuses to open an unsigned
app on first launch until you right-click it and choose *Open*, once.

Signing needs an Apple Developer ID, which is an account question rather than a
code one. When there is one, `tauri-action` takes `APPLE_CERTIFICATE`,
`APPLE_SIGNING_IDENTITY` and the notarisation credentials as repository
secrets, and nothing in the workflow needs to change beyond adding them.

## Running from source

```sh
npm --prefix ui ci
cd crates/dj-app && cargo tauri dev
```

Two environment variables are worth knowing:

| | |
|---|---|
| `DJMANZO_NULL_AUDIO=1` | a null audio backend, for a machine with no sound card |
| `DJMANZO_DEMO=/path/to/track.wav` | load a track and open a device on launch |

## The tests

```sh
cargo test --workspace --all-targets     # 1,739 tests
npm --prefix ui test                     # 66 tests
cargo clippy --workspace --all-targets   # clean, with -D warnings
```

`crates/dj-engine/tests/rt_safety.rs` is the one to keep green above all
others: it runs the audio path under an allocation-counting global allocator
and fails if anything on that thread allocates. See
[ARCHITECTURE.md](ARCHITECTURE.md).
