# Karaoke beta release — tonight

Written 16 October 2026 for a karaoke night the same evening. It holds
three things: what is already built, what can be built today (in **two
sessions that run at the same time on two different AIs**), and what
cannot be done today and why. The owner's words for these requests are
in [DIRECTIVE.md §126 and §127](DIRECTIVE.md).

**Read §4 before writing any code.** It is the rule that lets the two
sessions finish separately and merge without conflicts.

---

## 1. Already built (on `main`, or on the base branch below)

| Area | What a host has tonight |
|---|---|
| Taking the voice out | *As recorded · Guide · Out* per deck: the vocal stem where the record is separated, band-limited centre cancel where it is not yet |
| Words | From the file's tags, a `.lrc` beside it, or LRCLIB; timed by listening (whisper.cpp) where the machine can; the singers' screen with the line wiped in time, word by word where timed, next line, count-in, next singer |
| Singers' screen | Cover / Cover Art Archive / the record's colours behind the words; WebGL visuals answering the beat, the stems and the microphones; MilkDrop/Butterchurn presets; the words always render even if every visual fails |
| The rotation | Sign-ups by hand or from phones (QR), key and guide proposals, *Ready*, drag to reorder, *Load on N* in the singer's key, *Bring them in* at the next phrase, *Sang* |
| Microphones | One strip per input, each with its own chain: low cut, gate, pitch correction, EQ, compressor, de-esser, echo, room; presets *Singer / Soft / Loud / MC / Instrument*; a shared singers' monitor on outputs 7–8; **a monitor for each singer on 10+ outputs** (commit `78eeaab`, this branch) |
| Between singers | Break music from a playlist, chosen to lead into the next song, faded in and out; *Mix out of each song* |
| Room games | Scored singing and *Tonight's best*; versus teams; the room's applause as the verdict; the quiz from the collection, answered on phones |
| A singer's range | Measured on a strip, songs suggested with the key change each needs |
| Guest book | Every field asked for, consent per use; a 15-second voice take; a recording of their song with its own consent; a song brief (lyrics and style) for Suno from their record; WhatsApp links prepared for the host to send |
| Crowd | Comments and requests placed on the music (§119); host-approved photos on the singers' screen; a ticker and a line of the host's own |
| Look | Themes *Caribbean, Latin, Beach Sunset, Karaoke Neon, Spotlight, Velvet* and nineteen more; boards *Karaoke* and *Karaoke prep*; a *Karaoke night* preset pack |
| Older Macs | *djmanzo Karaoke* for Intel Macs back to 10.13, downloaded as `djmanzo.Karaoke_<version>_x64.dmg` |
| Playlists | Folders, smart playlists, drag and drop, reorder, history, the Next rail, the set plan, crates |

**Not in any download yet:** everything after v0.44.0 — the karaoke build
listening wherever the Mac can (`5e883b2`), a monitor for each singer
(`78eeaab`), and everything below. See §6.

---

## 2. Session A — playlists on the decks, and a karaoke deck layout

Run **here** (branch `claude/vigilant-tesla-xdqpmf`). Milestones in order;
each is committed and pushed when its gates are green, so the night has
whatever is finished.

| # | Milestone | Done when |
|---|---|---|
| **A1** | **A playlist drawer under each deck.** A tab at the bottom of every deck opens a drawer: the playlist chosen for that deck, its records in order, the one playing marked, a press loads one onto *this* deck. Remembered per deck. | Rust holds the per-deck queue (`dj_app::deck_queue`), the drawer draws it; a Rust test of the queue, a browser test of the drawer, driven in the app |
| **A2** | **Auto-advance and repeat.** Per deck: *Next when this ends* loads (and plays) the queue's next record when the deck runs out; *Repeat* starts the queue over. That makes a deck a filler deck: jingles and fillers going round. | Rust decides what comes next; a test of the end-of-record step; driven with two short records |
| **A3** | **Karaoke focus: one deck slim, the other big.** A press on a deck makes it a slim strip — title, play/pause, level, its drawer tab — and gives the other deck the room, its drawer open. A deck can be named the *filler* deck (A2 on, repeat on) from the same place. | Interface state remembered per deck; a browser test of the layout at 1280×800; driven |
| **A4** | Playlist extras, if time: running length and the key/BPM flow of a playlist in its header; *Shuffle* in the drawer | Each its own small commit |

