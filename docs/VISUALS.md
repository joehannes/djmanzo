# Visuals in layers (§122)

The owner's decision (27 September): *djmanzo's own layers, with MilkDrop as
one of them* — a WebGL layer stack fed by bands and separated stems, plus
Butterchurn (MIT) as a "main" layer choice, shipping only clearly licensed
presets. With it, from the same decisions: karaoke looks chosen per song by
the DJ's AI provider (one call a song), artist images from Wikimedia Commons
and TheAudioDB (Creative Commons only, credited — TheAudioDB's records turned
out to carry no such mark, see step 5), and a free community index of packs,
with the pack format it installs.

This is the plan for all of it, in the order it is built. Each step is a
commit of its own, gated, mutation-tested and driven.

## The rule that overrides everything here

**The words must render even if every visual fails** (KARAOKE.md §3). The
layers are drawn on a canvas *behind* the words, which stay DOM text; K1's
background — the record's cover and its colours, plain CSS — stays beneath the
canvas. A webview with no WebGL draws no layers and shows K1's background, as
today. Nothing the layers do can take the words down, and a layer that fails
to compile is left out rather than taking the stack with it.

## 1. What the layers hear

Everything a layer reacts to arrives in the snapshot djmanzo already sends
sixty times a second; nothing new crosses from Rust per frame but four
numbers a deck.

| Signal | From | Already sent |
|---|---|---|
| loudness, and bass / low mid / high mid / treble | the master bus (`dj_dsp::Spectrum`) | yes |
| where in the beat, and the tempo | the playing deck's grid (`beat_phase`, `effective_bpm`) | yes |
| the singer's voice | K3's strips (`master.vocals.strips[].level`) | yes |
| **each stem as heard: vocal, drums, bass, other** | the deck, after its own stem volumes and mutes | **added in step 1** |
| the record's colours | §110's eight bands, as K1 uses them (`singer_backdrop`) | yes |

**Step 1 — stem levels (built).** Each deck reports the peak of each of its
four stems per block, after the stem's own tone, volume and mute, so a muted
vocal is a silent vocal to the visuals too — and since a stem's tone carries
the deck's EQ and filter, a DJ who kills the lows kills the bass's light. Measured where the stems are
mixed, a running maximum per stem reset every block: no allocation, no lock,
four floats into the parameter registry beside the deck's own peak
(`deck.N.stem_vocal_level` … `stem_other_level`), and in the snapshot as a
deck's `stem_levels`. A record not yet separated, or a paused deck, reports
nothing for its stems.

They are measured **before the deck's fader**: what each stem contributes to
its deck, not how much of the deck the room hears. The stack weighs each deck
in by its share of what is heard — its fader and its side of the crossfader —
exactly as the Video panel weighs pictures (`videoMix.ts`), so a record being
cued with its fader down lights nothing, and both rules live in one place.

## 2. The stack

A **look** is an ordered list of layers, bottom first. A **layer** is:

- a **kind** — what it draws;
- a **signal** it answers to (one of the table above);
- a **colour** — the record's colours, or a fixed one;
- an **amount** (opacity) and a **blend** (over, add, screen).

The kinds djmanzo ships are its own, small fragment shaders, each a few dozen
lines and each answering to one number:

| Kind | Draws | Answers best to |
|---|---|---|
| `wash` | a slow field of the record's colours | loudness |
| `pulse` | rings from the centre on each beat, sized by the signal | bass, drums |
| `ripple` | interfering waves across the screen | mids, other |
| `sparks` | points that flare and fade | treble |
| `halo` | a glow around where the words are | the voice: vocal stem, or the singer's microphone |

The directive names the layers a look is made of — *background, main, fx,
bass, mids, treble, instruments, vocal* — and these are those, by what they
answer to: `wash` the background; `pulse`, `ripple` and `sparks` the bass,
mids and treble; the stems the instruments; `halo` the vocal. *Main* is the
slot a MilkDrop preset can take (step 4). *Artist images* (step 5) are a
layer kind too.

One canvas, one WebGL context, each layer drawn as a full-screen quad into it
in order with its blend. A look's layers are validated before they are drawn:
an unknown kind or signal is dropped with its name said, not guessed at.

