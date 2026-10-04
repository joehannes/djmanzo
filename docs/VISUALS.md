# Visuals in layers (§122)

The owner's decision (27 September): *djmanzo's own layers, with MilkDrop as
one of them* — a WebGL layer stack fed by bands and separated stems, plus
Butterchurn (MIT) as a "main" layer choice, shipping only clearly licensed
presets. With it, from the same decisions: karaoke looks chosen per song by
the DJ's AI provider (one call a song), artist images from Wikimedia Commons
and TheAudioDB (Creative Commons only, credited), and a free community index
of packs, with the pack format it installs.

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

## 3. Where it shows

First **behind the words on the singers' screen**, where it was asked for.
The DJ picks the look in the Singers panel, from those djmanzo ships, or
*Off*. Then the **Video** window (`docs/VIDEO.md`): when no deck with a
picture is heard, it shows the look instead of black, so the projector is
never dark.

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

## 5. Artist images

A layer kind showing images of the artist playing, from Wikimedia Commons and
TheAudioDB, each only where its licence is Creative Commons, the credit drawn
in a corner of the screen while it shows. Asked for once per artist and kept,
as K1 keeps covers.

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