## 3. Session B — the singers' screen, the room, themes, a song for the singer

Run **on the other AI**, on a branch of its own taken from the base commit
in §5. Same rule: each milestone committed and pushed when green.

| # | Milestone | Done when |
|---|---|---|
| **B1** | **Caribbean themes**: three or four, e.g. *Malecón Night*, *Carnaval* (vejigante colours), *Bioluminescent Bay*, *Bachata Rosa*. Each a row in `theme::ALL` and a palette that passes §30's contrast tests. | The theme tests pass with them in; each seen in the app |
| **B2** | **Emoji from the room on the singers' screen, held back.** A row of emoji on the room's phone page; each appears small, in a corner away from the words, fades within seconds; a cap per second and per phone; a host switch, off by default. Never over the line being sung. | Rust rate-limits and carries them; a browser test that they never overlap the lyric lines; driven |
| **B3** | **Lyric visuals**: a bar showing how long an instrumental break lasts; a soft glow on the word being sung; a colour per singer in a duet (from the strip they sing on). | Browser tests; driven with a timed `.lrc` |
| **B4** | **Karaoke vocal presets**: *Stadium*, *Radio voice*, *Big echo*, *Duet*, as more `dj_vocal::Preset`s — chain settings only. | `dj-vocal` tests; the presets in the strip's menu |
| **B5** | **A song for the singer, sent.** The song brief also draws on the venue, the night's genre and the DJ's branding (`kit`, `brand.rs`); one press copies lyrics and style for Suno's window; *Send their song*: the host pastes the suno.com link and djmanzo opens WhatsApp to that guest (only with their contact consent) with a personal message ready. | Rust builds the brief and the `wa.me` address; tests of both; driven up to the WhatsApp window |

## 4. How the two sessions stay out of each other's way

**Base.** Both start from the commit that adds this file on
`claude/vigilant-tesla-xdqpmf` (see §5). Session A keeps working on that
branch; session B makes its own branch from the same commit.

**File ownership.** A file in one session's column is not edited by the
other. A file in neither column may be edited by neither — if a session
finds it needs one, it writes that in its session notes for §6 instead.

| Session A owns | Session B owns |
|---|---|
| `ui/src/Deck.svelte` and any new `ui/src/Deck*.svelte` / `ui/src/deck*.ts` | `ui/src/SingerScreen.svelte`, `ui/src/Singers.svelte` |
| `ui/src/App.svelte` | `crates/dj-app/src/theme.rs`, `ui/src/controls/themes/` (`packages.ts`, `colors.ts`), `ui/src/theme.svelte.ts`, `ui/src/themes.test.ts`, `ui/src/theme-tokens.test.ts` |
| `ui/src/Library.svelte`, `ui/src/Crates.svelte` | `crates/dj-vocal/` (presets) |
| new `crates/dj-app/src/deck_queue.rs` | `crates/dj-app/src/guests.rs`, `share.rs`, `kit.rs`, `brand.rs` |
| `crates/dj-library/` (playlist queries) | `crates/dj-net/` (the room's page, emoji) |
| `crates/dj-app/src/snapshot.rs`, `ui/e2e/snapshot.json` — **only A changes the snapshot** | new `crates/dj-app/src/reactions.rs` (or similar) for emoji |
| `crates/dj-core/src/action.rs`, `vocabulary.rs` — **only A adds verbs** | `crates/dj-assistant/` (the song brief) |

**Shared files with a block each.** Two files every session must touch to
register new code. Each has two marked blocks far apart; a session adds
lines **only inside its own block**:

- `crates/dj-app/src/lib.rs` — the module list (`// karaoke beta,
  session A …` near the top, `… session B …` near the bottom) and the
  command list in `generate_handler!` (A's block at the top, B's at the
  bottom).

**Brand-new files** never conflict: new modules, new components, new
`ui/src/*.ts` files for each session's calls into Rust (rather than
growing `api.ts`), new `ui/e2e/*.spec.ts` files. Prefer them.

