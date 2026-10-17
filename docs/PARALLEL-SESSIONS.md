# Two development sessions at once

*Written 17 October 2026, after v0.45.0 (the karaoke beta, both of its
sessions merged into `main`).* The owner runs two sessions side by side, on
two accounts. This page is how
they work at the same time and merge afterwards without fighting each other.
It follows the shape that worked for the karaoke beta
([karaoke_beta_release.md](karaoke_beta_release.md) §4–§6): **each session owns
its files, shared files are touched only in marked places or not at all, and
one session does the fold.**

Read [HANDOFF.md](HANDOFF.md) and [CLAUDE.md](../CLAUDE.md) first. Every rule
there holds: no GPL/AGPL code, nothing on the audio thread that allocates,
locks, does I/O or blocks, no model identifier in anything pushed, and
[DIRECTIVE.md](DIRECTIVE.md) wins where it and the code disagree.

## 1. The two sessions

| | Session 1 — *Interface* | Session 2 — *Intelligence* |
|---|---|---|
| Runs in | the session that wrote this page | a second session, another account |
| Branch | `claude/vigilant-tesla-xdqpmf` | its own designated branch (if it has none, `claude/parallel-intelligence`) |
| Starts from | `claude/vigilant-tesla-xdqpmf`, which carries `main` at v0.45.0 and this page | the same branch, fetched fresh |
| Notes file | `docs/parallel/session-1.md` | `docs/parallel/session-2.md` |
| Folds into `main` | yes, §5, only with the owner's word | never; it pushes its own branch only |

Neither pushes to `main` or to the other's branch. Neither releases. Neither
opens a pull request unless the owner asks.

## 2. What each session builds

Each item is a 🟡 row of [DIRECTIVE-STATUS.md](DIRECTIVE-STATUS.md). **Read the
row's own "what is left" before starting it** — it names the gap more exactly
than this table, and some of what it names cannot be built here (a camera, a
microphone, a controller, a screen reader). Take items in order; stop an item
that turns out to need hardware and say so in the notes.

### Session 1 — Interface and karaoke

Checked against the code on 17 October: §117's hand-arranged boards, §118's
further guides, all twenty-three waveform layers and the beginner's and
professional's layouts are built, whatever older sentences said; the undrawn
`transition` surface was a second name for the Pair panel and has gone. Done
on this branch before Session 2 began: those corrections in DIRECTIVE-STATUS
and HANDOFF. What is open:

1. **Karaoke follow-ups the beta named** (KARAOKE.md, `docs/karaoke-beta/`):
   a way into *Find a song* from the Singers panel; an echo timed to the
   deck's tempo; the *not sure* mark on the guest form and phone that offers
   the guide voice; a duet whose lines carry no names told apart.
2. **The truth pass, finished**: every remaining sentence in HANDOFF,
   DIRECTIVE-STATUS and ROADMAP that names as missing something built.
3. **Library quality: the absent columns**, starting with *breakdowns* from
   `energy::trajectory` (§75). It needs a field in `dj-library`, which is
   Session 2's, so it waits for Session 2's first merge and is written in
   Session 1's notes until then.
4. **The fold** of Session 2's branch (§5).

### Session 2 — Intelligence (the assistant, learning, the senses)

1. **§40: join what the assistant is told with what the context engine
   holds.** The row says the two lists have not been joined; make the
   assistant's prompt read `dj_app::context`'s one object rather than its own
   list, with a test that fails when a context field is not told.
2. **§12 / §14: count what the DJ does to a suggestion.** §44's transaction
   has *Modify* and *Reject* and neither is counted; then FX combinations and
   cue conventions (which pad a DJ puts which kind of cue on) — evidence only
   from the DJ's own hand (`By::Machine` is never learned from), decayed on
   `learned::recency` like everything else.
3. **§16: knowledge packs.** The remaining packs, and the fields no table yet
   owns: energy curves, allowed BPM relationships, transition preferences.
4. **§34 / §36: the crowd signals that can be built against synthetic input**
   — room activity in low, mid and high bands, applause-like transients, crowd
   noise told apart from the music, optical-flow variance. Each tested through
   §106's stand-ins, each its own line with its own confidence, never a crowd
   score. Claim nothing about a real room.
5. **§106: stand-ins for the other hardware** — a controller and timecode
   vinyl, of the same kind as the camera and microphone stand-ins.

## 3. Who owns which files

A session edits **only** files it owns, files it creates, and the shared files
in §4 the way §4 allows. Needing a file the other owns: write the change you
need into your notes file, and the fold makes it.

