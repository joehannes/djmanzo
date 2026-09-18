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

**Where somebody is singing** — §25's `vocal` layer, §75's *vocal density* and
§27's *where the vocal enters*, all from a separator djmanzo already shipped.

`vocal` had carried *the analysis does not exist* beside it for the life of
§25's layer table. It did exist. `dj_stems::hpss` pulls a vocal stem out of a
record with **no model, no download and no runtime** — a DJ can solo the voice
today — and the waveform could not draw where that voice was.
`dj_analysis::presence` measures the same thing the separator defines, the
harmonic part of the centre channel between 200 Hz and 8 kHz, and measures it
**without reconstructing anything**: a presence curve is a question about the
masks, not about the audio, so there is no inverse transform and no second copy
of the record in memory. The two band edges moved to `dj_core`, because one
fact defined twice is a fact that drifts.

On one point it deliberately asks a sharper question than the separator can. A
separator has to produce audio that sums back to the mix, so its centre is the
mid channel as it finds it — and a hard-panned guitar appears in mid at half
amplitude whether anyone wanted it there or not. The first version read that
guitar as *half a voice*, which is what the tests caught; each bin is now
weighted by how centred it actually is.

Where it is wrong is said out loud rather than buried: **a centred synth lead
reads as a voice**, and the layer's own description says so. It wears
`Role::Uncertain`, which is what that role's doc reserved it for long before
this shipped.

- **§25 goes from seventeen of twenty layers to eighteen.** Drawn as a strip
  along the top of the overview — the trajectory owns the floor, and a second
  set of columns there would read as a second opinion about *where does this
  record go* when it answers a different question. A window nobody measured
  draws **nothing** rather than zero: absent and *nobody was singing* are
  different answers.
- **§27 is seven of seven.** The count guard on §27's list was written as an
  alarm rather than a tally — *when an analyser finds vocals and the `vocal`
  layer starts being drawn, this fails, and what it is asking for is a frame
  in `Ghost`* — and it fired exactly as designed. The entry is converted onto
  the outgoing record by the same multiplication the drop already used,
  written once now rather than twice. Drawn as a tick at the top of the ghost
  so it is not read as a second drop.
- **§75 goes from three of nine audio properties to four.** Its reason — *needs
  separation analysis nobody has written* — had stopped being true before it
  was written down.
- **A real defect in the palette.** `--uncertain` and `--shape` were both
  `var(--text-dim)`, justified by a comment saying there was no glance in which
  a DJ had to tell them apart. This layer is an estimate drawn on the overview
  beside the trajectory columns, which is exactly that glance. `--uncertain` is
  now a tint of the palette's own accent, derived so every package gets one
  free, and a vitest fails the day somebody points the two at one token again.
- **One number here is not a measurement, and says so.** `presence::STRONG` is the
  share at which djmanzo will call a lead *carrying* the record. That can only
  be set by listening to real records with real voices on them; this machine
  has none, only synthesised test signals. It is one constant rather than one
  per consumer, so whoever does the listening has a single number to change.

**Which of the four currents is carrying the record** — §25's `stems` layer,
and §75's *drum density*, from the same pass a commit later.

`stems` was the last layer filed under an analysis djmanzo could not do, and
the measurement written for `vocal` was already computing the masks the other
three need. It answers as a **partition**: the four shares of a moment add to
the whole of it, which is the numerical form of the separator's own central
invariant — the stems sum back to the mix — asked of the numbers instead of
the audio. That is what makes them one reading of a record rather than four
that could disagree about how loud it was at a given moment, and a test walks
every hop of a signal built to contain all four and says so.

- **§25 goes to nineteen of twenty layers.** The twentieth is the crowd, which
  needs a camera or a microphone this container does not have.
- **The layer table's own prediction was wrong, and is corrected rather than
  quietly dropped.** `Role::Uncertain`'s doc had said for years that *vocal and
  stem presence will each arrive with a confidence of their own*. Building it
  showed the difference: vocal presence is one number and can be drawn as
  strength; stem presence is a question about *which of four*, and a drawing
  that cannot say which answers nothing. `Role::Stems` draws each window in the
  colour of the fader that mutes the current it names.
- **The dominant current, not all four stacked.** Four segments in a
  four-pixel band is one pixel each, which is texture rather than information.
