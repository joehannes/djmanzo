# Resume — where the karaoke work stands (16 October 2026)

Read `CLAUDE.md` and `docs/HANDOFF.md` first; this file is only what was in
flight when the session paused. **Another session may be working on this
repository at the same time: fetch `main` and check before building on top.**

## Shipped today

| Commit | What |
|---|---|
| v0.43.0 (`535af79`) | djmanzo Karaoke for Intel Macs back to macOS 10.13; released, every platform green |
| v0.44.0 (`3163829`) | No model offered where nothing can listen; LRCLIB as served; released |
| `f40687f` | Singers panel in the order a host reads it |
| `e5bbfbf` | §118: *Rehearse it* sets the move up in the Practice lab |
| `2ef3a07` | A withheld lesson says what is holding it (`cockpit::Quiet`) |
| `5e883b2` | **Karaoke build listens wherever the Mac can**: `dj-whisper` crate + `djmanzo-listen` helper built for 10.15, `dj_app::listening` (probe, Settings → Speech recognition: Automatic / Off / Check again), release checks the helper |
| this commit | Song for a singer knows the night (`guests::with_night`: occasion, circumstances, host and host line from the press kit); Windows fix for the karaoke config test |

**Not released yet:** everything after v0.44.0. A v0.45.0 release gives the
friend's Mac the listening helper. To release: bump `version` in
`Cargo.toml` and `crates/dj-app/tauri.conf.json`, put the Unreleased
changelog entries under `## v0.45.0 — …`, push, wait for CI green, then
dispatch `release.yml` on `main` with `platforms: all`, `tag: v0.45.0`.
Check the *macOS Karaoke* job's new step "carries its speech recognition
helper" — it has never run on a real release yet.

**CI:** `5e883b2` was red on Windows only (the karaoke config test compared
`djmanzo-listen.exe` with the config's `binaries/djmanzo-listen`); fixed in
this commit. **The full gates did not run on this commit** (packed in a
hurry before a usage limit): its Rust tests for the changed code pass, but
run every gate in `CLAUDE.md` and watch CI first.

## Research answered (16 October)

- **Suno has no official public API.** platform.suno.com is a login wall; a
  partner developer API was announced 1 July 2026 for a curated group
  (Typeform intake). Terms ban automated access and commercial use of output
  not downloaded through an approved channel. Resellers (sunoapi.org etc.)
  are out. So the built flow stays: djmanzo drafts lyrics + style, opens
  suno.com in its own window, the DJ makes the song and pastes the link.
- **WhatsApp:** `wa.me/<digits>?text=…` needs no setup and is what djmanzo
  uses (`guests::compose`, consent-gated). Sending without a press needs the
  WhatsApp Business Cloud API (Meta business account, phone number ID,
  token, approved templates, recipient opt-in, billed per message). Only
  worth building as an optional advanced setting.

## Since this file was written — read this first

The karaoke requests below were then planned as two parallel sessions in
[docs/karaoke_beta_release.md](docs/karaoke_beta_release.md), with the
owner's words as DIRECTIVE.md §126–§129, and **items 1 and 2 below are
built** (session A, branch `claude/vigilant-tesla-xdqpmf`): a playlist
drawer under every deck with *Next when this ends* and *Repeat* (a filler
deck), one deck slim and the other given the room, plus a one-line top bar
and a ⚙ board of every source. Items 4–7 are session B's
(`claude/karaoke-beta-session-b`). What each session did and left is in
`docs/karaoke-beta/session-a.md` and `session-b.md`; the plan's §6 is how
they are merged. The list below is kept as it was written.

## Open karaoke requests from the owner (as written before the plan)

The owner asked for these "in time for the late-afternoon karaoke version";
order is the suggested one.

1. **A playlist drawer under each deck.** A stylish strip at the bottom of
   every deck widget that, on click, extends into that deck's playlist:
   choose a playlist, see its rows, load next, play the next when this one
   ends, repeat. Builds on `dj_library` playlists and the break-music code
   (`crates/dj-app/src/breaks.rs`), which already plays a playlist on a deck
   between singers.
2. **Karaoke layout: one deck big, one small.** One deck configured as the
   karaoke deck (large, its playlist visible), the other minimised to a strip
   as the filler deck, playing default jingles / filler items on repeat.
   Check whether workspace surfaces' `collapsed` or deck compositions can
   express a per-deck size before inventing one.
3. **Playlist feature, extended "in prudent and integral ways":** per-deck
   queue, auto-advance, repeat/loop, shuffle, history, M3U import/export —
   map what `dj_library::playlist` and the commands already do first.
4. **More themes / skins / effects for the app, with a keen eye on the
   Caribbean.** A theme is a palette in both modes + a package in
   `ui/src/controls/themes/packages.ts` + a row in `dj_app::theme`; the
   contrast and must-differ tests hold them (Spotlight failed the must-differ
   test once — expect it).
5. **Crowd emoji on the singers' screen, restrained.** Guests tap a few
   emoji on the room's phone page (server-rendered, no JS:
   `crates/dj-net/src/front.rs` routes, `page.rs`, a host switch in
   `stage.rs` `Rules`, off by default). The screen
   (`ui/src/SingerScreen.svelte`, foot area) shows them aggregated with
   counts in a corner, no motion beyond a fade, never over the words. Also
   record them as `crowd::Reaction { source: Room }`.
6. **Karaoke vocal effects:** presets beyond Singer/Soft/Loud/Mc/Instrument
   (`crates/dj-vocal/src/strip.rs` `Preset`, ~455–550) — slapback, stadium,
   radio — and an echo in time with the deck's tempo (`space.rs` `Echo`).
7. **Lyric styles for the singers' screen:** size and wipe colour (the wipe
   is fixed to `--stem-vocal`, `SingerScreen.svelte` ~581–595).
8. **Visuals:** Butterchurn (WebGL MilkDrop) already runs behind the words
   (`ui/src/milkdrop.ts`, `Visuals.svelte`, two presets in
   `crates/dj-app/src/looks.rs`); more presets need their licences checked
   first.

## Known gaps left behind

- *Rehearse it* does not reach a Practice lab popped out into its own window.
- Nothing has run the karaoke build on a real 10.13 Mac.

## Environment notes (this container)

- Disk fills up: `rm -rf target/debug/incremental` frees 6–14 GB; run gates
  with `CARGO_INCREMENTAL=0`. A full disk crashes Chromium mid-suite.
- The dashboard's `device group … one row` browser test fails here on any
  code (fonts) and passes in CI.
- Drive the app with a `--features custom-protocol` build. A build with
  `--no-default-features` beside `target/debug/djmanzo-listen` drives the
  karaoke listening path on Linux.
- A whisper Tiny model can be downloaded (pinned checksum) to
  `~/.local/share/app.djmanzo.desktop/models/whisper/`; `apt-get install
  espeak-ng` makes synthetic speech (GPL, test audio only, never shipped).
- Mutation scripts must restore files with a fresh mtime (`os.utime`), or
  cargo keeps the mutant binary.
