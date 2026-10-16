# A karaoke night with djmanzo Karaoke

For whoever is hosting, on the night. It assumes nothing about DJ software.
What each feature is and why it works the way it does is in
[KARAOKE.md](KARAOKE.md); this page is only what to press.

## Which download

| Your computer | Download |
|---|---|
| A Mac with an Intel processor, macOS 10.13 (High Sierra) or later — including 2011 MacBooks | **djmanzo Karaoke** `.dmg` (x86_64) |
| A newer Mac, or Windows, or Linux | the ordinary **djmanzo** download — it has every karaoke feature, and more |

*About This Mac* (the Apple menu) says which macOS a Mac runs. A 2011
MacBook's last is 10.13, High Sierra.

## Installing on an older Mac

1. Open the `.dmg` and drag **djmanzo Karaoke** into *Applications*.
2. The first time only: **right-click** (or Control-click) the app and choose
   **Open**, then **Open** again. The build is not signed with an Apple
   developer certificate, so a plain double-click is refused once.
3. If macOS says the app "is damaged and can't be opened", that is the same
   refusal worded worse. In *Terminal*:

   ```
   xattr -dr com.apple.quarantine "/Applications/djmanzo Karaoke.app"
   ```

It keeps its settings apart from the ordinary djmanzo, so both can live on
one machine.

## The first time it opens

A welcome asks a few questions; every one can be skipped (*Not now*). It
opens on **Karaoke**: two decks, the **Singers** panel on the right and your
music underneath. Press **Space** at any time and a guide shows every key.

**Your songs.** In the library at the bottom, the folder button adds a
folder of music. Each song is analysed once, in the background — a minute
or two per song on an old laptop, so add the folder well before the night.

**The words.** djmanzo finds a song's lyrics in three places: the words
saved in the file itself, a `.lrc` file with the same name beside it, or
LRCLIB on the internet, which it asks for your whole collection in the
background and keeps. The singers' screen itself never needs the internet —
so add your music while the laptop is online, the day before, and the words
are there on the night.

## Running the night

**Before the first singer:** *Presets* (Space o d) → *Karaoke night* →
**Open microphones** — the limiter on, the master and the booth a little
down, so a microphone pointed at a speaker has room before it howls.

**A singer.** In *Singers* (Space o i), type their name and start typing the
song — it searches your music — pick it, and **Add**. The card at the top is
whoever is up next:

- **Load on 1** / **Load on 2** — their song onto a deck, in the key they asked for.
- Under each deck number: **As recorded**, **Guide** (a little of the
  recorded voice left in to follow) or **Out** (the voice taken away).
- **−** / **+** — the key, a semitone at a time, without changing the tempo.
- **Bring them in** — mixes from whatever is playing into their song, on
  the next phrase.
- **Sang** — they go to the back of the queue, and the next card comes up.
- **Not here** — skips someone who stepped out.

**Changing the order.** Drag a singer onto someone else's row to give them
that place, or onto the big card to make them next — or press **⤒** on
their row. You can also drag a song from your music onto the Singers panel
to ask for it, or onto a deck to load it there.

**The singers' screen.** *Singers' screen — the words, on a display of its
own* opens a second window. Drag it onto the projector or TV, and make it
full screen there. It shows the words as they are sung, the next line, and
who is up next.

**Phones.** Tick *Singers put their names down from their phones*: a QR
code and an address appear, and anyone on the same Wi-Fi can put
themselves in the queue with a song from your music. Nothing they ask for
plays until you load it.

**Between singers.** Tick *Break music between singers* and choose a
playlist: music plays while the next singer gets ready, and leads into
their song.

**The Karaoke night presets** (Space o d), each for the deck you choose:
*Vocals out*, *Guide vocal*, *Everyone sings* (the record's own voice back,
for a group number), *Down a tone*, *Down a fourth* (a high song for a low
voice), *Up a tone*, *Original key*, and *Open microphones*.

**Screens for the night.** Press **0** for the launcher: the **Karaoke**
board puts the singers and your music side by side with the decks in brief
across the top; **Karaoke prep** is for the hour before. The theme button at
the top left has three made for karaoke rooms: *Karaoke Neon*, *Spotlight*
and *Velvet*.

**If anything goes wrong:** the **SAFE** button in the top bar takes every
control back, clears every effect and flattens the sound.

## What this build leaves out, and why

Built to reach a 2011 laptop, it leaves out two things that need a newer
Mac:

- **Timing the words by listening** (whisper.cpp needs macOS 10.15). Lyrics
  that come already timed — most of LRCLIB's, a timed `.lrc` or the file's
  own — still follow the song line by line; lyrics with no times are shown
  as they are, without the wipe.
- **Spoken commands**, for the same reason.

Taking the voice out uses djmanzo's built-in separator; the neural one the
newer builds carry has no version for Intel Macs.

## On an older laptop

- It starts with the lightest drawing (*Eco*) and steps up on its own when
  the machine keeps up.
- On macOS 10.13 the interface is drawn by an older Safari engine: some
  spacing is tighter and a few tints flatter than in the screenshots.
  Everything still works.
- Close panels you are not using; each one drawn costs the old processor a
  little.
- Nobody has run this build on a 2011 Mac yet — there was none to try it on.
  Try it a day before the party, not an hour before.
