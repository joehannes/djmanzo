# Karaoke beta — session B notes

What session B built, for folding into the shared documents once both
sessions are merged ([karaoke_beta_release.md §6](../karaoke_beta_release.md#6-after-both-folding-in-and-the-release)).
Only session B edits this file.

## Changelog lines (for CHANGELOG.md, under Unreleased)

- **Emoji from the room, held back.** With *Emoji from phones on the
  singers' screen* on (Singers panel; off until the host turns it on), the
  room's page shows a row of six emoji — 👏 ❤️ 🔥 😂 🎤 🙌. Each tap shows
  small in a corner of the singers' screen, rises a little and fades within
  three seconds (only fades with reduced motion), never over the words and
  never taking a press; five at most at once, one tap per phone every two
  seconds, six a second for the whole room.

## Status (for DIRECTIVE-STATUS.md §126 / §127, FEATURES.md, KARAOKE.md)

- **B2 built** (§126/§127, "live comments of the crowd on the karaoke lyrics
  screen as they input emoticons … don't overdo the visual influences"):
  `dj_net::reactions` (the book: glyphs only, `GAP` 2 s per phone,
  `ROOM_PER_SECOND` 6, `SHOWN` 3 s, `KEPT` 64, `PHONES` 300; `Reacted`
  tags), the room page's row and `POST /react` (`front.rs`, `page.rs`),
  `dj_app::reactions` (`room_reactions`, `room_reactions_set`),
  `RoomEmoji.svelte` on the singers' screen and the host's switch in
  `Singers.svelte`. Held by 6 `dj-net` tests (one over a real socket), 2 in
  `dj-app`, and `e2e/room-emoji.spec.ts` (4); 4 Rust and 4 interface
  mutants fail them. The session that began B2 left four fragments (two
  test modules, the end of the browser spec, the end of the component's
  styles); this was built to match them exactly.
- Not yet driven in the running application (WebKitGTK); the browser suite
  and a real-socket test hold it.
- Not yet: the room's taps are not written to the night's crowd log
  (`crowd::Reaction { source: Room }`) — `crowd.rs` is in neither column.

## For the host (KARAOKE-NIGHT.md)

**Emoji from the room.** In the Singers panel, tick *Emoji from phones on
the singers' screen*. Guests then see a row of emoji on the room's page;
each tap shows small in the corner of the singers' screen and fades within
seconds. It is off until you turn it on.

## For the next session (HANDOFF.md)

## Needed a file this session does not own

- `crates/dj-app/src/crowd.rs`, to record the room's taps in the night's
  crowd log. Not done.
