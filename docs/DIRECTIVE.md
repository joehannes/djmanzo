# The GUI directive, as the owner wrote it

This is the brief for the cockpit overhaul, reproduced **verbatim and
unedited**. It is 105 numbered sections plus the framing around them.

It is in the repository for one reason: it was not, and that was a real risk.
For several sessions it existed only in a conversation transcript, and
answering "where are we in the plan" meant recovering it from a JSONL file. A
brief that lives in a chat log is a brief that is one expired session away from
being lost, and the work that depends on it becomes unauditable.

## How to read it alongside the rest

- [`DIRECTIVE-STATUS.md`](DIRECTIVE-STATUS.md) marks every one of the 105 as
  done, part, open or a standing rule, with a script that counts the table so
  the number can be checked rather than taken.
- [`GUI-OVERHAUL.md`](GUI-OVERHAUL.md) is the analysis §99 asks for: the same
  material turned into nine phases with gates.

Where this file and an implementation disagree, **this file is what was
asked for**. The owner's framing is in the first three lines below, and it is
the governing constraint on how to read everything after it: *keep the
application, enhance it, do not coldly rewrite it.*

---

while you have been improving the GUI, I've been dreaming of what else I want to do to the APP/GUI and I've come up with my GUI redesign.
Basically I want to keep the App and augment and enhance and improve it and make it more interactive.
Please read following instructions carefully and follow them 99% and pretty literally. Why I am not saying 100% is, you coded the app and I want integrity to be kept and the App to be enhanced, not coldly rewritten. So  keep almost everything, and improve GUI/workflow and AI/AI assistance, as follows.
First: Still have github produce an installable deb package or produce it yourself and present it as downloadable link.
Then proceed with this:
# DJMANZO — ADAPTIVE PROFESSIONAL DJ COCKPIT
## Claude Code / Claude Opus 5 Engineering & Product Directive
You are working directly inside the `joehannes/djmanzo` repository.
Your task is to perform a **deep product, UX, workflow, visual-system, information-architecture and implementation overhaul of djmanzo's GUI and its surrounding adaptive-assistant experience**.
This is NOT a cosmetic restyling task.
Do not merely change colors, fonts, borders, spacing and shadows while retaining essentially the same interface architecture.
The objective is to evolve djmanzo into a **professional, adaptive, modular DJ performance environment** that can serve:
* complete beginners,
* hobbyist DJs,
* mobile/event DJs,
* wedding/private-event DJs,
* club DJs,
* open-format DJs,
* hip-hop / turntablist DJs,
* house / techno DJs,
* Latin / Caribbean DJs,
* VJs / audiovisual performers,
* advanced stem/remix performers,
* CDJ/DVS/controller users,
* laptop-only users,
* AI-assisted DJs,
* partially autonomous DJs,
* and fully supervised-autopilot sessions.
The application must remain immediately understandable and usable by a human DJ even when every AI feature is disabled.
The central philosophy is:
> **The system should adapt the presentation to the DJ, rather than forcing the DJ to adapt to the application.**
The interface must become more intelligent without becoming more intrusive.
---
# 1. FIRST: UNDERSTAND THE ACTUAL REPOSITORY
Before changing code, inspect the repository thoroughly.
Read the entire relevant contents of:
* `README.md`
* `docs/ARCHITECTURE.md`
* `docs/ASSISTANT.md`
* `docs/FEATURES.md`
* `docs/CONTROLLERS.md`
* `docs/KARAOKE.md`
* `docs/NETWORK-API.md`
* `docs/PARITY-AUDIT.md`
* `docs/QUICKSTART.md`
* `docs/RESEARCH.md`
* `docs/ROADMAP.md`
* `docs/SOURCES.md`
* `docs/VISUAL-LANGUAGE.md`
* `docs/VISUAL-TECH.md`
* relevant ADRs under `docs/adr/`
Then inspect the actual implementation rather than assuming the documentation perfectly matches the code.
At minimum inspect:
* `ui/src/App.svelte`
* `ui/src/Deck.svelte`
* `ui/src/Assistant.svelte`
* `ui/src/Browse.svelte`
* `ui/src/Library.svelte`
* `ui/src/Crates.svelte`
* `ui/src/Conduct.svelte`
* `ui/src/Coach.svelte`
* `ui/src/Fx.svelte`
* `ui/src/MasterMixer.svelte`
* `ui/src/Pads.svelte`
* `ui/src/Overview.svelte`
* `ui/src/Waveform.svelte`
* `ui/src/JogWheel.svelte`
* `ui/src/Automix.svelte`
* `ui/src/Sampler.svelte`
* `ui/src/Controllers.svelte`
* `ui/src/MappingEditor.svelte`
* `ui/src/Journal.svelte`
* `ui/src/Presets.svelte`
* `ui/src/Settings.svelte`
* `ui/src/ThemeSwitcher.svelte`
* `ui/src/Detached.svelte`
* the API/state files
* layout-related code
* world/living-interface code
* audio visualization support
* keyboard/controller mapping code
* tests and e2e tests
Inspect git history where useful to understand why existing UI decisions were made.
Do NOT rewrite architecture merely because something looks unconventional.
The repository deliberately separates realtime audio from UI concerns.
Preserve the realtime guarantees.
---
# 2. CURRENT ARCHITECTURE — TREAT THIS AS AN ASSET
The existing architectural ideas are valuable and should survive.
Current architecture includes, among other things:
* Rust realtime core
* Tauri 2
* Svelte 5 + TypeScript UI
* typed Action bus
* ParameterRegistry
* UI/controller/keyboard/script/network/assistant convergence onto the same Action vocabulary
* state snapshots
* deterministic session logging/replay
* Rust-side waveform rendering
* worker-based expensive processing
* cached analysis
* stem preparation ahead of the playhead
* configurable layouts
* CSS variable based theming/density
* MIDI/HID/controller mappings
* assistant integration
* audience request functionality
* audience sensing
* taste learning
* DJ technique catalogue
* planning
* automatic set assembly
* assistant takeover
* living/watershed visual language
Do not replace Svelte with another framework.
Do not move waveform rendering into an expensive per-frame DOM/JS loop.
Do not put AI, vision, network, filesystem, logging or expensive analysis onto the audio callback.
Do not bypass the Action bus.
Do not introduce a second hidden control path.
The GUI should remain a client of the same control system.
---
# 3. PRODUCT THESIS
Redefine the UI as:
## Adaptive Performance Environment
The application is a **modular cockpit**, not a fixed mixer skin.
The UI consists of composable surfaces and zones that can be:
* docked,
* resized,
* collapsed,
* expanded,
* stacked,
* detached,
* temporarily surfaced,
* pinned,
* contextually promoted,
* contextually demoted,
* or automatically rearranged.
However:
## CRITICAL RULE
The system may change **presentation priority**, but must not arbitrarily change **semantic control identity**.
A Play button must remain a Play button.
Deck 1 remains Deck 1.
A cue remains a cue.
A crossfader remains a crossfader.
A control can become more prominent, smaller, hidden behind an expandable surface, or moved into a contextual tool rail — but core semantics must remain stable.
Muscle memory is sacred.
---
# 4. DO NOT BUILD "AN AI DASHBOARD"
This is one of the most important requirements.
The AI should NOT dominate the interface.
Do not create a giant ChatGPT-like assistant occupying half the screen.
The AI is fundamentally a **context engine underneath the GUI**.
Its presence should normally be subtle.
It should surface:
* a recommendation,
* a preparation,
* a visual hint,
* a tiny status indicator,
* a confidence marker,
* a one-line reason,
* a staged action,
* an alert,
* or a temporary contextual control.
The DJ should not have to “go talk to the AI” to benefit from it.
The AI should work quietly in the background.
The normal experience should be:
> “The software seems to understand what I am about to do.”
not:
> “I am operating an AI application.”
---
# 5. THE NEW GUI MODEL
Design the application around these persistent conceptual regions:
## A. Mission Bar
Always-present but compact.
Contains only the most important live state:
* current session phase
* current occasion
* AI posture
* room status
* master/output health
* recording state
* current tempo
* maybe current time / set duration
* compact alerts
* hardware/device status
* performance confidence/warnings
It should behave more like an aircraft HUD than a conventional application toolbar.
Do not fill it with menus.
---
## B. Performance Zone
Primary visual area.
Normally contains:
* 2 decks
* 4 decks
* or 6 decks depending on context/layout/hardware
But it must support radically different arrangements.
Examples:
### Classic 2-deck
Large decks, central mixer, library below.
### 4-deck performance
Four readable decks with shared waveform intelligence.
### Club mode
Large central stacked waveforms, compact decks and a very readable mixer.
### Stem performance
Large stem-aware waveform and stem controls become first-class.
### Scratch mode
Jog surfaces and turntable-oriented controls expand.
### Laptop compact mode
Dense controls optimized for limited screen height.
### Preparation mode
Performance area shrinks and browser/planning surfaces expand.
### Practice mode
Two tracks become a comparative laboratory.
### Autopilot supervisory mode
Performance display becomes simplified and emphasizes:
* current
* next
* transition
* room response
* automation state
* emergency takeover
Do not make all these separate applications.
They are configurations of the same adaptive surface system.
---
# 6. DOCK / SURFACE MANAGER
The current `App.svelte` panel model is too restrictive if it permits essentially one side panel at a time.
Replace that conceptual limitation with a general **Surface/Dock Manager**.
A surface might be:
* Library
* Browser
* Prepare
* Playlist
* Set Plan
* Suggestions
* Assistant
* Conduct
* Coach
* Automix
* Sampler
* FX
* Stems
* History
* Requests
* Journal
* Audience
* Controllers
* Mapping
* Settings
* Session
* Practice
* Track Analysis
* Transition Lab
A surface should have metadata such as:
```ts
type SurfaceDefinition = {
  id: string;
  title: string;
  category: "performance" | "library" | "planning" | "assistant" | "utility";
  minSize: { width: number; height: number };
  preferredSize: { width: number; height: number };
  priority: number;
  performanceCritical: boolean;
  detachable: boolean;
  stackable: boolean;
  collapsible: boolean;
  contextual: boolean;
};
```
The surface system must support a persistent user configuration.
Store layout state as data, not scattered component conditionals.
Example concept:
```ts
type Workspace = {
  name: string;
  surfaces: SurfacePlacement[];
  density: DensityProfile;
  focus: WorkspaceFocus;
  theme: ThemeProfile;
};
```
---
# 7. WORKSPACE PRESETS
Ship intelligent workspace presets.
At minimum:
* Beginner
* Classic DJ
* Pro Performance
* 4 Deck
* 6 Deck
* Club
* Mobile DJ
* Wedding/Event
* Open Format
* Latin / Caribbean
* Scratch / Turntablism
* Stem Performance
* Mashup / Remix
* Preparation
* Set Planning
* Practice / Learning
* Autopilot
* Minimal
* High Contrast
* Laptop Compact
* Controller Focus
* CDJ / External Mixer Focus
* VJ / Visual Performance
* Karaoke / MC
But make these **starting points**, not rigid identities.
Every preset should remain editable.
---
# 8. ADAPTATION LEVELS
The application needs multiple layers of adaptive behavior.
## Level 0 — Static
User controls everything.
No learning.
No dynamic layout.
## Level 1 — Remember
The software remembers:
* workspace
* density
* library columns
* sorting
* panel positions
* preferred decks
* favorite pad pages
* preferred waveform display
* preferred controls.
## Level 2 — Suggest
The system recommends but never changes the state.
## Level 3 — Prepare
The system stages:
* next track
* cue position
* gain
* transition point
* loop
* likely stem mode
* browser result
* preparation surfaces
but waits for commitment.
## Level 4 — Assist
The system may make low-risk, reversible changes.
Examples:
* tiny gain normalization
* sync correction
* beat-grid confidence handling
* staging a candidate
* positioning a suggested cue
* keeping a transition aligned
* preparing a stem.
## Level 5 — Adaptive
The interface itself changes:
* density
* promoted controls
* surface sizes
* visual emphasis
* suggestion frequency
* theme
* information density.
## Level 6 — Autopilot
The assistant performs the mix under explicit guardrails.
The DJ can override immediately.
---
# 9. SEPARATE TWO CONCEPTS:
## AUTONOMY
How much the machine is allowed to do.
## CONFIDENCE
How certain the system is.
These are not the same.
Examples:
* high confidence + low autonomy = “I am very sure, but I will only suggest.”
* low confidence + high autonomy = invalid/unsafe.
* high confidence + high autonomy = automatic execution is possible.
* low confidence + low autonomy = show uncertainty / ask / do nothing.
Make this distinction fundamental throughout the system.
---
# 10. AI POSTURE SHOULD REMAIN COMPATIBLE WITH EXISTING DJMANZO
Preserve the conceptual posture model already documented:
* Off
* Watch
* Suggest
* Prepare
* Assist
* Autopilot
Do not replace it with dozens of independent switches.
Use these as the main autonomy axis.
---
# 11. ADD A CONTEXT ENGINE
Build an explicit internal concept:
```ts
type DJContext = {
  sessionPhase:
    | "setup"
    | "warmup"
    | "build"
    | "peak"
    | "release"
    | "cooldown"
    | "closing"
    | "emergency";
  occasion: OccasionProfile;
  musicContext: MusicContext;
  hardwareContext: HardwareContext;
  audienceContext: AudienceContext;
  djBehaviorContext: BehaviorContext;
  attentionBudget: AttentionBudget;
  performanceHealth: PerformanceHealth;
};
```
The context engine should become the common input to:
* adaptive GUI
* theme engine
* suggestions
* session planning
* assistant
* audience sensing
* visualization
* technique recommendations
* automation policy
Do not duplicate context logic inside each component.
---
# 12. LEARN THE DJ
The assistant should observe normal interactions.
Learn patterns such as:
* typical library sorting
* favorite metadata columns
* common search patterns
* preferred decks
* preferred transitions
* preferred transition durations
* preferred FX
* common FX combinations
* common pad pages
* cue conventions
* favorite loop sizes
* favorite stem operations
* common BPM ranges
* common harmonic movement
* genre preferences
* venue-specific behavior
* session-phase-specific behavior
* how quickly the DJ changes plans
* which recommendations are accepted
* which are ignored
* which are manually modified
* which staged actions get overridden
* how much assistance is normally tolerated.
But do NOT infer preferences from one or two actions.
Use:
* recency
* frequency
* context
* confidence
* decay
* explicit user preference
* acceptance/rejection feedback.
---
# 13. NEVER LEARN BADLY
Do not silently convert unusual behavior into permanent preference.
Example:
A DJ raises BPM dramatically once because the crowd suddenly explodes.
Do not conclude:
> "User likes enormous BPM jumps."
Instead:
> "Large jumps occasionally occur in high-energy contexts."
Contextual learning is essential.
---
# 14. BEHAVIORAL SIGNALS
Track useful events from the existing Action/event architecture.
Potential events:
* track searched
* track previewed
* track loaded
* track unloaded
* track staged
* candidate rejected
* candidate selected
* cue created
* cue moved
* loop created
* loop resized
* stem changed
* EQ change
* filter sweep
* FX action
* crossfader movement
* tempo movement
* sync enabled/disabled
* assistant suggestion accepted
* suggestion rejected
* staged action manually changed
* recommendation ignored
* track ejected early
* next track selected manually
* assistant takeover
* manual takeover
* workspace changed
* surface opened
* surface closed
* control density changed
* theme overridden.
The system should learn from the **sequence**, not isolated events.
---
# 15. AI SHOULD UNDERSTAND DJ TECHNIQUE
Expand the existing DJ technique catalogue into a formal knowledge model.
Each technique should encode:
```ts
type DJTechnique = {
  id: string;
  name: string;
  category: string;
  difficulty: "beginner" | "intermediate" | "advanced" | "expert";
  prerequisites: string[];
  requiredHardware?: string[];
  compatibleGenres: string[];
  compatibleContexts: string[];
  suitablePhases: string[];
  risk: "low" | "medium" | "high";
  reversibility: "instant" | "easy" | "manual";
  assistability:
    | "suggest"
    | "prepare"
    | "assist"
    | "autopilot";
  actions: string[];
  rationale: string;
};
```
Include comprehensive techniques.
Examples:
* beatmatching
* phrase matching
* EQ swap
* bass swap
* filter transition
* echo out
* reverb wash
* loop transition
* loop roll
* beat jump
* double drop
* acapella overlay
* instrumental overlay
* stem swap
* vocal tease
* rhythmic layering
* mashup
* breakdown transition
* drop transition
* cut transition
* hard cut
* quick fade
* long blend
* tempo ramp
* half-time transition
* double-time transition
* harmonic transition
* key shift
* scratch
* baby scratch
* chirp
* flare
* transform
* stab
* backspin
* brake
* reverse/censor
* sampler layering
* percussion layering
* drum substitution
* vocal extraction
* emergency loop
* emergency echo
* emergency safe-track selection
* mic ducking
* crowd-request handling
* wedding multi-generational transitions
* open-format reset
* peak-time escalation
* energy release
* closing transition.
Do not reduce professional DJ knowledge to BPM + Camelot.
---
# 16. CREATE DOMAIN KNOWLEDGE PACKS
Make DJ knowledge extensible.
At minimum define packs for:
* General
* Beginner
* Open Format
* House
* Techno
* EDM
* Hip-Hop
* R&B
* Latin
* Bachata
* Merengue
* Dembow
* Reggaeton
* Salsa
* Afro / Afrobeats
* Caribbean
* Wedding
* Private Event
* Beach / Resort
* Lounge
* Festival
* Club
* Mobile DJ
* Karaoke / MC
* Scratch / Turntablism
* VJ / Video
* DVS
* CDJ
* Stem / Remix
* Practice / Education.
A pack can define:
* genre relationships
* transition preferences
* useful techniques
* energy curves
* allowed BPM relationships
* half/double-time relationships
* suggestion weighting
* theme profile
* UI density
* pad defaults
* preferred columns
* suggested cues
* typical preparation
* risk tolerance
* automation policy.
Do not hard-code this logic into UI components.
---
# 17. THE GUI SHOULD ADAPT TO SESSION PHASE
Build a session-phase model.
At minimum:
### Setup
Prioritize:
* device health
* library
* preparation
* playlist
* room setup.
### Warm-up
Prioritize:
* next-track candidates
* gradual energy
* longer transitions
* understated visualization
* low suggestion noise.
### Build
Prioritize:
* compatible candidates
* transitions
* phrase structure
* energy trajectory.
### Peak
Prioritize:
* immediate control
* stems
* FX
* transition opportunities
* room response
* minimal UI clutter.
### Release
Prioritize:
* harmonic resolution
* crowd cooling
* longer blends
* reduced visual intensity.
### Closing
Prioritize:
* known anchors
* requests
* end-of-set state
* recording
* history.
The system may infer phase, but the DJ must always be able to override it.
---
# 18. ATTENTION BUDGET
Implement an explicit concept of information density.
Examples:
```ts
type AttentionBudget = {
  maxPromotedControls: number;
  maxSuggestions: number;
  maxTransientNotifications: number;
  allowContextualReflow: boolean;
  animationLevel: "none" | "low" | "normal" | "high";
};
```
When actively mixing:
* fewer suggestions
* fewer explanations
* no disruptive dialogs
* no major layout reflow
* only performance-relevant surfaces promoted.
When preparing:
* browser becomes richer
* metadata becomes richer
* AI explanations become available
* planning surfaces expand.
When learning:
* more visible explanations
* larger technique hints
* reversible experiments.
---
# 19. IMPORTANT: NO RANDOM UI REORGANIZATION
Adaptive does NOT mean unpredictable.
Never suddenly move the crossfader.
Never move the play button.
Never cause a major panel to disappear during a live transition.
Never make a control relocate because the AI thinks another control is more important unless that behavior was explicitly enabled.
Use:
* hysteresis
* minimum display times
* gradual transitions
* user locks
* pinned surfaces
* “freeze layout” control.
Layout adaptation must be predictable.
---
# 20. PLAYLIST / LIBRARY OVERHAUL
This is one of the highest priorities.
The current playlist/browser should become one of djmanzo's strongest differentiators.
Design a **DJ-native library surface**, not merely a spreadsheet.
It must support several representations of the same underlying collection.
## View 1: Performance Table
Compact, information-dense.
Recommended columns:
* title
* artist
* BPM
* key
* Camelot
* energy
* duration
* rating
* color
* play count
* last played
* genre
* year
* vocal availability
* stem availability
* phrase structure
* transition suitability
* request count
* AI confidence
* function tag.
Allow instant custom column configuration.
---
## View 2: Compact Cards
Use when album/artwork is valuable.
Each card should still be operational.
Card should support:
* preview
* load deck
* stage
* add to playlist
* add to prepare
* queue
* favorite
* compare
* “more like this”
* why suggested.
---
## View 3: Set Flow
Represent tracks as a sequence.
Show:
* energy curve
* BPM trajectory
* genre trajectory
* key trajectory
* transition links
* risk markers
* anchor tracks
* alternates
* optional branches.
This becomes a visual set-planning surface.
---
## View 4: Pair / Transition View
Two tracks side by side.
Show:
* waveforms
* BPM
* key
* phrase
* likely transition point
* predicted compatibility
* stems
* vocal conflict
* energy movement
* candidate techniques.
---
# 21. "PREPARE" MUST BE FIRST CLASS
The system should have a first-class Prepare space.
A track can move:
Library
→ Candidate
→ Prepare
→ Next
→ Deck
without being copied into awkward parallel systems.
The user should easily promote a track from Prepare into a playlist/set.
Maintain consistent gesture semantics.
Do NOT repeat the kind of inconsistent Prepare gesture behavior that current Engine DJ users have complained about.
---
# 22. NEXT-TRACK RAIL
Build a compact predictive next-track rail.
Show perhaps 3–8 candidates.
Each candidate gets:
* track
* BPM delta
* key relation
* energy delta
* genre relation
* phrase compatibility
* estimated transition type
* confidence
* one-line reason.
Example:
> +3 BPM · 8A→9A · energy +1 · vocal-compatible
Allow:
* audition
* stage
* load
* reject
* pin
* “more like this”.
Do not force the AI to explain every trivial thing.
---
# 23. TRACK FUNCTION TAGGING
Add functional semantics beyond genre.
Potential tags:
* opener
* warmup
* builder
* bridge
* peak
* floor-reset
* singalong
* vocal
* instrumental
* transition-tool
* breakdown-tool
* closer
* wildcard
* safe
* risky
* emergency
* request-friendly.
These are extremely useful to a DJ.
Make them visible and searchable.
---
# 24. SUPPORT PAIRS / RELATIONSHIPS
A very useful advanced feature:
Do not only store track metadata.
Allow the system to learn **track relationships**.
Examples:
* Track A → Track B works exceptionally well.
* Track A → Track C works only with an 8-beat loop.
* Track A vocal → Track B instrumental is a favorite.
* Track C usually follows D in the user's sessions.
* Track E is often selected after a crowd-energy drop.
Store these as confidence-weighted learned relationships.
A DJ should be able to say:
> “Why do I keep seeing these two together?”
and:
> “Save this transition.”
This directly addresses a current community pain point around remembering combinations that work particularly well.
---
# 25. WAVEFORM OVERHAUL
Do NOT abandon the Rust renderer.
Instead create a multilayer semantic visualization architecture.
Potential layers:
1. amplitude
2. spectral balance
3. beat grid
4. phrase structure
5. downbeats
6. cue markers
7. loop regions
8. saved loops
9. vocal presence
10. stem presence
11. transition opportunity
12. likely mix-out region
13. likely mix-in region
14. breakdowns
15. drops
16. energy trajectory
17. AI recommendation
18. crowd-response markers
19. uncertainty/confidence
20. future runway / track ending.
Waveform visualization should become **instrumentation**.
It should answer:
> What is happening?
> What is about to happen?
> What could I do?
> What will happen if I do it?
---
# 26. DIRECT MANIPULATION ON THE WAVEFORM
The waveform should become an interactive control surface.
Allow direct interaction with semantic elements.
Examples:
### Cue marker
Drag to move.
### Phrase marker
Drag to adjust.
### Transition start
Drag.
### Transition end
Drag.
### Loop
Resize.
### Beat jump
Contextual action.
### Suggested transition
Drag the suggested transition boundary.
### Stem region
Drag/adjust stem transition behavior.
### AI suggestion
Show as a ghost layer.
The DJ should be able to physically grab the thing they are thinking about.
Do not force them to edit a numerical property in a settings panel.
---
# 27. PREVIEW / GHOST TRACK
When a candidate track is selected, display a non-destructive ghost overlay.
Show:
* where its first strong phrase would align
* where the vocal enters
* where the drop occurs
* where the outgoing track becomes weak
* likely transition overlap
* key relationship
* BPM movement.
Make the future visible.
The DJ should understand:
> “If I bring this in here, this is what happens.”
before loading or committing.
---
# 28. STEM-AWARE UI
Modern DJ systems increasingly treat stems as performance primitives.
VirtualDJ, Serato, Traktor and djay now expose stems prominently, while djay has expanded this into Neural Mix crossfaders and stem-specific transitions.
Do not make stems merely four buttons labelled:
* vocal
* drums
* bass
* instruments.
Make stems integrated into the performance model.
Example:
Incoming vocal
→ suggested over outgoing instrumental
Incoming drums
→ suggested during outgoing breakdown
Bass handoff
→ visually represented at the confluence.
Support:
* stem crossfades
* stem mute
* stem solo
* stem FX
* stem loops
* stem routing
* stem-aware transition suggestions.
---
# 29. INTELLIGENT CONTROL HANDLES
Controls must become compact but powerful.
Use progressive disclosure.
A control can have:
### Level 1
Immediate value.
### Level 2
Direct adjustment.
### Level 3
Contextual options.
Example:
A tiny EQ knob:
* drag = adjust
* shift-drag = fine control
* double click = reset
* right click = contextual options
* long hold = expanded popover
* AI hover = suggestion
* MIDI = same underlying parameter.
Do not turn every knob into a huge widget.
---
# 30. COLOR SYSTEM
Color must communicate meaning.
Do NOT produce a neon application where everything is colorful and nothing is semantically distinct.
Define semantic tokens.
For example:
```ts
type SemanticColorRole =
  | "deck"
  | "accent"
  | "active"
  | "selected"
  | "warning"
  | "danger"
  | "success"
  | "uncertain"
  | "assistant"
  | "audience"
  | "incoming"
  | "outgoing"
  | "stem-vocal"
  | "stem-drums"
  | "stem-bass"
  | "stem-other";
```
Each theme maps these semantic roles to actual colors.
---
# 31. THEME ADAPTATION
Theme should respond to:
1. venue ambience
2. musical context
3. session phase
But use slow adaptation.
Never allow the interface to flicker from color to color every time the track changes.
Use:
* hysteresis
* smoothing
* minimum theme duration
* optional transition duration
* manual lock.
Example:
Warm-up:
* subdued
* low visual stimulation
Peak:
* stronger accents
* stronger waveform contrast
* emphasized performance controls
Cool-down:
* calmer palette
Beach / tropical:
* brighter warmth
* softer contrast where appropriate
Club:
* dark high-contrast performance theme
Daylight:
* daylight/high-contrast theme
---
# 32. THEME PACKS
Create a proper theme-pack architecture.
Initial themes:
* Studio Neutral
* Pro Dark
* Daylight
* High Contrast
* Minimal
* Club
* Festival
* Beach Sunset
* Caribbean
* Latin
* Wedding
* Lounge
* Scratch
* Stem Lab
* Cyber / Experimental
* Watershed Living.
The existing watershed metaphor should become a theme/world pack, not the only possible identity.
It is a good visual language.
It must not constrain DJs who do not want metaphors.
---
# 33. ACCESSIBILITY
Color alone must never encode critical state.
Use:
* shape
* position
* pattern
* opacity
* labels
* iconography
* motion
* texture
* border treatment.
Support:
* high contrast
* reduced motion
* daylight
* color-blind-safe options
* keyboard operation.
---
# 34. CROWD / AUDIENCE INTELLIGENCE
The application already has the beginnings of room sensing.
Make it substantially more useful.
Inputs can include:
### Audio
* room loudness
* low/mid/high room activity
* applause/clap-like transients
* crowd noise
* changes relative to baseline.
### Camera
Process locally by default.
Measure aggregate properties such as:
* movement
* optical-flow variance
* occupancy proxy
* crowd density proxy
* large-scale motion
* brightness
* lighting motion
* scene change.
Do NOT identify individuals.
Do NOT perform face recognition.
Do NOT infer sensitive personal characteristics.
The objective is room dynamics, not surveillance.
---
# 35. ROOM BASELINE
Never use simplistic global thresholds such as:
> “80 dB = high energy.”
Instead establish a venue/session baseline.
Compare:
* current room activity
* recent room activity
* earlier room activity
* activity at similar session phases.
This is essential because every room and microphone/camera is different.
The existing djmanzo approach of comparing the room against its own earlier state should remain the conceptual basis.
---
# 36. MULTI-SIGNAL CROWD MODEL
Create something like:
```ts
type AudienceContext = {
  activity: number;
  movement: number;
  cohesion: number;
  vocalResponse: number;
  roomNoise: number;
  visualChange: number;
  brightness: number;
  confidence: number;
  trend: "rising" | "stable" | "falling";
};
```
But these should be evidence signals, not pretend-objective measurements.
Do not expose a giant fake-precision “crowd score” as the primary UI.
Prefer:
> Crowd response ↑
> Floor activity ↓ after last transition
> Strong response to Latin block
> Room becoming less active
with a confidence indicator.
---
# 37. CAUSAL CROWD ANALYSIS
Do more than “measure the room.”
Correlate changes with DJ actions.
Example:
1. Track A was playing.
2. Transition to Track B occurs.
3. Crowd motion increases 12–30 seconds later.
4. Audio response increases.
5. Similar reaction has occurred in previous sessions.
The AI can infer:
> “This type of transition has historically improved room response here.”
That is more valuable than raw observation.
---
# 38. CROWD SIGNALS MUST NEVER CONTROL THE DJ WITHOUT PERMISSION
Audience sensing should affect:
* suggestion weighting
* context
* theme subtly
* recommended energy trajectory
* preparation
* autopilot decisions when explicitly enabled.
Never unexpectedly:
* change music
* change BPM
* manipulate the crossfader
* change EQ
* change theme dramatically.
unless the current autonomy posture explicitly authorizes it.
---
# 39. UI FOR AUDIENCE INTELLIGENCE
Do not create a giant analytics dashboard during a set.
Normal view:
small compact indicator:
> ROOM ↑
or
> ROOM ↓
or
> ROOM STABLE
with confidence.
Click / expand:
* activity trend
* movement
* audio response
* recent transitions
* response timing
* historical comparison.
The dashboard is available, but the DJ doesn't need to stare at it.
---
# 40. ASSISTANT SHOULD SEE EVERYTHING IMPORTANT
The AI context should include:
* current tracks
* play positions
* BPM
* beat grids
* key
* energy
* phrase structure
* stems
* current transitions
* cue points
* loop state
* FX
* mixer state
* library
* prepared tracks
* next candidates
* history
* session plan
* user preferences
* current venue/occasion
* audience context
* hardware
* assistant posture
* session phase
* recent actions
* current GUI focus.
But it must still have no privileged audio-engine path.
It proposes actions through the same controlled mechanism.
---
# 41. AI SHOULD BE ABLE TO OPERATE THE GUI INDIRECTLY
A particularly powerful future direction:
The assistant should be able to request semantic GUI operations such as:
* show Prepare
* expand next-track rail
* focus Deck 2
* show stem controls
* switch to compact mode
* open transition lab
* compare Track A and Track B
* pin room panel.
These should resolve through a typed UI command vocabulary.
Do NOT have the model emit arbitrary JavaScript or mutate the DOM.
Define structured UI actions.
---
# 42. AI SUGGESTIONS MUST BE EXPLAINABLE
Every non-trivial suggestion should have:
* what
* why
* confidence
* consequence.
Example:
> “Try Track B — +4 BPM, 9A→10A, instrumental intro, energy +1.”
The user should be able to click:
> Why?
and see a richer explanation.
Do not force explanations into the main interface.
---
# 43. SUGGESTION FATIGUE
The assistant must learn how much assistance the DJ tolerates.
Possible states:
* Silent
* Minimal
* Helpful
* Proactive
* Training
* Autopilot.
A DJ who ignores 20 consecutive suggestions should cause the suggestion rate to fall.
Do not spam.
---
# 44. TRANSACTIONAL AI ACTIONS
Any non-trivial AI action should operate as a transaction.
Example:
AI wants to:
* load track
* seek cue
* set loop
* set gain
* enable sync.
Stage those together.
Show:
> Prepared next transition
with:
* Accept
* Modify
* Reject.
Once accepted, the transaction becomes normal actions and gets logged.
All AI control remains replayable.
---
# 45. INSTANT MANUAL TAKEOVER
This must be one of the strongest ideas in the application.
If the AI is controlling a parameter and the human touches that parameter:
## HUMAN WINS IMMEDIATELY.
No:
> “Exit AI mode”
No:
> “Are you sure?”
No:
> “Disable autopilot first.”
One human action means:
> “I have this.”
AI should retreat from that local control.
The rest of the assistant can remain active.
This should work at parameter level, not only whole-application level.
---
# 46. SAFETY / GUARDRAILS FOR AUTOPILOT
Autopilot must enforce:
* maximum tempo jump
* configurable key behavior
* headroom
* gain safety
* limiter state
* maximum transition overlap
* no unexpected silence
* emergency track
* emergency loop
* fallback track
* no repeated track
* artist repetition policy
* user-blacklisted tracks
* user-blacklisted genres
* venue restrictions
* requested energy bounds
* crowd-response caution.
Autopilot must always be interruptible.
---
# 47. EMERGENCY UX
Add an extremely obvious emergency control.
Examples:
* SAFE
* HOLD
* TAKE OVER
* PANIC → safe transition
* RETURN TO LAST STABLE STATE
The actual semantics should be carefully designed.
The important principle:
> When something goes wrong, the user must not search through menus.
---
# 48. PERFORMANCE / LAPTOP MODE
The app must detect limited machines.
The existing rendering/performance architecture should remain.
Adaptive GUI should degrade gracefully:
* reduce animation
* reduce visual layers
* reduce audience polling frequency
* reduce expensive previews
* reduce theme effects
* reduce CPU-heavy visual analysis
* preserve audio first.
The interface must explicitly prioritize:
## AUDIO > CONTROL > VISUAL EFFECTS
Never the reverse.
---
# 49. PROFESSIONAL WORKFLOW PRINCIPLE
A professional DJ often has only moments to act.
Therefore:
### One glance
should tell:
* what is playing
* what is next
* where am I
* what is about to happen
* what needs attention.
### One gesture
should accomplish:
* play
* cue
* load
* transition
* stem change
* loop
* takeover.
### One shortcut
should reach:
* next candidate
* prepared transition
* safe state
* assistant
* room context.
---
# 50. DON'T OVER-MODALIZE
Avoid dialogs.
Prefer:
* popovers
* inline expansion
* overlays
* contextual rails
* command palettes
* temporary surfaces
* drag targets.
A modal dialog during a live set is usually a UX failure unless genuinely necessary.
---
# 51. COMMAND PALETTE
Add a powerful command surface.
Keyboard shortcut:
`Cmd/Ctrl + K`
Possible commands:
* Load to Deck 1
* Load to Deck 2
* Show Prepare
* Show Suggestions
* Start Recording
* Enable Stems
* Switch Workspace
* Set 8 beat loop
* Key shift +1
* Compare tracks
* Save transition
* Start Autopilot
* Disable Autopilot
* Set occasion
* Freeze layout
* Show Room
* Take Over.
This can also become the semantic interface exposed to voice/AI.
---
# 52. HARDWARE-FIRST THINKING
The software GUI must remain useful:
* with no hardware
* with a small controller
* with a 4-channel controller
* with a motorized controller
* with CDJs
* with external mixer
* with DVS
* with MIDI-only setup
* with keyboard-only setup.
The screen should not unnecessarily duplicate controls when physical controls already exist.
This is a major opportunity for adaptive UI:
### Hardware present
Reduce redundant GUI controls.
### Hardware absent
Expand direct manipulation.
### Controller with displays
Move useful metadata toward feedback channels.
---
# 53. CONTROLLER-AWARE GUI
The UI should know:
* number of decks actually controllable
* available physical knobs
* jogs
* pads
* stem controls
* mixer channels
* displays
* LED feedback.
Use that to determine which GUI surfaces deserve prominence.
Example:
If a controller has dedicated stem pads:
* compact GUI stem panel.
If there are no stem controls:
* expand stem controls.
---
# 54. PROFESSIONAL FUNCTIONAL PRESETS
Create presets that configure multiple systems simultaneously.
A preset should be capable of defining:
* workspace
* surfaces
* density
* theme
* waveform layers
* pad pages
* assistant posture
* assistant suggestion rate
* technique packs
* genre packs
* session-phase weights
* audience integration
* automation limits.
Example:
## "Beach Sunset / Latin Resort"
Could automatically select:
* warm visual theme
* compact two-deck mode
* larger playlist
* room indicator
* Latin/Caribbean technique pack
* audience requests
* gradual-energy planning
* lower visual intensity.
The user can override anything.
---
# 55. VISUAL LANGUAGE ARCHITECTURE
Do not delete the existing watershed concept.
Refactor it conceptually into:
```text
Visual Language
  ├── Semantic Tokens
  ├── Theme
  ├── World
  ├── Density
  ├── Motion
  ├── Waveform Language
  ├── Iconography
  └── Surface Styling
```
"Watershed" becomes:
```text
World = Watershed
```
Other worlds can exist later.
Examples:
* Neutral Pro
* Studio
* Club
* Circuit
* Watershed
* Minimal
* Instrument
* Blueprint
* Nightlife.
The semantic meaning of controls must remain invariant across worlds.
---
# 56. VISUAL FEEDBACK SHOULD BE FUNCTIONAL
Never add animation because it looks cool.
Every animation should communicate something:
* beat
* motion
* state
* arrival
* transition
* warning
* confidence
* automation
* crowd trend.
Respect reduced-motion preferences.
---
# 57. WAVEFORM COLOR MUST BE SEMANTIC
Avoid arbitrary rainbow waveforms.
Color can encode:
* frequency bands
* stems
* phrase structure
* state
* incoming/outgoing identity.
But never overload the same color with multiple meanings.
Use texture/shape/opacity when necessary.
---
# 58. INFORMATION HIERARCHY
Design a clear hierarchy:
## Tier 1 — glanceable
* play state
* track identity
* BPM
* position
* phase
* level
* next track.
## Tier 2 — performable
* cue
* loop
* EQ
* filter
* pitch
* stems
* FX
* crossfader.
## Tier 3 — contextual
* suggestions
* explanations
* transition planning
* room response
* technique advice.
## Tier 4 — preparation
* metadata
* library
* analysis
* tags
* history
* settings.
Do not expose Tier 4 information when Tier 1/2 is demanding attention.
---
# 59. DENSITY SYSTEM
Create proper density presets:
* Relaxed
* Standard
* Compact
* Pro Dense
* Ultra Dense.
Density must modify the system coherently.
Do not have 40 unrelated font-size values.
Preserve the useful aspect of the current root `--density` approach.
---
# 60. RESIZABILITY
The application must work well at:
* 1280×720
* 1280×800
* 1440×900
* 1920×1080
* 2560×1440
* 4K
* HiDPI
* laptop screens
* ultrawide screens.
Test both landscape and constrained-height configurations.
Never allow silent clipping.
The existing repository has previously had problems where panel content was cut off by hidden overflow.
Design explicitly against that class of bug.
---
# 61. MOBILE / PHONE AS SECONDARY SURFACE
Do not redesign the desktop app into a mobile app.
Instead think of the phone/camera as an optional companion surface.
Potential phone uses:
* crowd sensor
* remote deck control
* requests
* second-screen library browser
* room monitor
* assistant interaction
* camera input
* lighting/visual remote
* preparation.
The main desktop cockpit remains primary.
---
# 62. AUDIENCE CAMERA PRIVACY
Default philosophy:
* aggregate metrics
* local processing
* minimal/no raw persistence
* no identity recognition
* no individual profiling.
Expose explicit controls:
* Camera enabled
* Microphone enabled
* Observe room
* Store aggregate observations
* Use observations for suggestions
* Send observations to cloud AI
These must be separate.
---
# 63. AI PRIVACY
Also distinguish:
* AI enabled
* AI sees session state
* AI sees play history
* AI sees room data
* AI may call cloud model
* AI may learn preferences
* AI may change GUI
* AI may execute audio actions.
Do not make all of that one toggle.
---
# 64. CURRENT COMPETITOR LESSONS TO INCORPORATE
Use current competitor research as design input, not as a reason to copy their skins.
### VirtualDJ
Learn from:
* customizable layouts
* skins
* SideView
* Sandbox
* live suggestions
* extensive mappings
* stems
* automix
* visualizations
* programmable behavior
* custom interfaces.
VirtualDJ explicitly treats customization as a core product capability and allows hardware-specific or special-purpose interfaces.
### rekordbox
Learn from:
* multiple screen layouts
* Intelligent Cue Creation
* Intelligent Playlist
* vocal detection
* Mix Point Link
* Dual Player
* Sub Browser
* collection filter
* hardware-consistent design.
rekordbox's current workflow direction is especially relevant to the adaptive-preparation concept.
### Serato
Learn from:
* Prepare
* History
* Browse
* dynamic waveform orientation
* 4-deck mode
* stems
* practice mode
* hardware integration.
### Traktor
Learn from:
* flexible beatgrids
* stems
* separate grid/cue concepts
* Pattern Player
* external sync
* hardware-oriented workflows.
### Mixxx
Learn from:
* configurable skins
* customizable effect controls
* crates/playlists
* search/sort
* Auto DJ
* user-configurable layouts.
### djay
Pay attention to:
* Neural Mix
* crossfader/stem integration
* dynamic transition tools
* 4-deck evolution
* direct interaction with waveforms
* Automix integration
* the fact that modern AI features are moving into the main performance interaction model.
### Engine DJ
Study the workflow complaints as much as the features.
The current community shows demand for better:
* Prepare → Playlist flow
* metadata columns
* editing metadata during performance
* consistent gestures.
### Lexicon
Study why DJs pay attention to dedicated library-management software.
The lesson is:
> the library is not an administrative side effect of DJing; it is part of the instrument.
---
# 65. COMMUNITY RESEARCH PRINCIPLE
Continue researching recent DJ discussions while implementing.
Look for:
* complaints
* workflow pain
* repetitive tasks
* feature requests
* “what do you wish DJ software did?”
* library frustrations
* playlist frustrations
* hardware frustrations
* UI complaints
* things users say make one application “just work”
* things that break professional muscle memory.
Use current discussions from:
* Reddit DJ communities
* VirtualDJ forums
* Serato forums
* Native Instruments community
* Engine DJ community
* Pioneer/AlphaTheta communities
* DJ TechTools
* Digital DJ Tips
* professional DJ educators.
Do not blindly copy forum requests.
Identify patterns.
---
# 66. DJ WORKFLOW KNOWLEDGE
Build around the real loop:
```text
Observe
→ Select
→ Preview
→ Prepare
→ Load
→ Cue
→ Align
→ Shape
→ Transition
→ Observe response
→ Adapt
→ Repeat
```
Every GUI region should clearly serve one or more steps of this cycle.
If a surface exists without a clear workflow purpose, question whether it belongs.
---
# 67. THE SESSION IS A LOOP, NOT A SCREEN
The application's central conceptual object should become:
## Session
A session contains:
* timeline
* tracks
* transitions
* room state
* DJ state
* audience response
* workspace
* learned context
* notes
* requests
* AI interventions
* manual interventions
* phase
* set arc.
The GUI is simply a live window into that session.
---
# 68. TRANSITION OBJECT
Consider introducing an explicit semantic transition object.
For example:
```ts
type TransitionPlan = {
  outgoingDeck: number;
  incomingDeck: number;
  startPosition: number;
  endPosition: number;
  durationBeats: number;
  style: TransitionStyle;
  bpmDelta: number;
  keyRelation?: string;
  outgoingStems?: StemPlan;
  incomingStems?: StemPlan;
  eqPlan?: EQPlan;
  fxPlan?: FXPlan;
  confidence: number;
  reasons: Reason[];
};
```
This object can drive:
* waveform visualization
* suggestions
* preview
* AI preparation
* autopilot
* practice
* replay.
That would unify many currently separate concepts.
---
# 69. PRACTICE LAB
A professional GUI overhaul should include a strong Practice surface.
Two tracks can be explored without altering the live master.
The user can:
* compare BPM
* compare phrase structure
* test transitions
* experiment with stems
* test FX
* create loops
* compare EQ strategies
* hear alternative transitions
* save successful transitions.
This is where the “sandbox” concept becomes truly powerful.
---
# 70. LEARNING MODE
Use the action log.
The system knows what the DJ actually did.
After a practice session:
> “You changed the low EQ 3 beats before the phrase boundary.”
> “Your timing drifted slightly before the vocal entry.”
> “You successfully used a 16-beat phrase transition.”
Do not give fake scores unless there is a meaningful measured basis.
Prefer actionable feedback over gamification.
---
# 71. "WHAT SHOULD I DO NEXT?"
Make this an optional compact assistant affordance.
Not a giant chat.
Possible contextual answers:
> Next likely move: bring the incoming instrumental in on the next 16-bar boundary.
> Safer option: stay with the current track for another phrase.
> Crowd response is falling; consider a familiar track rather than increasing BPM.
> This vocal overlaps the outgoing vocal; an instrumental stem blend is cleaner.
These are contextual suggestions.
The DJ remains the authority.
---
# 72. USER OVERRIDE MATRIX
Create an explicit matrix specifying what AI may do at every posture.
Example:
| Action                | Suggest | Prepare |  Assist | Autopilot |
| --------------------- | ------: | ------: | ------: | --------: |
| Recommend track       |       ✓ |       ✓ |       ✓ |         ✓ |
| Load next deck        |         |       ✓ |       ✓ |         ✓ |
| Set cue               |         |       ✓ |       ✓ |         ✓ |
| Gain match            |         |       ✓ |       ✓ |         ✓ |
| Sync                  |         |         |       ✓ |         ✓ |
| EQ adjustment         |         |         | limited |         ✓ |
| FX                    |         |         | limited |         ✓ |
| Crossfader            |         |         |         |         ✓ |
| Genre/track selection |         |         |         |         ✓ |
| Layout adaptation     |       ✓ |       ✓ |       ✓ |         ✓ |
Make this system configurable.
---
# 73. AI SHOULD KNOW WHAT IS "EXPENSIVE"
The existing concept of `mistakes_are_costly` is valuable.
Expand this into a risk model.
High-risk moments:
* active transition
* loud peak
* microphone speech
* critical vocal drop
* DJ making fast manual changes
* audience request response
* unexpected device state.
During high-risk moments:
* reduce suggestions
* disable speculative layout changes
* prevent risky automation
* emphasize direct manual controls
* show only high-confidence interventions.
---
# 74. CONTEXTUAL CONTROL RAIL
Create a compact contextual rail.
It should contain whichever 4–8 controls are most relevant now.
Example:
During a stem transition:
* vocal
* drums
* bass
* instrumental
* stem FX
* loop.
During scratch mode:
* jog
* scratch mode
* brake
* reverse
* cue.
During preparation:
* cue
* loop
* phrase
* tags
* rating
* transition points.
This is the core idea of adaptive UI.
---
# 75. VISUAL CONTROL OF AUDIO FEATURES
Where meaningful, expose audio properties visually.
Examples:
* spectral balance
* vocal density
* drum density
* bass activity
* breakdown
* drop
* energy
* transient density
* phrase boundaries.
Allow clicking/dragging where this maps to a genuine action.
Do not confuse visualization with control.
Every interactive visual needs clear semantics.
---
# 76. LIBRARY "AI LENS"
The library can have a toggle for:
> AI Lens
which adds:
* likely next
* user affinity
* crowd suitability
* current phase suitability
* transition risk
* novelty
* familiarity
* function tags.
This must never replace the standard library view.
---
# 77. EXPLORATION VS PERFORMANCE
The UI should recognize two fundamentally different mental modes:
## Exploration
User wants:
* discovery
* comparisons
* metadata
* broad browsing
* analysis
* experimentation.
## Performance
User wants:
* speed
* confidence
* low cognitive load
* immediate control.
The system should transition between these modes gracefully.
---
# 78. "FREEZE"
Add:
## Freeze Layout
When enabled:
* no automatic rearrangement
* no automatic surface resizing
* no theme changes unless explicitly permitted
* no surprise visual transitions.
This is the professional safety valve.
---
# 79. "LOCK MY WORKFLOW"
Allow the DJ to lock:
* workspace
* panel arrangement
* control density
* theme
* waveform style
* assistant layout behavior.
The AI can still use the underlying state without rearranging presentation.
---
# 80. PERSONA LEARNING
Eventually learn:
> “This DJ likes a very dense layout.”
> “This DJ almost never uses Automix during peaks.”
> “This DJ prefers long blends.”
> “This DJ uses stems mostly for vocals.”
But express learned behavior as editable preferences.
The system should show:
> Learned preference
and let the user reject/modify it.
---
# 81. USER PROFILE BY CONTEXT
Do not store one universal DJ profile.
Store conditional profiles.
Example:
```text
Johannes
  ├─ Club / Peak
  ├─ Beach / Sunset
  ├─ Wedding
  ├─ Latin
  ├─ Practice
  └─ Open Format
```
Each may have different:
* density
* technique preferences
* genre weights
* transition style
* automation tolerance.
---
# 82. PERFORMANCE METRICS FOR THE REDESIGN
Measure:
* time to load track
* time to find next track
* clicks to load next track
* clicks to stage track
* time to transition
* accidental control activations
* number of unnecessary panel changes
* average visible controls
* suggestion acceptance
* suggestion rejection
* takeover frequency
* UI FPS
* audio xruns
* time to recover from errors.
Do not optimize aesthetics alone.
---
# 83. IMPLEMENTATION STRATEGY
Do not rewrite all components simultaneously.
Proceed in vertical slices.
## PHASE 0 — AUDIT
Produce:
* component inventory
* state inventory
* current UI map
* feature inventory
* action inventory
* parameter inventory
* existing layout capabilities
* visual token inventory
* performance constraints
* current UX problems.
Write this as a document in the repository.
---
## PHASE 1 — DESIGN SYSTEM CORE
Build:
* semantic tokens
* density system
* theme model
* motion model
* surface model
* dock model
* workspace schema
* adaptive-context schema.
Do not yet redesign every component.
---
## PHASE 2 — SHELL
Replace the rigid panel model with the workspace/surface architecture.
Keep existing surfaces functional.
Goal:
> New shell, old functionality.
---
## PHASE 3 — PERFORMANCE COCKPIT
Redesign:
* decks
* waveform
* mixer
* contextual rail
* mission bar.
Goal:
> Dramatically better performance workflow.
---
## PHASE 4 — LIBRARY
Redesign:
* browser
* playlist
* prepare
* suggestions
* set flow.
Goal:
> Make track selection as good as the mixing interface.
---
## PHASE 5 — INTELLIGENCE
Implement:
* context engine
* learned behavior
* AI surface promotion
* AI suggestion rail
* preparation transactions
* takeover behavior.
---
## PHASE 6 — ROOM
Integrate:
* audience context
* room baseline
* causal response
* subtle Room HUD.
---
## PHASE 7 — THEMING
Implement:
* semantic theme packs
* contextual theme adaptation
* watershed as a theme/world.
---
## PHASE 8 — SPECIALIST WORKSPACES
Implement:
* practice
* scratch
* stems
* wedding
* mobile
* VJ
* karaoke
* autopilot.
---
# 84. TEST EVERY STEP
Do not declare success because the build compiles.
Test:
* 2 decks
* 4 decks
* 6 decks
* no hardware
* controller
* keyboard
* external device
* 1280×720
* 1280×800
* 1920×1080
* 2560×1440
* 4K
* low-GPU machine
* reduced motion
* high contrast
* dark
* light/daylight
* active recording
* stems
* FX
* browser
* assistant
* autopilot
* hardware disconnect
* device failure
* long playlists
* very long track names
* missing artwork
* unanalysed tracks.
---
# 85. TEST MUSCLE MEMORY
This is critical.
After redesign:
* Play button position should be stable.
* Cue should be stable.
* Deck identity should be stable.
* Crossfader should be stable.
* EQ order should remain stable.
* Jog behavior should remain stable.
* Standard shortcuts should continue to work.
* Controller mappings should still produce the same semantic actions.
---
# 86. TEST THE ACTION BUS
Every new UI action must ultimately resolve into the existing action system.
No component may secretly mutate engine state.
No AI feature may bypass the action bus.
No alternate UI implementation may create its own duplicate control semantics.
---
# 87. TEST STATE CONSISTENCY
If track loading originates from:
* browser
* assistant
* preset
* controller
* keyboard
* network
* drag & drop
the resulting UI must look identical.
One source of state truth.
---
# 88. TEST ADAPTATION
Create deterministic tests for:
* session phase change
* hardware change
* audience response change
* user behavior pattern
* AI posture change
* confidence change
* context change.
The layout must respond deterministically.
---
# 89. VISUAL REGRESSION
Create visual regression coverage for major workspace configurations.
At minimum:
* Classic 2 deck
* Pro 2 deck
* 4 deck
* Compact laptop
* Preparation
* Practice
* Autopilot
* Club
* Watershed
* High Contrast.
---
# 90. PERFORMANCE REGRESSION
Measure:
* UI frame rate
* audio xruns
* memory
* CPU
* worker utilization.
Do not let visually sophisticated changes compromise realtime audio.
---
# 91. DO NOT OVERENGINEER PREMATURELY
When implementing the redesign:
Prefer:
* explicit models
* declarative layout
* composition
* data-driven configuration
* semantic state
* typed commands.
Avoid:
* giant generic component frameworks
* unnecessary dependency proliferation
* abstract abstractions with no concrete use
* hidden reactive loops
* global mutable state unrelated to existing architecture.
---
# 92. DO NOT MIGRATE TECHNOLOGY
Remain on:
* Svelte 5
* TypeScript
* Tauri 2
* existing Rust architecture
* existing Action bus
* existing ParameterRegistry
* existing waveform rendering architecture.
Improve architecture where necessary, but do not use the redesign as an excuse for a technology rewrite.
---
# 93. IMPORTANT PRODUCT PRINCIPLE
The best interface is not the one that shows the most.
It is the one that shows:
> **exactly what matters next.**
This should become the governing design principle.
---
# 94. SECOND IMPORTANT PRODUCT PRINCIPLE
Do not make the UI “smart” by hiding controls unpredictably.
Instead:
> **promote what matters, compress what does not, and preserve the underlying map.**
The DJ must be able to expand anything immediately.
---
# 95. THIRD IMPORTANT PRODUCT PRINCIPLE
The assistant should feel like:
> an extremely experienced second DJ sitting beside the user
not:
> a chatbot attached to DJ software.
It should:
* notice
* prepare
* suggest
* remember
* adapt
* warn
* help
* execute when authorized
* get out of the way immediately when the DJ takes over.
---
# 96. FOURTH IMPORTANT PRODUCT PRINCIPLE
Audience sensing should feel like:
> “the software is listening to the room with me”
not:
> “the software thinks it knows my audience.”
Keep confidence explicit.
---
# 97. FIFTH IMPORTANT PRODUCT PRINCIPLE
Theme is atmosphere.
Semantics are sacred.
A theme can change the mood of the interface.
It must not change what a control means.
---
# 98. SIXTH IMPORTANT PRODUCT PRINCIPLE
A professional DJ should be able to work for an entire night without needing to hunt through menus.
Everything common should be:
* visible,
* near,
* one gesture away,
* one shortcut away,
* or one contextual reveal away.
---
# 99. WHAT I EXPECT FROM YOU BEFORE CODING
Do not immediately start modifying dozens of `.svelte` files.
First produce a concrete internal redesign analysis with:
1. Current UI architecture map.
2. Existing UI strengths.
3. Existing UI weaknesses.
4. Competitor comparison.
5. DJ workflow model.
6. New information architecture.
7. Surface/dock model.
8. Workspace model.
9. Adaptive context model.
10. AI integration model.
11. Playlist redesign.
12. Waveform redesign.
13. Theme architecture.
14. Audience model.
15. Implementation dependency graph.
16. Migration plan.
17. Testing plan.
18. Performance plan.
Then identify which elements can be implemented immediately using existing state/actions and which require backend additions.
Do not invent backend capabilities merely because a UI would like them.
---
# 100. USE EXISTING FUNCTIONALITY AGGRESSIVELY
Before building something new, ask:
> Is this capability already in djmanzo?
The repository already contains much more functionality than the visible GUI may expose elegantly.
The redesign should primarily improve:
* integration
* visibility
* information architecture
* control proximity
* adaptation
* discoverability.
Do not duplicate working engine functionality.
---
# 101. CONSIDER THE CURRENT FEATURE SET AS A HIDDEN ENGINE
The existing feature set already covers a remarkably broad surface:
* multi-deck
* sync
* quantize
* slip
* reverse
* key shift
* sandbox
* stems
* stem EQ/filter
* FX
* sampler
* pads
* browser
* playlists
* smart folders
* imports
* controller mappings
* MIDI/HID
* motorized platters
* OSC
* network control
* phrase detection
* suggestions
* transition planner
* deterministic replay
* genre families
* automatic set assembly
* assistant takeover
* posture/occasion
* taste learning
* set planning
* audience requests
* room sensing
* learning
* journal
* more-like-this.
Do not treat this as a small DJ application needing more buttons.
Treat it as a **large capability graph needing a much better operating environment**.
---
# 102. FINAL OUTPUT FROM THIS TASK
When the implementation is complete, leave the repository with:
## A. New UI architecture
Clear component/surface structure.
## B. New adaptive context model
Reusable by assistant, GUI, audience and planning.
## C. New workspace system
Persisted and extensible.
## D. New theme system
Semantic, adaptive, accessible.
## E. New library / playlist interface
Designed for actual DJ workflows.
## F. Enhanced waveform interaction
Useful, semantic and directly manipulable.
## G. Contextual control layer
Promotes the controls needed now.
## H. AI integration
Subtle, configurable, explainable, interruptible.
## I. Audience context
Useful but not intrusive and privacy-conscious.
## J. Specialist workspaces
Based on actual DJ roles and workflows.
## K. Tests
Functional + visual + performance.
## L. Documentation
Update the appropriate docs to describe the new architecture.
---
# 103. SUCCESS CRITERIA
The overhaul is successful only when all of the following are true:
### Human DJ usability
A competent DJ can use the software immediately without learning the AI.
### Professional density
An experienced DJ can make fast, precise adjustments without opening configuration panels.
### Adaptability
The same application can feel simple for a beginner and extremely dense for a professional.
### Modularity
A user can construct a personal workflow.
### Predictability
Adaptive behavior never feels random.
### AI subtlety
The AI is present everywhere useful but visually dominant nowhere.
### Instant takeover
Human input always wins.
### Library quality
Finding the next track is as strong as mixing it.
### Visual utility
Visualizations convey actionable musical information.
### Theme semantics
Color and visual change communicate useful information.
### Audience awareness
Room feedback improves decision-making without pretending to be omniscient.
### Performance
Audio remains the highest-priority subsystem.
### Extensibility
New surfaces, themes, techniques and domain packs can be added without rewriting the shell.
---
# 104. CODING STYLE
While implementing:
* keep components focused
* extract semantic reusable primitives
* prefer typed interfaces
* keep data models explicit
* document non-obvious interaction decisions
* do not bury behavior in CSS hacks
* do not introduce magic numbers without explanation
* preserve testability
* preserve keyboard accessibility
* preserve controller parity
* preserve action-log semantics.
When creating new design primitives, use names that reflect DJ semantics rather than generic UI jargon where appropriate.
Examples:
* `PerformanceSurface`
* `TransitionRail`
* `TrackCandidate`
* `NextTrackRail`
* `ContextRail`
* `RoomHud`
* `MissionBar`
* `Workspace`
* `WaveformLayer`
* `ActionTransaction`
* `AdaptiveContext`
* `DJTechnique`
* `ThemePack`
---
# 105. FINAL INSTRUCTION
Be willing to change the GUI architecture substantially.
Do not be timid merely because the existing UI works.
But do not recklessly rewrite working audio/control architecture.
Your mission is:
> **Transform djmanzo from a feature-rich VirtualDJ-class application into an adaptive professional DJ cockpit that feels simpler to a beginner, faster to a professional, and uniquely intelligent to everyone.**
The defining sensation should be:
> “It gives me exactly what I need, exactly when I need it, and otherwise stays out of my way.”
Do the research.
Inspect the actual code.
Respect the existing architecture.
Design before refactoring.
Implement incrementally.
Run the application.
Test real workflows.
Fix interaction problems rather than merely styling them.
And continuously ask:
> **What is the DJ trying to do right now, what information does the DJ need for that decision, and what is the shortest reliable path from intention to action?**
That question should determine the GUI.