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
- Not yet: A3 (one deck slim, the other big), A4 (shuffle, BPM/key flow).

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

## Needed a file this session does not own

- `crates/dj-app/src/lib.rs` outside the marked blocks: one call in the
  snapshot pump, `crate::deck_queue::tick`, beside `tick_breaks`. Session B
  has no reason to edit the pump; if it did, the two edits are a few lines
  apart and may need a hand at merge.
