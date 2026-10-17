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
- §16, §34/§36, §106 — not started.

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

## HANDOFF traps

- `cargo fmt` re-wraps a long line between an edit script's read and its next
  match: re-read after every `cargo fmt` before matching text in an edit.
