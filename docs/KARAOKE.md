# Karaoke

> **What is built, and what is still only written here** (updated 2026-09-25,
> §107). This document was the plan; for a long time almost none of it reached
> a DJ. Built: **the singer rotation** (`dj_app::karaoke`, the *Singers*
> surface, the *Karaoke* activity on F8) — first come first served, one song a
> turn, not ready is the bottom, a singer's key remembered with the song, the
> up-next song loaded onto a deck in the singer's key. Built earlier and
> still true: lyrics fetched from LRCLIB, synced where the database has them.
> Built since: **taking the voice out** — one knob per deck
> (`deck N voice <0..1>`), a gain on the vocal stem where the record is
> separated and the band-limited centre cancel of §1.3 on the whole mix where
> it is not yet; *As recorded · Guide · Out* on the Singers surface, and a chip
> on the deck while the vocal is not as recorded. And **the singers' screen**:
> a panel for a second display with the line being sung wiped in time (word by
> word where the lyric is Enhanced LRC), the next line, a count-in and the next
> singer (`dj_library::lrc`, `singer_lyrics`, `SingerScreen.svelte`). And
> **break music between singers** (`dj_app::breaks`, a row on the Singers
> surface): the host names a playlist, a deck and a level (*Quiet*,
> *Background*, *Full*); two seconds after the room goes quiet a record from
> that playlist fades in along its own playhead, and the moment anything else
> starts it fades out, pauses and hands the fader back where the host had it.
> Each break takes the playlist's next record. It only ever plays a record it
> put there itself — the next singer's song loaded on the break deck by
> mistake is left alone, and the row says why nothing is playing.
>
> **Next** (added 2026-09-27): the milestones were re-cut as K1–K9 in
> [ROADMAP.md](ROADMAP.md#karaoke) after a comparison with KaraFun. K3, the
> singers' microphones, is designed in §6 below; K4–K9 are outlined there.

Two independent halves, usable together or separately:

1. **Take the voice out** — a ladder of techniques, best first, with the
   trade-offs stated so you can pick per track.
2. **Put the words up** — timed lyrics on a second screen, with artwork and
   beat-reactive visuals.

And a third that both of them assume and nothing yet provides: **carry the
singers' voices in** — a microphone per singer, each with a chain of its own,
and a monitor they can hear themselves in. That is K3, in §6.

The first two are separate on purpose. Plenty of DJs want lyrics on screen over a full
mix as a sing-along, and plenty want an instrumental with no lyrics at all.

---

## 1. Removing the voice

There is no single best method — it depends on what the track is and what you
already own. djmanzo offers four, in quality order, and picks the best available
per track unless you override it.

### 1.1 The instrumental you already have

Checked first, and routinely forgotten. Many DJs already own official
instrumental, karaoke or acapella-minus versions. Nothing beats a real
instrumental: it is what the mix engineer intended, with no artefacts at all.

The library links a track to its instrumental counterpart — by tag convention,
by filename, or by hand — and karaoke mode simply plays it. Free, perfect, zero
CPU.

### 1.2 Neural stem separation

The default when no instrumental exists. The [stem engine](ARCHITECTURE.md#6-stem-engine)
already separates every track into vocals, drums, bass and other; karaoke mode
mutes the vocal stem.

Quality is far ahead of anything signal processing can do. The model has learned
what a voice looks like spectrally and removes it wherever it sits in the stereo
field, leaving centred kick and bass intact. The cost is the look-ahead
separation window — a few seconds on first play, instant thereafter from the
cache.

**Where it runs.** The model runs on ONNX Runtime, which every package
carries except one: the Linux `.deb`, `.rpm` and AppImage, the Windows `.msi`
and `.exe`, and the Apple Silicon Mac app. The Intel Mac app has none to
carry — Microsoft stopped publishing ONNX Runtime for Intel Macs after 1.23,
older than djmanzo can use — so there the built-in separator (vocals as what
is centred in the harmonic part of the vocal band) and §1.3 below do the
work, and the stems panel says which it is using. On Windows, ONNX Runtime
also needs Microsoft's Visual C++ 2015–2022 runtime, which most machines
already have; where it is missing, the stems panel says so and the built-in
separator takes over.

**Vocal *reduction* rather than removal.** Ducking the vocal stem to around
-12 dB instead of killing it leaves a guide vocal underneath. For a nervous
singer, a hesitant crowd, or a song nobody quite remembers, this is the setting
that actually saves the moment — and it is trivial once the stem exists.

### 1.3 Band-limited centre cancellation

The classic trick, done properly. A lead vocal is normally panned dead centre,
so subtracting right from left cancels it.

The naive version is bad, and it is worth being precise about why: **everything**
centred disappears with it — kick, snare, bass, often the lead instrument. The
result is thin and hollow, "like a telephone", and on a dance floor the missing
kick is fatal.

So djmanzo does not do the naive version. It splits the signal by frequency and
cancels the centre **only in the vocal band** (roughly 200 Hz to 8 kHz),
passing centred low end and top through untouched:

```
  input ─┬─ below 200 Hz ────────────────────── keep centre  ─┐
         ├─ 200 Hz – 8 kHz ─→ mid/side ─→ drop mid ──────────┼─→ sum
         └─ above 8 kHz ─────────────────────── keep centre  ─┘
```

The kick and bass survive, the cymbals keep their air, and the voice mostly
goes. It is still worse than stem separation — anything else centred in that
band goes with the voice — but it costs almost nothing, works instantly on any
track with no analysis, and needs no model, no GPU and no cache. It is the right
answer on a slow laptop, and the right answer for a track you just dragged in
thirty seconds ago.

Adjustable: cancellation depth, and both crossover points, because the ideal
band depends on the singer and the arrangement.

### 1.4 Nothing

Full mix, lyrics on screen, sing along over the record. Frequently what a party
actually wants.

### Choosing

| | Quality | Cost | Works on |
|---|---|---|---|
| Official instrumental | Perfect | None | Only what you own |
| Neural stems | Excellent | Separation window, then cached | Anything |
| Band-limited centre cancel | Fair | Negligible | Anything, instantly |
| Full mix | — | None | Anything |

The setting is per track and remembered, so a track you tuned once stays tuned.

---

## 2. Putting the words up

### Where lyrics come from

Four sources, tried in order, because each covers what the previous one misses:

1. **Embedded in the file.** `SYLT`/`USLT` frames in ID3, `LYRICS` in Vorbis
   comments. Read with `lofty`. Free, offline, instant. Synced if `SYLT`.
2. **A sidecar `.lrc` file** next to the track — the format the karaoke world
   already uses, and what most people's existing collections have.
3. **[LRCLIB](https://lrclib.net/)** — a free, open (MIT) database of around
   three million synchronised lyrics, built for FOSS players. **No API key, no
   account, no configuration.** This will cover most commercial music.
4. **Transcription**, when the first three come up empty.

**Built (K1):** `dj_library::own_words` reads the first two. Embedded words are
a `SYLT` frame stamped in milliseconds (one stamped in MPEG frames needs the
frame rate and is left out), or the lyrics tag — `USLT`, Vorbis `LYRICS`, MP4
`©lyr` — which counts as timed when its text is LRC, as taggers often store it.
A sidecar is `song.lrc` (or `.LRC`) beside `song.mp3`, and must have a timed
line. One refinement to the order: **the first source with timed words wins**,
and plain words only when none is timed — an untimed tag must not hide a timed
`.lrc`. The lyrics sweep reads them before asking LRCLIB (LRCLIB's timed words
beat a record's own untimed ones, never the other way), keeps them when the
network refuses, and the singers' screen reads them again as each record is
loaded, so a `.lrc` dropped in after the sweep is on screen next time.

### Transcription, and why it works better here than usual

Transcribing lyrics from a full mix is notoriously unreliable — the instruments
drown the words. But djmanzo has something a lyrics tool normally does not:
**the isolated vocal stem.**

Running speech recognition on a clean, separated vocal is a dramatically easier
problem than running it on a mix. The same stem engine that makes karaoke
possible also makes the transcription good.

**whisper.cpp**, compiled into djmanzo (`dj_app::whispercpp`), gives each
word its own time. The owner first chose WhisperX for that; it needed
Python and PyTorch — 2.6 GB — and could not ship with djmanzo, so on
27 September 2026 the owner asked for something under 100 MB in the
installer, with a better model than `base` taking no more than about a
minute and a half a song, downloaded on demand when too big to bundle
(§122). whisper.cpp runs the same Whisper models from C++ in two megabytes
of program; the model is one file the DJ picks from a list that says, for
each, its size and download time, how well it hears, and how long a song
takes on this machine. It listens to the **separated vocals** when
separation has finished — the mix otherwise, and it says so. See
[RESEARCH.md](RESEARCH.md#words-in-time-under-100-mb-bundled-122-asked-27-september-2026)
for the measurements and licences.

- **Words already known** — tags, `.lrc`, LRCLIB — are what the singers
  read. They steer Whisper as its prompt, and each known word takes the time
  of the heard word it matches; one it missed is spread between its
  neighbours, and a timed line keeps its own time if nothing in it was heard.
- **No words anywhere**: what Whisper heard is kept, word by word.
- **`small` is recommended**: measured at 73 s for a 4½-minute record on a
  four-core machine, inside the owner's minute and a half, with most words
  right over the mix. Every run is timed against that limit and says
  whether it met it; what it took is kept, so the list's next estimate is
  this machine's own. The Windows installer offers `small` as it
  installs; on every system the welcome guide offers it at first run.
- **Word times** come from Whisper's own attention and land within about a
  quarter of a second of WhisperX's aligner — close enough to wipe a line
  by, not to the syllable. **For English**, the aligner the owner asked for
  (`dj_stems::align`, wav2vec 2.0, a 95 MB download from the same list)
  then places every word by its letters: on a 4½-minute record its starts
  agreed with WhisperX's to a median of 0.02 s (80 % within 0.08 s), and it
  added 10 s to the run.

### Forced alignment — the case nobody handles

The most common real situation is **lyrics exist but are not synced**: a text
file, a tag, a web page. Plain words, no timings.

Given unsynced text *and* an isolated vocal, timings can be recovered by forced
alignment — matching the known words against the vocal audio. This turns "I have
the lyrics somewhere" into a fully synced karaoke track, which is the single
highest-value operation in this whole feature and one most karaoke software
simply does not do.

Results are kept in the library as the record's timed words, in enhanced LRC
(a time before every word), so the work is done once. This is what djmanzo
does when the words are known (`dj_app::whispercpp::place_known`).

### Ahead of time, or live

- **Ahead of time**, as part of library analysis: everything ready before the
  night starts. The right default.
- **Live**, for a track dropped in mid-set: lyrics fetched and aligned in the
  background while the track plays, appearing when ready. A late arrival is
  better than nothing, and the UI says it is working rather than looking broken.

---

## 3. The karaoke screen

A **separate window**, for a second monitor, projector or TV. The DJ's screen
and the singers' screen are different screens showing different things — the DJ
needs decks, the room needs words.

```
┌──────────────────────────────────────────────────────────────┐
│                                                              │
│              [ artwork / visualiser background ]             │
│                                                              │
│                                                              │
│         Y  ahora  que  te  vas                               │  ← current line,
│         ▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░                                │    wipe-highlighted
│                                                              │    in time
│              dime  quién  me  va  a  querer                  │  ← next line
│                                                              │
│                                            ● ● ● ●           │  ← count-in
└──────────────────────────────────────────────────────────────┘
```

Standard karaoke conventions, because they are conventions for good reasons:
current line large with a per-word or per-syllable wipe, next line previewed so
singers can breathe, a count-in before entries, and a clear indication when an
instrumental break is running and how long it lasts.

### Backgrounds

1. Album art embedded in the file.
2. **[Cover Art Archive](https://coverartarchive.org/)** via MusicBrainz — free,
   open, no API key.
3. A generated abstract background derived from the track's own spectrum, so
   there is never a blank screen.

**Built (K1):** `dj_app::backdrop` and `singer_backdrop`, in that order. The
record's own cover is served by `art://`, as the library's cards are. A record
tagged with the MusicBrainz release it came from has that release's front
cover asked of the Cover Art Archive (`front-500`), once: the image, or the
archive's word that there is none, is kept under the application's data
folder, so a night without a network still has its covers and the archive is
not asked again. Beneath either, always, the record's own colours — §110's
colour for each of its eight bands, weighted by how much of the record sits
in it, drawn as four slow glows — which are also the whole background when
there is no cover, and what shows while one loads or when it fails. A veil
over both keeps the words at about 13:1 contrast in the middle of the screen
(measured in the running application with a pale cover; 4.9:1 without it).
Plain CSS, no GPU, so nothing here can take the words down. A record's
spectrum lands seconds after it loads; until then the screen is told the
colours are pending and asks again, for a minute at most.

### Visuals

Beat-reactive animation driven by what the engine already computes: the beat
grid, band energy from the EQ crossovers, and — the fun one — **the singer's own
microphone**, so the visuals respond to the person holding it rather than only
to the record.

### One rule that overrides all of it

**The words must render even if every visual effect fails.**

Lyrics are a text layer that never depends on the GPU. The visualiser sits
behind them and degrades in tiers: full WebGL where the platform delivers it,
a cheap canvas tier below that, and a plain gradient if both fail. On a Linux
machine where [WebKitGTK silently falls back to software rasterisation](adr/0004-waveform-rendering-strategy.md),
the animation may drop to something simple — and the karaoke still works
perfectly, because nobody in the room is there for the background.

This is also where WebGL is genuinely *appropriate*, unlike the waveform: a
dropped frame on a background animation is cosmetic. A dropped frame on a
scrolling waveform is a mixing error.

---

## 4. Voice control

Karaoke is exactly the moment the DJ's hands are full and there is a queue of
people at the booth. So it is fully controllable by voice through the
[assistant](ASSISTANT.md), like everything else:

> "Karaoke on deck two." · "Bring the vocal back a bit." · "Restart the verse." ·
> "Next singer." · "Pon el karaoke."

All of it is ordinary action text on the bus, per
[ADR-0005](adr/0005-assistant-speaks-only-actions.md) — nothing here gets a
special path.

A **singer queue** rounds it out: names and songs, add by voice, shown on both
screens, so the room can see who is next.

### The guest book (§123)

Every singer the rotation marks as having sung is written into a **karaoke
journal** (`dj_app::guests`, the Guest book on the Singers surface) with the
song, the time and the event being played. The host opens a guest and asks
three questions, in the words `guests::ASKS` holds and the surface shows
unchanged, each answered separately and stamped with when and to which
wording:

| Question | Without it |
|---|---|
| Keep my details after tonight | The guest is tonight's only, and leaves the journal — with anything recorded of them — at *New night*. |
| Keep my email, phone or WhatsApp and social profiles, and send me what is made of tonight | Those fields cannot be typed, and Rust clears them on save whatever is sent. |
| Record about fifteen seconds of my singing and use it with an AI music service | No recording is attached; taking it back deletes the ones there are. |

Under sixteen — the GDPR's age unless a country lowers it — contact and voice
are refused, because that needs a parent and a booth cannot check one. The
rest of the record (age, home town, nationality, favourite band, genre and
song, native and other languages, a note) is what the song for them is made
from. The journal is one file in djmanzo's settings folder and nothing in it
leaves the machine; a guest can be given their own record as a file and
forgotten with everything recorded of them, and the host can export the whole
journal as a table.

**Their voice.** Once a guest has agreed to it and that answer is saved, their
record offers *Record 15 seconds of their voice* (the up-next card's *Guest
book* button opens the singer's record). The engine's recorder takes the
microphone on its own — after its gain, before the music or the master's
effects — and stops itself at fifteen seconds; the take is written as a WAV
beside the journal and put on the song they are singing, once it is marked as
sung. Consent is checked once, by the record, **before anything is written**:
a take whose guest took consent back while it ran is never on disk, and a take
nobody asked for (the recorder can be started by a script or a controller as
`sampler voice 15`) is thrown away. WAV rather than MP3, which the owner
named: every MP3 encoder worth having is LAME, whose LGPL licence ADR-0002
does not admit, and a WAV is what an AI music service takes anyway.

**The words of their song.** For a guest who agreed to a song being made for
them and has sung, their record has *A song for …*. The DJ adds the language
the song was sung in — or leaves it for the model to know from the song — and
a few ideas of their own, and the DJ's AI provider writes the words
(`dj_assistant::song`): about the guest, the song they sang, the event, place
and date, in the song's language and again in the guest's own when it
differs, with section tags a music service reads. A style line comes with
them, blending the guest's favourite genre, song and band, the song they
sang, and the genres of the event being played.

What the model is told is only what the song needs: the name the host wrote,
the song, where and when, the favourites and languages, and the DJ's ideas —
never an email, phone number, age, home or nationality. What comes back is
checked before it is kept: the style has the favourite band and song and the
song sung taken out of it, and so is any part phrased as a comparison ("in the
style of …", "like …"), whoever it names — the guest's song is to be their
own, not a copy of somebody else's — and the style and each version are cut, at a comma or a
line, to a length a music service's boxes take. The words are kept on the
guest's song and copied with one press each.

**Making it, and sending it.** *Open Suno* opens suno.com in a window of its
own, where the DJ signs in, pastes the words and the style, uploads the voice
— *Show their voice file* shows the newest recording in the file manager, or
opens its folder where no file manager answers — and makes the song. djmanzo
does not work Suno's page: Suno has no public API and its terms forbid robots
and scraping, and the window has no way into djmanzo either, because no
capability names it and Tauri lets remote content reach no command without
one. With the song made, the DJ pastes its link and presses *WhatsApp* or
*Mail*: the message opens written and addressed to the guest — only one who
agreed to be contacted, at the number or address on their record — and the
DJ reads it and sends it. WhatsApp needs the number with its country code.

---

## 5. Where it fits

The milestones live in [ROADMAP.md](ROADMAP.md#karaoke); this is where each
piece of this document lands among them.

| Piece | Needs | Milestone | Where it stands |
|---|---|---|---|
| Band-limited centre cancellation | M1 | **K1** | shipped, behind the voice knob |
| Lyrics from LRCLIB | M3 | **K1** | shipped |
| Lyrics from tags and a sidecar `.lrc` | M3 | **K1** | shipped |
| Singers' screen: wipe, next line, count-in, next singer | M3 | **K1** | shipped |
| Singers' screen background: art, Cover Art Archive, generated | M3 | **K1** | shipped |
| Stem-based removal and a guide vocal | M6 | **K2** | shipped |
| Transcription and word times (whisper.cpp, in djmanzo) | M6 | **K2** | shipped; `small` measured at 73 s for a 4½-minute record, and the English aligner beside it |
| Singer queue | — | **K2** | shipped as the rotation, with the guest book |
| Beat- and microphone-reactive visuals | M2, §122 | **K2** | shipped behind the words: djmanzo's own layers on the beat, the stems and the singers' microphones, picked in the Singers panel ([VISUALS.md](VISUALS.md)), MilkDrop among them, the DJ's own MilkDrop presets, the artist's photographs from Wikimedia Commons, credited, a look picked per record by the DJ's AI when turned on, and signed packs of looks and presets, checked before installing; packs made of the DJ's own presets, signed with their own key, and browsed from a community index |
| Voice control | A2 | **K2** | waits on A2 |
| N microphones, a chain each, a singers' monitor | M1 | **K3** | the chains, the engine, the input, the screen, the monitor, reconnecting and the chain behind a row built; a singer's own settings next — §6 below |
| Signing up from a phone, host permissions, photos, ticker | the audience page | **K4** | outlined |
| Break music that leads into the next song | the planner | **K5** | outlined |
| Scored singing, from the separated vocal | K3, M6 | **K6** | designed (§9) |
| A quiz made from the collection | K4, M6 | **K7** | outlined |
| A singer's range, and the key to sing in | K3, K6 | **K8** | outlined |
| Phones as microphones, per-singer monitors, duet parts, pitch correction, recorded performances | K3, K4, K6 | **K9** | named |

---

## 6. The singers' microphones (K3)

Everything above takes the voice *out*. Nothing yet carries the singer's voice
*in*: djmanzo has one microphone strip, built for a DJ talking over the music,
and a karaoke night can have a queue of people with a microphone each.

### What a singer's strip is

One per input the interface has. The strip carries the input channel, a gain,
an on/off switch, a send to the headphones, a send to the singers' monitor,
and **talkover** — kept per strip, so one strip can still be the MC's
microphone while the singers' strips leave the music alone. Talkover under a
singer would pull the backing track down every time they sang, which is the
opposite of karaoke.

Behind each strip, a full vocal chain of its own, in this order, every stage
bypassable:

```
  input ─→ high-pass ─→ gate ─→ EQ ─→ compressor ─→ de-esser ─→ echo ─→ reverb ─→ sends
```

Independent on every strip, on purpose: two singers sharing a stage do not want
the same reverb, and a guest who shouts wants a harder compressor than one who
whispers.

### What it costs, and how that is kept honest

N full chains cost N times one. That is kept in check by three things, not by
sharing effects:

- **A strip that is closed, or has been gated for longer than its reverb tail,
  is not processed at all.** The tail is allowed to finish first — cutting a
  reverb short is audible.
- **The reverb and echo are chosen to be cheap enough to run N copies of**: a
  small feedback-delay network rather than a convolution.
- **Each strip publishes what it costs**, so the interface can say *this rig
  is too much for this machine* before the room hears it.

### Where it sits

A crate of its own, `dj-vocal`, depending on `dj-dsp` and `dj-core` and never
on `dj-engine`. The engine holds it the way it holds the CLAP processor and
calls it once per block with the input frames; it adds each strip into the
main, cue and monitor buses and returns the gain the music should take, which
is what `MicFrame::music_gain` already does for one strip. Nothing in it
allocates after the interface opens: the strips are sized to the input count
then.

- **One device in and out.** The microphones and the music run on the same
  interface. Two devices are two clocks, and two clocks drift.
- **One multichannel input stream, one ring.** The host opens every input the
  interface has; `dj-vocal` splits the channels. The ring still leaves through
  the retirement queue as `Retired::MicInput` does today.
- **Vocals join the main bus before the master chain**, so the limiter that
  protects the PA from a record protects it from a singer too.
- **A singers' monitor** on its own output pair — `BusLayout::monitor`, beside
  the booth pair, which stays the DJ's. The music at a level of its own plus
  every strip's monitor send. Built as one bus that can later become one per
  singer.
- **Parameters** as `ParamId::Vocal(StripId, StripParam)`, the same shape as a
  deck's. The existing `Mic*` parameters remain and mean strip 0, so a
  controller mapping or keyboard layout written for the microphone keeps
  working.

### The delay a singer hears

A voice through a computer arrives late by the input buffer, the ring and the
output buffer together. `mic.rs` already says so; a DJ making an announcement
barely notices it, and a singer hearing themselves in a monitor notices it
quickly. So the round trip is measured and shown on the strip, and there is a
small-buffer setting for karaoke — with the trade stated: smaller buffers ask
more of the machine.

### What this makes possible

Everything that has to listen to a singer: K6's scoring reads pitch per strip,
K8 measures a singer's range on one, §123's fifteen-second voice take records
the strip the singer is on rather than "the microphone", and K9's recorded
performances are a strip plus the record.

**Built, first step — the crate (`dj-vocal`).** Everything above that lives
inside the crate: `Vocals` reads one ring of interleaved frames, channel *n*
to strip *n*, all of a frame or none (a ring that runs dry is silence and a
count, never half a frame); each `Strip` runs the chain in the order drawn,
every stage bypassable — a high-pass, a gate with hysteresis and hold, a
three-band EQ, a soft-knee compressor on a held peak, a de-esser that turns
down only the band above its frequency, an echo, and a four-line feedback
delay network reverb whose time is the time asked for — then pans, and sends
to the room, the DJ's headphones and the singers' monitor at a level each.
Talkover is the strip's own, and the music takes the lowest gain any
talking-over strip asks for. A strip that is closed, or whose voice has been
gone for longer than its echo and reverb take to die away, is not processed;
the room's hum under the gate's threshold does not keep it working, and the
first word over it wakes it. Everything is sized when the strips are made —
the echo's and reverb's buffers for their longest settings — so no setting
allocates, and a counting allocator holds eight strips with every stage on,
settings changed mid-way and the ring running dry to none. What a chain
costs is measured (`measure_chain`): one with every stage on took about
0.6 % of a core of this machine (a 2.1 GHz Xeon), so sixteen singers would
be about a tenth. Held by fifteen tests; four mutants — the tail ignored,
the room's hum counted as a voice, half a frame read, an allocation per
frame — each fail one. **Not yet:** the engine does not hold it, the host
does not open a multichannel input, and there is nothing on screen; that is
the second step, with the `ParamId::Vocal` parameters, strip 0 as today's
`Mic*`, the singers' monitor bus and the measured round trip. Nothing here
can say how it sounds — this container has no microphone and no speakers.

### The second step, designed

**A strip's settings, and a singer's.** Two layers, the second over the first.
*The strip's own* — its preset and every setting on it — is the host's rig and
lives in `vocal.json` beside the other settings, restored when djmanzo starts,
so microphone 3 is still the MC's microphone tomorrow. *A singer's* — the
compressor and reverb a guest sounded best with — is kept only where the
singer is: on the guest's record in the guest book when they have agreed to
be kept (§123's *keep* consent; a voice's settings with a name on them are
about a person), and otherwise on their place in tonight's rotation, gone at
*New night* with everything else about them. When the host puts a singer on a
microphone — a strip is assigned from the rotation, the way a deck is — the
singer's settings are laid over the strip's, and *Keep for this singer* is the
one press that writes them back. Presets name what the strip is for, not what
is on it: *Singer*, *Soft singer*, *Loud singer*, *MC* (talkover on, no
reverb), *Instrument* (no gate, no de-esser).

**Eight strips at once.** The host is running a room, not mixing a record, so
a strip's face is what a host acts on in a hurry: who is on it (the singer's
name, or *Mic 3*), open or closed, a fader, a level meter with the gate's
state on it, and how hard the compressor is working — one row each, eight rows
in the space the Singers surface already has. The chain is behind a press on
the strip, and the preset is one tap. Above the rows, one line says what the
rig costs — *4 of 8 microphones working, about 3 % of the machine* — from each
strip's measured cost and whether it is idle, and turns into a warning before
the sum reaches what the machine has to give. The round trip — input buffer,
ring and output buffer, measured when the interface opens — is on the same
line, because a singer who hears themselves late will say so, and the host
needs the number to answer.

**An input that vanishes mid-song.** The interface is unplugged, or its
driver stops delivering. The music never stops for it: the ring runs dry,
every strip is fed silence, the reverb and echo tails finish as they would
after a phrase — no click from a chain cut off mid-tail — and any talkover
lets the music back up. The host sees *Microphones lost — reconnect the
interface* the moment the starvation count climbs for half a second, and
djmanzo tries to reopen the input every two seconds, off the audio thread,
until it answers; the strips' settings survive, because they are the strips',
not the stream's. One cable pulled from one socket cannot be told from a
silent singer in the samples, so it is not guessed at: an open strip with no
signal for ten seconds says *nothing on Mic 3* on its row, and no more.

**Built so far of the second step — the engine.** `dj-engine` holds a
`Vocals` (`Command::Vocals`, the replaced rack leaving through
`Retired::Vocals`) and takes a strip's settings by value
(`Command::VocalStrip`). The voices join exactly where the DJ's microphone
does — before the master rack and the limiter — and the music takes the lowest
gain anything talking over it asks for, the DJ's microphone or an MC's strip;
each strip's headphone send reaches the cue bus; with the decks going out
separately the singers' ring is still drained. `vocal_inputs`,
`vocal_working` and `vocal_starved_frames` are published. Held by an engine
test (a singer is heard over music that does not move; the MC is heard over
music that drops; a rack taken away comes back through the retirement queue)
and by `rt_safety` (eight strips singing and falling silent, settings changed,
the rack replaced and removed: no allocation, both racks returned) — three
mutants, each failing one: the voices left out of the master, the MC's gain
ignored, the old rack dropped on the audio thread.

**And the host.** `dj-audio` opens an input with every channel it has, as it
has them (`open_input_all`; all of a frame or none into the ring), and the
host builds a rack as wide as what opened — an interface with more inputs than
`MOST_STRIPS` has its first ones used and the rest read and let go, so frames
stay whole — lays the settings kept in `vocal.json` on its strips, off the
audio thread, and hands it to the engine. A device change closes the singers'
input with every other input; closing it takes the rack out of the engine.
Commands: `vocals_open`, `vocals_close`, `vocals_state` (strips, how many are
working, the starvation count, and what one full chain costs here, measured
once) and `vocal_strip_set` (kept, then sent). Held through the real host
thread and the null backend's two-channel input — the engine ends up holding
two strips, a strip's settings reach the file, closing empties the engine,
a device change leaves no capture open — with two mutants failing them: the
device change leaving the singers' input running, and closing without taking
the rack away.

**And on screen.** Each strip publishes its level, whether its gate is open,
how hard its compressor works and whether it is working
(`ParamId::Vocal(strip, …)`, a block of four per strip after the globals;
`MAX_VOCAL_STRIPS` in `dj-core` is held equal to `dj_vocal::MOST_STRIPS` at
compile time), and the snapshot carries them as `master.vocals`. Presets are
`dj_vocal::Preset` — *Singer*, *Soft singer*, *Loud singer*, *MC*,
*Instrument* — each the chain for its job, keeping what the host set by hand
on the row (open, fader, pan, sends); `vocal_strip_preset` applies one. The
Singers surface's *Microphones* section (`ui/src/Microphones.svelte`) is the
design above: an interface picked and opened, then a row a strip — name,
switch, fader, meter lit by the gate, compression, preset — and one line
with how many are working, their share of the audio thread from the measured
chain cost, and the round trip; *Microphones lost — reconnect the interface*
once the starvation count has climbed for half a second; *nothing on Mic n*
after ten seconds of an open strip's silence, counted from when it opened.
Held by an engine test (the singer's strip working with a level, a strip the
rack does not have reading nothing), a preset test, a golden of Rust's answer
(`ui/e2e/vocals.json`) and two browser tests — mutation-tested: the switch
sending the strip unchanged, and the lost-input check never firing, each
fail one. **Driven in the running application** with the null backend's
two-channel input: the section offered it, *Open the inputs* gave two rows,
*Open* on Mic 1 wrote `"open": true` to `vocal.json`, a restart brought it
back open, and *nothing on Mic 1* appeared after ten silent seconds — which
driving it found first appearing at once, the clock having counted the time
the strip was closed.

**And the singers' monitor.** On an output with eight channels or more, the
pair after the headphones (`BusLayout::monitor`, channels 7–8) is the
singers' wedge: the music at a level of its own and each singer at their
strip's monitor send, through a third limiter — the master's twin, so the
wedge and the PA a singer hears behind them arrive together. The music there
is the decks' sum *before* the talkover and the master gain: an MC speaking
does not take the song out from under a singer, and the DJ turning the room
up or down does not change what the singer sings against. Before the master
rack too, so an echo thrown over the room is not in a singer's ears. Its
level is `monitor music <dB>` on the action bus — so on a controller and in
a script as well — from −60, which is *off*, not quiet, to +12; the snapshot
carries it as `master.vocals.monitor_music_db`, `null` on an output with no
pair for it, and the *Microphones* section draws a slider for it or, on a
narrower output, says what would give the singers one. Sending the decks or
the stems out on pairs of their own takes the pair, and a controller whose
mapping names its own sockets gives none. Not kept between runs,
as the booth's level is not. Held by an engine test (the wedge's music at
its own level and deaf to the master gain; a singer heard in it; the MC
pulling the room down and not the wedge; *off* silent), a bus test,
`rt_safety` on eight outputs and a browser test — mutation-tested: the wedge
taking the ducked music, taking the master gain, and *off* left sixty
decibels down each fail the engine test (the last only once the test listened
with nobody singing — it had passed at first, the level hiding inside the
tolerance), and the slider sending the wrong level fails the browser's. **Driven in the
running application** on the null backend's two outputs: the section said
there was no monitor on this output and what would give the singers one —
first worded as *on its last pair*, which is wrong for an interface with more
than eight, and now *on outputs 7 and 8*.
**And an input that goes comes back by itself.** The host watches the
engine's count of frames the singers' ring could not supply; climbing for half
a second is the interface gone, and from then on the host lets the silent
stream go and opens the same device again every two seconds, off the audio
thread, until it answers. Only the stream and its ring are replaced
(`Command::VocalInput`, the old ring leaving through the retirement queue):
the rack, its strips, their settings and tails stay in the engine throughout,
so a singer whose cable goes back in finds their microphone as they left it. A
device that answers with a different number of inputs is not the one that
went, and is let go again. The section's alert says djmanzo is trying. Held by
an engine test (a new ring into the same rack, the strip heard again without
its settings being sent), the watch's rule on a clock the test holds (a hiccup
is not a loss; due at half a second; then every two seconds, never sooner),
`rt_safety` with the ring replaced mid-set, and the real host thread with the
null backend's interface pulled out and plugged back — the silent stream let
go, the rack kept, a new stream opened and the ring running again; the rule
never firing, and the engine ignoring the new ring, each fail them. Writing
the engine test found a fault of its own: a strip that had stopped still
showed what its meters last read, because an idle chain is not run and its
meters stop with it — an MC's row, with no reverb tail to decay through, kept
its level lit and its compression shown for as long as the MC was quiet. An
idle strip now reads as stopped: no level, no compression, no gate open
(a `dj-vocal` test, failing before the fix).
**And the chain is behind a press on the row.** *Mic 1* on a row is a button:
pressed, every stage of that strip opens under it — the low cut, the pan and
the three sends, talkover, then gate, EQ, compressor, de-esser, echo and room
in the order the voice goes through them, each switched on or off and its
numbers on sliders. Nothing on that screen decides what a voice may be put
through. Every number a strip takes has a range in one table in Rust
(`dj_vocal::LIMITS`, from `dj_vocal::range`), and the screen draws each slider
over exactly that range — the row's fader too — from the answer it is given
(`VocalsDto::limits`); a stage switched on starts at Rust's starting point for
it (`StripSettings::every_stage`, the stages' own defaults), not one the screen
guesses. And the host does not trust the screen to have kept to the ranges:
what it is sent is held to them before it is kept or played
(`StripSettings::held`, in `set_vocal_strip`), and so is a `vocal.json` read
from disk — a number past its range comes back at the edge of it, one that is
not a number at all at its default. Before this, nothing past the fader and
the high-pass was bounded: a compressor's makeup gain of +200 dB would have
reached the engine as sent. Held by a `dj-vocal` test that reads every number
off the settings themselves and requires a range for each and no other, one
that sends every number far past its range both ways and as not-a-number, a
host test (the screen's wild values and a hand-edited file both held, the
answer carrying the table), and a browser test (the chain behind the press,
every slider over Rust's range, an echo switched on at Rust's starting point,
one number moved and the rest of the strip sent exactly as it was, the room
switched off). Mutation-tested: `held` skipping one number, a range's name
drifting from its field, the host keeping or reading settings without holding
them, a stage switched on at the screen's own starting point, sliders over a
range of the screen's, and a move rebuilding the strip from defaults each
fail one of them. Driving it in the running application found what no test
had: the Singers surface docked at the side is narrower than a row's six
things, so every row had always run past the panel's edge — the preset cut
off — and the chain, as wide as its row, came out clipped with a scrollbar
under it. A row now stays inside its panel, its level and compression going
under the fader where the panel is narrow, and the chain fits the row — held
by a browser test that narrows the list, and the preset select let give way
because WebKitGTK draws one wider than the tests' Chromium does.
**And a singer's own chain, laid over the strip's.** A strip's settings are
now two layers, as designed. What the voice goes through — the preset, the
low cut, talkover and every stage, `dj_vocal::Chain` — is split from the row,
which stays the rig's whoever is singing: open or closed, the fader, pan and
the three sends (`StripSettings::chain` and `with_chain`; a preset is now
that same split, so the two cannot disagree about which is which). The up-next
singer has a button a microphone beside *Load on 1*, and behind a row the
host can put anybody in tonight's rotation on it, or nobody
(`vocal_strip_singer`); the row then carries the singer's name in place of
*Mic 1*. Put on a microphone, a singer sings through the chain kept for them
— from their guest-book record when they agreed to be kept, else from their
place in tonight's rotation, else the strip's own to start from — laid over
that strip's row. While they are on it, a change to the row is the rig's and
goes to `vocal.json`; a change to the chain is theirs, played and not kept,
so a compressor turned up for one voice does not become the rig's. *Keep for
this singer* (`vocal_strip_keep`) keeps it: on `Guest::microphone` with
§123's *keep* consent, on `Singer::microphone` without. A guest's record only
gains one by that press, never from the form, and loses it when they are no
longer to be kept; a rotation place loses it at *New night*, which also takes
everybody off the microphones. One singer is on one microphone: put on
another, they leave the first, which goes back to the rig. Who is on which is
never written down — a restart gives every strip back to the rig. Held by a
`dj-vocal` test that reads every field of a strip off its settings and
requires each to be the chain's or one of the row's six, never both or
neither; a host test that reads what the engine is sent off the command
queue itself (Ana on Mic 1 through the rig's chain; her compressor up and the
fader down, the fader alone reaching `vocal.json`; kept on her rotation place;
off, the rig as it was; on Mic 2, her chain over Mic 2's row; Ben's MC chain
kept in the guest book because he agreed to be kept; Ana put where Ben is
taking him off and leaving Mic 2; *New night* taking everybody off and Ana's
chain with her place, Ben's staying until he is no longer to be kept); a
journal test (the form can neither set nor clear it); a golden of Rust's
answer with a singer on a strip; and two browser tests.
**And the microphone is Mic 1.** With the singers' strips in and nothing
attached to the DJ's own microphone, the `Mic*` parameters are strip 0's:
the engine publishes its switch, fader, talkover, headphone send, level and
starvation count under the microphone's names and says so
(`GlobalParam::MicStrip`, the snapshot's `master.mic.strip`). And a `mic`
action — from a key, a controller, a script or the Booth's panel, all one
road (`perform_by`) — goes to strip 0 through the host, which keeps that
strip's settings: `mic on`, `mic gain`, `mic cue_on`/`cue_off` and
`mic talkover_on`/`off` become strip 0's settings, held, kept in
`vocal.json` and sent, so the Microphones row says what the controller did.
They are written in the session log but not sent to the engine, so the DJ's
own channel is not left armed for whatever is plugged into it later. The
headphone send is a level on a strip and a switch on the microphone: switched
on, a strip already sending keeps its level, one that was not sends all of
it. Talkover's depth, threshold and timing are the DJ's own microphone's and
go to it as before; a strip's talkover has no such settings, so the Booth's
panel does not offer them while it is Mic 1, nor the ducking meter that reads
the DJ's channel — it names the strip instead, and still offers to attach a
microphone of the DJ's own, which makes the `mic` actions that microphone's
again. With a singer on Mic 1, talkover is part of their chain (a `Chain`
field), so `mic talkover_on` changes theirs. Held by an engine test (the
parameters following strip 0 open and closed, a microphone of the DJ's own
taking them back, nothing present with the rack gone), a host test through
the real host thread and the null interface (`mic on`, a gain, the
headphones and talkover reaching strip 0, `vocal.json` and the engine's
`Mic*`; the ducker's depth reaching the DJ's channel; the action in the
session log; the headphone level kept; the DJ's channel not armed; attached,
the actions the DJ's microphone's again and Mic 1 left alone) and two
browser tests. Driving it in the running application found two things no
test had. The Booth's switch put Mic 1 on air and `vocal.json` said so, but
the Singers surface's row went on saying *Closed*: that section asked Rust
for the strips only when their number changed or when it changed one itself.
The host now counts every change it makes to a strip
(`GlobalParam::VocalSettingsRev`, the snapshot's
`master.vocals.settings_rev`) and the section asks again when the count
moves, so a row says what a key, a controller, the Booth or the rotation did
(a host assertion and a browser test, the second failing with the count
ignored). And the Booth's panel said *the input is not keeping up — 256
frames missed* for the rest of the night: an input whose first block arrives
after the output's misses a few hundred frames once, and the warning read
the count rather than whether it was climbing — true of the DJ's own
microphone too, and with Mic 1 it would have been every karaoke night. It
now warns while the count climbs and for two seconds after, and not for the
count it first sees (a browser test, failing with the old condition).
**And a guest's voice take is the strip they sing on.** §123's fifteen
seconds of a guest's voice came from the DJ's own microphone — silent on a
night run on the singers' microphones. Each strip now gives its voice before
its space — after the fader, the low cut, gate, EQ, compressor and
de-esser, before the echo, the room, the pan and the sends
(`StripFrame::voice`, `Vocals::voice`) — and the recorder can take it
(`RecordSource::Singer`, `sampler voice 15 singer 2`, and `sampler record 3
singer 1` for a pad). A guest on a microphone is recorded from it; a guest on
none, with the microphone being Mic 1, from Mic 1; otherwise from the DJ's
own microphone as before (`voice_take_line`). The singer, not the room: an AI
music service is given a voice, not a reverb tail. Held by a `dj-vocal` test
(the voice there while they sing and gone the moment they stop, while the
room still rings in the mix), an engine test (a take from the second of two
singers, with the first singing, the DJ talking and a record playing, holds
the second alone, at its level, and says where it came from), an action test
(singers counted from 1 in words, 0 and 17 refused) and the host test above
(the strip a guest is on, Mic 1, and the DJ's own microphone again once
attached).
**And a buffer for singing.** The *Microphones* line, under how late a singer
hears themselves, offers the interface's buffer — the choice Settings has,
each size with what it is in milliseconds each way at the rate the input
opened at. Choosing one says the trade first — sooner for a singer, more for
the machine, back up if it crackles, and that opening the interface again
stops the music and empties the decks, so it is a setting for before the
night — and nothing is reopened until *Reopen at 128 frames* is pressed.
Then the interface opens again at that size, through the same road as
Settings' *Reconnect*, kept the same way, and the singers' inputs open again
on the same interface with their strips and whoever is on them, which are
Rust's. Not offered in a popped-out Singers window, which cannot reopen the
interface. Held by a browser test (the sizes in milliseconds, the trade said
on choosing, nothing reopened by choosing, the offer gone for the size it
has, and on the press the interface opened at that size and the inputs on
the same interface) — mutation-tested: the inputs not reopened, the current
size still offered, and the size not passed on each fail it. Driven: 256
frames read *about 11 ms late*; 128 chosen, the trade shown, nothing moved;
pressed, *about 5 ms late*, both rows back — and the decks empty, as said.
**K3 is built.** What it cannot say is how any of it sounds: this container
has no microphone and no speakers.

**The order of the second step.** The engine holds a `Vocals`, built off the
audio thread when the interface opens and installed by a command, the old one
leaving through the retirement queue; the host opens the input with every
channel it has, into one ring; `ParamId::Vocal(strip, param)` publishes each
strip, strip 0 answering to today's `Mic*` parameters; the voices join the
main bus before the master chain; eight or more outputs give the singers'
monitor its own pair (`BusLayout::monitor`, after the booth); then the
Singers surface's rows. `dj-engine` then depends on `dj-vocal` as well as
`dj-dsp` and `dj-core` — still nothing that does I/O.

## 7. Singers from the room (K4)

*Designed 10 October 2026, before any code, as the roadmap asks of every K
after K3.* The outline: a guest puts themselves on the rotation from their
phone, through the page the room already reaches for requests; their key and
guide vocal arrive as proposals; whoever is called presses *Ready*; the host
decides who may add, how many songs each, and whether a phone may choose a
key; the singers' screen gains an up-next ticker, a line of the host's own and
the logo; guests' photos appear under the words through a queue the host
approves.

### One page, two books

The same server, port and QR code as requests (`dj_net::front`), so a room
that already scans one sticker scans nothing new. The page gains a second
part, *Sing*, shown only while the host has sign-ups open — a switch of its
own, off by default and separate from requests, because a bar can take
requests all night and karaoke sign-ups for one hour of it. Behind it, a
second book beside the request book, under the same rule: **a phone can add
to a book and reach nothing else.** `Doorman` gains `POST /sing` and `POST
/ready`, and holds what it held before — an `Arc<Front>`, now carrying both
books — so there is still no path from the room to a deck, a parameter or
the rotation itself.

The application moves what the book holds into the rotation, on its own
thread, as the host would: `Rotation::ask(name, song)` — first come, first
served, one song a turn, the rules §107 already keeps — with the singer
marked as having come from a phone. The rotation stays the host's: moving,
removing and calling are unchanged, and a phone never sees another singer's
song.

### What a phone may choose

**A song from the songbook.** The songbook is the records in the collection
that have words — a karaoke song with no words on the screen is a failure on
stage. The phone types, and `GET /songbook?q=` answers with at most twelve
matches, title and artist only: no path, no file, no hash, and a token valid
for this night that names the record to `POST /sing`. Off when the host turns
it off; a room where the songbook is not offered writes its song as text,
which arrives like a request — matched to a record by the host before it can
be called.

**Its own name, once.** One phone puts down one singer: a phone that signed
up *Ana* cannot also put down *Bob* — friends sign up on their own phones,
and the host adds anyone by hand. Counted, like requests, by the phone's
address, with the caveat requests already say: a venue that puts every phone
behind one address makes that one phone. Names are 40 characters at most.

### What the host decides

In the Singers panel, *From phones*: sign-ups open or closed; songs waiting
per singer — one, two or three, one by default, so a phone cannot fill the
night; whether a phone may propose a key; whether it may propose the guide
vocal. Each says on the page what it allows, so a guest is never shown a
choice that will be refused.

### Proposals, not settings

A key (seven semitones either way, as `KEY_RANGE` already bounds it) and the
guide vocal (*As recorded · Guide · Out*) chosen on a phone are **proposals**:
the host's row shows *asks −2 · guide* with *Take* and *No thanks*, and
nothing changes until the host takes it. What a phone sends cannot move a
deck, by construction — it has no path to one — and cannot move a key the
host set, by this rule.

### Ready

The phone that signed up shows its own place — *3rd* — and nothing about
anybody else's song; no time is promised, because a host who lets one singer
do two songs makes every estimate wrong. Up next, the page shows one large
*I'm ready*; pressing it marks the singer ready on the host's row. The host
still calls: a singer not ready when called goes to the bottom, as §107's
rule already says, phone or not.

### The singers' screen

An up-next ticker under the words: the next singer with their song, as the
screen already showed (§107), and the two after them by name only — their
songs are theirs to announce. A line of the host's own (80 characters: *Last
orders at one*). The logo `brand.rs` already keeps, in a corner. All three
below the words and their veil, and none can cover a lyric.

### Photos

A guest may send one photo from the page, shown on the screen while they sing
— never before the host approves it. JPEG or PNG, at most 5 MB, decoded and
drawn again at 1280 pixels on its long side, which drops what the file
carried besides the picture — where it was taken, and when. Kept in memory
only, listed in the Singers panel to approve or refuse, and gone at *New
night*: never written to disk, never in the guest book. The page says what
happens to it before the button: *shown on the screen in this room tonight,
if the host approves it.*

### Not here

The guest book. A name typed on a phone is tonight's rotation only. A guest
is kept, with their own consent, at the host's screen (§123); the page does
not ask, because a consent ticked on a phone in a loud bar is not one.

### The order

1. **Sign-ups** — the book, the page's *Sing*, the songbook, the host's
   switches and limits, the rotation fed from it, *I'm ready*.
2. **Proposals** — key and guide vocal, taken or refused at the host's row.
3. **The screen** — the ticker, the host's line, the logo.
4. **Photos** — sent, approved, shown, forgotten.

Each is held by tests over a real socket, as requests are, and driven with a
browser playing the phone against the running application.

**Step 1, sign-ups — built (10 October).** `dj_net::stage` is the second
book: the songbook and its search, the host's rules, one name per phone and
one phone per name (a name the host put in by hand is the host's), the songs
waiting counted with the rotation's, *ready* only from the phone of the
singer up next. A token names a record only for the songbook it came from,
and the same songbook again keeps its tokens, so the host's panel can keep it
current without breaking a phone's search. `Doorman` serves the *Sing* part
on `/`, in the page's language, and takes `POST /sing` and `POST /ready`.
`Rotation::from_phones` takes what the book holds — first come, first
served, the record's file found by its id — and gives the book back the
calling order; a thread does it every two seconds, so a name arrives whether
or not the Singers panel is open, and every change to the rotation's file
now takes one lock (`AppState::change_karaoke`), because two writers of one
file each reading before the other writes is a name lost. *New night*
forgets every phone's name before it clears the rotation, so none is taken
in on its way out. Where the songbook is not offered, a song is written in
and arrives without a record, for the host to find.

**Step 2, proposals — built (10 October).** Two more of the host's
switches, *May ask for a key* and *May ask for the guide vocal*, both off.
Where on, the phone's search form offers the key — fifteen choices, *7
semitones lower* to *7 semitones higher*, *As recorded* between — and the
record's own singer (*As recorded*, *Quietly, as a guide*, *Taken out*),
says *The DJ sees what you ask for and decides*, and carries the choice
into each *Sing this*. `Stage::sign_with` keeps only what the host allows —
a part from a page opened before the host changed their mind is dropped,
not refused — holds the key to seven semitones, and counts *as recorded*
as no proposal. The song arrives in the rotation with `asks`; the host's
row says *asks key -2 · guide* with *Take* and *No thanks*, and the
calling order the same with ✓ and ✗. Taken, the key is the song's key and
the guide its `voice`, which *Load on* sets on the deck with the key;
refused, the song is as it was. Not yet: a refusal sends the phone back
with its name but not its choices.

**Step 3, the screen — built (10 October).** Under the words: *Next:* the
next singer and their song, as before; *then* the two after them, names
only; and the host's line, written in the Singers panel's *On the singers'
screen* — one line, eighty characters at most, whitespace folded, kept with
the rotation (`Rotation::line`) and gone at *New night*. The DJ's logo, from
`brand.rs`, sits in the top right corner, beside the song's name and away
from the words, and follows a new logo without a restart.

**Step 4, photos — built (11 October). K4 is complete.** *May send a
photo*, off. Where on, a phone with a singer down sees *A photo of you*,
says what happens to it before the button, and sends one JPEG or PNG
(`POST /photo`, a form with a file). The request server takes more than its
64 KB only where a handler says so for one path (`Handler::most_body`) —
here 8 MB — and only four such bodies at once across the server, so a room
sending photos together cannot hold the memory of a room. `dj_net::photo`
decodes it within limits (12,000 pixels a side, a file claiming more
refused before its pixels are read), turns it as the phone was held — the
EXIF orientation is applied before the EXIF is thrown away, or every
portrait would lie on its side — draws it at 1280 on the long side and
writes a new JPEG with nothing in it but the picture. The bytes as sent
never reach the book. One photo per singer, a new one waiting again, sixty
a night, in memory only. The host's Singers panel lists them — *Show it*,
*Refuse*, and once shown *Take it down* — and the singers' screen asks for
the approved photo of the singer singing, and draws it in the bottom left
corner while they sing, never before. *New night* forgets them all.

## 8. Break music that leads somewhere (K5)

*Designed 11 October 2026, before any code.* The outline: at a hybrid gig the
music between singers leads *into* the next singer's song — break records
ranked against that song's key and tempo by the Next rail's own scorer — and
a planned transition carries the room from the dance set into a karaoke song
and back out. A karaoke-only program cannot do this, because it does not mix;
djmanzo already has every part that does, and K5 is the joining of them.

What exists: §107's break music (`dj_app::breaks`) fades a host-named
playlist in on one deck after two seconds of quiet and out when anything
else plays, walking the playlist in its order. The Next rail ranks records
with `dj_library::suggest::score`. The planner (`dj_app::plan`) chooses where
a mix starts, how long it runs and in what style, and the automix performs a
held transition with the style table in `dj_app::shape`. None of this is
replaced.

### Chosen for the next song

When break music asks for a record, it asks the scorer which record in the
playlist leads best into **the song of the singer up next**
(`Rotation::up_next`), and plays that one. The song is judged as it will be
*sung*: a song the host has moved two semitones down is ranked in its new
key, which needs `MusicalKey::transposed` (a semitone is seven hours round
the Camelot wheel); its tempo is the record's own, since a key change keeps
the tempo. The score is the move *from* the break record *to* the song —
`score(&Playing::of(candidate), Trajectory::Hold, song)` — the same scorer,
weights and reasons the rail draws, so a break record and a rail suggestion
can never disagree about one pair. *Hold*, because a break should leave the
room where the last singer left it.

- **Not twice in a night** while any of the playlist is left unplayed: the
  records played are kept with the settings (`Settings::played`) and
  forgotten at *New night* or once every record has had its turn.
- **Ties keep the host's order**, walking from where it got to, so a
  playlist the scorer cannot tell apart plays as it did before.
- **Nothing to rank against** — nobody waiting, a song written in with no
  record found, or a record not analysed — and the record just sung is the
  target instead (`Rotation::history`'s last): the music leads *out of* the
  karaoke rather than into it. With neither, the walk, exactly as §107.
- **A switch**, *Lead into the next song*, on by default; off is §107's walk.
- **The panel says why**: *For Ana — Dancing Queen: same key · tempo fits*,
  from the typed reasons, so a host who disagrees can see what with.

Chosen when it is loaded. A host who reorders the rotation during a break
does not cut the record off; the next one is ranked against the new order.

### Bringing them in

The singer is called and their song is on a deck (*Load on*). Today the host
presses play on it and break music fades out over two seconds wherever it
is. K5 adds *Bring them in* to the up-next row, shown when the song is
loaded, paused, and something is playing to the room — break music or the
DJ's own dance record, which is how a dance set becomes a karaoke hour.

Pressing it plans the mix from the record playing into the song with the
planner, and holds it as §68's transition object, so it is drawn on the
waveforms like any other. Two differences from a mix out of a dance record:

- **Soon, not late.** The planner answers *the last* phrase boundary that
  leaves room; a singer standing at the microphone wants *the next* one
  that does, at least eight beats ahead so the press is never late —
  `plan::soonest`, beside `plan`.
- **The song keeps its tempo.** Automix syncs the incoming record to the one
  the room is dancing to. Here the singer rehearsed at the record's own
  speed, so it is the record playing that moves to the song's tempo before
  the blend, and the song plays exactly as it was recorded. Where the
  tempos are too far apart for that, the planner already says *Cut*.

The mix is performed by the automix's own machinery, once, for this
transition — the style table, the faders, the effects — and then the
automix is as it was: off stays off. The song starts from its first beat,
or from the cue point if the host set one. Break music lets go of its deck
as the mix finishes, and its fader goes back where the host left it.

### Back out

Between singers, a song ending and two seconds of quiet is §107's break: it
stays the default, because applause wants the room quiet. A switch, *Mix out
of each song*, off by default, has break music bring its record in under
the song's last phrase instead — the planner's ordinary mix-out, at the
break level. A record mixed in under one song and leading into the next
has two neighbours, so it is ranked against both — the two scores added —
and against the song ending alone when nobody is waiting, which is how the
end of the karaoke hour leads back into the dance set. The tempo rule is the
one above: here the break record is the incoming one, and it is the one
synced.

### Not here

Pitching the karaoke song to suit the break record. The song's key is the
singer's; break music fits around it, never the reverse. And no automatic
*Bring them in*: a singer is ready when the host can see them at the
microphone, which djmanzo cannot.

### The order

1. **Chosen for the next song** — the transposed key, the ranking, *played*,
   the switch, the reasons on the panel.
2. **Bringing them in** — `plan::soonest`, the held transition, the song
   keeping its tempo, the one-shot performance.
3. **Back out** — the mix-out switch.

Each is held by tests on the machine and the ranking, and driven in the
running application with a rotation, a break playlist and the decks.

**Step 1, chosen for the next song — built (11 October).**
`MusicalKey::transposed` moves a key by semitones round the wheel, checked
against the names of the chromatic scale in both rings.
`breaks::Settings::choose` ranks the playlist as above — the move from each
record into the song as sung, summed over the songs given, *Hold*, played
records waiting, an unanalysed one last, ties in the walk's order — and
keeps `played` with the settings, forgotten at *New night* and with a new
playlist. `load_break` gives it the next singer's song, or the last one sung
when nobody is waiting, or nothing. The Singers panel has *Lead into the
next song*, on, and under the phase what the record on the deck was chosen
for, in the rail's own line: *break-3-match for Ana's Song in A minor −2 BPM
· 8A · level*; *after*, leading out.

**Step 2, bringing them in — built (11 October).** `plan::soonest` answers
the bring-in's question: the next phrase boundary at least eight beats
ahead (the next bar where a record has no phrases), a 16- or 8-beat mix
that leaves the tail margin, the style chosen exactly as `plan` chooses
it; `Transition::plan_soonest` holds it, and a replan asks the same
question again. *Bring them in* on the up-next row appears once the
song is on a deck, paused, and something else is playing — break music's
deck first — and says why when pressed too late (*Not enough of the record
on deck 2 is left to mix Ana in from*). Pressed, the transition is armed
and the automix performs it **once** (`Automix::perform_once`) whether or
not it is on: the song plays, *then* the record going out is synced to
it, so the song keeps its tempo; the record fades from where its fader
was, not from full — which corrects every automix mix, since a DJ's
outgoing fader below full used to jump up as the mix began; at the end the
record is unsynced, given back its own pitch and ejected, and the automix
stands as it was. A one-shot that stops fitting the decks, or whose mix is
cleared, is dropped rather than turned into an end-of-file mix of the
automix's own. Break music stands aside while it runs and takes its deck
back after, the fader where the host had it. The row reads *Bringing Ana
in from deck 2…* until the mix is done.

**Step 3, back out — built (11 October). K5 is complete.** *Mix out of
each song*, off. Where on, while the up-next singer's song plays and is
within thirty seconds of its end, break music asks for its record then —
ranked against the song ending and the song after it, both as they will be
sung — and once it is on its deck arms the planner's ordinary mix out of
the song's last phrase, which the automix performs once, as for *Bring
them in*, but with the record synced to the song as in any mix, and
brought up to the break level rather than to full (`perform_once(level)`).
The song's deck is ejected as the mix ends, as the automix leaves any
record it has mixed out of. Break music then finds its own record playing
and adopts it as a break under way, so the next singer's song fades it out
and the fader goes back where the host had it before the mix moved it
(`Breaks::keep_fader`). Once per song. The panel's line names the song it
led out of; the scorer's reasons on it are for that song only, although
the choice weighed the next one too.

## 9. Scored singing (K6)

*Designed 11 October 2026, before any code.* The outline: the reference
melody comes from the separated vocal, so any record in the collection can
be scored, not only records somebody wrote notes for; pitch is read live on
each K3 strip, octave-agnostic, with a timing tolerance; the score is on the
singers' screen while the song runs and a leaderboard after it; a duet is
scored per strip, with teams for a *versus* round; applause from §35's room
reading is the alternative verdict.

What exists: `dj_analysis::melody` reads a pitch contour with YIN — ten
points a second, 70–700 Hz, the strongest periodic thing in a melodic band —
for the hum search. `dj_stems` separates a record into four stems and keeps
them in its chunk cache. `dj_vocal` runs a chain per singer on the audio
thread, fed by one lock-free ring of interleaved input frames (`rtrb`). The
rotation knows whose song is on which deck, and in what key.

### The reference: the separated vocal's own line

When a record that has words is separated, its **vocal stem** — not the
mix, which is where the hum search's contour has to guess between the
voice, the bass and a pad — is read with the same YIN at **twenty points a
second** (a sung note is rarely shorter than 150 ms; twenty holds its
onset to 50 ms) and kept as the record's *vocal line*: per point a pitch in
cents above a fixed reference, or none where nobody sings. Kept in the
library beside the hum search's melody (`vocal_lines`, by record, with the
separator that produced it, so a better separator's line replaces a worse
one's, as the stems cache already insists). A record with no separation
has no line and is **not scored**; the screen says *not scored — this song
has not been separated*, never a number made up from the mix.

What the line cannot be, said in the code: a backing vocal or a harmony
the separator leaves in the stem is part of it, and YIN takes the stronger
voice; a record with a rapped verse has a line of noise there, which the
scoring sees as unvoiced and leaves out rather than marking anyone down.

### The singer: pitch per strip, off the audio thread

The audio thread does no pitch detection. Each open strip **decimates its
input to 8 kHz** (the rate melody already works at — a fixed-tap low-pass
and every sixth sample at 48 kHz, a fixed fractional step at other rates,
no allocation) and pushes it into a
lock-free ring of its own, one per strip, sized for a second. A **listening
thread** drains the rings, runs YIN on 50 ms windows every 50 ms, and
publishes per strip a pitch (cents) and a confidence. A strip whose ring
overflows drops samples and counts them — a busy machine loses score
points, never audio. The gate already knows when a singer is quiet; a
closed gate is *not singing*, not a wrong note.

### The score

For each reference point where the line has a pitch, while that singer's
song plays:

- **Octave-agnostic.** The difference is taken in cents, folded into one
  octave (±600): a baritone singing a soprano's line an octave down sings
  it right.
- **In the key it is sung in.** The reference is moved by the song's key
  (`Request::key`) — the deck plays it there, so the singer sings it there.
- **A timing tolerance.** The singer's pitch is taken at the best of the
  points within 150 ms either side, so a singer a breath behind is not wrong.
- **Graded, not binary.** Within 50 cents is a hit; 50–150 is half; beyond,
  none. The score is the hits over the line's sung points, as a percentage,
  so a song with long instrumental stretches is not easier or harder for it.

The score is per **strip**, so a duet is two scores; the singer is whoever
the rotation put on that strip (K3's *on a microphone*). Held by tests over
synthetic voices — a voice on the line scores 100, an octave down the same,
a semitone sharp throughout half, a tritone or a fifth off nothing, 150 ms
late as on time, and a quarter of a second late loses only the start of
each note. (This said *a quarter-second late still near 100* until the
tests were written: with a 150 ms tolerance it cannot be, and the tests say
what it is instead.)

### On the screen, and after

While the song runs the singers' screen shows each singing strip's name and
running score small, below the words, never over them — K1's rule. When the
host presses *Sang*, the score is kept with the performance in the history
(`Sung::score`) and the screen shows it for a few seconds, then *Tonight's
best* — the top five of the night — until the next song starts. A host can
switch scoring off for the night; off, nothing is listened to and nothing
is shown. Scores are tonight's only, unless §123's guest has consented to be
kept, in which case they go with the performance in the guest book like the
key does.

### Versus, and the room's verdict

**Versus**: the host names two teams of singers; a team's score is the mean
of its singers' songs that round, shown as two bars. **Applause** (§35's room
reading, from the room microphone, never a singer's strip): after *Sang* the
room's loudness over the next eight seconds, against the night's own
applause so far, as a verdict the host can choose instead of — not as well
as — the pitch score.

### Not here

Pitch correction (K9, decided with K6 because a corrected voice scores what
the corrector sang — when K9 builds it, the score reads the strip *before*
correction). Scoring against notes somebody wrote: djmanzo has none, and the
separated vocal is the point. Anything kept about a singer's voice beyond
tonight without §123's consent.

### The order

1. **The reference** — the vocal line read from the separated vocal stem,
   kept per record and separator, and drawn under the words for the host.
2. **The singer** — decimation and a ring per strip, the listening thread,
   pitch and confidence per strip.
3. **The score** — the comparison, the running score on the singers'
   screen, `Sung::score`, *Tonight's best*, the host's switch.
4. **Versus and applause.**

Each held by tests over synthetic voices and driven with a synthetic
singer fed into a strip through the null backend's inputs — this container
has no microphone, so nothing here can say how a real voice scores.

**Step 1, the reference — built (11 October).** `melody::line` reads twenty
points a second in cents above A4 (440 Hz), with a floor at about −50 dBFS
so the separator's bleed between phrases is a rest and not the bassline.
Once every chunk of a record that has words has been separated, the
separation's own thread reads the line from the vocal stem (the first of
the four; a record with any chunk still missing has no vocal yet, not a
vocal with a hole) and keeps it in `vocal_lines` under the separator's
name; a stall of a minute with nothing new gives up until the next load.
A line sung for less than a twentieth of the record (`LEAST_SUNG`) is a
separator that found no single voice and cannot score anyone. The Singers
panel draws the up-next song's line, broken at its rests — or says *Not
scored: this song has not been separated yet*, or *no single voice was
found in this song's separated vocal* — and asks again every five seconds
until it arrives. Driven: a synthetic tune in A minor over a kick read as
88 % sung (it rests for two half-bars in sixteen), C4 at −900 cents and E4
at −500 within its vibrato, and was drawn; the K5 test song — chords and a
kick, no tune — read as 0.08 % sung, which is what made *no single voice*
a thing the panel says rather than an empty box it draws.

**Step 2, the singer — built (11 October).** `Vocals::listen`, called where
the host builds the rack, makes a ring per strip — a second long at 8 kHz —
and the audio thread hands each open strip's microphone into it as it comes
in, before its chain, a box average over each output sample's span (the
phase carries the fraction, so 44.1 kHz works as 48 does), silence while the
strip is closed, and a full ring drops and counts (`unheard`) rather than
waiting; a counting allocator holds it at no allocation with the rings
filling and nobody reading. The host gives the reading ends to
`listen::ears()`; a thread drains them every 50 ms, keeps the last two
windows of each, and reads the last with `melody::live_pitch` — the same
YIN and floor as the line — publishing `VocalParam::Pitch` and
`VocalParam::Sung` per strip. The snapshot says a pitch only while one is
heard, and each Microphones row shows it as a note (*A3*, *E4 +20*). The
null backend sings `DJMANZO_NULL_VOICE` into its first input. Driven: a
looping voice, A3 for two seconds and E4 for two, opened on Mic 1 read
*A3* and *E4* by turns on its row.

**Step 3, the score — built (11 October).** `score::Score` settles each
point of the line once the song is 150 ms past it, by the best of what was
heard within 150 ms of it, folded to an octave, moved by the song's key and
graded (50 cents a hit, 150 half); rests are not marked; a jump back of
more than a second carries on from where the song now is. The listening
thread hands every reading to `commands::score_heard`, which reads the
rotation at most once a second, scores while the up-next singer's song
plays on a deck — each strip for the singer on it, a new singer on a strip
starting afresh, one taken off keeping what they had — and keeps nothing
while it is not playing, so a finished song waits for *Sang*. *Sang* keeps
the score of the singer who sang, from their own microphone and nobody
else's (`Scoring::score_of`), with the song in the history and among
tonight's (`Rotation::tonight`); *New night* forgets tonight's and puts
scoring back on. The Singers panel has *Score the singers* and each
singer's running score on the up-next row; the singers' screen shows the
running scores below the words while a song is sung and *Tonight's best* —
five, highest first — between songs. Driven with a song whose voice holds
A3 and the null input singing: A3, Ana read *100%* on the panel and kept
100 at *Sang*; B3, Bo read *0%* and kept 0; the singers' screen then read
*Tonight's best: 1. 100 Ana, 2. 0 Bo*. The drive found the list's numbers
at the screen's edge, away from their centred names — fixed, and nothing
but looking at it would have seen it.
