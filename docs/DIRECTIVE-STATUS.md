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
| 11 | Add a context engine | ⬜ | Phase 5 |
| 12 | Learn the DJ | 🟡 | Taste learned from play history ships. Persona learning does not |
| 13 | Never learn badly | ⬜ | |
| 14 | Behavioural signals | ⬜ | Needs an events table with decay |
| 15 | AI should understand DJ technique | ✅ | The technique catalogue ships |
| 16 | Domain knowledge packs | 🟡 | Genre families ship; packs as a format do not |
| 17 | GUI adapts to session phase | 🟡 | Occasion-aware density ships; phase-driven layout does not |
| 18 | Attention budget | 🟡 | `cockpit::Attention` exists with the rule that matters — while performing, the interface may not reflow — but nothing consults it yet |
| 19 | No random UI reorganisation | ⚖️ | Enforced by a golden-order test: the deck's control order is asserted in full and fails if anything moves |
| 20 | Playlist / library overhaul | 🟡 | Function tags and their filtering ship. The four views — table, cards, **Set Flow**, pair — do not. Set Flow is the strongest competitor gap |
| 21 | "Prepare" must be first class | 🟡 | The gesture works today (Library → sidelist → deck, one press each). It is not yet a top-level surface |
| 22 | Next-track rail | 🟡 | Suggestions exist inside the sidelist; there is no rail with deltas and reasons |
| 23 | Track function tagging | ✅ | Ten functions, closed vocabulary, migration 10, browser picker, and `for is opener` in smart folders |
| 24 | Pairs and relationships | ⬜ | Needs new storage with confidence decay |
| 25 | Waveform overhaul | ⬜ | |
| 26 | Direct manipulation on the waveform | ⬜ | |
| 27 | Preview / ghost track | ⬜ | Needs `TransitionPlan` as a first-class object |
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
| 40 | Assistant sees everything important | 🟡 | `SessionContext` is narrower than the directive's `DJContext` |
| 41 | AI can operate the GUI indirectly | ⬜ | ADR-0008 makes it possible — a layout is data — but nothing does it |
| 42 | Suggestions must be explainable | ✅ | The transition planner states where and how, with its reasoning |
| 43 | Suggestion fatigue | 🟡 | `Attention::performing()` caps suggestions at one; not yet consulted |
| 44 | Transactional AI actions | ⬜ | |
| 45 | Instant manual takeover | ✅ | Per parameter. Touching a control wins |
| 46 | Guardrails for autopilot | ✅ | Careful mode holds the controls that cannot be undone by pressing them again |
| 47 | Emergency UX | ⬜ | `Attention::emergency()` exists as a type only |
| 48 | Performance / laptop mode | 🟡 | The interface measures its own frame rate and says what a low one means. Density adapts |
| 49 | Professional workflow principle | ⚖️ | |
| 50 | Don't over-modalize | ✅ | The dock manager is this section: panels stopped taking turns |
| 51 | Command palette | ⬜ | 148 commands are ready for one |
| 52 | Hardware-first thinking | ⚖️ | The pad zone is a page strip and eight pads because that is what hardware has |
| 53 | Controller-aware GUI | 🟡 | Mappings ship; the interface does not reflect what is plugged in |
| 54 | Professional functional presets | 🟡 | Four layout presets ship; they are not the functional ones this asks for |
| 55 | Visual language architecture | ✅ | ADR-0009 and a validated token set |
| 56 | Visual feedback should be functional | ⚖️ | |
| 57 | Waveform colour must be semantic | ⬜ | The renderer emits amplitude only |
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
| 68 | Transition object | ⬜ | The planner exists; the object does not |
| 69 | Practice lab | ⬜ | |
| 70 | Learning mode | ✅ | The coach ships |
| 71 | "What should I do next?" | ✅ | The assistant's next step is shown before it happens |
| 72 | User override matrix | ⬜ | |
| 73 | AI knows what is expensive | ✅ | `mistakes_are_costly` reaches the deck as careful mode |
| 74 | Contextual control rail | ⬜ | Phase 3's remaining half |
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
| Library quality — finding a track as strong as mixing it | ⬜ | Function tags are a start; the four views are the answer |
| Visual utility — visualisations carry actionable information | ⬜ | The waveform still draws amplitude only |
| Theme semantics — colour communicates | 🟡 | The roles exist; the stylesheet does not use them yet |

## The count

Of the 105 sections: **29 done, 33 part, 24 open, 19 standing rules.**

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
**29 of 86 deliverable sections are complete and 33 more are partly there.**

That is the same state the phase view calls "about 40%", counted a different
way: 29 whole plus 33 halves over 86 is 53%, and the phase view is stricter
because a phase only closes when its gate is met. Neither number is wrong;
the phase view is the one to quote, because a gate is a fact and a half is a
judgement.

The done work is deliberately front-loaded. `GUI-OVERHAUL.md` §20 records that
everything downstream depended on the widget-tree renderer; that dependency is
discharged, which is why sections 6, 23, 59 and 60 could ship at all.