**Session 1 owns:**
`ui/src/` (all of it, except the panels Session 2 owns below), `ui/e2e/`
(except Session 2's new specs), `crates/dj-render/`,
`crates/dj-app/src/{boards,dashboard,widgets,guide,welcome,gig,leader,layout,waveform,cockpit,mission,tiers,uiop,handle,looks,columns,library,ghost,transition,plan}.rs`,
and **`crates/dj-core/` exclusively** — `param.rs`, `action.rs` and
`vocabulary.rs` carry counts and fixed-size arrays that two sessions cannot
both append to. Session 2 needing a new parameter or action writes it in its
notes; Session 1 adds it at the fold.

(`crates/dj-library/` is Session 2's; Session 1 asks for a library change in
its notes.)

**Session 2 owns:**
`crates/dj-assistant/` (except `song.rs`, karaoke's and so Session 1's), `crates/dj-library/`,
`crates/dj-hid/`, `crates/dj-dvs/`,
`crates/dj-app/src/{assistant,context,signals,profile,persona,setting,packs,packindex,senses,sight,listen,response,audience,staged,decide,night,timecode,practice,score,mood}.rs`,
and these panels: `ui/src/{Assistant,Coach,Night,Packs,RoomSense,Tonight,Controllers}.svelte`, `ui/src/night.ts`, `ui/src/night.test.ts`,
plus any new `ui/e2e/*.spec.ts` it creates (named for its work, e.g.
`assistant-context.spec.ts`).

**The karaoke files are Session 1's** now the beta is merged:
`crates/dj-app/src/{guests,guide_offer,reactions,suno,theme,karaoke,quiz,range,voice,monitors,deck_queue,find_song,sources_board,whispercpp}.rs`,
`crates/dj-assistant/src/song.rs`, `crates/dj-net/`, `crates/dj-vocal/`,
`crates/dj-engine/`. **`crates/dj-app/src/commands.rs` is neither's to grow**:
a new command goes in a module of its own and is registered in the session's
marker block (§4); an existing command a session must change is named in its
notes first.

## 4. Shared files, and how each may be touched

| File | Rule |
|---|---|
| `crates/dj-app/src/lib.rs` | Only inside a session's own marker block: add `// Parallel, session 1 (docs/PARALLEL-SESSIONS.md §4): its modules` … `// End of session 1's modules.` after the karaoke blocks, and the same pair in the `invoke_handler` list for commands. Session 2 uses `session 2`. One line elsewhere (a pump hook) only if unavoidable, and named in the notes. |
| `crates/dj-app/src/snapshot.rs`, `ui/src/api.ts` | Append new fields at the **end** of a struct or type, never between existing ones. Name each in the notes. |
| `ui/e2e/*.json` fixtures blessed with `DJMANZO_BLESS` | Either may re-bless. At the fold a conflict here is never hand-merged: take either side and re-bless. |
| `ui/e2e/shell.ts` | Neither edits it. Answer your own commands in the test, as `deckqueue.spec.ts` does. |
| `ui/src/App.svelte` | Session 1's. Session 2 needing a panel mounted writes it in its notes. |
| `Cargo.toml`, `Cargo.lock`, `ui/package.json`, lockfiles | No new dependency without writing its licence in the notes; `docs/RESEARCH.md` is updated at the fold. Prefer none. |
| `CHANGELOG.md`, `docs/{DIRECTIVE-STATUS,FEATURES,HANDOFF,ROADMAP,RESEARCH}.md` | **Neither edits them.** Each session writes its changelog lines, status row text and handoff notes in its notes file; the fold writes them in once. |

## 5. Merging

- **Session 2** commits and pushes its branch after every finished item, each
  commit through the full gates in CLAUDE.md, and keeps
  `docs/parallel/session-2.md` current: what is built, what is not and why,
  the changelog lines, the DIRECTIVE-STATUS text, anything it needs in a file
  it does not own, any trap worth a line in HANDOFF.
- **Session 1** does the same on its branch, and **merges Session 2's branch
  into its own** at the end of each of Session 2's items when asked, or at
  the end: merge (no rebase, no force-push), full gates on the merged tree,
  drive the interface under Xvfb, fold both notes files into the shared docs,
  push its branch. **Into `main` only on the owner's word.**
- Session 2 may merge Session 1's branch into its own to stay current. It
  never pushes to Session 1's branch.
- A conflict outside the shared files means the ownership in §3 was broken:
  stop and tell the owner rather than pick a side.

## 6. Before every commit (both sessions)

```
cargo fmt --all
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
cargo test --workspace --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cd ui && npx svelte-check && npx vitest run && DJMANZO_CHROMIUM=/opt/pw-browsers/chromium npx playwright test
```

Then mutation-test the load-bearing new test, and drive the application under
Xvfb (CLAUDE.md has the commands) for anything a person sees. Commit messages
end with the session's own attribution lines and name no model.
