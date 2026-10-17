# Session 2 — Intelligence: notes

See [../PARALLEL-SESSIONS.md](../PARALLEL-SESSIONS.md). Kept current after every item: built, not built and why, changelog lines, DIRECTIVE-STATUS text, changes needed in files owned by the other session, HANDOFF traps.

**Base.** This branch (`claude/peaceful-dirac-5ptpkm`) is `main` at `cbfdd12`
(v0.45.0, both karaoke sessions merged and folded) with Session 1's
`14c93b7` merged in — the owner's choice on 17 October, rather than a reset to
`14c93b7`, which would have dropped the karaoke fold from this branch. **The
karaoke fold is already on `main`**: Session 1's item 5 is done, and Session 1's
branch wants `main` merged in before its own fold.

**Owner's rule for this session (17 October):** only Session 2's files and new
files. Every edit needed in `commands.rs`, `state.rs`, `control.rs`, `host.rs`,
`dj-audio`, `dj-control`'s bus, `dj-core`, Session 1's or karaoke files is
written under *Needed* below for the fold.

## Status

- **§40 built** — the briefing reads the context engine's one object.
- **§12/§14 built, in part** — effects run together, cue conventions, and
  the transaction's Modify/Reject counted and told to the assistant; the
  Modify/Reject count waits on three lines in `commands.rs` (below).
- **§16 built, waiting on the fold for the picker** — thirteen more packs
  (`pack::READY`) and the three fields no table owned (tempo window, arc,
  transition preferences), each held to the library's own tables. The
  picker shows them, and the rail and planner read the new fields, once the
  fold makes the changes listed under *Needed*.
- **§34/§36 built against stand-ins** — eight more signals from the room
  surface's camera and microphone, each placed against its own night on its
  own line with its own confidence (`dj_assistant::floor`). Proven against
  drawn footage and generated sound in Chromium and run in the shipped
  webview under Xvfb with WebKitGTK's mock devices; **nothing here has been
  near a real room**, a real camera or a real microphone.
- §106 — not started.

## Changelog lines (for CHANGELOG.md, under Unreleased)

- **The assistant is told what the context engine knows.** Besides what is on
  the decks, its briefing now carries §11's music context (the tempo the room
  is hearing, its key, how many decks are playing and ready, how far apart the
  playing tempos are), the machine (sample rate, output latency, the
  controller, MIDI, a cue output), how the DJ is working tonight (gestures a
  minute, the commonest gesture, suggestions taken and let pass), the
  attention budget, and whether djmanzo is keeping up (audio load, dropouts,
  the limiter, the background threads) — one list rather than two that drift.

