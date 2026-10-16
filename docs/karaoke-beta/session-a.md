# Karaoke beta — session A notes

What session A built, for folding into the shared documents once both
sessions are merged ([karaoke_beta_release.md §6](../karaoke_beta_release.md#6-after-both-folding-in-and-the-release)).
Only session A edits this file.

## Changelog lines (for CHANGELOG.md, under Unreleased)

- **A playlist under every deck.** A tab on each deck's bottom edge opens
  that deck's playlist: choose one of your lists, press a record to load it
  on that deck, or ▶ to load and play it. The one on the deck and the one
  next are marked, with the list's running time.
- **A deck that goes on by itself — a filler deck.** In the same drawer,
  *Next when this ends* loads and plays the next record when the deck runs
  out, and *Repeat* goes round again: jingles and fillers on one deck while
  the other carries the singers. A deck you pause is left alone. Each deck's
  choice is kept for next time.
- **A one-line top bar.** The bar at the top is one line now; on a smaller
  screen its buttons become icons, and ⋯ shows the night's readings and the
  guides when you want them.
- **⚙ Sources and connections.** One press shows everything djmanzo can
  find, play, buy, fetch lyrics or covers from, what works now, and where
  to set up the rest.
- **One deck slim, the other big.** ⇤ in a deck's header makes it a slim
  strip — the record, the time left, play — and gives the other deck the
  room, its playlist open. *Full size* puts both back.

## Status (for DIRECTIVE-STATUS.md §126 / §127, FEATURES.md, KARAOKE.md)

- **A1 + A2 built** (§127, "a stylish GUI extension at the bottom of each
  turntable/mixer widget, that on click extends to a playlist for that
  mixer" and "the other one with default jingles/playlist items that can be
  repeated as fillers"): `dj_app::deck_queue` — a playlist per deck, where
  the deck is in it (`at`), *advance* and *repeat*, kept in
  `deck_queue.json`; `next` and `ran_out` as plain functions; a tick on the
  snapshot pump beside break music, the record decoded off it.
  `DeckDrawer.svelte` draws it over the deck's foot (it moves nothing on the
  deck). Lists only — a smart list or a folder is not offered.
  Held by 3 Rust tests (4 mutants fail them) and a browser test (5 mutants,
  one of which survived first: a drawer back in the deck's flow passed a
  test measuring the deck's height, which is its row's, not its content's;
  the test now measures the deck's foot). Driven in WebKitGTK: deck 1's
  record ran out and the queue loaded and played the next; the drawer named
  the list, its records, the next one. The drive found two faults, fixed:
  the select lost its value when the lists arrived after the queue, and a
  record taken again (same title) left the drawer stale — it now asks every
  two seconds while open, and "on the deck" needs the deck's own title too.
- **A3 built**: with two decks, ⇤ in a deck's header makes it a slim strip
  (its record, the time left, play/pause, *Full size*, its playlist tab) and
  the other deck takes the width and opens its playlist
  (`deckFocus.svelte.ts`, remembered in the window's storage). Held by a
  browser test (3 mutants fail it); driven in WebKitGTK.
- **A5 built** (§128, "make the navbar a one-liner ... auto-condense the
  items into symbol only buttons if they don't fit"): the top bar is one
  line, fitted by measuring — full words; then icons only (the words stay
  as names, visually hidden, so screen readers, tests and voice still find
  "Browse"); then the night's readings and Guides fold behind ⋯, which
  shows them again on demand; only if even that does not fit does the bar
  wrap, and then it folds nothing. djmanzo's own bar (no toolbars, §117) is
  one line at 1400, 1280 and 1024 wide. **The full toolbars (§117's
  opt-in) keep their rows**: folding their Activities, Workspace and Layout
  away would hide what that mode is for, and §109's and §115's tests caught
  exactly that — the decks moving when the activity strip or a proposal
  came in. Held by `e2e/navbar.spec.ts` (3 mutants fail it); the whole
  browser suite's last full run: 868 passed, 1 failed (§115's proposer, then fixed and confirmed with 84 layout tests and `whisper.spec.ts`; no second full run), with `whisper.spec.ts`'s too-long sentence
  made a little longer: the bar's slimmer padding gave it just enough room
  to fit, which was the one thing that test assumes it cannot do).
- **A6 built** (§128, "a cogwheel in the navbar and ... a configuration
  dashboard ... for connecting sources"): ⚙ in the bar opens *Sources and
  connections* — every source by job (finding songs, playing them, buying
  them, lyrics, songs like this one, covers and artists), whether it works
  now, and *Set up →* to Settings. `dj_app::sources_board` builds it from
  the catalog's live status, `dj_sources::stores`, the karaoke sources
  (the record's words, LRCLIB, Cover Art Archive, the Next rail) and
  whether an AI provider has a key. 2 Rust tests (4 mutants fail them,
  one after a fix: whether a key is set is now its own function, tested
  on an in-memory store) and a browser test (1 mutant). Not yet: *Set up →*
  opens Settings at its top, not at the section.
- Not yet: A4 (shuffle, BPM/key flow), A7 (finding a song a singer cannot
  name — see the plan; `dj_app::memory` already does words, a description
  and a hum by key and tempo).

## For the host (KARAOKE-NIGHT.md)

**A filler deck.** Put your jingles and fillers in a playlist. Under the
deck you want them on, press the *Playlist* tab, choose it, and tick *Next
when this ends* and *Repeat*: when one ends the next starts, round and
round. A press on any record in the drawer loads it on that deck; ▶ loads
and plays it.

## For the next session (HANDOFF.md)

- The drawer can only be driven so far under Xvfb: its playlist `select` is
  native and does not answer synthetic input. Set `deck_queue.json` in the
  settings folder instead (`[{"playlist": <id>, "advance": true, "repeat": true}]`).
- A playlist row made by hand in `library.db` needs `system` NULL, not 0, or
  `playlists()` leaves it out.
- **A djmanzo left running under Xvfb starves `framerate.spec.ts`**: its
  slow-interface chip never appears, on untouched code too. `pkill -x
  djmanzo` before the browser suite. Cost three runs on 16 October.
- The top bar's fitting is in `App.svelte` (`fit`, `refit`): levels 0–3,
  measured with a ResizeObserver. A new item in the bar is fitted for free;
  one that must never fold away must not carry `foldable`.

## Needed a file this session does not own

- `crates/dj-app/src/lib.rs` outside the marked blocks: one call in the
  snapshot pump, `crate::deck_queue::tick`, beside `tick_breaks`. Session B
  has no reason to edit the pump; if it did, the two edits are a few lines
  apart and may need a hand at merge.
