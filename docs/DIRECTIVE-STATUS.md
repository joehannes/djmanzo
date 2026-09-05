# The 105 sections, and where each one stands

The owner's directive — *DJMANZO — ADAPTIVE PROFESSIONAL DJ COCKPIT* — is 105
numbered sections. [`GUI-OVERHAUL.md`](GUI-OVERHAUL.md) is the analysis §99
asked for and turns them into nine phases; this file is the other view, section
by section, so "where are we" is answerable from a file rather than from
somebody's memory.

**It is written that way for a reason.** This question was asked once when the
directive's text was no longer in context, and the honest answer at the time
was "I can only defend twelve of them". A status that lives in a file does not
have that failure mode.

## How to read the marks

- ✅ **done** — shipped and, where it is testable, tested.
- 🟡 **part** — some of it is real; the rest is named in the row.
- ⬜ **open** — not started.
- ⚖️ **standing rule** — a constraint rather than a deliverable. It is honoured
  or it is not; it never becomes "done" because it never stops applying.

Counting rules and rounding are at the bottom, so the numbers can be checked
rather than taken.

## The table

| § | Section | | Where it stands |
|---|---|---|---|
| 1 | Understand the actual repository | ✅ | 37 components, 17,614 lines, 148 commands, 448 parameters — counted, not remembered |
| 2 | Treat the current architecture as an asset | ⚖️ | Nothing rewritten. The action bus, ParameterRegistry and waveform renderer are untouched |
| 3 | Product thesis: an adaptive performance environment | 🟡 | The first real adaptation ships — the interface picks its density from the window. The rest of the thesis is phases 5–8 |
| 4 | Do not build "an AI dashboard" | ⚖️ | Nothing AI-shaped has been added to the chrome |
| 5 | The new GUI model | 🟡 | The performance zone is rebuilt; the Mission Bar is not |
| 6 | Dock / surface manager | ✅ | Side and bottom docks, several surfaces at once, framed and closable, persisted. `cockpit::Surface`/`Dock`/`Workspace` |
| 7 | Workspace presets | 🟡 | Three ship in Rust (Perform, Prepare, Read the room); no picker in the interface yet |
| 8 | Adaptation levels | 🟡 | Five density bands, derived from measured deck heights. The wider notion of adaptation levels is not built |
| 9 | Separate autonomy from confidence | ⬜ | Modelled in the audit; not in the code |
| 10 | AI posture stays compatible with djmanzo's | ⚖️ | The six postures and nine occasions are untouched |
| 11 | Add a context engine | 🟡 | `dj_app::context` reads **the night** from the records that have actually been played — their loudness and their tempo — and publishes a phase, an energy, a confidence and its typed reasons on every snapshot. It will not guess: under three analysed records the answer is `None`, which is what `SessionContext.session` had been hardcoded to since it was written. The assistant panel draws it beside the occasion, so what the DJ declared and what the records say sit together. What §11 also asks for and this does not do is *unify*: the occasion, the hardware, the audience, the DJ's behaviour, the attention budget and the performance health are all still their own types in their own modules. One of them has stopped being an orphan: the attention budget's `promoted_controls` is what sizes §74's rail |
| 12 | Learn the DJ | 🟡 | Taste learned from play history ships. Persona learning does not |
| 13 | Never learn badly | ⬜ | |
| 14 | Behavioural signals | ⬜ | Needs an events table with decay |
| 15 | AI should understand DJ technique | ✅ | The technique catalogue ships |
| 16 | Domain knowledge packs | 🟡 | Genre families ship; packs as a format do not |
| 17 | GUI adapts to session phase | 🟡 | Occasion-aware density ships, and the phase is now a real reading rather than a `None` — the living interface's energy comes from the night instead of from the master meter. Phase-*driven layout* still does not: nothing rearranges when the room turns |
| 18 | Attention budget | 🟡 | `cockpit::Attention` exists with the rule that matters — while performing, the interface may not reflow. Its `promoted_controls` now has a reader: §74's rail is sized to the tightest of the four budgets, and a test holds every mode to it rather than to a number written down twice. The other three fields — how many suggestions, how many notices, how much motion — are still consulted by nothing |
| 19 | No random UI reorganisation | ⚖️ | Enforced by a golden-order test: the deck's control order is asserted in full and fails if anything moves |
| 20 | Playlist / library overhaul | 🟡 | Function tags and their filtering ship, and three of the four views do. **Set Flow**: a dockable surface drawing the plan as a sequence with the seam between each pair — its deltas, its confidence, and whether it needs a cut rather than a blend. **Pair**: its own surface, the two records side by side with the seam between them, each with its waveform, and the mix point drawn *on* the outgoing one. **Compact cards**: the browser's second representation, the sleeve read out of the file's tags and served as an image, falling back to lettering; the same rows, the same sort and the same actions as the table, from one snippet so the two cannot drift. **The performance table** now offers fourteen columns with a picker that applies as it is pressed and is remembered: title, artist, album, genre, year, BPM, Camelot, the key by name, energy, time, rating, plays, last played and phrase length. Thirteen of §20's twenty, plus the colour, which is a stripe on the title rather than a column of its own. What is left has no data behind it rather than no column: **vocal** and **stem availability** need the separator's output kept, **transition suitability** and **AI confidence** are relative to a track nothing has named, **request count** is not joined to the library, and **function tags** are stored per record and would need a bulk read. Two card actions are missing for the same kind of reason: **preview** needs somewhere to listen that is not a deck, **queue** needs a play queue this application does not have |
| 21 | "Prepare" must be first class | ✅ | Its own dockable surface beside the browser, not a strip inside it. One gesture — `→` on a browser row — hands a track over; `prepare.svelte.ts` is the only path between them, so there is no second, differently-behaved way to set a track aside |
| 22 | Next-track rail | 🟡 | The rail ships as its own dockable surface, following whichever deck is playing: up to eight candidates, each with one line of deltas (`+3 BPM · 8A→9A · +1 dB`), a confidence bar, and load / set aside / more-like-this / pin / pass. Two of the fifteen things §22 lists are not there — **audition**, which needs a preview player djmanzo does not have, and the **estimated transition type**, which means running the M8 planner per candidate |
| 23 | Track function tagging | ✅ | Ten functions, closed vocabulary, migration 10, browser picker, and `for is opener` in smart folders |
| 24 | Pairs and relationships | ⬜ | Needs new storage with confidence decay |
| 25 | Waveform overhaul | 🟡 | **Fifteen of the twenty layers ship** — this row said one until it was audited against the code, which is the kind of error a status file exists to prevent. Drawn today: amplitude; **spectral balance**, the three bands split at the mixer's own 300 Hz and 4 kHz crossovers so the colour matches what the LOW/MID/HIGH knobs act on (`dj_render::tile::Palette::colour_for`, tested); the beat grid; downbeats; **phrase starts**, in their own colour and drawn even at zooms where the beat lines are too dense to show; cue markers; the active loop region; and the **transition** as a mix-in and a mix-out, dashed to say they are proposed rather than placed; and **breakdowns and drops** — where the drums leave and where they come back, found per beat from the kick band against the record's own level, drawn as a dim band along the lane's top edge ending in a bright tick. The planner reads that layer too, so a transition says how many of its beats have no drums under them — which is §25's fourth question, *what will happen if I do it*, answered rather than drawn. And **saved loops**, drawn as a bracket along the bottom edge with the slot they are recalled by — hollow where the running loop is solid, because they are the same idea in two states and a second colour for the second state is what §57 forbids. Pressing one plays it, which is §26's answer to the same layer; the pad page that recalls them could not light a single slot before this, because a saved loop is the library's and nothing carried it to the interface. And the **energy trajectory**, on the overview: how hard the drums drive, beat by beat, with 1.0 the record's own normal. The same measurement the breakdowns are thresholded from and divided by the same reference, so the band and the tick read as consequences of the line rather than as assertions beside it — and a record that never loses its drums has a trajectory too, which it used to lose along with the empty breakdown list. On the overview and not the lane because at 256 frames per pixel about ten beats are visible, over which a trajectory is a straight line. **Confidence** is drawn as well, and this row said otherwise: a grid the analyser doubts has been drawn faintly since `dj_render::tile::UNSURE_ALPHA` was written, and there is a test for it. Not drawn: vocal presence, stem presence, AI recommendations, crowd-response markers, and the runway to the end of the record |
| 26 | Direct manipulation on the waveform | 🟡 | Six of §26's nine examples ship: the **transition start and end**, the **cue markers**, the **loop**, whose edges are drawn and dragged, and the **phrase boundary**, which is dragged to the beat a phrase starts on. The frame under the hand goes to djmanzo, which snaps it to the nearest beat, clamps it into the record and refuses an empty slot; the lane draws what comes back. It goes through the same action the pads send, so it lands in the session log and is remembered with the record — a moved boundary is stored as the DJ's own answer, which a later re-analysis may not overwrite. A **saved loop** is on the lane too and is played from there, which is the sixth: not a drag, because what a DJ does with a region they can see is start it. Still numbers in panels: beat jumps, stem regions, AI suggestions |
| 27 | Preview / ghost track | 🟡 | **The ghost draws.** The incoming record is laid over the outgoing lane in the pair view, from the point the mix begins, at a zoom that makes one of its beats one of the outgoing's — a record drawn at its own frame rate would visibly drift against a lane it is meant to be beatmatched to. See-through and not recoloured: the ghost's colour is its own spectral balance exactly as the lane's is, and what tells them apart is solid against not. No second player and no second decode — the incoming deck's own tiles, which already exist. This row said the preview "needs a player djmanzo does not have", which was true of playing one and not of drawing one. Six of §27's seven bullets are answered: where the mix would land, where the outgoing record becomes weak, **where the incoming record's own breakdown and drop are** — drawn inside the ghost, so they fade with it: what the new record does is part of the preview and not a fact about the audio playing — the overlap, the key relation and the tempo movement. Missing: **where the vocal enters**, which needs the separator's output kept rather than only played |
| 28 | Stem-aware UI | ✅ | The stems module ships, folding so it costs a row when unused |
| 29 | Intelligent control handles | ⬜ | |
| 30 | Colour system named for meaning | 🟡 | Fourteen semantic roles exist as types with tests; the stylesheet still uses the appearance tokens |
| 31 | Theme adaptation | ⬜ | |
| 32 | Theme packs | 🟡 | Themes ship; packs as a format do not |
| 33 | Accessibility | 🟡 | Roles and labels throughout, and the tests query by role rather than by class. No audit has been run |
| 34 | Crowd / audience intelligence | 🟡 | RoomSense and audience requests ship |
| 35 | Room baseline | ⬜ | |
| 36 | Multi-signal crowd model | 🟡 | Light, movement, loudness and time of day ship |
| 37 | Causal crowd analysis | ⬜ | Needs action↔room time-series storage |
| 38 | Crowd signals never control the DJ unasked | ⚖️ | |
| 39 | UI for audience intelligence | 🟡 | RoomSense is nested inside the assistant rather than promoted |
| 40 | Assistant sees everything important | 🟡 | `SessionContext` is narrower than the directive's `DJContext`, but no longer empty where it matters most: the session phase and energy are read and published (§11) |
| 41 | AI can operate the GUI indirectly | ⬜ | ADR-0008 makes it possible — a layout is data — but nothing does it |
| 42 | Suggestions must be explainable | ✅ | The transition planner states where and how, with its reasoning |
| 43 | Suggestion fatigue | 🟡 | `Attention::performing()` caps suggestions at one; not yet consulted |
| 44 | Transactional AI actions | ⬜ | |
| 45 | Instant manual takeover | ✅ | Per parameter. Touching a control wins |
| 46 | Guardrails for autopilot | ✅ | Careful mode holds the controls that cannot be undone by pressing them again |
| 47 | Emergency UX | 🟡 | **Take over** sits in the top bar beside REC and Mark, which is the whole of what this section asks: *when something goes wrong, the user must not search through menus*. The gesture existed inside the Conduct panel, which is a panel you have to open. One press stops the automix, drops an acting assistant to `suggest`, and releases every control it could have moved — and **does not touch the audio**: no fader, no stop, no crossfader cut. §47 lists "PANIC → safe transition" and that is deliberately not this, because an emergency control that changes what a room can hear is worse than whatever it was pressed about, and there is no version of "safe" that is safe on every record. It says what it stopped, because a press that appears to do nothing cannot be told from one that failed. Not done: **return to last stable state**, which would move faders under a hand from a snapshot, and which djmanzo does not keep. `Attention::emergency()` is still a type nothing reads |
| 48 | Performance / laptop mode | 🟡 | The interface measures its own frame rate and says what a low one means. Density adapts |
| 49 | Professional workflow principle | ⚖️ | |
| 50 | Don't over-modalize | ✅ | The dock manager is this section: panels stopped taking turns |
| 51 | Command palette | ✅ | `Ctrl/Cmd + K`, assembled in Rust from `dj_core::vocabulary` and the cockpit's own surfaces rather than a written list, so it cannot offer a command djmanzo does not have. **What you type is an entry**: `deck 2 loop 8` parses, so the top row runs it — which is the only way the verbs taking an argument are reachable, and what §51 means by "the semantic interface exposed to voice/AI" |
| 52 | Hardware-first thinking | ⚖️ | The pad zone is a page strip and eight pads because that is what hardware has |
| 53 | Controller-aware GUI | 🟡 | Mappings ship; the interface does not reflect what is plugged in |
| 54 | Professional functional presets | 🟡 | Four layout presets ship; they are not the functional ones this asks for |
| 55 | Visual language architecture | ✅ | ADR-0009 and a validated token set |
| 56 | Visual feedback should be functional | ⚖️ | |
| 57 | Waveform colour must be semantic | 🟡 | Colour on the lane encodes two of the five things §57 lists: **frequency bands**, split at the mixer's own crossovers, and **phrase structure**. The rule that section actually turns on — *never overload the same colour with multiple meanings* — is now a test rather than an intention: `the_phrase_line_is_not_a_colour_the_waveform_can_be` measures each palette's marker against every mixture of its three bands, with and without the RMS veil, and it found that the phrase marker had always been drawn in **exactly** the high band's colour. Not encoded: stems, state, and incoming/outgoing identity. **Measured across all fourteen palettes** rather than argued about: every theme's `--accent` and `--warn` sit inside the waveform's colour space, and `pkg-industrial`'s `--warn` *is* the high band exactly. Those two tokens are the cue markers and the transition marks. Both are left as they are, because both carry §57's own escape clause — a numbered flag and a dashed line — and recolouring them would need fourteen palettes' worth of invention to fix something shape already answers. What is not left is a marker *tinting* the audio: the loop band was a sixteen-per-cent wash of `--accent-2` across the lane, so the spectral colouring inside a loop was a lie, and in one theme the wash was exactly the mid band. It is a bar at the bottom edge now, like the breakdown layer, over nothing |
| 58 | Information hierarchy (tiers) | 🟡 | The four tiers are modelled; the rail that would use them is not built |
| 59 | Density system | ✅ | Five bands, and the fixed-pixel blocks that ignored them are fixed — density moves a deck 122 px now, against 68 before |
| 60 | Resizability | ✅ | The band follows the window, with a test at five heights |
| 61 | Phone as a secondary surface | ✅ | Room sensor and audience requests over LAN |
| 62 | Audience camera privacy | ⚖️ | Local only; nothing leaves the machine |
| 63 | AI privacy | ⚖️ | Local models, a spend cap, and the budget shown |
| 64 | Competitor lessons | ✅ | Researched live and recorded in the audit with sources |
| 65 | Community research principle | ✅ | DJ forums are blocked at the egress gateway; web search was used instead and that limitation is written down |
| 66 | DJ workflow knowledge | ✅ | The workflow model is audit §9 |
| 67 | The session is a loop, not a screen | 🟡 | |
| 68 | Transition object | 🟡 | `dj_app::transition::Transition` ships: the two decks, where the mix starts and ends in frames and seconds, its length, style, tempo delta, key relation, the pair's confidence on the rail's scale, and the typed reasons. djmanzo **holds** one, it can be moved, shortened and restyled, and an edit **re-derives the reasons over the new geometry** rather than keeping the planner's. The Pair surface reads it. What §68 lists and it does not carry: the stem, EQ and FX plans, because nothing in djmanzo decides those yet. **The automix performs it**: with one set up, the mix runs where the planner chose and the DJ approved, into the deck they named, at their length and in their style — rather than out of the end of the file into whichever deck was free, which is all it can know by itself. **The autopilot performs it too**, and says whose mix it is — a human's length is not trimmed by the occasion, because §45's rule does not stop at the controls. It also carries what §68's own field list left out: **where the incoming record comes in**, in its own frames, and the tempo ratio that puts one of its beats on one of the outgoing's. `startPosition` says where on the outgoing record a mix begins and says nothing about what arrives there, which is the half a preview needs — see §27. Absent for a record with no phrase structure to enter on, which is a real answer rather than a failure. What does not read the object yet: replay, which still reconstructs a transition from the action log |
| 69 | Practice lab | ⬜ | |
| 70 | Learning mode | ✅ | The coach ships |
| 71 | "What should I do next?" | ✅ | The assistant's next step is shown before it happens |
| 72 | User override matrix | ⬜ | |
| 73 | AI knows what is expensive | ✅ | `mistakes_are_costly` reaches the deck as careful mode |
| 74 | Contextual control rail | 🟡 | **The rail ships.** Four modes — scratching, stems, preparing, mixing — each promoting six controls, on every loaded deck. Which mode a deck is in comes from what the DJ has just done: a hand on the platter, a muted stem, a stopped deck. That ordering matters as much as the lists do — every change is a consequence of an action they took, so the rail never rearranges itself under a hand reaching for it, which is the failure that makes adaptive interfaces hostile. Six because that is `Attention::performing().promoted_controls`, the tightest of the four budgets: sized to the tightest, the rail does not shrink when the music starts. §74's three named lists are followed where the vocabulary has a verb for them, and where it does not the gap is named rather than filled by inventing one — **stem FX** is a continuous per-stem filter and not a switch, and **tags, rating and transition points** are a library row and a panel. The fourth mode is a choice: §74 gives no list for the ordinary case of a record playing |
| 75 | Visual control of audio features | 🟡 | |
| 76 | Library "AI lens" | ⬜ | |
| 77 | Exploration vs performance | 🟡 | `cockpit::Focus` models it; nothing switches on it |
| 78 | "Freeze" | 🟡 | `Workspace.frozen` exists and is stored; nothing honours it |
| 79 | "Lock my workflow" | 🟡 | Same field, same gap |
| 80 | Persona learning | 🟡 | Taste ships; persona does not |
| 81 | User profile by context | ⬜ | |
| 82 | Performance metrics for the redesign | ✅ | Every claim in this work carries a measurement, and the ones that could not be measured say so |
| 83 | Implementation strategy | ✅ | Nine phases in `GUI-OVERHAUL.md` §21 |
| 84 | Test every step | ✅ | Clippy, the full Rust suite, svelte-check, vitest and Playwright green before each commit |
| 85 | Test muscle memory | ✅ | The golden deck order |
| 86 | Test the action bus | ✅ | Pre-existing and still green |
| 87 | Test state consistency | 🟡 | Loading from every source is not yet asserted to produce identical state |
| 88 | Test adaptation | ✅ | `density.spec.ts` — the deck fits at every window tall enough, and a shorter window never gets a looser interface |
| 89 | Visual regression | ⬜ | The ten workspace configurations are not captured |
| 90 | Performance regression | 🟡 | Geometry is ratcheted; frame rate is not |
| 91 | Do not overengineer prematurely | ⚖️ | |
| 92 | Do not migrate technology | ⚖️ | Still Svelte 5, TypeScript, Tauri 2, the action bus, the ParameterRegistry, the Rust waveform |
| 93 | Show exactly what matters next | ⚖️ | The governing principle, and what the pinning work served |
| 94 | Promote and compress; never hide unpredictably | ⚖️ | Density compresses; nothing is hidden. The deck's own map is unchanged |
| 95 | The assistant is a second DJ, not a chatbot | ⚖️ | |
| 96 | "Listening to the room with me" | ⚖️ | Confidence is explicit where the room is read |
| 97 | Theme is atmosphere; semantics are sacred | ⚖️ | The token whitelist enforces the boundary in code |
| 98 | A whole night without hunting through menus | 🟡 | Better — every performing control is one screen away — but the command palette and the rail are what finish it |
| 99 | Analysis before code | ✅ | 24 sections and 700 lines, written before any `.svelte` file was touched |
| 100 | Use existing functionality aggressively | ✅ | The audit's own finding: this is mostly an integration problem, exactly as this section predicts |
| 101 | The feature set is a hidden engine | ✅ | |
| 102 | Final output from this task | ✅ | `GUI-OVERHAUL.md`, and this file |
| 103 | Success criteria | 🟡 | See below — four of ten met |
| 104 | Coding style | ⚖️ | |
| 105 | Final instruction | ⚖️ | |