- **§75 goes to eight of nine audio properties**, and the count written into
  that row a commit ago — *four* — was a slip, corrected here: three were
  exposed before either of these shipped, and §75's own energy, breakdown and
  drop made six. The one left is *transient density*.
- The four stem
  labels, keys and colour tokens moved out of `Stems.svelte` into
  `ui/src/stems.ts` — a second copy of that order is how the bass fader ends
  up coloured like the vocal band.

**§20's vocal column** — *vocal availability*, the reading §20 has listed as
needing "analysis nobody has written" since the row existed.

The library stores the **strongest window's share** rather than a verdict: the
line between *has a vocal* and *does not* is `presence::STRONG`, the one stated
guess in that measurement, and baking today's guess into a DJ's database would
mean re-analysing a library to re-tune it. Deciding on the way out means
re-tuning re-reads the column.

Three answers, kept apart end to end: a mark for a record with a lead in it, a
dash for one measured and found to have none, and a **blank** for one nobody
has analysed. A DJ hunting for an acapella acts on *there is none* and skips
*no answer yet*, and a column that drew both the same would send them to the
same place.

- **§20 goes to fifteen of twenty columns.** Of the five left, four are
  decisions rather than gaps. The fifth is *stem availability*, and its reason
  has changed rather than held: the separator can split any record on demand,
  so availability here means **ready** rather than possible — and the stem
  cache is keyed by chunk, so there is no cheap per-row answer to give a table
  drawing a hundred rows.
- **Adding it found the browser fixture had been drifting.** §20's column list
  was typed out by hand under a comment claiming fourteen while holding
  fifteen, and nothing compared it with `Column::ALL` — so the sixteenth column
  simply had no row to tick, and the test that needed one failed for a reason
  that had nothing to do with the column. It is a golden file blessed from Rust
  now, like §54's setups and §16's packs.
- Library schema 16. `NULL` and `0.0` are different rows and a mutation run
  proves it: reading one as the other, never writing the column, and letting an
  import blank a reading the analyser made all fail.

Four more mutations were run against the partition — each current folded into
*other* in turn, and the side channel judged by the centre's answer — and all
four were killed.

Four mutations were run against the measurement and two of the tests written
for it were thrown away for passing on the wrong thing — a click train that
read low because of the band rather than the harmonic mask, and chopped noise
that read low because of its duty cycle. The test that stands builds the two
spectrograms by hand.

---

## v0.16.0 — What djmanzo knew and never used

Eight things djmanzo had already learned, stored or been told, and acted on
nowhere. Each one was found the same way: by asking *who consults this?* of a
table that looked finished.

**The knowledge pack you chose reaches the rail** — §16. `Pack::families` was
parsed, stored, sent to the interface and read by nothing, so pressing *Latin*
narrowed what the coach taught and changed nothing about what djmanzo offered.
It now prefers the music the night is made of, and where a pack says this music
does not pair across half and double time it **takes the credit back** rather
than adding a penalty — so the pairing scores what an unrelated tempo scores,
which is what "not part of this music" means.

**The rail follows the night, until you say otherwise** — §17. Its phase model
moved panels and nothing else; *gradual energy*, *energy trajectory*,
*harmonic resolution*, *crowd cooling* and *known anchors* were listed as
belonging to the planner and belonged to nobody. The Next rail now opens on the
direction the night is going and says so. One press of lift, hold or ease takes
it over for the rest of the night, and it never goes back to following on its
own.

**How you actually join records reaches the planner** — §81. djmanzo could tell
you *at weddings you mostly fade* and then propose a blend, every time. Your
learned transition style now decides — in the one case where the music has left
the choice open. A key clash still gets the echo and an unreachable tempo still
gets the cut; three previous evenings do not outvote the two records in front
of you.

**What you did last year counts for less than what you did last week** — §12's
decay, on the same hundred-and-eighty-day half-life taste already used. A habit
given up fades out of your profile instead of outvoting the present forever. It
never reaches nothing, so a DJ who has been away is told what they used to do
rather than told nothing at all.

**Worker utilization is measured** — §90. The background threads reported
whether they were *working*, which is a light; a worker that flickers busy once
a minute and one pegged for an hour both show it. Both threads now account for
their own time, reported separately because they mean opposite things: a busy
library worker is a queue being got through, and a busy interface builder is
§90's own warning.

