# Session 1 — Interface and karaoke: notes

See [../PARALLEL-SESSIONS.md](../PARALLEL-SESSIONS.md). Kept current after
every item: built, not built and why, changelog lines, DIRECTIVE-STATUS text,
changes needed in files owned by the other session, HANDOFF traps. Folded into
the shared documents at the fold (§5).

## Status

### Done before Session 2 began (17 October)

- **The undrawn `transition` surface went.** `cockpit` declared it and no
  panel drew it; the Pair panel already draws §68's transition object, so the
  declaration and its tier entry were removed rather than a second panel
  built. `ui/e2e/surfaces.json` re-blessed. Already written into
  DIRECTIVE-STATUS §5.
- **Stale status corrected** (already in DIRECTIVE-STATUS and HANDOFF):
  adaptability and visual utility met (eleven of thirteen success criteria);
  all twenty-three waveform layers drawn.
- **`main` at v0.45.0 merged** into this branch; every gate green on the
  merged tree (920 browser tests on a fresh bundle).

### Item 1: karaoke follow-ups

- **Find a song from the Singers panel — built.** *Can't name it?* beside
  the request form opens §128's guided questions (`FindSong.svelte`) under
  it. A record they find goes to the song box **as the record**
  (`FindSong`'s new `onTrack`), so the request carries its track and path;
  *Ask the assistant with all of this* lists the assistant's guesses there —
  one the collection has goes in as the record, one it does not as words,
  which the rotation takes as typed. Held by `e2e/singers-findsong.spec.ts`;
  4 interface mutants fail it (found record sent as words, owned guess taken
  as words, `onTrack` bypassed, panel left open). Driven in WebKitGTK: the
  questions open under the form and fit the panel.
- **An echo in time with the record — built.** `dj_vocal::EchoSettings`
  gains `beats` (one of `ECHO_BEATS`: ¼, ½, ¾, 1, 1½, 2; absent from strips
  kept before it, never serialised when unset, so no fixture moved) and
  `EchoSettings::timed(bpm)` — a note longer than the longest echo is halved
  onto the beat, one shorter than the shortest doubled.
  `dj_app::echo_tempo` follows the record the room is hearing
  (`context::carrying`, effective tempo) from the pump; when it moves by half
  a BPM or more every strip with a timed echo is sent again, and
  `commands::as_played` times every other send. Only what is played is timed;
  what is kept is the note length and the delay for when nothing plays. Under
  Echo, *In time* (No / the six) in `StripChain.svelte`; the Delay slider is
  disabled and reads *in time* while timed. Held by
  `strip::tests::an_echo_timed_to_the_record_follows_its_tempo`, three
  `echo_tempo` tests (one through `AppState` and the engine's command queue)
  and `microphones.spec.ts`'s *in time* test; 5 Rust and 2 interface mutants
  fail them. Driven: *½ beat* chosen on Mic 1, kept in `vocal.json` as
  `"beats": 0.5`, Delay greyed. **Not heard**: no audio device here.
- **Not sure of it — built (host's form).** `karaoke::Request::unsure`,
  `Rotation::unsure_of_last`, `karaoke_ask`'s optional `unsure`;
  `guide_offer::offer` offers the guide for a song the singer is not sure of
  even when the guest book has them singing it ("Not sure of it, Ana: …"); a
  voice already chosen still wins. A *Not sure of it* tick beside the request
  form, cleared after each request; the song marked *not sure of it* on the
  up-next card and *not sure* in the rotation. Held by the two `guide_offer`
  tests (extended) and `guide-offer.spec.ts`'s new test; 3 Rust and 2
  interface mutants fail them. **Not yet: the phone's form** — the room's
  page (`dj_net`) has no such mark; a phone can already ask for the guide
  itself (K4).
- **A duet marked by part — built.** Lyrics that mark parts rather than
  name singers — "M:", "F:", "D:" (the karaoke `.lrc` convention) and the
  words spelled out — are read on the singers' screen (`lyricBreak.ts`:
  `partsOf`, `partOf`'s `order`): the first part the song gives goes to the
  first singer on a microphone, the other to the second (who is the man and
  who the woman is not djmanzo's to guess; who comes in first is the
  song's); the mark is neither drawn nor sung as a timed word; a line sung
  together is everyone's (`data-together`), in no one singer's colour.
  Held by a unit test and `lyric-visuals.spec.ts`'s new test; 5 mutants
  fail them. Not read with one singer on the microphones, where the marks
  still show — a duet sung alone is rare and the marks say whose line it is.

### Item 1, continued: what the karaoke beta's notes left open

- **Like this one, from the request form — built.** For a song picked from
  the collection, *Like this one* lists five records like it (`similar_to`,
  the Library's and the Next rail's own) with why; one taken goes into the
  request as the record. Held by `singers-findsong.spec.ts`'s second test; a
  mutant taking it as words fails it. Driven against the real command: *two*
  offered *one* with "key clash (11B) · 120 BPM fits · same level" — the
  reasons are a DJ's mixing words, which a karaoke host may not need.
- **The skin picker in Settings → Appearance too — built**, under *How much
  of it*. Each picker is now its own radio group (`$props.id()`): two on
  screen sharing `name="skin"` were one group, and a choice in one left the
  other showing nothing chosen. Held by `skins.spec.ts`'s new test; 3
  mutants fail it. Not driven in WebKitGTK (the browser test opens both).
- **The room's taps in the night's crowd log — built.** `dj_app::room_taps`:
  the pump keeps every tap newer than the last one kept in tonight's crowd
  log, once, as a reaction from the room with no name and the glyph as its
  words (`crowd::Source::Room`), so it is placed against the music like the
  rest of §119's reactions. Held by
  `room_taps::tests::the_rooms_taps_reach_the_nights_crowd_log_once` (through
  `AppState`); 3 mutants fail it. A tap from a real phone was not driven.
- **Fast search modes — built** (§128, "different fast search modes to
  find songs for karaoke from different sources"). Above the request form,
  *By name* (the collection, as before), *By a line* (the words of the
  records whose lyrics djmanzo has — `words_search`, the memory panel's) and
  *Online* (every connected source at once — `search_sources`, the
  browser's *Sources*). A record found by a line goes in as the record; a
  song found online as words, and is not searched for again once taken.
  Held by `singers-findsong.spec.ts`'s third test; 3 mutants fail it.
- **Still open from those notes:** nothing proved on a signed-in Suno or
  WhatsApp (needs an account and a phone).

### Item 2: the truth pass, and what it turned up

- **§117's last gap built, and the row closed ✅** (written into
  DIRECTIVE-STATUS already): its "Not yet" — the §118 guides and welcome on
  the dashboard — was the one thing the row still named. The dashboard
  (`0`) now has a *Guides* section: the six guides (`guide <topic>`), each
  opened as its top-bar button opens it (`Guides.svelte`'s `openTopic`), and
  *Welcome*. `leader::runnable` accepts both runs; `dashboards.json`
  re-blessed. Held by a dashboard test and `dashboard.spec.ts`'s guide test;
  3 mutants fail them. The count: 79 done, 31 part, 19 standing rules, of
  129.

### Session 2 folded in

- **§40 and §12/§14 merged** into this branch (17 October). Its *Needed*
  lines in `commands.rs` made here: `dj_context` is now
  `context::gather_from(&state, &snapshot_now(&state))`, and
  `staged_accept` / `staged_reject` note the DJ's verdict
  (`Night::note_verdict`). Not held by a test through the commands
  themselves — Session 2's `staged` tests hold the verdicts.

- **Round 1 folded in full** (17 October): §16, §34/§36 and §106 merged
  too, and §16's fold made — the thirteen packs joined `pack::ALL` (21),
  `KnowledgePackDto` and `api.ts` carry `tempo_window`, `arc` and
  `transitions` and the Settings picker says them under each pack, the rail
  takes back a fit wider than the chosen pack holds
  (`Reason::TempoWiderThanPack`, three wordings in `commands.rs`), and the
  planner's `usual_style` falls back to the pack's first style
  (`usual_or_pack`). `packs.json`, `setups.json` and `welcome.json`
  re-blessed. Not done (optional in its notes): the set planner offering the
  pack's arc (round 2, Session 1), `nights` columns for §12 across nights
  (round 2, Session 2), the stand-ins wired into the running application
  (round 2, Session 2), `floor_saw`/`floor_read` answered in `shell.ts`.
  Both sessions' changelog lines, status text and traps are now in
  `CHANGELOG.md`, `DIRECTIVE-STATUS.md` and `HANDOFF.md`.

### Changes in shared files (for the fold)

- `crates/dj-app/src/lib.rs`: a *Parallel, session 1* modules block after
  session A's (`echo_tempo`, `room_taps`), and two pump lines after the deck
  queue's tick: `crate::echo_tempo::tick(&state, &snapshot);` and
  `crate::room_taps::tick(&state);`.
- `crates/dj-app/src/commands.rs` (existing functions, no new command):
  `played_vocal_settings` and `send_vocal_strip` made `pub(crate)`;
  `as_played` times the echo; `karaoke_ask` takes `unsure: Option<bool>`.
- `ui/src/api.ts`: `StripSettings.echo.beats?`, `RotationSong.unsure?`,
  `karaokeAsk(…, unsure = false)` (sent only when true).

### Changelog lines (for the fold)

- *Can't name it?* on the Singers panel: the guided questions of *Find a
  song*, and what they find goes straight into the request — the record when
  the collection has it.
- An echo in time with the record: a singer's echo set in beats follows the
  tempo of the record the room is hearing.
- *Not sure of it* beside a request: the guide voice is offered for that
  song even if the singer has sung it before.
- A duet whose lyrics mark parts ("M:", "F:", "D:") rather than name the
  singers gives each part to a singer on a microphone, in their colour.
- *Like this one* beside a request picked from the collection.
- The skin picker in Settings → Appearance as well as on the Singers panel.
- The room's emoji taps are kept in the night's crowd log.
- The request form searches by name, by a line of the words, or online.

### DIRECTIVE-STATUS text (for the fold)

- §128 / §126: the three above, as *built*, with the phone's *not sure* mark
  as what is left.
