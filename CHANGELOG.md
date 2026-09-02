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

## v0.10.0 — Find a record by humming it, and a layout that is measured

**Find a record from what you remember** is finished. The hum is now compared
*as a melody* against a stored pitch contour for every record, not only read for
its key and tempo: ten pitch points a second found with YIN, matched on
octave-folded intervals so the key it was hummed in does not matter, and located
with subsequence DTW so the answer is *where in the record*. It still does not
name a record you do not own, and the panel says so beside the button.

**A layout is a tree of named widgets** (ADR-0008, W1 and W2). Thirty-three
named widgets with their slots, settings and ranges; a layout is a tree of them
in JSON; existing flat layouts upconvert on load. A skin may set twenty-three
design tokens and each value is checked against the shape that token takes, so
a layout stays data and cannot become a program.

**The first screen is measured, not remembered.** A browser test opens the
interface at djmanzo's own 1280x800 and reads where the controls actually land,
against a snapshot captured from the running application. It found the master
strip's second row below the fold — now one row instead of two — and a split-cue
button drawn on top of the output meters. It also found that the crossfader is
still below the fold with records loaded, which is recorded as a failing test
rather than a fixed problem.


## v0.8.0 — Timecode vinyl you can switch on, and controllers that work

**Timecode vinyl became reachable.** `dj-dvs` could decode a control record and
`Command::SetTimecode` could install one, and nothing in the application could
send that command. Now there is an input picker, a relative/absolute switch and
a live calibration reading in Settings, and `write_timecode_signal` renders
djmanzo's own control signal to a WAV — burn it to a CD or play it off a phone
and any turntable or CD deck drives a deck, without buying a record.

The reading distinguishes three states from one number, and has to: negative is
"not on a record", zero is "on one and hearing nothing" — a dead cartridge, a
lifted needle, the wrong input — and above that is reading.

**Real controller mappings**, transcribed from Pioneer's own MIDI message
lists: DDJ-SR, CDJ-3000, DDJ-200, and a family file covering DDJ-400, DDJ-FLX4,
DDJ-FLX2 and DDJ-SB3. None has been run against the hardware, and each says so
in its first paragraph.

Two things the mapping format could not previously express, both of which would
have shipped inside those files:

- **14-bit faders.** Every Pioneer, Denon and Native Instruments fader arrives
  as two control changes. Binding the high byte alone put a pitch fader on 128
  steps — 0.125% each, audible when beatmatching.
- **Centred jog wheels.** A platter reports movement, not position. Read as a
  fader, its centre landed a hair above zero and drove the deck forwards with
  nobody touching it.

**And the bug that hid all of it.** Mapping selection took the first file whose
`device` appeared in the port name, and `generic-2-deck` claims `"MIDI"` —
which is in nearly every ALSA port name. Every controller in the world was
handed to the generic mapping.

Also fixed: changing the audio output device left the microphone and every
control record running into rings belonging to a discarded engine. The
microphone went silently dead on a reconnect while still holding a sound card
open. `NullBackend` gained an input device, which is why it could be tested at
all.

## v0.1.0 — Beta: a playable instrument

The first build worth downloading. M0 through M5 are substantially complete and
the controller layer (M4) has arrived, which is what turns a set of panels into
something a DJ can actually play.

**These builds are unsigned and un-notarised.** macOS will refuse to open the
app until you right-click it and choose Open, once. See
[QUICKSTART.md](docs/QUICKSTART.md).

### Added

- **The keyboard as a controller.** Not a pile of shortcuts — the same
  vocabulary, the same file format and the same validation as a MIDI mapping,
  so a laptop with nothing plugged into it is a playable instrument. 76 keys,
  laid out so the two hands mirror each other, with a live sheet that lights a
  key while it is held. Keys are named by physical position, so the layout
  holds on an AZERTY or QWERTZ keyboard.
- **MIDI controllers.** `dj-hid`: a mapping engine over the action bus, TOML
  mapping files, 7-bit and 14-bit controls, all three encoder conventions, and
  bundled mappings so a fresh install works with nothing configured. Every
  action in a file is checked when the file loads, so a typo is a message when
  you choose the mapping rather than a control that silently does nothing an
  hour into a set.
- **Performance**: slip mode, reverse, censor, loop roll, the slicer, brake and
  backspin, a pad zone with seven pages, a four-bank sampler with recording
  into a slot, and an FX rack of three chained slots per deck and on the master
  with eleven effects and chain presets. Two, four or six decks.
- **Library**: SQLite collection with playlists, crates, smart folders, play
  history, duplicate detection, and importers for rekordbox, Serato, Traktor
  and iTunes — including the cue and grid tags written into the audio files.
- **Analysis**: BPM, beat grid, key and loudness, with grid editing by shift,
  scale and tap.
- **Recording to disk**: the master, post-limiter, into a 16-bit WAV.
- **Music sources**: Spotify, YouTube, YouTube Music, Jamendo and the Internet
  Archive, each with an honest account of what it does and does not permit.
- **The assistant** and preset packs, both speaking the same action vocabulary
  as everything else — so their work is auditable, reversible and replayable.

### Changed

- **The sound card opens on launch** and the choice is remembered. Waiting to
  be told to connect meant loading a track and pressing play did nothing, with
  no visible reason.
- **A first run offers to scan your music folder** in one click rather than
  opening a file dialog onto a folder whose location you did not choose.
- `Action::parse` refuses trailing words. `deck 1 volume 0.5 extra` used to
  parse as `deck 1 volume 0.5`, quietly swallowing the typo.

### Fixed

- Encoder direction. The convention was guessed from the byte, which meant a DJ
  turning an *absolute* encoder down from 60 to 30 got a beat jump *forward* —
  30 is a position below centre to one convention and thirty clicks clockwise
  to another. Mappings now declare which their hardware sends.

### Known limits

- Unsigned builds on both platforms; signing needs an Apple Developer ID.
- No HID, no controller feedback (LEDs, displays), no motorised platters yet.
- Neural stems, CLAP hosting, Pro DJ Link and StagelinQ are designed but not
  built.
- The waveform will not scroll smoothly on a machine without hardware-accelerated
  compositing. The interface says so rather than looking broken; the audio
  engine is unaffected.
- **Nothing here has been through a real gig.** That is what a beta is.

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