**The coach waits for a moment to be heard in** — §11. It knew nothing about
whether you were mid-blend or standing between records, so a new move to learn
could arrive in the middle of a mix. §58 puts technique advice in the
contextual tier and §18's mixing budget stops at the second — the correction
about the mix you are in is still made, because that is the one thing a coach
is for.

**Two more waveform layers** — §25, now sixteen and seventeen of twenty.
**Likely mix-in**: where a record can be joined, opening on its first whole
phrase and closing at its first drop, because a mix started after the drop has
thrown the drop away. **Saved loops**: eight slots, a recall per slot, and
until now no way to see where any of them were without pressing one.

### Under it

`context::CONSUMERS` holds §11's nine consumers against the code, and a test
reads each named module's source — so a row claiming a reader that has stopped
reading fails rather than becoming a sentence nobody can check. Four tilts now
sit on one suggestion score, and one test holds all four below what a single
key relation is worth: taste, the night's profile, the pack and the phase break
ties, and none of them overrules the mixing.

---


**djmanzo remembers which records you put together** — §24's learned track
relationships, and the gesture it names: *"Save this transition."*

**Only what was kept, never what merely happened.** Every mix a night contained
is already derivable from the action log — `dj_app::mixes` does it — so storing
those as relationships too would be a second copy that eventually disagrees
with the log it came from. What cannot be derived is that the DJ thought one
was worth having back, and that is the whole of what `kept_pairs` holds.

**Directional, and the primary key says so.** A bachata that lands beautifully
after a merengue is not the same claim in reverse, and a DJ who kept one
direction has said nothing about the other.

**Confidence-weighted, and the weight is a count.** Keeping the same pair again
is a stronger claim about it rather than a duplicate row. A count rather than a
score because it is a fact rather than a judgement, and the place to make the
judgement is where it is used.

**And it is used where the question gets asked.** §24 wants a DJ to be able to
say *"why do I keep seeing these two together?"* — so the Next rail says it,
first among the reasons, because it is the one thing on that line the DJ said
rather than djmanzo worked out. The weight is capped at three: without a
ceiling a pair kept twenty times would outrank every musical fact about every
other candidate and the rail would stop showing anything else. "You have done
this before" is a strong reason, not an override — deliberately less than a key
match and a tempo match together.

Applied *after* scoring rather than inside it. The scorer is a pure function
over two records and its whole test suite rests on that; the history lives in a
database, and the layer that has one is the layer that applies it.

**What §24 lists and this does not have**: "A → C works only with an 8-beat
loop", "A vocal → B instrumental", and "E is often selected after a crowd-energy
drop". The first two need a transition object that records what was *done* to
the stems, and the third needs crowd sensing this container has no camera for.

**What you do, and when** — §14's behavioural signals, read through §13's rule.
They ship together because either alone is worse than neither: signals with no
rule for reading them is how the wrong preference gets learned, and a rule with
nothing to read is a comment.

§13 is the important half and the directive spells out the sentence it does not
want: *not* "user likes enormous BPM jumps", but "large jumps occasionally
occur in high-energy contexts". So it is a **type** here rather than a warning.
A signal carries what was done and the phase the night was in as one value that
cannot be taken apart, and a `Tendency` — the only thing that generalises —
cannot be constructed without a phase, nor from fewer than four occurrences
**in that same phase**. "The DJ likes large tempo moves" is not a sentence this
module can produce.

That is the discipline `Grounds` already uses for §9: a rule enforced by a
constructor is still there in the seventh place somebody needs it, and a
runtime check is one `if` away from being forgotten.

**The night now keeps its arc.** `Night::phase_at` answers what the night was
at a moment that has already passed, from the changes it recorded rather than
from whatever phase it is now — because attributing a whole set's gestures to
the phase it happens to be in at the end is exactly the mis-learning §13 is
about. It refuses to backdate: the first stretch of every night reads as
nothing, because nothing could read it yet. §67 lists the set arc as part of a
session, and this is it.

**Twelve gestures, coarser than the vocabulary.** Six EQ verbs are six
spellings of one thing a DJ would call riding the EQ, and counting per verb
would need six times the evidence to notice it. A test asserts every gesture
named is one the bus can actually produce — a signal djmanzo watches for and
never sees would look like the DJ never doing it.

