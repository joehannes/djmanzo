# Changelog

Versioning follows semver, with one project-specific convention:

> **Minor and major tags produce release builds. Patch tags do not.**
>
> `.github/workflows/release.yml` triggers on the tag pattern `v[0-9]+.[0-9]+.0`
> — a semver tag whose patch component is `0` is by definition a minor or major
> bump. So `v0.2.0` and `v1.0.0` build installers; `v0.2.1` is recorded in git
> and stays quiet.
>
> Patch tags are for marking incremental work as it lands. Minor tags are for
> when a milestone completes and there is something worth downloading.

---

## Unreleased

**Fixed: the autopilot could never mix.** Its idea of a deck to mix *into* was
one with nothing loaded, while the record it would mix is read off that same
deck — so the two could never both be there. It staged a track, the deck became
loaded, and from then on it said "nothing staged to mix into" for the rest of
the night. Its own unit tests could not see it: their fixture has an idle deck
*with* a record on it, which is a situation the function feeding them could not
produce. A deck that is **not playing** is somewhere to mix into, which is the
definition the automix has always used.

**Fixed: a record with no phrase structure was not staged at all.** The same
function built the incoming track's description with `?` on the phrase fields,
so a live recording or an ambient record — anything the analyser finds no
phrases in — was dropped rather than described. `Incoming::phrase` is an
`Option` precisely because plenty of records have none, and the planner already
answers "bar line, no phrase structure" rather than refusing. It now uses
`phrase_of`, which is the one place that rule lives.

Both were found by driving the autopilot rather than by reading it: with a
transition set up and the posture on Autopilot, the assistant said "nothing
chosen to play next" about a record cued on the deck in front of it.