## §103's ten success criteria, judged honestly

| Criterion | | |
|---|---|---|
| Human DJ usability — usable without learning the AI | ✅ | The AI has never been in the way; §4 was honoured from the start |
| Professional density — fast adjustment without opening panels | ✅ | Every performing control is on one screen at 1280×800 |
| Adaptability — simple for a beginner, dense for a professional | 🟡 | Density adapts; the layouts that would express the two ends do not |
| Modularity — a personal workflow can be constructed | 🟡 | Surfaces dock and persist; workspaces cannot yet be saved from the interface |
| Predictability — adaptation never feels random | ✅ | Bands rather than a continuous ratio, so the interface settles; asserted by test |
| AI subtlety — everywhere useful, dominant nowhere | ✅ | |
| Instant takeover — human input always wins | ✅ | |
| Library quality — finding a track as strong as mixing it | 🟡 | Function tags, Set Flow, the pair view, the card view and a configurable fourteen-column table all ship. What is missing is the columns with no data behind them — vocal and stem availability, transition suitability, request count — and preview and a queue |
| Visual utility — visualisations carry actionable information | 🟡 | The waveform is coloured by spectral balance and carries the grid, downbeats, phrase starts, cues, the loop, the proposed transition, and **the breakdowns and drops** — the first two layers that answer *what is about to happen* rather than *what is there*. See §25 for the twelve of twenty that ship |
| Theme semantics — colour communicates | 🟡 | The roles exist; the stylesheet does not use them yet |

