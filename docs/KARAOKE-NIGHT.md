# A karaoke night with djmanzo Karaoke

For whoever is hosting, on the night. It assumes nothing about DJ software.
What each feature is and why it works the way it does is in
[KARAOKE.md](KARAOKE.md); this page is only what to press.

## Which download

| Your computer | Download |
|---|---|
| A Mac with an Intel processor, macOS 10.13 (High Sierra) or later — including 2011 MacBooks | **djmanzo Karaoke**: `djmanzo.Karaoke_<version>_x64.dmg` on the [releases page](https://github.com/joehannes/djmanzo/releases/latest) |
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

**A monitor for each singer** (an interface with ten outputs or more).
Under *Microphones*, tick *A monitor for each singer*: Mic 1's singer hears
outputs 7–8, Mic 2's 9–10, and so on. Press a singer's name for *Their
monitor* — more of themselves, less of the others, less of the song —
and djmanzo keeps it with them. With eight outputs there is one monitor,
7–8, that everyone shares.

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

**A filler deck.** Put your jingles and fillers in a playlist. Under the
deck you want them on, press the *Playlist* tab, choose it, and tick *Next
when this ends* and *Repeat*: when one ends the next starts, round and
round. A press on any record in the drawer loads it on that deck; ▶ loads
and plays it.

**Emoji from the room.** In the Singers panel, tick *Emoji from phones on
the singers' screen*. Guests then see a row of emoji on the room's page;
each tap shows small in the corner of the singers' screen and fades within
seconds. It is off until you turn it on.

**The guide voice.** When the song up next is new to the singer, the
up-next card shows *Guide voice*, already ticked. Load as usual and the
original singer stays quietly under them; untick it if they know the song.

**Duets.** Put each singer on their microphone (Singers panel, *Mic 1*,
*Mic 2*). Their names appear on the singers' screen in their own colours,
lit while they sing; a lyric line starting with a name ("Ana: …") is
wiped in that singer's colour.

**Karaoke presets.** Each microphone's *is for* menu now has *Stadium*,
*Radio voice*, *Big echo* and *Duet* beside *Singer*. They change only the
sound of the voice; the fader, pan and where it is sent stay as you set
them.

**A guest's song, sent.** In the guest's record, *Fill in Suno's form*
opens Suno in djmanzo's own window and puts in the words and the style.
Sign in there the first time; check what it filled and press Create
yourself. Open the finished song in that window and its link appears in the
card. Then *In djmanzo's window* opens WhatsApp Web at the guest's chat with
the message written (sign in once with your phone's QR code) — or
*WhatsApp* for the app. You always press send.

**If anything goes wrong:** the **SAFE** button in the top bar takes every
control back, clears every effect and flattens the sound.

## Listening: on by itself where the Mac can

Two things listen — **timing the words by listening** (*Words in time*, on the
Singers panel) and **spoken commands** — and they need macOS 10.15 (Catalina)
or later and a processor from about 2013 on. You do not need to know which
Mac it is: djmanzo checks when it starts and switches them on by itself if
the Mac can run them.

- **On a Mac that can**, *Words in time* offers the speech models to download
  (Base, about 60 MB, is the one to take) and times the words of any record.
- **On a Mac that cannot** — a 2011 MacBook stops at macOS 10.13 — it says
  why, and offers nothing to download. Lyrics that come already timed — most
  of LRCLIB's, a timed `.lrc` or the file's own — still follow the song line
  by line; lyrics with no times are shown as they are, without the wipe.
- **Settings → Speech recognition** says what it found, has *Check again*, and
  can turn it **Off** if you would rather it never listened.

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