**The autopilot defers to the transition you set up.** It was still planning its
own while the automix performed the held one — two things deciding the same mix
differently, which is the whole of what §68 is complaining about, and an
assistant announcing one mix while another was about to happen. It now proposes
the held transition when there is one, says whose mix it is ("the blend you set
up, over 32 beats"), and does not let the occasion trim a length a human chose:
§45's rule that human input wins does not stop at the controls.

Both read it through the same `automix::Setup`, rather than each growing a
shape of its own for the same thing.

**The mix point is dragged on the waveform** — §26, which says it plainly:
*"The DJ should be able to physically grab the thing they are thinking about.
Do not force them to edit a numerical property in a settings panel."* The pair
view shipped with ±16 buttons, which is exactly the panel that section is
complaining about. The buttons stay as the keyboard path; the transition's
start and its end are now grabbed where they are drawn.

What crosses to djmanzo is the **frame under the hand** — a place in the record,
which is all a pointer honestly knows. Which beat that is stays Rust's
arithmetic: it has the grid, the tempo and the record's own sample rate, and an
interface working it out would be a second opinion about where beat 275 is. The
drag is snapped to the nearest beat and clamped into the record, and the panel
draws what comes back rather than where the finger went.

The lane's marks are dashed rather than solid for the same reason they always
were: a cue is somewhere the DJ put, and these are somewhere djmanzo is
proposing.

**djmanzo reads the night** — the directive's §11, first half. `dj_app::context`
works out what the set is doing from the records that have actually been played:
their loudness and their tempo, compared against the night's own range rather
than against a table of genres. It publishes a phase, an energy, a confidence
and its typed reasons on every snapshot, and the assistant panel draws them
beside the occasion — so what the DJ declared the night is, and what the records
say it is doing, sit together.

`SessionContext.session` had been the literal value `None` since it was
written, with a comment saying it would stay that way until something actually
read the room. It is read now, and the living interface's energy comes from the
night rather than from the master meter — the meter says what the mixer is
doing, and pulling the bass out for eight bars is not the room cooling down.

**It will not guess.** Under three analysed records the answer is still `None`,
because a night with two records in it has no shape: they can only rise or
fall, and either reading would be one record's mastering away from the
opposite. The panel says so in those terms. This is the same field that was
once defaulted to *Peak at 0.95*, so the interface announced peak time thirty
seconds into a warm-up.

**And it does not twitch.** An ambiguous stretch keeps the phase it had rather
than falling back to a default — the interface morphs to this, and a phase
flickering between *Heat* and *Peak* while a DJ was looking at it would be
worse than one that lags. The density bands settle for the same reason.

Verified in the running application: three records into a flat demo set the
panel read *Warming up · holding at 22% · 3 records · 25% sure*.

**The automix performs the transition you set up.** Setting one up in the pair
view used to mean djmanzo *knew* about a mix nothing would ever run: the
automix went on mixing out of the end of the file, into whichever deck happened
to be free, at its own length and style. Now, with a transition held, it mixes
where the planner chose and the DJ approved, into the deck they named, and
takes on that length and style visibly rather than running something its own
controls do not describe.

Without one it behaves exactly as before — this is an addition, and the
module's honest description of its own limit ("mixes out of the end of the
file, because it cannot hear where the outro is") is now the *fallback* rather
than the whole story.

**Setting one up still does not hand the mix over.** The pair view says which
it is: `Automix will run this at 2:10` when the mix has been handed over, and
`Held, and nothing will run it: automix is off` when it has not. An interface
that let "set up" imply "will happen" would be lying at the one moment a DJ is
deciding whether to keep their hands free.

A transition is performed once. After it finishes the playhead is *past* its
start point for the rest of the record, so a rule that only asks "are we there
yet" answers yes on every tick from then on — the automix would re-mix into the
same deck for ever. Adjusting the transition makes it a new intention and it
can run again, because a DJ who moves a mix point after djmanzo has already
mixed there is asking for a different mix.

**Fixed: pump tests that asserted the operating system was prompt.** Three
snapshot-pump tests slept a fixed 50–600 ms and assumed a thread had been
scheduled in the window. On a loaded macOS runner one had not, and CI went red
on a machine where nothing was wrong. They wait for the thing to happen now,
with a deadline long enough that only its absence trips it — and the one that
failed has its heartbeat pushed out to a minute, because a mutation run showed
that a generous wait let the *heartbeat* satisfy an assertion about *change
detection*. A red build that means nothing is worse than no build.

**Fixed: a deck's clock was wrong for any record not at the device's sample
rate.** A deck's playhead and length are counted in the *file's* frames — the
engine resamples on the way out, which is why the waveform lines up with the
playhead at all — and the snapshot was dividing them by the *device's* rate. A
two-and-a-half-minute 44.1 kHz record on a 48 kHz device read 2:17, and the
remaining time a DJ mixes by was out by thirteen seconds. 44.1 into 48 is the
commonest pairing there is.

The engine now publishes `DeckParam::SourceRate` beside the frames it is
counted in, and the deck's clock and the automix's transition length both use
it. Latency and the two-card bridge keep the device's rate, which is what they
are actually measured in. The waveform's between-snapshot interpolation was
running 8.8% fast for the same reason and is right now too.

Found by driving the application with a synthesised 44.1 kHz record and
noticing the deck disagreed with the length in the library — no test could see
it, because every fixture used one rate for both.

**The transition is an object** — the directive's §68. `dj_app::transition`
holds one mix: the two decks, where it starts and ends in both frames and
seconds, how long it runs, which way, what the tempo and the key do across it,
how well the two records go together, and the typed reasons for all of it.
djmanzo holds the one you set up, so it survives closing the panel you set it
up in, and it is dropped rather than drawn stale when either record leaves its
deck.

**It can be moved, shortened and restyled — and an edit re-derives the
reasons.** `plan::evaluate` is split out of the planner for exactly this: a
transition nudged off its phrase boundary stops claiming to land on one. Keeping
the sentence the planner wrote would be a confident lie about the one fact the
panel exists to report. A style the planner would not have chosen is still
allowed, and the reasons go on saying the tempos clash: this reports, it does
not veto.

**Pair, a surface of its own** — §20's fourth view. The two records side by
side with the seam between them: BPM, key in Camelot and in notation, energy,
phrase structure, function tags, and each record's waveform with **the mix
point drawn on the outgoing one**. Its confidence is the number the Next rail
draws, from the same scorer, because two figures on one screen that both claim
to say how well two records go together and disagree is worse than one.

Found by driving it rather than by type-checking it: the seam was underneath
the two records and fell below the fold of the docked panel, so it is between
them now; the outgoing lane zooms out in octave steps until the mix point is on
screen, because a lane centred on the playhead cannot show a mix two minutes
ahead; and the "rushed" warning now counts the beats left *after* the mix ends,
which is what its condition tests — it read "only 277 beats left · 32 beats
left" before.

**The command palette** — `Ctrl/Cmd + K`, the directive's §51. Every entry is
generated in Rust from `dj_core::vocabulary` — the same 82 verbs the parser
accepts, the assistant is told about and a MIDI mapping produces — or from a
surface the cockpit publishes. It therefore cannot offer a command djmanzo does
not have, and a verb added to the vocabulary appears in it without anyone
remembering to add it.

**What you type is itself an entry.** `deck 2 loop 8` parses, so the top row
runs it verbatim. That is the only way the verbs taking an argument — a loop
length, a key shift, a pitch — can be reached at all, because a list of buttons
would have to invent the number, and it is what §51 means by closing with "this
can also become the semantic interface exposed to voice/AI".

Matching is a subsequence rather than a substring, so `d2p` finds
`Deck 2 · play`; it lives in Rust with the ranking, because a matcher in the
interface would be a second opinion about which command you meant. Only the
decks the rig actually has are offered. `Ctrl/Cmd + K` is the one key djmanzo
takes globally — a plain `k` typed into the browser's search box is still a
`k`, which is asserted by test.


## v0.14.0 — Prepare, Next and the set plan become surfaces of their own

**Set Flow** — §20's third view. The set plan is a dockable surface now rather
than a page inside the browser's folder tree, and it draws what a plan is
actually made of: not a list of tracks but a list of **transitions**. Between
every pair of records is the seam that joins them — `+3 BPM · 8A→9A · +2 dB` —
and a seam that needs a cut rather than a blend says so, in the one colour on
the panel that interrupts a scan. The number of difficult joins is stated above
the list, because twenty-five rows is more than anyone reads before deciding
whether to keep a plan: two is a plan to play, eleven is a plan to rebuild.

A seam is judged by the same scorer the Next rail uses, so a pair of records
gets one answer rather than two that can disagree. Grammar is the exception the
scorer cannot see: dembow into four-on-the-floor is a cut however well the
tempos match, and `dj_core::genre` is what knows that.

**Every density band's floor moved up 40 px.** Adding one destination to the
top bar pushed that row onto another wrapped line, and the top bar is pinned —
so forty pixels came out of every stage at every window height, and a 1,100 px
window went back to a deck that did not fit. The browser sweep caught it. The
floors are re-derived from that measurement rather than guessed, and a Rust
test now reads `ui/e2e/shell.ts` and fails when the harness's copy of the table
drifts from the real one, which is how the sweep came to be measuring an
application that no longer existed.

**The next-track rail** — the directive's §22, and a surface of its own rather
than a tab inside Prepare. It follows whichever deck is playing without being
told, and shows up to eight candidates, each on **one line of deltas**:
`+3 BPM · 8A→9A · +1 dB`. Deltas rather than values, because `127 BPM` needs
the DJ to remember what is playing before it means anything and `+3` does not.
A key clash is on the line, not hidden behind the score — a suggestion that
conceals its worst feature is one you learn not to trust after being caught by
it once. A phrase structure that *was* found is not mentioned, because phrase
lengths divide each other in practice and eight rows of "nothing to worry
about" is not information.

Each row carries a confidence bar, derived in `dj_library::suggest` from the
range its own weights can reach, and five gestures: load to a deck, set aside
into Prepare, more like this, pin to the top, pass. Pin and pass are about the
next few minutes and are not written down — "not that one" while a record is
playing is not "never suggest this again", and a rail that quietly learned the
first as the second would hide a collection from its owner.

The ranking now also reports **where the genre goes** and scores it at zero.
Crossing families is a technique, not a mistake — a bachata after a merengue
is most of what a Dominican set is — so djmanzo says the change is happening
and declines to have an opinion about it. A penalty would quietly rank a set
into one genre, which is the opposite of what the rail is for.

The Next tab is gone from Prepare rather than duplicated: two places that
suggest the next record are two places that will disagree.


**Function tags** — what a record is *for*, which is not what it is. Genre says
a record is bachata; it does not say whether it opens a room, lifts one that is
already moving, or is what you reach for when the floor has emptied and you
need it back inside ninety seconds. Ten of them: opener, builder, peak, floor
reset, singalong, closer, transition tool, safe, risky, emergency.

A closed vocabulary rather than free text, because the whole value of the tag
is that it means the same thing on every record and can therefore be searched
and counted — free text gives you `opener`, `Opener`, `open`, `warmup` and
`warm-up` in the same collection inside a month, which is five columns and no
answers. Ten and no more, because a vocabulary a DJ cannot hold in their head
is one they will not use consistently, and inconsistent tags are worse than
none: they look like data.

Set on a selection in the browser, in its own row rather than beside genre and
colour — those write what a record *is*, this writes a judgement about when to
play it, and folding them together would mean colouring eight tracks silently
replaced their functions. Every function is offered even at zero, with a count,
because a picker that hides what you have never used never suggests using it.

**Smart folders can filter on them** — `for is opener`, `function is peak`,
`not for is risky and bpm > 120`. A function is a row in another table rather
than a column, so it compiles to an `EXISTS` rather than a join: a record
carrying three functions still appears once, where a join would list it three
times and look like a duplicate-detection bug. Negation is `NOT EXISTS`,
because an absent function is a *value* — a record nobody has tagged genuinely
is not an opener — unlike an absent tempo, which is an unknown a filter must
not assert anything about.

**Prepare is its own surface.** It was mounted by the browser, so it could only
exist where the browser existed and only at the size the browser left it. It
docks like anything else now: its own frame, header, close button and place in
the persisted workspace, open beside the decks with the browser along the
bottom or open on its own. One gesture reaches it — `→` on a browser row — and
`prepare.svelte.ts` is the only path between the two, because a second way to
set a track aside is how a gesture starts behaving differently depending on
where you made it.

**A deck pins its channel strip only when it can afford to.** Pinning was the
fix for the deck's volume fader and filter sitting behind the master strip, and
it was unconditional, which is what was wrong: a pinned region is `flex: none`,
and a `flex: none` region in a column with less room than it wants does not
scroll — it overflows. Four decks with a surface docked at 1280×800 put the
first deck at 22 px tall with its 300 px strip drawn straight across the master
strip. Two faults behind it. `.decks.four` and `.decks.six` set
`grid-auto-rows: min-content` meaning "let the extra rows scroll", except
nothing scrolls them, so the free space went to whichever row was not
`min-content` — 115 px against 433. And pinning had no price. Now every deck
row is `minmax(0, 1fr)`, and a deck pins only when it has room for the strip
*and* a waveform: measured, 168 px of strip on one line above about 530 px of
deck width and 300 px wrapped below it, plus 140 px for the waveform, overview
and progress bar. Under that the deck is one scrolling column — everything
reachable, in the same order, nothing painted over anything.

The layout budget grew with it: `ui/e2e/budget.spec.ts` measures all six shapes
the top bar can be put into — two, four and six decks, docked and not — rather
than the opening screenshot alone. 42 tests, from 17.

**`docs/DIRECTIVE-STATUS.md`** answers "where are we in the 105 sections" from a
file rather than from memory, with a script that counts the table so the
number can be checked rather than taken.


## v0.13.0 — Everything a DJ touches, on one screen

The first screen is for mixing, and for the first time it actually is.

At djmanzo's own default 1280×800 with two records loaded, the waveform, the
pad grid, the deck's channel strip, the cue, the crossfader assignment, the
crossfader, master gain, the headphone cue, split and the limiter are all
there without scrolling. The crossfader had ended up below the fold three
times in three different forms; most recently about 280 px past the bottom,
with a deck's own volume fader and filter down there with it.

Scaling got the deck from 878 px to 685 — the pad grid stopped taking its
height from the deck's *width*, the faders and knobs started answering to the
density setting they had been ignoring, and djmanzo picks a density band from
the window it was given. Scaling could not finish it, so the rest is **pinning,
twice**: the master strip came out of the scrolling stage, and each deck's body
scrolls with its channel strip pinned. What goes below the fold on a short
window is the waveform's tail and the loop rows.

The booth — microphone, automix, plugin insert, master effects — is a dock
surface now rather than a slab under the decks, which is what it always was by
its own description.


**Every control a DJ touches is on one screen at 1280×800.** The deck's own
volume fader and filter were the last two below the fold, behind the pinned
master strip. The deck now does what the master strip does one level up: its
body scrolls and its channel strip is pinned, so the waveform and the pads go
below the fold on a short window instead of the controls touched continuously.
Nothing moved in the reading order.

Three things had to change for a deck to be able to pin anything. A grid row's
height is its content's, so the deck grid's rows are `minmax(0, 1fr)` — an
earlier attempt set `max-height: 100%` on the deck and moved nothing, because
100% of an `auto` row is the content again. The stage stopped scrolling, since
a child cannot pin itself inside a parent that grows to fit it. And the booth
controls — microphone, automix, plugin insert, master effects — became a dock
surface, which is what they always were: the things set up once a night rather
than reached for during a mix.

Two bugs found by running it rather than by testing it. The waveform vanished
entirely on the first attempt: a flex child's `flex-shrink` is 1, so a lane
with a fixed pixel height and nothing inside holding it open went to zero, and
no scrollbar appeared because nothing overflowed. And the waveform lane was
the last block still ignoring the density setting — it is drawn by Rust at a
pixel height, so it takes the scale as a number rather than in CSS, which
would have stretched tiles rendered for a different size.


## v0.12.0 — Two things at once, and every master control on screen

The cockpit redesign's first three phases, and they are the first ones a DJ can
see.

**More than one panel can be open.** The shell held a single variable naming
one of eight panels, so opening the assistant closed the browser — which is why
the room and the library could never be looked at together. Surfaces dock now:
beside the decks and along the bottom, several at once, each in a titled frame
that closes from its own header, arranged by their own preferred size rather
than by a table of special cases. The arrangement is checked in Rust and
survives a restart.

**Every master control is on screen at 1280×800**, for the first time. The
crossfader has gone below the fold three times in three different forms; most
recently about 280 px past the bottom. The pad grid stopped taking its height
from the deck's *width*, the faders and knobs started answering to the density
setting they had been ignoring, djmanzo picks a density band from the window it
was given, and the master strip came out of the part of the stage that scrolls.
A deck went from 878 px to 685.

**The deck is drawn from a layout tree** rather than from its own markup
(ADR-0008 W3), with a golden order asserting the tree still produces the deck
djmanzo draws.

Three bugs surfaced on the way and are fixed: an unconfigured djmanzo was being
handed the stripped-down *Starter* preset by a command nobody had asked; the
layout budget had been measuring a deck with no pad zone for three runs; and a
`dj-net` test raced its own port about one run in three.

**What is still wrong, and said plainly.** The deck's own volume fader and
filter sit behind the pinned master strip, in the part of the stage that
scrolls. The stage has 559 px and a deck wants 685, and scaling cannot close
that — something on the deck has to fold or pin. It is recorded as a failing
test with the real reason rather than a passing one against a page coordinate.


**Every master control is on screen at djmanzo's own default window size**, for
the first time. The crossfader has ended up below the fold three times in three
different forms; most recently it was about 280 px past the bottom of an 800 px
window, with master gain, the headphone cue, the split button and the limiter
beside it.

Three things fixed it. The pad grid stopped taking its height from the deck's
*width* — a pad is a fixed-aspect SVG stretched to its grid cell, so two rows
were 197 px tall because a two-deck column is wide, and the pad zone shrank when
four decks were on screen and the DJ had more to hit. The SVG faders and knobs
now answer to `--density` instead of being a fixed count of device pixels; they
simply ignored the setting before, which is why "denser" never bought the room
it looked like it should. And djmanzo picks a density band from the window it
was given, unless a layout or workspace names one — the interface adapting to
the DJ, and standing down the moment the DJ decides. A deck is 685 px, down
from 878.

That still was not enough, and the arithmetic says why: the stage has 559 px, so
a deck would need about a 0.64 scale against a 0.80 floor. What closed the gap
was taking the master strip out of the scrolling stage. It had been inside it,
under decks taller than the stage, so it scrolled away with them. Nothing moved
in the reading order — deck, crossfader, deck — it simply stopped being part of
what scrolls, which is what every DJ application does with its mixer.

**The deck's own volume and filter are still not reachable**, and the harness
now says so instead of passing. It measured a page coordinate, which went green
the moment the numbers dropped under 800 — while in the running application both
sat behind the pinned strip in the part of the stage that scrolls. A screenshot
showed it; the assertion could not. It asks whether a control is inside the box
that clips it now, and that is recorded as a failing test rather than a fixed
problem.


**The dock manager** (cockpit Phase 2). The shell held one `panel` variable
containing one of eight names, so exactly one panel could be open — and the
consequence, which the audit named as its headline finding, is that a DJ could
not look at the room and the library at the same time. Not a decision anybody
made; just what one variable does.

Surfaces are now placed in docks: a side dock beside the decks and one along
the bottom, several at once, each in a titled frame that closes from its own
header. Where a surface goes comes from its own preferred size rather than a
table of special cases — wider than tall goes below, taller than wide goes
beside — so the library runs along the bottom while the assistant stands next
to the decks, with the decks still there. The arrangement lives in Rust,
is checked there against what can be drawn, and survives a restart.

**Nine more `null` answers in the browser harness**, all the same bug as the
one that hid the pad zone: the stub answers an unknown command with `null`, no
Rust type is ever null, and a component that reads a field off the answer
throws and ends the render pass. They were invisible because nothing had ever
opened those panels under the harness. The dock tests open every one of them
and refuse a page error, so this class is now loud instead of silent.


**The deck is drawn from the layout tree** (ADR-0008, W3). `Deck.svelte` no
longer holds the deck's shape in its own markup: it renders a list of named
widgets in the order the resolved tree gives, and the flat layout upconverts
into exactly the deck djmanzo already drew — asserted as a golden order, so a
control cannot move underneath a DJ as a side effect of a format change. Six
widgets the vocabulary was missing are in it: the jog, the channel fader, the
cue, the crossfader assignment, the progress bar and the times.

**Two bugs it surfaced.** A DJ who had never opened the layout picker was being
handed the *Starter* preset by `layout_tree` — no pads, no loops, no effect
rack, no beat jump, no filter, no keylock. It had been answering that way for
releases and nothing noticed, because the interface read only the tokens out of
that answer and drew the deck from its own markup. Nothing chosen now means the
full deck.

And the layout budget had been measuring a deck with **no pad zone** for three
runs. The browser stub answers an unknown command with `null`, `stems_status`
was not in its list, and a component read a field off that answer — so the
deck's subtree threw, Svelte abandoned the render pass, and 197 px of pad zone
was simply absent while every geometry assertion stayed green. A deck measures
878 px, not 675. The crossfader is about 280 px below the fold, not 77, and a
deck's own volume fader and filter are below it too. All three are recorded as
failing tests rather than fixed problems; the numbers are now the ones a DJ
meets.

## v0.11.0 — The interface is audited, and its vocabulary written down

Nothing a DJ touches has changed since v0.10.0, and that is deliberate. This
release is the two steps that have to come before an interface is rearranged:
finding out what is actually there, and agreeing what the pieces are called.

**The interface is audited** (`docs/GUI-OVERHAUL.md`). Thirty-seven components
and 17,614 lines, 148 commands, 448 parameters, counted rather than remembered,
with the state each component owns and where it comes from. The finding that
matters is structural: the browser and the assistant are *siblings under one
panel slot*, so exactly one of them can be open. A DJ cannot see the room and
the library at the same time — not because anyone decided that, but because the
component tree was shaped that way three years of features ago.

**The vocabulary the cockpit is assembled from** (`crates/dj-app/src/cockpit.rs`).
Fourteen semantic colour roles, five densities, four motion levels, seventeen
surfaces, and the docks and workspaces they arrange into — types with tests and
no renderer, so the redesign has names to argue about before any `.svelte` file
is opened. Attention states are here too, including the one that governs the
rest: while performing, the interface may not reflow.

The rule the whole redesign is built on is recorded in both files. Presentation
priority may change; **semantic control identity may not**. A fader is a fader
wherever it is drawn. Muscle memory is not a thing to be improved.

## v0.10.0 — Find a record by humming it, and a layout that is measured

**Find a record from what you remember** is finished. The hum is now compared
*as a melody* against a stored pitch contour for every record, not only read for
its key and tempo: ten pitch points a second found with YIN, matched on
octave-folded intervals so the key it was hummed in does not matter, and located
with subsequence DTW so the answer is *where in the record*. It still does not
name a record you do not own, and the panel says so beside the button.

**A layout is a tree of named widgets** (ADR-0008, W1 and W2). Thirty-three
named widgets with their slots, settings and ranges; a layout is a tree of them
in JSON; existing flat layouts upconvert on load. A skin may set twenty-three
design tokens and each value is checked against the shape that token takes, so
a layout stays data and cannot become a program.

**The first screen is measured, not remembered.** A browser test opens the
interface at djmanzo's own 1280x800 and reads where the controls actually land,
against a snapshot captured from the running application. It found the master
strip's second row below the fold — now one row instead of two — and a split-cue
button drawn on top of the output meters. It also found that the crossfader is
still below the fold with records loaded, which is recorded as a failing test
rather than a fixed problem.


## v0.8.0 — Timecode vinyl you can switch on, and controllers that work

**Timecode vinyl became reachable.** `dj-dvs` could decode a control record and
`Command::SetTimecode` could install one, and nothing in the application could
send that command. Now there is an input picker, a relative/absolute switch and
a live calibration reading in Settings, and `write_timecode_signal` renders
djmanzo's own control signal to a WAV — burn it to a CD or play it off a phone
and any turntable or CD deck drives a deck, without buying a record.

The reading distinguishes three states from one number, and has to: negative is
"not on a record", zero is "on one and hearing nothing" — a dead cartridge, a
lifted needle, the wrong input — and above that is reading.

**Real controller mappings**, transcribed from Pioneer's own MIDI message
lists: DDJ-SR, CDJ-3000, DDJ-200, and a family file covering DDJ-400, DDJ-FLX4,
DDJ-FLX2 and DDJ-SB3. None has been run against the hardware, and each says so
in its first paragraph.

Two things the mapping format could not previously express, both of which would
have shipped inside those files:

- **14-bit faders.** Every Pioneer, Denon and Native Instruments fader arrives
  as two control changes. Binding the high byte alone put a pitch fader on 128
  steps — 0.125% each, audible when beatmatching.
- **Centred jog wheels.** A platter reports movement, not position. Read as a
  fader, its centre landed a hair above zero and drove the deck forwards with
  nobody touching it.

**And the bug that hid all of it.** Mapping selection took the first file whose
`device` appeared in the port name, and `generic-2-deck` claims `"MIDI"` —
which is in nearly every ALSA port name. Every controller in the world was
handed to the generic mapping.

Also fixed: changing the audio output device left the microphone and every
control record running into rings belonging to a discarded engine. The
microphone went silently dead on a reconnect while still holding a sound card
open. `NullBackend` gained an input device, which is why it could be tested at
all.

## v0.1.0 — Beta: a playable instrument

The first build worth downloading. M0 through M5 are substantially complete and
the controller layer (M4) has arrived, which is what turns a set of panels into
something a DJ can actually play.

**These builds are unsigned and un-notarised.** macOS will refuse to open the
app until you right-click it and choose Open, once. See
[QUICKSTART.md](docs/QUICKSTART.md).

### Added

- **The keyboard as a controller.** Not a pile of shortcuts — the same
  vocabulary, the same file format and the same validation as a MIDI mapping,
  so a laptop with nothing plugged into it is a playable instrument. 76 keys,
  laid out so the two hands mirror each other, with a live sheet that lights a
  key while it is held. Keys are named by physical position, so the layout
  holds on an AZERTY or QWERTZ keyboard.
- **MIDI controllers.** `dj-hid`: a mapping engine over the action bus, TOML
  mapping files, 7-bit and 14-bit controls, all three encoder conventions, and
  bundled mappings so a fresh install works with nothing configured. Every
  action in a file is checked when the file loads, so a typo is a message when
  you choose the mapping rather than a control that silently does nothing an
  hour into a set.
- **Performance**: slip mode, reverse, censor, loop roll, the slicer, brake and
  backspin, a pad zone with seven pages, a four-bank sampler with recording
  into a slot, and an FX rack of three chained slots per deck and on the master
  with eleven effects and chain presets. Two, four or six decks.
- **Library**: SQLite collection with playlists, crates, smart folders, play
  history, duplicate detection, and importers for rekordbox, Serato, Traktor
  and iTunes — including the cue and grid tags written into the audio files.
- **Analysis**: BPM, beat grid, key and loudness, with grid editing by shift,
  scale and tap.
- **Recording to disk**: the master, post-limiter, into a 16-bit WAV.
- **Music sources**: Spotify, YouTube, YouTube Music, Jamendo and the Internet
  Archive, each with an honest account of what it does and does not permit.
- **The assistant** and preset packs, both speaking the same action vocabulary
  as everything else — so their work is auditable, reversible and replayable.

### Changed

- **The sound card opens on launch** and the choice is remembered. Waiting to
  be told to connect meant loading a track and pressing play did nothing, with
  no visible reason.
- **A first run offers to scan your music folder** in one click rather than
  opening a file dialog onto a folder whose location you did not choose.
- `Action::parse` refuses trailing words. `deck 1 volume 0.5 extra` used to
  parse as `deck 1 volume 0.5`, quietly swallowing the typo.

### Fixed

- Encoder direction. The convention was guessed from the byte, which meant a DJ
  turning an *absolute* encoder down from 60 to 30 got a beat jump *forward* —
  30 is a position below centre to one convention and thirty clicks clockwise
  to another. Mappings now declare which their hardware sends.

### Known limits

- Unsigned builds on both platforms; signing needs an Apple Developer ID.
- No HID, no controller feedback (LEDs, displays), no motorised platters yet.
- Neural stems, CLAP hosting, Pro DJ Link and StagelinQ are designed but not
  built.
- The waveform will not scroll smoothly on a machine without hardware-accelerated
  compositing. The interface says so rather than looking broken; the audio
  engine is unaffected.
- **Nothing here has been through a real gig.** That is what a beta is.

## v0.0.2 — Headphone cue routing

M1 continues. No release build (patch tag, by design).

### Added
- **Headphone cue (PFL)** — per-deck cue send, cue/master blend, split cue, and
  a booth output with independent level.
- `BusLayout` derives master/booth/cue channel assignments from the device's
  channel count: 2 channels is master only, 4 adds cue, 6 adds booth.
- The audio host now opens **four channels when the device has them**, so cue
  works on the controller interfaces that support it.
- Release workflow and this changelog.

### Changed
- Deck gain staging split into **trim** and **fader** stages so the cue send is
  genuinely pre-fader — you can cue a track with its channel fader all the way
  down, which is the entire reason PFL exists.
- Decks report both pre-fader and post-fader peak levels; they answer different
  questions (what to set trim by, versus what reaches the master).

### Verified
- The cue bus never reaches the master, tested directly — previewing a track
  must never be audible to the room.
- Cue, split-cue and booth paths all proven allocation-free on the audio thread.

## v0.0.1 — Foundations

- **M0 walking skeleton**: seven crates, Tauri 2 shell, Svelte 5 UI, CI on macOS
  arm64 and Ubuntu. Realtime engine with an action bus, lock-free parameter
  registry, and `Arc` retirement so track buffers are never freed on the audio
  thread.
- **Isolator EQ and filter sweep**: Linkwitz-Riley crossovers give a true band
  kill rather than a deep shelf; single-knob filter with a bit-exact bypass.
- **`dj-secrets`**: API keys in the OS keychain, never a config file.
- **Band-limited centre cancellation**: karaoke vocal removal that keeps the
  centred kick and bass.
- Design docs for the assistant (A1–A6) and karaoke (K1–K2) tracks.