**Step 2 — built.** Where each part lives:

- **The looks are data, in Rust** (`dj_app::looks`). The DJ's AI provider
  will be told them by name and description (step 6), and packs will add to
  them (step 7). The choice is kept in `looks.json`, because the screen that
  draws it is another window: the Singers panel writes it, and the singers'
  screen asks for it every three seconds, as it asks for the rotation.
- **Checking a layer is the interface's** (`ui/src/looks.ts`). The interface
  is what knows what it can draw. Its lists of kinds, signals and blends are
  held to Rust's by a test that reads `looks.rs`.
- **The signals** are in `looks.ts` too:
  - **Stems** are weighed by each deck's share of what is heard (step 1).
  - **The beat** is the most-heard deck with a grid: struck on the beat and
    falling through it.
  - **The voice** is the loudest singer's microphone.
  - **Every signal is followed as a light follows sound:** up in 30 ms and
    down over 250 ms. The arithmetic depends on time, not frames, so a
    projector at 30 frames a second looks like a screen at 120.
- **The pictures** are `ui/src/visualStack.ts`: WebGL 1, one shader of a few
  dozen lines per kind. The shaders write premultiplied colour, so *over*,
  *add* and *screen* are three blend functions.
- **The component** is `ui/src/Visuals.svelte`: the canvas, and a frame loop
  that writes to it directly.

djmanzo ships five looks, all its own: *Glow*, *Pulse*, *Waves*, *Sparkle*
and *Everything*.

**Their `halo` answers to the singers' microphones, not the vocal stem.** On
a karaoke screen the record's singer is usually the one thing taken *out*, so
the vocal stem is near silence exactly when a glow around the words is
wanted. A test in `dj_app::looks` holds every built-in halo to `voice`.

**What it costs.** Measured in this container, driving the real application
under Xvfb, where WebGL is drawn by the processor (llvmpipe). The singers'
screen was a window of 640×360 showing *Everything*, and the figure is its
WebKit process's processor time:

| What the screen shows | Processor time |
|---|---|
| K1's background alone | about 100% |
| *Everything* at full size, sixty a second | about 217% |
| *Everything* at half size each way, thirty a second | about 133% |

The layers are soft fields and glows, and a canvas stretched to fill the
screen looks as one drawn whole, so **half size and thirty a second are the
setting everywhere**. Choosing by renderer was tried and does not work:
WebKitGTK names its renderer *Apple GPU* whatever draws it (HANDOFF's
traps). On a graphics card the cost is presumably far smaller. Nothing here
can measure that.

### Nothing the layers do takes the words down

- **The canvas lies inside K1's background and under its veil.** The veil
  darkens the middle of the screen, where the words are, so the layers never
  take the contrast a singer reads by.
- **A picture that will not compile is left out and named.** The name goes
  in the window's console and on the canvas's `data-failures`. The other
  layers are drawn.
- **No WebGL, or a lost context, leaves the canvas clear.** K1's background
  shows as before.
- **A system asking for reduced motion gets no layers at all.** The Singers
  panel says so, so a DJ is not left wondering why nothing moves.

**Tested:**
- 3 in Rust:
  - every built-in look drawable and named once, its halo on the voice
  - a choice kept between runs, and off a choice
  - a look not there refused, and forgotten if kept from before
- A golden `ui/e2e/looks.json`, generated from `built_in()`.
- 16 of the arithmetic (vitest).
- 11 in a browser, reading the canvas back off the GPU:
  - every layer drawn under the veil, with the words above
  - the canvas at half size and thirty frames a second
  - the wash dark when the room is quiet and full when it is loud
  - the drums lighting the pulse, and a closed fader putting it out
  - one picture failing to compile, and the rest drawn
  - no WebGL
  - reduced motion
  - off
  - a look picked in another window, followed and turned off
  - the panel's list
  - the panel's note on reduced motion

**Driven in WebKitGTK:**
- *Everything* behind the words, with the beat's rings moving out between
  captures.
- The picker in the Singers panel, kept in `looks.json`.
- The screen following a change within its poll.

Not seen: a graphics card, macOS and Windows.

## 3. Where it shows

