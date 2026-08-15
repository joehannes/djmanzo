# Changelog

Versioning follows semver, with one project-specific convention:

> **Minor and major tags produce release builds. Patch tags do not.**
>
> `.github/workflows/release.yml` triggers on the tag pattern `v[0-9]+.[0-9]+.0`
> — a semver tag whose patch component is `0` is by definition a minor or major
> bump. So `v0.2.0` and `v1.0.0` build installers; `v0.2.1` is recorded in git
> and stays quiet.
>
> Patch tags are for marking incremental work as it lands. Minor tags are for
> when a milestone completes and there is something worth downloading.

---

## Unreleased

## v0.0.2 — Headphone cue routing

M1 continues. No release build (patch tag, by design).

### Added
- **Headphone cue (PFL)** — per-deck cue send, cue/master blend, split cue, and
  a booth output with independent level.
- `BusLayout` derives master/booth/cue channel assignments from the device's
  channel count: 2 channels is master only, 4 adds cue, 6 adds booth.
- The audio host now opens **four channels when the device has them**, so cue
  works on the controller interfaces that support it.
- Release workflow and this changelog.

### Changed
- Deck gain staging split into **trim** and **fader** stages so the cue send is
  genuinely pre-fader — you can cue a track with its channel fader all the way
  down, which is the entire reason PFL exists.
- Decks report both pre-fader and post-fader peak levels; they answer different
  questions (what to set trim by, versus what reaches the master).

### Verified
- The cue bus never reaches the master, tested directly — previewing a track
  must never be audible to the room.
- Cue, split-cue and booth paths all proven allocation-free on the audio thread.

## v0.0.1 — Foundations

- **M0 walking skeleton**: seven crates, Tauri 2 shell, Svelte 5 UI, CI on macOS
  arm64 and Ubuntu. Realtime engine with an action bus, lock-free parameter
  registry, and `Arc` retirement so track buffers are never freed on the audio
  thread.
- **Isolator EQ and filter sweep**: Linkwitz-Riley crossovers give a true band
  kill rather than a deep shelf; single-knob filter with a bit-exact bypass.
- **`dj-secrets`**: API keys in the OS keychain, never a config file.
- **Band-limited centre cancellation**: karaoke vocal removal that keeps the
  centred kick and bass.
- Design docs for the assistant (A1–A6) and karaoke (K1–K2) tracks.