**Six of §14's twenty are absent and named as such**: track searched,
previewed, staged, candidate rejected, candidate selected, and assistant
suggestion accepted. None is an action — searching is a query, previewing needs
a player djmanzo does not have, and the rest are interface gestures that never
reach the bus. Approximating them from something else would be inventing
exactly the unusual behaviour §13 is about.

Shown beside "what you reach for" rather than in a panel of its own: both are
djmanzo saying what it has worked out about this DJ, and two homes for that is
two things to go and check. The sentence arrives written from Rust, so the
interface cannot make a claim Rust would not.

**At hand: the four to eight controls that matter now** — §74's contextual
control rail, which the directive closes by calling "the core idea of adaptive
UI".

**It reads the hands, not the night.** §11's context engine reads where the set
is in its arc; that is the wrong clock for this. A rail answers "what am I
doing this second", and the answer changes when a hand lands on a platter —
minutes before any engine would notice the tempo had risen. So the reading is
the snapshot: a hand on the jog, a stem pulled, a record playing against
another, a deck cued and waiting. Both readings exist and neither replaces the
other.

**Every control is an action djmanzo already accepts**, carried as the exact
text the parser takes — so a press is the same event as typing it, mapping a
controller to it, or the assistant asking for it. A test parses every control
of every state, so the rail cannot offer a verb djmanzo does not have. Unlike
the palette it *may* carry an argument: `loop 4` and `eq_low 0` are exactly
what "most relevant now" means, and choosing the number is this module's job.

**A latched control offers the other half.** "bass out" becomes "bass in" once
the low band is out, and "loop 4" becomes "loop off" once one is running. A row
with both side by side is a row where half the buttons are always wrong.

**It says which deck and why**, always. A row of controls that silently became
a different row is a row a DJ stops trusting, and the whole of §74 rests on
being trusted enough to reach for without looking. Which deck is its own
judgement: §41's `ui focus` fades after six seconds because attention is a
moment, so the rail follows the hands instead — a platter being touched, then
stems being played, then the record being got ready, because during a set the
deck that is *not* playing to the room is the one being worked on.

**A rule that would have misfired on every deck**, caught by writing the test
before trusting the fixture: "a stem volume away from unity" is true of every
deck in the first second after launch, because the registry reads zero for a
stem the engine has not published yet. Comparing the four stems against *each
other* says the same thing about a pulled stem and nothing at all about four
that have never been touched.

**Stem FX, tags, rating and transition points** are on §74's list and are not
here. The first is a rack of four numbers rather than a button; the other three
belong to a record rather than a deck, and editing a rating from a rail would
be the browser's job done where a DJ cannot see which record they were
changing.

**The collection as cards, and the artwork to put on them** — §20's second
view, and the last of its four. Three shipped already: the performance table,
Set Flow and the pair view.

**djmanzo read every tag except the pictures**, so the view §20 describes as
"when album/artwork is valuable" had no artwork to be about. It reads them now,
preferring the front cover and falling back through the other picture types
rather than taking whichever one the tagger happened to store first — a card
showing the publisher's logo instead of the sleeve is a card that looks broken.

Covers are served on their own URI scheme, like waveform tiles and for the same
reason: a grid asks for fifty images at once, and base64 through the bridge
would cost a third more bytes, block the main thread decoding them, and defeat
the browser's own image cache. Each answer is kept — **including the absence of
one**, which is the half that matters: a hand-organised collection is mostly
untagged files, and without remembering that, every scroll would re-probe every
one of them from disk to learn nothing.

**Each card is operational**, which §20 asks for in as many words. Load onto a
deck, set aside, more like this, favourite, and the reasons when the list being
shown is a suggestion. Three of §20's card actions — *stage*, *add to prepare*
and *queue* — are one gesture in djmanzo, because Prepare is where a set-aside
record goes and the Sidelist is what Prepare holds; three buttons doing one
thing would be three chances to wonder which you wanted. *Favourite* is the
five-star rating djmanzo already has rather than a flag of its own.

**Two of §20's list are absent and left out rather than faked.** *Preview* needs
a player that auditions a record without a deck, which djmanzo does not have —
the same gap §27's ghost track waits on. *Compare* puts two records side by
side, and the pair view compares two *decks*.

**A record with no cover still says something about itself**: its key and its
tempo, over a colour taken from the Camelot hour. Not a placeholder icon
repeated down the grid, which is a wall of one shape.