**Tests that need Rust's answers** pass them in the test's own `openShell(page, …, answers)`
— the stub answers any command from that table — so neither session
edits `ui/e2e/shell.ts`.

**Nobody edits, until §6:** `CHANGELOG.md`, `docs/DIRECTIVE-STATUS.md`,
`docs/FEATURES.md`, `docs/ROADMAP.md`, `docs/HANDOFF.md`,
`docs/KARAOKE.md`, `docs/KARAOKE-NIGHT.md`, version numbers, `Cargo.toml`
/ `Cargo.lock` / `package.json` / `package-lock.json` (so: **no new
dependencies**). Each session writes what those files should say — its
changelog lines, its status rows, its host-page lines — in its own notes
file: [`karaoke-beta/session-a.md`](karaoke-beta/session-a.md) or
[`karaoke-beta/session-b.md`](karaoke-beta/session-b.md).

**Gates.** Every commit passes the five in `CLAUDE.md`, with the
mutation test of the new test, and is driven in the app where it shows.

## 5. Base, branches and the order of merging

- **Base commit:** the commit that adds this file, on
  `claude/vigilant-tesla-xdqpmf` (its parent is `78eeaab`, on top of
  `main`'s `5e883b2`).
- **Session A:** `claude/vigilant-tesla-xdqpmf`.
- **Session B:** any new branch from the base commit.
- **Merging:** either may merge into `main` first; the second merges
  cleanly by §4. Then §6.

## 6. After both: folding in and the release

One short step, by whichever session finishes last:

1. Merge both into `main` (or `main` into the later one, then fast-forward).
2. Fold both notes files into `CHANGELOG.md` (under *Unreleased*),
   `docs/DIRECTIVE-STATUS.md` (§126, §127), `docs/FEATURES.md`,
   `docs/KARAOKE.md`, `docs/KARAOKE-NIGHT.md` and `docs/HANDOFF.md`.
3. Run every gate on the merged tree; drive the app once.
4. **Release v0.45.0 — only with the owner's word.** Bump
   `Cargo.toml`, `crates/dj-app/tauri.conf.json` and `ui/package.json`
   together; dispatch `.github/workflows/release.yml` with `tag: v0.45.0`
   (a pushed tag is refused from an agent session). The release builds
   every platform and the *macOS Karaoke (Intel, 10.13+)* build; allow the
   better part of an hour, so start it well before the night.

## 7. Not doable today, and why

| Asked for | Why not today |
|---|---|
| **Suno through an official API** | Suno has no public API. It runs a partner intake with no launch date ([MBW](https://www.musicbusinessworldwide.com/suno-explores-developer-api-seeking-apps-that-unlock-experiences-generative-music-makes-possible-for-the-first-time/)). Every "Suno API" offered elsewhere reverse-engineers the web app, against Suno's terms, and accounts are suspended for it. Built instead: the brief, the Suno window, and B5's copy and send. djmanzo fills Suno's form the day its API opens. |
| **Sending to WhatsApp automatically** | Needs WhatsApp's Business Cloud API: a Meta business account, a verified sending number, templates approved in advance for any message the business starts, per-message pricing, and each guest's opt-in. None of it can be set up in an afternoon, and it is the owner's account to open. Today: `wa.me` with the message ready, the host presses send (A6's rule: djmanzo prepares, a person sends). |
| **Duet parts** (lead apart from backing vocals) | Needs a separation model whose licence ADR-0002 admits; none has been read yet. |
| **Phones as microphones** | A phone's browser opens its microphone only on a secure page; the room page is plain HTTP on the LAN. Needs the DJ's own domain and certificate, or an app. |
| **Restart the verse; the queue by voice** | Left out on purpose (KARAOKE.md §4): djmanzo does not know where a verse starts, and a name heard in a loud room is the likeliest thing to come out wrong. |
| **Anything heard** | No session here has speakers, a microphone, a ten-output interface or a 10.13 Mac. Try the build on the real laptop and interface before the guests arrive. |
