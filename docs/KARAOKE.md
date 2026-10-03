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
| Beat- and microphone-reactive visuals | M2, §122 | **K2** | waits on the visual engine |
| Voice control | A2 | **K2** | waits on A2 |
| N microphones, a chain each, a singers' monitor | M1 | **K3** | the chains, the engine, the input, the screen, the monitor and reconnecting built; a singer's own settings and the chain on screen next — §6 below |
| Signing up from a phone, host permissions, photos, ticker | the audience page | **K4** | outlined |
| Break music that leads into the next song | the planner | **K5** | outlined |
| Scored singing, from the separated vocal | K3, M6 | **K6** | outlined |
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
**Not yet:** a singer's own settings laid over the strip's, and the chain
behind a press on the row.

**The order of the second step.** The engine holds a `Vocals`, built off the
audio thread when the interface opens and installed by a command, the old one
leaving through the retirement queue; the host opens the input with every
channel it has, into one ring; `ParamId::Vocal(strip, param)` publishes each
strip, strip 0 answering to today's `Mic*` parameters; the voices join the
main bus before the master chain; eight or more outputs give the singers'
monitor its own pair (`BusLayout::monitor`, after the booth); then the
Singers surface's rows. `dj-engine` then depends on `dj-vocal` as well as
`dj-dsp` and `dj-core` — still nothing that does I/O.