**One mix can be heard back on its own** — the last of §68's list, which asks
for the transition object to drive *replay*. `replay::Window` names a stretch
of a set, `crate::mixes` supplies the two numbers, and **hear it again** on any
row of Tonight's mixes re-renders that handover to a WAV with eight seconds of
run-up and four of tail.

**A window is not a seek**, and that is stated rather than hidden. The engine's
state at any moment *is* the whole set up to it — which record is on which
deck, where its playhead is, where every fader was left — so everything before
the window is still rendered and thrown away. A replay that jumped in would be
a different set that happens to share a clock, and it would be silently
plausible, which is worse. A mix from the third hour therefore costs three
hours of rendering; replay runs to no deadline and is far faster than real
time, but it is not free, and the button says so.

`Rendered` reports the cost and the output as separate numbers now. They differ
whenever there is a window, and telling a DJ their twenty-second mix is three
hours long would be a confident wrong answer about a file just written.

**Found by driving it: the answer landed below the fold.** The row grows by a
line when the path arrives, and in a docked panel that line is exactly the one
pushed out of sight — a result nobody can see reads as a button that did
nothing. This project has shipped that twice, in two different panels. The
answer scrolls itself into view now.

**The night knows its own mixes** — §67 says the session contains transitions
and §68 asks for those transitions to be explicit objects. `dj_app::transition`
holds the mix that is *about* to happen; `dj_app::mixes` holds the ones that
already did, and a **Tonight's mixes** surface shows them: what went into what,
how long it took in beats, and what kind of mix it was.

**Derived, never recorded.** The obvious implementation writes a transition
into the session file as it is performed, and `SessionEvent` already argues
against it about loading: an event with two spellings makes two takes recorded
through different paths diff as different sets. A mix *is* its actions, so
recording it beside them would be recording it twice and inviting the two to
disagree. Deriving it instead means every set djmanzo has ever recorded gains
its mixes, including the ones recorded before any of this existed.

**One rule, not a recogniser per gesture.** A handover is *the room stopped
hearing one record and started hearing another* — audibility computed the way
the engine computes it, the channel fader times the crossfader gain for that
deck's assignment, through `dj_dsp`'s own curve. A crossfader sweep, two
channel faders crossing, and a fader against an assignment are all one event to
the room, and a module with a branch for each would disagree with itself the
first time somebody used two at once.

**The style is a fact from the log, not a guess.** Length decides between a cut
and a mix first, because that is what the room hears. Within a mix the most
specific gesture names it: a vocal taken across is a vocal drop, an echo
engaged over the outgoing record is an echo, the outgoing bass pulled out is a
blend, and a crossfade with none of them is a fade. A gesture on the *incoming*
deck names nothing — the bass coming out of the record arriving is a DJ making
room for it, and attributing that to the record leaving would call almost every
mix a blend until the word stopped meaning anything.

`dj_assistant::coach::CUT_MAX` is public now and shared, so the coach cannot
call a handover a cut while the session's own list of it says otherwise.

**Two things found by opening it in the running application.** A pair of short
titles was drawn as two columns with the arrow stranded between them. And a
third surface in a side dock squeezed to a single row cut through the middle of
its letters — flex children shrink to nothing, so the dock never reached the
height that would make it scroll. Side surfaces have a floor now.

**The record says where it can be left, and the grid says when it is guessing**
— two more of §25's twenty layers, both from arithmetic djmanzo already does.

**Mix-out.** `plan::mix_out` answers where a record can structurally be left:
the window opens at the last beat where the longest transition the planner will
propose still leaves its tail margin intact, and closes at the last beat where
the shortest one does. Inside it, every length djmanzo would suggest fits; after
it, whatever you start is `Reason::Rushed`. Both edges come from the planner's
own constants, so the band and the warning cannot disagree on screen about the
same mix. It is a fact about the *record*, not about the playhead or the pair —
and it has a type that cannot express a playhead, so it cannot come to depend on
one.

Drawn on the overview as well as in the scrolling lane, and the overview is the
view it is really for: a lane runs at a couple of hundred frames per pixel, so a
band twenty beats from the end is off screen until you are already inside it.

**Uncertainty.** The rasteriser has always faded beat lines by the grid's
confidence, and that fade cannot be *read*: at overview zoom the grid is
suppressed entirely for density, so faint and absent look identical and neither
says whether the analyser was guessing. A hatch under the record says so, over
the record's own length and nothing more, gated on the same threshold that
disables Sync rather than on a second one.