- **What the DJ does, told to the assistant.** Which effects the DJ runs
  together tonight (two in one deck's chain, brought in by hand), which pad
  holds which kind of cue across their records (the record's start, a
  phrase start, the last fifth; their own labels and colours — a pad says
  nothing until it holds a cue on four records and one kind is most of
  them, and recent records weigh more), and what they did with tonight's
  staged transactions (accepted as offered, accepted after changing,
  rejected) — counted, never turned into a preference.

- **Thirteen more knowledge packs, and the three things a pack could not
  say.** Bachata, Merengue, Dembow, Reggaeton, Salsa, Caribbean, Afro /
  Afrobeats, R&B, EDM, Wedding, Beach / Resort, Club and Practice /
  Education join the eight, each built from what djmanzo already knows (one
  family of the genre map with its parent's curriculum, or a §81 occasion
  with the whole catalogue up to practised moves). Every pack now also says
  how close two tempos have to be to count as a direct mix in that music
  (techno three percent, house four), the set shape that kind of night
  usually takes, and the joins it reaches for when the records leave the
  choice open — never an echo, which is djmanzo's answer to a key clash,
  and never instead of the cut a tempo that does not work demands.

- **The room surface reads eight more things, each on its own line.** The
  room's sound in three bands (low, mid, high), applause-like clicks (sharp
  clicks that keep to no beat — a hi-hat does not count), the room over the
  music (how much louder the microphone is than the music djmanzo is sending
  explains, which is how a crowd's noise is told from the record), uneven
  movement and moving together (a coarse optical flow across the picture),
  and the picture changing as a whole. Each is compared only with the same
  room earlier tonight, says how far it can be trusted, and says beside every
  reading what it is easily confused with. Nothing adds them up: there is no
  crowd score, by §36's own rule. Built and tested against drawn footage and
  generated sound; not yet against a real room.

## DIRECTIVE-STATUS text

- **§40**, replacing the row's last sentence (*"…§11's `musicContext`,
  `hardwareContext` and `djBehaviorContext` are gathered now … which the
  briefing does not yet read …"*): **The two lists are joined (17 October).**
  The briefing (`assistant::context_lines`) now reads §11's object —
  `context::gather_from`, built from the same snapshot the briefing reads, so
  its decks and its context are one moment — and tells every field the §40
  list does not already carry, one line per field under §11's own name
  (`context::told`, rendered from the serialised object so a field added to a
  context struct is told without anyone remembering to). The phase, the
  occasion and the room stay with §40's own items (`context::TOLD_BY_SIGHT`),
  told once: the room keeps its *only while watched* rule. Held by
  `context::tests::every_field_of_the_object_is_told_or_carried_by_a_named_item`
  (every leaf of the object reaches the briefing, walked independently of the
  renderer; every field told by a §40 item names an item that exists; none
  told twice) and `assistant::tests::the_briefing_reads_the_context_engines_one_object`
  (through `AppState`); six mutants fail them. §11's consumer table now names
  `assistant.rs` reading `context::gather_from`. What is left: the library,
  out by design.
- **§11**: the *assistant* consumer reads the object in `assistant.rs` now, not
  only through `commands.rs`'s `dj_context`.

- **§12**, replacing *"Not learned — … FX combinations and cue conventions
  … and which suggestions are manually modified or which staged actions are
  overridden (§44's transaction has Modify and Reject, and neither is
  counted yet)"*: **FX combinations, cue conventions and the transaction's
  verdicts are counted (17 October).** *FX combinations*:
  `signals::effect_combinations` — two effects in one rack (one deck's chain
  or the master's: two decks' effects are two records' effects), counted when
  the DJ's hand brings one in while the other runs; djmanzo's own throws make
  no pair but are followed. *Cue conventions*: `dj_library::conventions` over
  `Library::cue_evidence` — each stored cue's place on its record's grid (the
  record's start, a phrase start within a quarter beat, the last fifth,
  elsewhere), the DJ's label and colour; a slot says nothing until it holds a
  cue on four records (`ENOUGH`) and one kind is 60 % of its weighted evidence
  (`MOST`), each record weighed on `learned::recency` by its last play (or its
  adding); every stored cue is the DJ's own, since djmanzo never places one.
  *Verdicts*: `staged::Verdict` — accepted as offered, accepted after
  changing a move (a no-op is not a change), rejected — tallied on the night
  (`Night::note_verdict`); counts only, on this row's and §14's grounds, and
  nothing here changes what is staged next. All three reach the assistant's
  briefing (`assistant::habit_lines`), `none` until there is one. Held by
  `signals::tests::effects_run_together_in_one_rack_by_the_djs_hand`,
  `conventions::tests` (3), `store::tests::cue_evidence_carries_each_records_grid_and_recency`,
  `staged::tests::a_transactions_verdict_says_whether_the_dj_changed_it`
  and `assistant::tests::the_briefing_tells_the_djs_habits_as_counts`; eleven
  mutants fail them. **Not yet:** the verdicts are counted only once
  `commands.rs` reports them (see *Needed*), so in the running application the
  line reads `none` until the fold; the effect pairs and verdicts are
  tonight's and not yet kept across nights (they would go beside the gesture
  counts in the `nights` table, written by `commands::night_setting`); *how
  quickly the DJ changes plans* is not built.

- **§16**, replacing *"What is left for this row: the remaining twenty-two
  packs, and the §16 fields no table yet owns (energy curves, allowed BPM
  relationships, transition preferences)"*: **Twenty-one packs, and the
  three fields owned (17 October).** Thirteen more — Bachata, Merengue,
  Dembow, Reggaeton, Salsa, Caribbean, Afro / Afrobeats, R&B, EDM, Wedding,
  Beach / Resort, Club, Practice / Education — written to the same rule as
  the eight: a genre pack is one family of `dj_core::genre` (or the few a DJ
  names together) with its parent's curriculum, an occasion pack is a §81
  setting with the catalogue up to practised moves. *Allowed BPM
  relationships*: `Pack::tempo_window`, only ever a narrowing of the
  scorer's `suggest::TEMPO_TOLERANCE` (techno and Beginner 3 %, house 4 %),
  read by `suggest::wider_than_pack` — which reads only the scorer's
  `TempoFits`, leaving half time to `half_time` and a far tempo to
  `TempoFar`. *Energy curves*: `Pack::arc`, a `setlist::Arc` by name
  (`Arc::from_name`, `None` for a name it does not know rather than the
  default journey), offered and never imposed — the arc stays the DJ's to
  say, which is §11's refusal above. *Transition preferences*:
  `Pack::transitions`, only styles `plan`'s `STYLE_IS_TASTE` lets a profile
  choose (cut, fade, blend) — never `Echo`, the music's answer to a key
  clash — so the planner's refusal to blend records that cannot be blended
  stays the planner's. Held by `pack::tests` (every family and setting
  exists, ids distinct across both lists, transition preferences are taste
  and said once, the waiting packs are not chosen by slug before the fold),
  `setting::tests::every_packs_tempo_window_and_arc_are_the_libraries_own`,
  `suggest::tests::a_packs_tempo_window_narrows_only_the_fit` and
  `setlist::tests::an_arc_is_named_and_read_back_by_the_same_word`; twelve
  mutants fail them. What is left: the other nine of §16's thirty (Private
  Event, Lounge, Festival, Mobile DJ, Karaoke / MC, VJ / Video, DVS, CDJ,
  Stem / Remix), whose curriculum nobody here has checked — the module docs'
  reason, unchanged.

