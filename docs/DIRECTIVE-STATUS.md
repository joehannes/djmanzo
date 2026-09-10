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
| 9 | Separate autonomy from confidence | ✅ | `dj_assistant::Warrant`. The invalid cell is **unrepresentable**: `Act` and `Mix` carry a `Grounds` whose constructor is private and refuses a certainty below `Fair`, so no caller anywhere can write down a warrant to act on a read something disagrees with. Asserted over the whole 6×3 matrix, and the consequence — an unclear night stages instead of mixing — is asserted in the autopilot |
| 10 | AI posture stays compatible with djmanzo's | ⚖️ | The six postures and nine occasions are untouched |
| 11 | Add a context engine | 🟡 | `dj_core::ContextEngine` ships and is the common input: the phase, its certainty, what produced it and which way the evidence disagrees, from two sources and nothing else — what the DJ declared, and where the last few minutes sit in the whole night's own range. `dj_app::night` feeds it from the snapshot the pump has just built. Five of `DJContext`'s eight fields are real (`sessionPhase`, `occasion`, `attentionBudget`, `audienceContext` through RoomSense, `performanceHealth` through the frame monitor); `musicContext`, `hardwareContext` and `djBehaviorContext` are not gathered yet |
| 12 | Learn the DJ | 🟡 | Taste learned from play history ships, and **persona learning now has its shape**: §13's `Tendency` for gestures and §81's `Profile` for whole kinds of night, both constructor-enforced so neither can generalise on too little. What is missing is the other half — nothing yet *reads* a profile: it is offered to the DJ and consulted by nothing |
| 13 | Never learn badly | ✅ | A **type**, not a warning. `dj_app::signals::Signal` carries the gesture and the phase the night was in as one value that cannot be taken apart, and `Tendency` — the only thing that generalises — is constructible solely through `tendencies()`: never without a phase, never on fewer than four occurrences *in that same phase*. So the directive's own bad sentence, "user likes enormous BPM jumps", is not one this workspace can produce; "you sometimes move the tempo by hand when the night is at its peak" is. The wording is written in Rust, so the interface cannot make a claim Rust would not |
| 14 | Behavioural signals | 🟡 | Twelve of §14's twenty gestures, derived from the action log rather than recorded separately — so a set from before this existed has them too. Coarser than the vocabulary on purpose: six EQ verbs are one gesture a DJ would name, and counting per verb would need six times the evidence. A test asserts every gesture named is one the bus can produce. The other six — track searched, previewed, staged, candidate rejected, candidate selected, assistant suggestion accepted — are not actions and are absent rather than approximated. Decay is not implemented: tonight's log is the window, which is the honest scope until sessions are read back across nights |
| 15 | AI should understand DJ technique | ✅ | The technique catalogue ships |
| 16 | Domain knowledge packs | 🟡 | Genre families ship; packs as a format do not |
| 17 | GUI adapts to session phase | 🟡 | Occasion-aware density ships, and peak time now narrows the attention budget — but only on a read worth acting on. Phase-driven *layout* does not |
| 18 | Attention budget | ✅ | `Attention::for_context` derives it in one place from the context engine, in order of severity — something broken, two records audible, peak time, room to think — and it is published on the snapshot and written onto the root as `data-motion`, where the stylesheet honours it. The rule that matters is enforced rather than hoped for: while two records are audible the interface may not reflow |
| 19 | No random UI reorganisation | ⚖️ | Enforced by a golden-order test: the deck's control order is asserted in full and fails if anything moves |
| 20 | Playlist / library overhaul | 🟡 | Function tags and their filtering ship, and **all four views** do. **Set Flow**: a dockable surface drawing the plan as a sequence with the seam between each pair — its deltas, its confidence, and whether it needs a cut rather than a blend. **Pair**: its own surface, the two records side by side with the seam between them, each with its waveform, and the mix point drawn *on* the outgoing one. **Cards**: sleeves read out of the files' own tags and served on their own URI scheme, each card carrying its operations — deck, set aside, more like this, favourite, and the reasons when the list is a suggestion; *preview* and *compare* are named as absent rather than faked, and *stage*/*prepare*/*queue* are djmanzo's one set-aside gesture. The performance table is the browser today, at fewer columns than §20 lists |
| 21 | "Prepare" must be first class | ✅ | Its own dockable surface beside the browser, not a strip inside it. One gesture — `→` on a browser row — hands a track over; `prepare.svelte.ts` is the only path between them, so there is no second, differently-behaved way to set a track aside |
| 22 | Next-track rail | 🟡 | The rail ships as its own dockable surface, following whichever deck is playing: up to eight candidates, each with one line of deltas (`+3 BPM · 8A→9A · +1 dB`), a confidence bar, and load / set aside / more-like-this / pin / pass. Two of the fifteen things §22 lists are not there — **audition**, which needs a preview player djmanzo does not have, and the **estimated transition type**, which means running the M8 planner per candidate |
| 23 | Track function tagging | ✅ | Ten functions, closed vocabulary, migration 10, browser picker, and `for is opener` in smart folders |
| 24 | Pairs and relationships | 🟡 | **"Save this transition"** ships: a *keep* on any of tonight's mixes stores the pair, directionally, with what the mix was — and keeping the same one again strengthens it rather than duplicating it, which is §24's confidence weight as a count. Only what was *kept* is stored: every mix a night contained is already derivable from the log, and a second copy of those would eventually disagree with it. The Next rail answers "why do I keep seeing these two together?" with a reason placed first, weighted below a key match and a tempo match together — a strong reason, not an override. What §24 also lists and this lacks: "A vocal → B instrumental" is now derivable — the style is in the log and `dj_app::shape` says what each style does to the stems, so a kept vocal drop *is* a record of one — but "works only with an 8-beat loop" needs a transition object that records the loop that was running, and "often selected after a crowd-energy drop" needs crowd sensing |
| 25 | Waveform overhaul | 🟡 | The architecture ships: `dj_render::layer` names §25's twenty layers, what each encodes, which half of the renderer draws it and whether it exists. **Eleven do** — amplitude, spectral balance, beats, phrases, downbeats, cues, the loop region, the seam, the mix-out window, the grid's own uncertainty and the runway. A golden file and a browser test keep the count honest in both directions: everything on screen carries a `data-layer` that must be in the table, in the overview as well as the lane. The nine that do not exist are named rather than forgotten, and reserve no colour |
| 26 | Direct manipulation on the waveform | 🟡 | The mix point is a handle on the outgoing waveform — drag it or use the arrow keys, and it moves in whole beats because a mix point between two beats is not on the grid. The waveform reports a position; Rust decides what it means and re-derives the reasons. The rest of §26's list — cue markers, phrase markers, loop edges, stem regions — is not draggable yet |
| 27 | Preview / ghost track | ⬜ | The object it waited on ships — see §68. What is missing is the preview itself: a second render of the outgoing track, which needs a player djmanzo does not have |
| 28 | Stem-aware UI | ✅ | The stems module ships, folding so it costs a row when unused |
| 29 | Intelligent control handles | 🟡 | **Progressive disclosure on a knob, with the table in Rust.** §29's last bullet — "MIDI = same underlying parameter" — is ADR-0003 stated as a control requirement, so `dj_app::handle` answers each gesture with **action text** rather than a number: a drag, a double-click, a menu entry and a MIDI CC all end up as the same action. Level one is the readout, which stays the interface's because it is about how a number reads. Level two is drag, **shift-drag at a quarter speed** (on the keyboard too), and **double-click to the parameter's own unity** — the one fact every call site used to spell out for itself, three EQ bands each naming `1` and the filter naming `0` with nothing making a fourth agree. Level three is a short contextual menu on **right-click or press-and-hold**, the second because a booth has trackpads and a control reachable only by right-click is one half the room cannot reach; it takes focus so Escape closes it, and every entry parses as an action, asserted by test. The menu is deliberately never more than four entries — §29's own warning is *do not turn every knob into a huge widget*, and a contextual menu is exactly where a knob grows into one. What §29 lists and this does not have: **the AI hover**, which would show what the assistant would set the control to — the assistant stages whole moves rather than single parameter values, so there is nothing yet for a hover to read. Six controls are covered (three EQ bands, filter, volume, pitch), not every parameter in the registry: a table of four hundred entries is one nobody keeps true |
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
| 40 | Assistant sees everything important | 🟡 | `SessionContext` now carries the phase, its certainty, its basis and the drift, and the autopilot reads the certainty. Still narrower than `DJContext`: the music, hardware and behaviour contexts are not gathered |
| 41 | AI can operate the GUI indirectly | ✅ | `dj_app::uiop` — `ui show prepare`, `ui pin room`, `ui focus 2`, generated from `cockpit::surfaces()` so a panel djmanzo does not have cannot be asked for. Gated by §72's `adapt_layout` row, applied in Rust and announced on an event, because a panel the assistant opened has to appear without anybody pressing anything. Reachable by hand from the palette too. Density is deliberately excluded: the interface measures its own |
| 42 | Suggestions must be explainable | ✅ | The transition planner states where and how, with its reasoning |
| 43 | Suggestion fatigue | 🟡 | `Attention::performing()` caps suggestions at one and the cap is now derived and published; no surface draws suggestions against it yet |
| 44 | Transactional AI actions | ✅ | `dj_app::staged`. The whole next transition — load, cue, trim, sync, mix — staged as one thing with Accept, Modify and Reject, in a strip under the top bar rather than a panel. Accepting runs every chosen move through the same `perform_step` the automatic tick uses, so there is no second execution path and the whole thing logs and replays. Partial success is reported as partial |
| 45 | Instant manual takeover | ✅ | Per parameter. Touching a control wins |
| 46 | Guardrails for autopilot | ✅ | Careful mode holds the controls that cannot be undone by pressing them again |
| 47 | Emergency UX | ✅ | **SAFE**, beside REC and Mark, and `safe` on the action bus so it is also on a controller and in a script. Takes every control back, drops anything staged, clears every rack, flattens the EQ and filter, restores master gain and the limiter. It never stops a record, moves a fader or touches the crossfader — asserted over the text of every action it expands into, because what it refuses to do is the part that matters |
| 48 | Performance / laptop mode | 🟡 | The interface measures its own frame rate and says what a low one means. Density adapts |
| 49 | Professional workflow principle | ⚖️ | |
| 50 | Don't over-modalize | ✅ | The dock manager is this section: panels stopped taking turns |
| 51 | Command palette | ✅ | `Ctrl/Cmd + K`, assembled in Rust from `dj_core::vocabulary` and the cockpit's own surfaces rather than a written list, so it cannot offer a command djmanzo does not have. **What you type is an entry**: `deck 2 loop 8` parses, so the top row runs it — which is the only way the verbs taking an argument are reachable, and what §51 means by "the semantic interface exposed to voice/AI" |
| 52 | Hardware-first thinking | ⚖️ | The pad zone is a page strip and eight pads because that is what hardware has |
| 53 | Controller-aware GUI | 🟡 | Mappings ship; the interface does not reflect what is plugged in |
| 54 | Professional functional presets | 🟡 | Four layout presets ship; they are not the functional ones this asks for |
| 55 | Visual language architecture | ✅ | ADR-0009 and a validated token set |
| 56 | Visual feedback should be functional | ⚖️ | |
| 57 | Waveform colour must be semantic | ✅ | Every drawn layer declares what its colour *means*, and a test refuses two unrelated layers on one meaning — a property of the set, which is why the set exists. Grouped roles are named and counted: the three grid layers are one meaning at three weights, which is texture rather than a second colour. An unbuilt layer reserves nothing |
| 58 | Information hierarchy (tiers) | 🟡 | The four tiers are modelled; the rail that would use them is not built |
| 59 | Density system | ✅ | Five bands, and the fixed-pixel blocks that ignored them are fixed — density moves a deck 122 px now, against 68 before |
| 60 | Resizability | ✅ | The band follows the window, with a test at five heights |
| 61 | Phone as a secondary surface | ✅ | Room sensor and audience requests over LAN |
| 62 | Audience camera privacy | ⚖️ | Local only; nothing leaves the machine |
| 63 | AI privacy | ⚖️ | Local models, a spend cap, and the budget shown |
| 64 | Competitor lessons | ✅ | Researched live and recorded in the audit with sources |
| 65 | Community research principle | ✅ | DJ forums are blocked at the egress gateway; web search was used instead and that limitation is written down |
| 66 | DJ workflow knowledge | ✅ | The workflow model is audit §9 |
| 67 | The session is a loop, not a screen | 🟡 | The action log has been the session since M0, and it now *contains transitions* the way §67 lists them: `dj_app::mixes` reads a night back as the mixes in it — what went into what, over how many beats, and what kind of mix each was — derived from the log rather than recorded beside it, so a set recorded before any of this has them too. **Tonight's mixes** is the surface. The rest of §67's list is elsewhere and not yet gathered into one object: the timeline, the room, the DJ's own state, requests and the set arc each have a home of their own |
| 68 | Transition object | ✅ | `dj_app::transition::Transition` carries every field §68 lists: the two decks, where the mix starts and ends in frames and seconds, its length, style, tempo delta, key relation, the pair's confidence on the rail's scale, the typed reasons, and now the **stem, EQ and FX plans**. Those four come from `dj_app::shape`, which is one table saying what a style does beyond the faders — and it is the table **the automix performs**, so what the pair view says a style will do before it is pressed is what happens when it is. They are derived from the style rather than stored on the object, because a stored copy is a second answer that a restyle can leave behind. djmanzo **holds** a transition; it can be moved on the waveform, shortened and restyled, and an edit **re-derives the reasons over the new geometry** rather than keeping the planner's. **The automix performs it** — its decks, its start, its length, its style — instead of deciding its own, and the autopilot defers to it rather than pushing settings in; where it does not apply it is ignored rather than forced. The object exists on **both** sides of a mix: `dj_app::mixes` derives the performed one out of the log, and it **drives replay** — `replay::Window` renders one handover back to a WAV with its run-up, from the set the DJ is playing. Everything before the window is still rendered and discarded, because the engine's state at a moment is the whole set up to it; that cost is stated rather than hidden. Of the seven things §68 says such an object could drive, five do: the waveform, the suggestions, the AI preparation, the autopilot and the replay. The other two are not the object's absence but their own sections' — preview is §27, which needs a player, and practice is §69 |
| 69 | Practice lab | 🟡 | **Two records explored without altering the live master**, which is the sentence §69 rests on. `dj_app::practice` builds a set file for a mix that was never played and `replay` renders it headless — no fader moves, no deck is touched, and the record playing to the room keeps playing. **The real automix writes it**: it is stepped offline against a simulated playhead and the actions it emits *are* the rehearsal, so what you hear is what djmanzo would perform rather than a second implementation of a crossfade. Rehearsing an alternative style **does not restyle the held mix** — the transition is cloned — so the four files of one pair are §69's "hear alternative transitions" as things on disk. A rehearsal is cheap where a replay is not: it is synthetic, so it has no history to be faithful to and costs its own length whatever hour of the night it is. "Save successful transitions" is §24's *keep*, which ships. Comparing BPM, phrase structure and keys is the pair view's and is drawn there rather than a second time. What §69 lists and this does not have: experimenting with stems, testing FX and creating loops **live** — those need a second engine feeding the headphone output, which is real work and is not this. Nothing here can say whether any of it *sounds* right; there is no audio device in this container |
| 70 | Learning mode | ✅ | The coach ships |
| 71 | "What should I do next?" | ✅ | The assistant's next step is shown before it happens |
| 72 | User override matrix | ✅ | `dj_assistant::authority` — the directive's ten capabilities against six postures, verbatim and asserted against the directive's own table. The second of three gates the assistant passes, so it is what actually stops a mix rather than a diagram. Configurable both ways, except that Off and Watch cannot be widened |
| 73 | AI knows what is expensive | ✅ | `mistakes_are_costly` reaches the deck as careful mode |
| 74 | Contextual control rail | ✅ | **At hand**: `dj_app::at_hand` reads the snapshot for what the hands are doing — a platter touched, stems being played, a record against another, one cued and waiting — and answers with four to eight controls, each an action the parser already accepts. A latched one offers the other half, so "bass out" becomes "bass in" once the low band is out. The panel names the deck and why, because a row that silently became a different row is a row nobody trusts. Which deck is its own judgement: §41's focus fades after six seconds by design, so it follows the hands instead. §74's stem FX, tags, rating and transition points are named as absent — a rack of four numbers, and three things that belong to a record rather than a deck |
| 75 | Visual control of audio features | 🟡 | |
| 76 | Library "AI lens" | 🟡 | **A toggle that adds, and can never replace.** §76 closes with "this must never replace the standard library view", and the shape keeps that true rather than promising it: `library_lens` is handed the ids the table is *already showing* and answers about those. It does not query, filter or order the collection, so turning it off leaves the standard view exactly as it was — because the lens was never inside it, and a browser test compares the standard headings with it on and off. Six of §76's eight columns ship, each from something djmanzo already knows: **likely next** from the same scorer the Next rail uses, **user affinity** from `dj_library::learned`, **phase suitability** from the record's own function tags against the night's phase, **transition risk** as the scorer's own bad news kept as named reasons rather than boiled to a number, **novelty and familiarity** from the play count and the last-played date, and **function tags**. A record cannot follow itself, so the row for what is playing has no next and no risk. A column djmanzo cannot answer is **blank, never zero** — an empty cell means no opinion and a zero means bad, and they are different answers. What §76 lists and this does not have: **crowd suitability**, which needs to know what the room is doing and so needs a camera or a microphone in it — named in the header rather than silently dropped. And novelty and familiarity are this DJ's own history, not the room's ears: the crowd's familiarity with a record is not something djmanzo can know |
| 77 | Exploration vs performance | 🟡 | `cockpit::Focus` models it; nothing switches on it |
| 78 | "Freeze" | 🟡 | `Workspace.frozen` exists and is stored; nothing honours it |
| 79 | "Lock my workflow" | 🟡 | Same field, same gap |
| 80 | Persona learning | 🟡 | Taste ships; persona does not |
| 81 | User profile by context | 🟡 | **Conditional profiles, never one universal one.** `dj_app::setting::Setting` is §81's own six — club, beach, wedding, latin, practice, open format — and it is **told, never inferred**: djmanzo reads the arc of a night from the music and is right to, but nothing in the signal says *wedding*, and guessing it would file a whole night's habits under the wrong name. A `nights` row per session carries the setting, which is the one thing nothing can derive. Genre weights are **derived** from `history ⋈ nights ⋈ tracks`; the other four of §81's five are read off the action log as the night goes, because the log does not outlive the run that made it. `dj_app::profile::Profile` has private fields and is constructible only through `profiles()` — §13's discipline at the scale of a whole night: nothing at all under three nights of a setting, and each field silent until half the nights that spoke agree. A gesture counts once per night, not once per press. The sentence is written in Rust and always names the setting and the evidence. Drawn in two places for two reasons: the picker sits in **The night**, beside the arc it is the other axis of; the profiles sit in the assistant beside §13's tendencies, because both are djmanzo saying what it worked out about this DJ. What §81 lists and this does not have: nothing in the list, but the profiles are **read and not yet acted on** — no suggestion, layout or posture consults them, which is what would make them worth having |
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
| Library quality — finding a track as strong as mixing it | 🟡 | Function tags, Set Flow and the pair view ship; two of §20's four views and the wider column set do not |
| Visual utility — visualisations carry actionable information | ⬜ | The waveform still draws amplitude only |
| Theme semantics — colour communicates | 🟡 | The roles exist; the stylesheet does not use them yet |

## The count

Of the 105 sections: **41 done, 40 part, 5 open, 19 standing rules.**

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
**41 of 86 deliverable sections are complete and 40 more are partly there.**

That is the same state the phase view calls "about 40%", counted a different
way: 41 whole plus 40 halves over 86 is 71%, and the phase view is stricter
because a phase only closes when its gate is met. Neither number is wrong;
the phase view is the one to quote, because a gate is a fact and a half is a
judgement.

The done work is deliberately front-loaded. `GUI-OVERHAUL.md` §20 records that
everything downstream depended on the widget-tree renderer; that dependency is
discharged, which is why sections 6, 23, 59 and 60 could ship at all.