**Found by running the interface: the band never appeared at all.** The first
version asked the *library* for the record's grid, and a freshly loaded deck does
not have one there — the demo run showed both decks reading 123.7 BPM at full
confidence with un-analysed library rows and no band on either lane. Every
browser test passed, because the harness answers `waveform_info` itself. It now
reads the grid the tiles are rasterised from, which is the better answer anyway:
the band lines up with the beat lines beside it by construction, and follows a
hand-edited grid instead of the analyser's first opinion. There is a Rust test
for the wiring now, not only for the arithmetic.

**Eleven of twenty.** The overview's loop and cue markers are stamped with the
layer they are, so the check that reads every `data-layer` off a rendered page
covers that view too.

**The waveform's layers are named, counted and checked** — §25's "multilayer
semantic visualization architecture". Twenty layers live in
`dj_render::layer`, each saying what it encodes, which half of the renderer
draws it — the Rust rasteriser or the interface over the top, neither a
fallback — and whether it exists yet. Nine of the twenty do.

A list of twenty in a document is a list that quietly stops matching the code.
This one is checked in both directions: a golden file keeps the interface's copy
honest, and a browser test reads every `data-layer` off a rendered page and
fails if anything on screen is not in the table. "Nine of twenty" is a fact
rather than a recollection.

**§57 becomes a constraint rather than a warning.** "Never overload the same
colour with multiple meanings" is a property of the whole *set*, so it cannot be
enforced one layer at a time — which is the argument for the table existing.
Every drawn layer declares what its colour means and a test refuses two
unrelated layers on one meaning. Grouped roles are allowed and named: the three
grid layers are one meaning at three weights, which is texture, not a second
colour. A layer nobody has built reserves nothing, because that is how a palette
runs out for no reason.

**Two new layers**, both answering questions §25 puts to the waveform that
amplitude alone cannot. **Runway** — how much record is left, as a wash over the
last thirty seconds. **Seam** — what a mix *covers*, as a region between its two
marks rather than only the marks themselves.

**A stacking bug, found by looking at the application.** The runway shipped
under the tiles and was invisible: `z-index: 0` against tiles at `z-index: auto`
that come later in the DOM. The browser test passed anyway, because Playwright's
`toBeVisible` asks whether an element has a box, not whether anything can be
seen of it. The layer order is written down explicitly now — record, washes,
positions, playhead — and a test compares computed z-indices rather than
presence.

**One transition, performed by whoever is driving** — §68's remaining half. The
transition object has existed since the pair view shipped, and the automix went
on deciding its own decks, its own moment and its own length regardless. A DJ
who spent a minute adjusting a mix point and then handed over watched it be
ignored: two answers to one question, which is exactly what §68 says an
explicit transition object is for.

The automix performs the held mix now — its decks, its start, its length and
its style. **The start is the part that matters.** Without one the handover is
"the end of the file minus the transition length", which the automix's own
documentation is honest about being wrong for any record with applause on the
end. With one, somebody has actually decided, and a human may have dragged it.

**Where it does not apply it is ignored, not forced.** A plan about deck 1 says
nothing about a mix out of deck 3, and performing it anyway would be worse than
djmanzo's own answer — which is what it falls back to. And a held mix is spent
once performed: re-running a mix that has happened is not a mix.

**The autopilot defers to it too.** Its mix step used to push a style and a
length into the automix and fire; where djmanzo is already holding a mix for
those two decks it now says only "go", and reports "performing the mix you set
up".

**The panel says which promise is in force.** "Mixes out of the end of the
file" and "mixes where you said" are different things, and a panel that showed
them identically would be hiding the one thing that changed. The style and
length controls are dimmed rather than disabled while a held mix is in charge —
they still work, and what they set is the mix *after* this one.

Mutation testing earned its keep here. Three mutations, and the second one
survived: the test for "a plan about other decks is ignored" named a deck that
was not in the rig at all, so it passed whether or not the check existed. It
names a deck that is present now.

**The mix point can be grabbed** — the directive's §26, which is blunt about
it: "the DJ should be able to physically grab the thing they are thinking
about. Do not force them to edit a numerical property in a settings panel."
The pair view's move buttons were that settings panel. The mix point on the
outgoing waveform is a handle now — drag it, or focus it and use the arrow
keys, because a mouse is not the only hand.