- **§34**, replacing *"What is left is most of §34's list … they are not
  built: room activity in the low, mid and high bands, applause-like
  transients, crowd noise told apart from the music, optical-flow variance,
  an occupancy proxy, a crowd-density proxy, lighting motion and scene
  change."*: **Five of them built (17 October), against stand-ins.**
  `dj_assistant::floor` places each of eight signals against its own night
  (`Floor::line`, the room's `Against` scale, `ENOUGH` looks in the last three
  minutes, `SETTLED` before a placement stops being *early*), one line each,
  always — a signal with nothing to say says *not yet* and why. Measured by
  the room surface from the frames and window of sound it already reads
  (`ui/src/floor.ts`, numbers only cross to Rust): *room activity in three
  bands* (`bands`, power summed across each band's bins of the analyser's own
  spectrum, smoothing turned off so a look hears its own moment);
  *applause-like transients* (`clicks`, sharp slices of the window's first
  difference, discounted by how much they keep to one gap — a hi-hat on the
  grid scores nothing); *crowd noise told apart from the music*
  (`Signal::OverMusic`: the microphone's loudness against the master's level,
  a running straight-line fit over the night in Rust, which refuses while the
  music has not moved two decibels or the microphone does not follow it);
  *optical-flow variance* (`flow`, 8-pixel blocks of the 64×48 frame matched
  within 3 pixels, a move counted only where it fits clearly better than
  staying put); *scene change* (`sceneChange`, the brightness histogram
  against the last look's). Held by `floor::tests` (6, in `dj-assistant`),
  `dj_app::floor::tests` (3), `ui/src/floor.test.ts` (14) and
  `ui/e2e/floor.spec.ts` (5: through Chromium's stand-in camera and
  microphone — a bass tone sits in the low band and a hi-hat in the high one;
  a room clapping reads 0.52 where the equally sharp hi-hat reads 0.00; a pan
  moves together at 0.88 where scattered figures read 0.36 and twice as
  uneven; a lighting cut reads a scene change of 1.0 where the pan reads
  0.05; a sound that switches from bass to hats lets go of the bass within
  the look); sixteen mutants fail them, and a seventeenth — the wrong FFT length passed at the call site — was made impossible by reading it off the spectrum. Driven in the shipped webview under
  Xvfb with WebKitGTK's mock camera and microphone (`DJMANZO_MOCK_CAPTURE`):
  the eight lines appear under *Beyond light, movement and loudness*. **None
  of it has been near a real room.** What is left: an occupancy proxy, a
  crowd-density proxy and lighting motion (as distinct from scene change).
- **§36**, replacing *"What §36's `AudienceContext` names and djmanzo does not
  read is cohesion … vocal response … and visual change"*: **Cohesion and
  visual change are read (17 October), against stand-ins.** *Cohesion* is
  `Signal::Together` — how much of the picture's movement goes one way (the
  length of the blocks' summed movement over the sum of their lengths);
  *visual change* is `Signal::Scene`. Each on its own line beside the room's
  three, with its own confidence and its blind spot said beside it (a
  sweeping light moves the whole picture one way; the lighting rig changing
  looks like the room changing), and nothing combines them — a test holds
  that each line speaks only of its own signal, and another that no line
  names a mood. The notable ones reach the assistant's briefing, only while
  somebody is looking (`dj_app::floor::briefing`, the room's rule). What is
  left: *vocal response* — the room over the music in the voice band only —
  is not split out; `OverMusic` is broadband.
- **§106**, after *"…through a stand-in of the same kind"*: §34/§36's
  signals have stand-ins of their own (`ui/e2e/floor/media.ts`: a pan,
  scattered figures, a lighting cut, a room clapping, a hi-hat on the grid,
  a bass tone, and one that switches from the bass to the hats).

## Needed in files this session does not own

- `crates/dj-app/src/commands.rs`, `dj_context`: its body is now
  `crate::context::gather_from(&state, &snapshot_now(&state))` — the same
  gathering, written once in `context.rs`. Until the fold the two are copies;
  replace the body with that call.

- `crates/dj-app/src/commands.rs`, §12's verdicts — three lines:
  - `staged_accept`, before the moves run:
    `state.night().note_verdict(staged.accepted());`
  - `staged_reject`, before `state.clear_staged()`:
    `if state.staged().is_some() { state.night().note_verdict(crate::staged::Verdict::Rejected); }`
  - (`staged_choose` needs nothing: `Staged::choose` counts the change.)
- Optional, for §12 across nights: `nights` columns for the effect pairs and
  verdicts (a migration in `dj-library`, this session's), filled by
  `commands::night_setting` — not built, since its writer is `commands.rs`.

- **§16 at the fold** (all in files this session does not own):
  - `crates/dj-assistant/src/pack.rs` is this session's, but the move is the
    fold's: append `READY`'s thirteen to `ALL` (`ALL: [Pack; 21]`), delete
    `READY` and `every()` (or keep `every()` as `ALL.iter()`), and delete
    `the_waiting_packs_are_not_chosen_by_slug_before_the_fold`. Do it in the
    same commit as the next two lines, or `packs.spec` fails.
  - `ui/e2e/packs.spec.ts:55`: `toHaveCount(8)` → `toHaveCount(21)`.
  - `ui/e2e/packs.json`: re-bless with
    `DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture`.
  - `commands.rs`, `KnowledgePackDto` (≈19799) and `knowledge_packs`: append
    `tempo_window: f64`, `arc: String` (empty for none) and
    `transitions: Vec<String>` (lower-case style names); `api.ts`'s
    `KnowledgePack` gets the same three at the end. Optional for the picker:
    a line under each pack, e.g. *"holds tempo within 3 % · journey · blends"*.
  - `commands.rs`, `with_pack` (≈10850): after `also_in_pack`, when
    `dj_library::suggest::wider_than_pack(&s, pack.tempo_window)`, take the
    `TempoFits` credit back and push a new `Reason::TempoWiderThanPack
    { window }` (dj-library's `Reason`, this session's — but adding the
    variant breaks the three exhaustive matches in `commands.rs` at ≈11809,
    ≈11881 and ≈11953, so the variant and its three wordings land
    together at the fold: *"wider than this pack holds (3 %)"*, rank 2,
    *"wider than the pack"*). The take-back is `also_in_pack`'s pattern.
  - The planner's `usual` (`commands.rs` ≈9942, `ghost.rs`, `autopilot.rs`,
    `transition.rs`, all Session 1's or shared): when the DJ's profile has no
    usual style, pass `pack.transitions.first().copied()`. `choose_style`
    already applies it only where tempos and keys agree and only if
    `STYLE_IS_TASTE` contains it, so no pack can override a cut or an echo.
  - The set planner (`commands.rs` ≈9163 parses the arc name): when the DJ
    has not said an arc, the interface may *offer* the pack's
    (`Arc::from_name(pack.arc?)`), never default to it silently.

- **§34/§36 at the fold**: nothing is required — `RoomSense.svelte` and the
  new `floor.ts` are this session's, the commands are registered inside
  `lib.rs`'s *Parallel, session 2* marker block (`floor::floor_saw`,
  `floor::floor_read`; module `pub mod floor;` in the modules block), and the
  floor lives on `Night` (this session's), not on `AppState`. Optional: answer
  `floor_saw`/`floor_read` in `ui/e2e/shell.ts` so other specs that open the
  room can see the lines; until then the shell answers both with `null`, and
  the surface draws nothing for a `null`.

## HANDOFF traps

- `cargo fmt` re-wraps a long line between an edit script's read and its next
  match: re-read after every `cargo fmt` before matching text in an edit.
- A §16 pack's `transitions` is taste in `plan`'s sense, which includes
  `Cut` and excludes `Echo` (`STYLE_IS_TASTE`): echo is the planner's answer
  to a key clash. It reads backwards from DJ talk, where an echo-out is a
  style; `pack::tests::transition_preferences_are_taste_and_said_once`
  holds it.
- `ui/e2e/floor/media.ts` (like `senses/media.ts`) **reuses** a file already
  in `/tmp/djmanzo-floor`: after changing a generator, delete the directory,
  or the test runs against the old footage.
- `vitest run` prints a test's `console.log` only with `--reporter=verbose`;
  without it a probe looks like it printed nothing.
- An `AnalyserNode`'s frequency data is smoothed across reads by default
  (`smoothingTimeConstant` 0.8). Read once a look, that carries the last
  look's spectrum into this one for many looks; the room surface sets it to 0
  and `floor.spec.ts`'s *switch* sound fails without it.