First **behind the words on the singers' screen**, where it was asked for —
built in step 2. The DJ picks the look in the Singers panel, from those
djmanzo ships, or *Off*. Then the **Video** window (`docs/VIDEO.md`): when no
deck with a picture is heard, it shows the look instead of black, so the
projector is never dark.

**Step 3 — built.** The Video panel has *With no picture*: *Black*, or one of
the looks, kept in `looks.json` as the singers' screen's choice is. The look
is drawn beneath the pictures, at the opacity the pictures leave:
`1 − whole`, where *whole* is how much of what is heard has a picture
(`videoMix.ts`). A record with a picture heard alone covers it, and a record
without one lets it through. Its colours are those of the record most of the
room is hearing.

- **Held by:**
  - two browser tests: the look whole, gone, then half, as decks start and
    the crossfader moves; and picked and set back to *Black* in the panel
  - Rust's test of each screen keeping its own choice
- **Mutation-tested:** three mutants fail them:
  - the look always whole
  - the panel picking for the singers' screen
  - the video choice kept as the singers'
- **Driven in WebKitGTK:** docked, the beat's rings moving between captures,
  with no deck carrying a picture.

**Not tuned.** The looks' brightness follows the master's RMS loudness and
the bands as `dj_dsp::Spectrum` measures them. On this container's synthetic
demo records they are dim, and nothing here can say how bright they read on
real music in a real room.

## 4. MilkDrop: Butterchurn as the main layer

Butterchurn (MIT) renders MilkDrop presets in WebGL. It is fed the time-domain
sound it expects from what djmanzo has: the bands, rebuilt as a sum of tones
at each band's centre, so its bass, mid and treble readings follow the room's.
Its canvas is the stack's *main* layer.

**Presets.** The MilkDrop community's presets are separate works whose
licences are mostly unstated; the owner's rule is that only clearly licensed
ones ship. So djmanzo ships presets **written for it**, under its own licence,
and lets the DJ load their own preset files from disk — theirs to answer for,
never redistributed. Whether any published collection carries a licence clear
enough to ship is a question to answer before any is bundled, not after.

**Step 4a — built: MilkDrop as the main layer, with djmanzo's own presets.**

- **Which Butterchurn.** `butterchurn` 3.0.0-beta.5 (MIT, July 2025), not
  2.6.7. Both evaluate a preset's equations as JavaScript with `new Function`
  when given them so; only the 3.0 beta can instead compile MilkDrop's own
  equation language to WebAssembly (`eel-wasm`), and with `onlyUseWASM` it
  never reaches `new Function` at all. djmanzo's content rules gain
  `'wasm-unsafe-eval'` — WebAssembly may be compiled — and still forbid
  `'unsafe-eval'`. A Rust test holds both halves.