**It moves in beats, not in pixels.** A mix point between two beats is a mix
point that is not on the grid, and djmanzo's whole answer here is about the
grid — so a drag says "this many beats later" and the snapping falls out of the
arithmetic rather than being a rule applied afterwards. The beat length comes
from the transition itself, which spans a known number of beats between two
known frames, so no tempo has to be inferred from a deck.

**The waveform works nothing out.** It reports where the handle was let go;
`transition_adjust` decides what that means and re-derives the reasons. Drag
the mix off its phrase boundary and it stops claiming to land on one — which is
§68's rule, now reachable with a hand.

**Only once djmanzo is holding the mix.** A proposal is an opinion and
`transition_adjust` refuses to move one, so a handle offered before *Set up*
would be a control that does nothing — the same rule the buttons beside it
already follow.

Two things came out of building it. The drag listens on the window rather than
calling `setPointerCapture`: djmanzo runs in WebKitGTK, the browser tests run
in Chromium, and capture is exactly the sort of thing that differs between them
— a control that passes its test and does nothing where it ships. And
`waveform_info` had never been answered by the browser harness, so every
waveform lane in the pair view had rendered as an empty box in every test that
has ever run over it, including the ones whose commit said "each with its
waveform". The lanes are drawn in the tests now.

**The assistant can ask for the interface, not only the controls** — the
directive's §41, and the last of `GUI-OVERHAUL.md`'s phase 5. `dj_app::uiop` is
a second closed vocabulary — `ui show prepare`, `ui pin room`, `ui focus 2` —
generated from `cockpit::surfaces()`, so a panel djmanzo does not have cannot be
asked for and the refusal happens at the parse rather than being applied and
quietly doing nothing. The model never emits JavaScript and never touches the
DOM, which is the thing §41 is emphatic about.

It is deliberately **not** on the action bus. An action is something the engine
does, and the engine has never heard of a panel; putting `ui show prepare` into
`dj_core::Action` would push the cockpit into the crate at the bottom of the
dependency graph and make a session replay depend on the interface it was
recorded against. Two closed vocabularies, kept equally strict, not mixed.

**Density is the one item on §41's list left out**, on purpose. The interface
picks its density from the window it is in, by measurement; an assistant
overriding that would be adaptation fighting adaptation, with the DJ unable to
tell which had last word.

**§72 gates it.** `adapt_layout` is its own row of the matrix, so "suggest
records but never touch my layout" is a setting rather than a feature request —
and an operation the posture refuses is reported to the DJ rather than
swallowed.

**It is reachable by hand too.** The palette offers pin, unpin and focus
alongside the surfaces, because §51 calls the palette the semantic interface
and an operation only the assistant could reach would be a control nobody can
press. Pinning especially: it is the per-surface half of freezing a layout and
there had been no gesture for it anywhere.

**One duplicated table removed.** Where a surface lands when it opens was a
`Record<Drawn, Dock>` in `App.svelte`; it is `cockpit::Surface::home` now,
because the moment the assistant could open a panel there were two answers to
"where does this go" — and the second one occasionally named a dock the surface
is not allowed in, which the resolver dropped with a note nobody reads. A test
asserts every home is a dock that surface can actually be placed in.

**The assistant asks before it acts** — the directive's §44. A non-trivial move
is a **transaction** now: load the record, cue it to its first phrase, trim it
to match, engage sync, run the mix — staged together as *Prepared next
transition*, with Accept, Modify and Reject, and carried out only then. Five
separate prompts is five chances to say yes to half a plan, which leaves a
record loaded and cued that nothing is going to mix.

Accepting is not a second way of doing things. Every move is an
`autopilot::Step` and Accept runs it through the same `perform_step` the
automatic tick uses, so what a press does and what the tick does cannot drift
apart. **Modify is a checkbox**: "load it and cue it, but I will bring it in
myself" is one click on one row, and it leaves the rest of the plan intact.
Partial success is reported as partial — a transaction that stopped at its
third move says which one and why, rather than reporting "accepted".