## The count

Of the 105 sections: **31 done, 40 part, 15 open, 19 standing rules.**

Counted by a script over this table rather than by hand, and the first hand
count was wrong in all four columns — which is the argument for the script.
Re-run it against this file:

```
python3 - <<'EOF'
import pathlib, re
from collections import Counter
rows = re.findall(r'^\| (\d{1,3}) \| .*?\| (✅|🟡|⬜|⚖️) \|',
                  pathlib.Path("docs/DIRECTIVE-STATUS.md").read_text(), re.M)
print(len(rows), Counter(m for _, m in rows))
EOF
```

Standing rules are counted separately on purpose. Folding them into "done"
would inflate the number — a constraint honoured is not a feature delivered —
and they cannot be "open" either, since they are being obeyed. Excluding them,
**31 of 86 deliverable sections are complete and 40 more are partly there.**

That is the same state the phase view calls "about 40%", counted a different
way: 31 whole plus 40 halves over 86 is 59%, and the phase view is stricter
because a phase only closes when its gate is met. Neither number is wrong;
the phase view is the one to quote, because a gate is a fact and a half is a
judgement.

The done work is deliberately front-loaded. `GUI-OVERHAUL.md` §20 records that
everything downstream depended on the widget-tree renderer; that dependency is
discharged, which is why sections 6, 23, 59 and 60 could ship at all.