- **Its sound.** Butterchurn takes a block of samples and finds its own bass
  (20–320 Hz), mid (320–2,800 Hz) and treble (2,800–11,025 Hz). djmanzo's
  audio is Rust's, so the block is **made from the master's four bands**: a
  tone at 100, 600, 2,000 and 6,000 Hz, each as loud as its band, carried on
  from one block to the next (`ui/src/milkdrop.ts`; vitest measures each
  tone's strength with a Goertzel filter). No audio context is created.
- **In the stack.** A `milkdrop` layer names its preset; Butterchurn draws
  into a canvas of its own at the stack's size, which the stack takes in as a
  picture — a texture drawn with the layer's amount and blend, under the
  other layers. Butterchurn is loaded only when a look has such a layer.
- **The presets.** *Drift* (slow rolling waves) and *Bloom* (a burst of
  colour whose trails spread on the kick), in Butterchurn's own JSON with
  equations only in MilkDrop's language and empty warp and composite shaders
  (Butterchurn's defaults) — no shader code shipped. A Rust test holds that
  no preset carries JavaScript and each has the four shapes and four waves
  Butterchurn's WebAssembly path indexes. Served by `milkdrop_preset`. Two
  looks use them, *Drift* and *Bloom*. **First drafts**: tuned only against
  this container's synthetic demo records.

**What driving it in WebKitGTK found** — each passed every browser test, and
each stopped MilkDrop dead in djmanzo's Linux webview:

1. **An `OffscreenCanvas` has no WebGL 2 there.** Butterchurn draws into one
   whenever the class exists; WebKitGTK 2.52 has the class and gives it no
   WebGL 2 context, so Butterchurn failed at its first framebuffer. djmanzo
   now hides the class from Butterchurn while it is made, where an offscreen
   canvas cannot have WebGL 2, and it draws into an ordinary canvas. A
   browser test gives Chromium the same refusal.
2. **Asynchronous WebAssembly never settles there.** `WebAssembly.instantiate`
   neither resolved nor rejected — not even for an empty module — while
   `new WebAssembly.Module` compiled at once under the same content rules.
   Butterchurn's loaders compile asynchronously, so the first preset waited
   forever with nothing to say. While a preset loads, djmanzo compiles
   synchronously; Chromium refuses a synchronous compile of more than a few
   kilobytes on the main thread with a `RangeError`, and that falls back to
   the asynchronous call. Whether the content rule is what stalls it was not
   settled: the diagnostic would have meant switching on `'unsafe-eval'`,
   and that is not done, even for a moment.

After both, in WebKitGTK: *Bloom* and *Drift* drawn in the Video panel,
moving between captures, a Butterchurn frame taking about 8 ms at 177×100.

**Tested:**
- Rust: the shipped presets in MilkDrop's language only and whole; the
  content rules allowing WebAssembly and forbidding `eval`; every MilkDrop
  layer naming a shipped preset.
- 7 vitest: the tones in Butterchurn's bands, each as loud as its band and
  only its band, silence and no clipping, carried on between blocks; a
  module got where the asynchronous calls never settle (as in WebKitGTK),
  and the asynchronous call used where a synchronous one is refused (as in
  Chromium); a MilkDrop layer kept only with its preset.
- 3 browser tests under djmanzo's real content rules, read back off the
  GPU: MilkDrop drawn and moving; drawn where an offscreen canvas has no
  WebGL 2; and, with WebAssembly refused, left out and named while the rest
  is drawn.
- **Mutation-tested:** thirteen mutants fail them.

**Step 4b — built: the DJ's own presets.** In the Singers panel, *Add
MilkDrop presets…* takes MilkDrop's own `.milk` files or Butterchurn's
`.json` and copies each into `milkdrop/` beside the other settings, only if it
can be drawn. Each that can becomes a look, *Yours: <name>*, wherever looks
are picked. The Video panel's list is read again when it is opened.

- **A `.milk` file is read by Rust** (`dj_app::milk`). The `key=value`
  settings become Butterchurn's base values by a table of the 37 names
  MilkDrop and Butterchurn spell differently, taken from Butterchurn's own
  reader `milkdrop-preset-utils` (MIT; data, not a dependency). The numbered
  equation lines are joined in the order of their numbers, not of the file —
  `per_frame_10` after `per_frame_2`. The four custom waves and shapes keep
  theirs.
- **MilkDrop 2's shaders are HLSL**, which nothing here translates. They are
  left out, the preset draws with Butterchurn's default pair, and the list
  says so.
- **A Butterchurn `.json` is taken as it is** when its equations are in
  MilkDrop's language. One with only JavaScript equations is refused with the
  reason, and any JavaScript beside MilkDrop's own is stripped.
- **Either is made whole**: four shapes, four waves, every field a string.
- **Refused, each with why:** larger than 512 KB, not `.milk` or `.json`,
  nothing MilkDrop in it.
- **A preset's name finds a file only among those listed**, never by joining
  it to a path: `yours/../x` reaches nothing.
- **Removing one** clears any screen showing its look.
- **They are the DJ's to answer for**: djmanzo draws them and never sends them
  anywhere, which the panel says.

**Tested:**
- 4 Rust tests of the reading:
  - a MilkDrop 2 file read whole — renamed and plain settings, lines past
    nine in the order of their numbers, a custom wave and shape, HLSL left
    out and said
  - any preset made whole
  - JavaScript refused or stripped
  - only `.milk` and `.json` taken
- 2 Rust tests of the folder:
  - taken, refused with why (JavaScript, the wrong kind, too large), offered
    as a look, drawn from the file, a broken one listed with no look, and
    removed with the choice
  - no name reaching outside the folder
- 2 browser tests:
  - added from the dialog, offered, refused with why, and removed
  - one drawn behind the words under the real content rules
- **Mutation-tested:** eleven mutants fail them. The path test first passed
  with a name joined straight to the folder: Linux will not resolve `..`
  through a folder that does not exist, and the test had none. It makes the
  folder now.

**Driven in WebKitGTK:** a `.milk` of djmanzo's own making, written as
MilkDrop writes them, with a custom wave drawn from its own per-point
equations. It sat in the folder, was chosen for the Video panel as *Yours:
Night Drive*, and drew — its ring and trails moving between captures.

## 5. Artist images

A layer kind showing images of the artist playing, from Wikimedia Commons and
TheAudioDB, each only where its licence is Creative Commons, the credit drawn
in a corner of the screen while it shows. Asked for once per artist and kept,
as K1 keeps covers.

**Built (5 October), from Wikimedia Commons.** An `artist` layer shows
photographs of the heard deck's artist. A new look, *Artist*, has it over the
record's colours, pressing in on the beat, with the singer's glow on top.

- **Whose:** the deck heard most — its fader and its side of the crossfader,
  by the rule the Video panel uses. While nothing is heard the last artist
  stays. A record naming no artist shows no one's.
- **Found through Wikidata** (`dj_app::artists`):
  - by the MusicBrainz artist id the record was tagged with, when it was
    (`dj_library::tags::artist_id`);
  - else by name, among items that are MusicBrainz artists at all, taking the
    first whose own name — a label or an alias, in any language — is that
    name. A search for *Adele* also finds singers merely called Adele
    somewhere in their item.
  - When a credit with guests ("A feat. B") finds no one, its lead artist is
    searched for.
- **The pictures:** the item's main picture first, then the photographs in
  its Commons category whose file names name the artist — how a concert
  photograph is told from the album covers, logos, icons and strays filed
  there. JPEG only, at least 800 pixels wide, at most six, asked for 1280
  wide.
- **The licence:** CC BY, CC BY-SA or CC0, any version or port, with an
  author to credit.
  - Not Commons' *public domain*, which there is often the United States'
    alone, while the screen may be anywhere.
  - Not anything non-commercial or without derivatives (Commons hosts none,
    but the check does not rely on that), the GFDL or the Free Art License.
- **The credit**, drawn in the corner while its photograph shows and faded
  with it: *Photo: author · licence · Wikimedia Commons*. Each file's page,
  where its whole credit is, is kept beside it.
- **Asked once per artist and kept** in `artists/<key>/`: the photographs
  and their credits, or that there are none. The key is the MusicBrainz id,
  or a hash of the name — never the name, which could name a path.
  - An answer that did not come is not kept: no network, or Wikimedia asking
    to wait. It is waited on for ten minutes, said why, then asked again.
  - Two screens following one deck ask once between them.
  - Requests go one at a time, with a User-Agent naming djmanzo and where to
    reach its makers, and `Retry-After` honoured up to half a minute, as
    Wikimedia's 2026 rules ask.
  - The network is reached only while a look with an artist layer is drawn.
- **On screen:**
  - Each photograph fills the screen as a cover does: cropped, never
    stretched.
  - It drifts slowly and presses in a little with its signal, darker towards
    the edges so the words and the credit stay readable.
  - Each is held twelve seconds, then fades into the next.
  - A new artist's photographs fade in once the last artist's have faded out.
  - A stack with a photograph in it is drawn at full size, within the same
    960×540 cap: a soft field stretched looks as it would drawn whole, and a
    face does not.
- **Handed to the page as `data:` URLs.**
  - Not by the `art:` scheme, because WebGL will not draw another origin's
    picture.
  - Not as raw bytes: see below.

**TheAudioDB is not asked.** Its terms say to check each artist's
`strCreativeCommons` tag, and no artist record carries one. That was looked
for on 4 October across four artists, by search and by id, through the v1
API; the v2 API needs a paid key to look. The decision is "each TheAudioDB
image used only when its record says Creative Commons", and so none can be.
Its free key is for development projects only, besides: a published djmanzo
would need the DJ's own paid key. If its records ever carry the tag, it would
come in behind the DJ's own key, off by default, as Musixmatch will.
`docs/RESEARCH.md` has the row, corrected.

**Driving it found three things, none visible to a browser test:**
1. **Raw bytes reach WebKitGTK's page as an array of numbers.** The first
   version returned the JPEG as a binary IPC response, which Chromium's page
   receives as an `ArrayBuffer`. djmanzo's Linux webview received a plain
   array of 172,199 numbers, and the photograph every browser test drew was
   *Loading error.* there. Pictures go as `data:` URLs now, the same
   everywhere, and the browser tests' stand-in for Rust answers in that shape.
2. **Loading a record erased its tags.** The decoder read no tags from a
   `lofty`-written ID3v2.4 MP3, and loading upserted what it read over what
   the scan had read: the artist, genre, label, year and comment were gone
   from the collection each time a record was played. The deck said
   *daft-demo*, and the artist layer had no one to show. Loading now fills
   only what the collection lacks, and the deck shows the collection's
   names. This is its own commit, with a test for each.
3. **A failed asking left the layer empty for good.** A record restored onto
   a deck as the application starts can be asked about before the collection
   has it. The layer now asks again five seconds later.

After those, in WebKitGTK:
- A record tagged as Daft Punk, with their MusicBrainz id, chose *Artist* in
  the Video panel.
- djmanzo asked Wikimedia. On this container's shared address Wikimedia
  refused (429), as it does most of the time here: nothing was kept, and the
  panel showed the record's colours.
- With the real Commons photograph and its real credit kept where djmanzo
  keeps them (the answer Wikimedia gave the live check), the waiting panel
  asked again and drew the photograph:
  - the right way up and cropped to fill
  - drifting between captures
  - *Photo: Sony Music Entertainment · CC BY 4.0 · Wikimedia Commons* in its
    corner, sized to the picture rather than the window

**Live, outside the app:** the whole asking ran against Wikimedia with real
answers — the search by id, the item's statements, the picture's and the
category's licences, and two photographs. The third was refused (429) for ten
minutes running, so as designed nothing was kept.

**Tested:**
- 13 Rust tests:
  - only CC BY, BY-SA and CC0 taken, every other licence refused
  - a credit as plain words a corner holds
  - photographs told from covers, logos and strays in a real category's mix
  - a file names them or it is not theirs
  - the one called by the name picked, not those merely called it somewhere
  - the lead artist of a credit with guests
  - asked once per artist and kept, a deprecated picture never asked for
  - a refusal asked again, and no one found kept
  - by name, the lead artist found when the whole credit is not
  - a download must be a JPEG, and a refusal stops the asking
  - no name from the interface reaching outside the folder
  - two screens asking once between them, and a failure waited on
  - the artist id read from a file's tags
- 5 vitest:
  - three of the slideshow: held, faded, and the next; a single photograph
    held; one artist fading out before the next fades in
  - two of whose: the heard deck's artist by fader and crossfader; no one
    while nothing is heard
- 4 browser tests:
  - **drawn behind the words**, under djmanzo's real content rules, read back
    off the GPU:
    - the right way up
    - asked, told it is being asked for, asked again
    - credited in the corner while it shows
    - drawn at full size
  - asked again after a refusal
  - with nothing that may be shown, the rest of the look and no credit
  - a look without the layer asks for no one
- **Mutation-tested:** 14 Rust and 9 interface mutants fail them.

## 6. Looks chosen by the AI, per song

One call per song to the DJ's provider, given the record's genre, tempo, key,
energy and title, and the names and descriptions of the looks there are; it
answers with one of the names, which is checked against the list. Off unless
the DJ turns it on, as every AI feature is.

## 7. Packs, and the community index

A **pack** is a folder of looks, presets and the images they need, with a
manifest (name, author, licence, version, and the SHA-256 of every file) and
a signature over the manifest by its author's key. djmanzo installs a pack
only if every file matches and the signature verifies, and shows the licence
before it does. The **index** is a public list of packs hosted wherever their
makers choose — a JSON file of names, descriptions, licences, addresses and
the key each is signed with — which djmanzo reads and installs from. Free; no
accounts, no payment, no hosting by djmanzo.