**§72's override matrix, as a table with three gates behind it.**
`dj_assistant::authority` holds the directive's matrix verbatim — ten
capabilities against six postures — and it is now the *second* of three
questions asked before anything moves: has a hand already claimed this control
(`Takeover`), does the posture permit this kind of thing at all (the matrix),
and is the read of the night sure enough to act on (§9's `Warrant`). Three
questions rather than one because a DJ whose machine did nothing deserves to
know which of them said no. It is configurable in both directions, except that
Off and Watch cannot be widened: those words would stop meaning anything.

**A refused move is shown greyed, never dropped.** A plan that silently stopped
after the trim would leave a DJ wondering what djmanzo thinks a cued deck is
for — and the obvious fix, turning the level up one notch, is invisible to
somebody who was never told what was going to happen.

**SAFE** — §47's emergency control, beside REC and Mark, and the only control
in that row that is coloured when nothing is wrong. It takes every control back
from the assistant, throws away anything staged, clears every effect on every
deck and on the master, flattens all three EQ bands and the filter, restores
master gain and re-engages the limiter.

What it refuses to do is the part that took the thought. **It never stops a
record, never moves a channel fader, and never touches the crossfader.** An
emergency control that silences the floor is far worse than the emergency: a DJ
who hits SAFE because an effect ran away has a problem, and one who hits SAFE
and gets silence has a disaster. It is `safe` on the action bus, so it is on a
controller pad and in a script as well as on screen, and every part of it is an
ordinary action that logs and replays.

**A half-built path found and finished.** `Step::Cue` was never emitted by
anything, and the action text it would have dispatched — `deck N seek_beat` —
is not a verb the parser accepts, so the one thing `Posture::Prepare` promises
in its own documentation ("loaded, cued to the phrase and gain-matched") had
never happened. Cueing is real now, and the *intent* is what is staged: where
"the phrase" is depends on a grid that arrives with the analyser seconds after
the load, so it is resolved at the moment of obedience rather than guessed when
the plan was built.

Found by driving it: pressing "Prepare the next transition" when there was
nowhere to stage produced nothing at all on screen, which reads as a broken
button. It says why now, in the autopilot's own words.

**The context engine** — the directive's §11, and the thing §9, §12, §14 and
§17 have all been waiting on. `dj_core::ContextEngine` answers one question,
in one place, for every consumer: **what is tonight?** The theme, the attention
budget, the assistant and the autopilot read its answer instead of each working
one out, which is what §11 asks for in the sentence "do not duplicate context
logic inside each component".

**It reads the night against itself.** "Loud" is a number about a room, a rig
and a mastering engineer, so there is no threshold at which a set is at its
peak. What is portable is a comparison with the same night earlier on, through
the same output — the argument `dj_assistant::room` already makes about a
camera, applied to a master bus, and now sharing its histogram rather than
keeping a second copy of it. Loudness and tempo are placed against tonight's
own spread of both, averaged because they are confounded differently: the
master fader moves one and not the other.

**It says nothing until it can say something true.** Six minutes of music
before the evidence may name a phase, and nothing at all if the night has not
varied enough to place a reading in. The first version of this module defaulted
to *Peak* at *0.95 energy* on every snapshot, and this is the guard against
that returning: a test drives eighty-nine readings and fails if a phase is
named.

**Where your word and the evidence disagree, you win.** An occasion is a
statement about the night; a histogram is not. So the declaration is the phase,
djmanzo says which way its own reading points, and the arc marks both. It
reports the disagreement; it never overrules the person who has been in the
room all night.

**§9, as a type rather than as a rule.** Autonomy — the posture — and certainty
stay orthogonal, and the one combination §9 calls invalid is not
*representable*: `dj_assistant::Warrant::Act` and `::Mix` carry a `Grounds`
whose constructor is private and refuses a certainty below `Fair`. The
consequence a DJ actually meets is that a night the music contradicts stops the
autopilot mixing unasked — it stages the record and says why — while everything
the room cannot hear carries on.

**The attention budget is consulted at last.** §18's `cockpit::Attention` has
existed as a type with nothing reading it; it is now derived from the same
context, published on the snapshot, and reaches the stylesheet as
`data-motion`. Two records audible means the interface may not reflow, because
somebody is reaching for it. A failed recording or a headphone card that has
stopped taking audio means nothing moves at all.

**The night, a surface of its own.** It shows its working rather than asking to
be believed: the arc with the phase marked, what produced it, how sure that
makes it, which way the music disagrees, and what all of that currently allows
the assistant to do. Before anything has read the night it says so, with the
count — an empty panel reads as broken.

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
