# Karaoke

Two independent halves, usable together or separately:

1. **Take the voice out** — a ladder of techniques, best first, with the
   trade-offs stated so you can pick per track.
2. **Put the words up** — timed lyrics on a second screen, with artwork and
   beat-reactive visuals.

They are separate on purpose. Plenty of DJs want lyrics on screen over a full
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

### Transcription, and why it works better here than usual

Transcribing lyrics from a full mix is notoriously unreliable — the instruments
drown the words. But djmanzo has something a lyrics tool normally does not:
**the isolated vocal stem.**

Running speech recognition on a clean, separated vocal is a dramatically easier
problem than running it on a mix. The same stem engine that makes karaoke
possible also makes the transcription good.

- **Whisper** (already present for [voice control](ASSISTANT.md#4-voice)) with
  word-level timestamps, run over the vocal stem.
- **HeartTranscriptor**, the lyric-recognition model that ships alongside
  [HeartMuLa](https://github.com/HeartMuLa/heartlib), which is purpose-built for
  real-world music rather than speech.

### Forced alignment — the case nobody handles

The most common real situation is **lyrics exist but are not synced**: a text
file, a tag, a web page. Plain words, no timings.

Given unsynced text *and* an isolated vocal, timings can be recovered by forced
alignment — matching the known words against the vocal audio. This turns "I have
the lyrics somewhere" into a fully synced karaoke track, which is the single
highest-value operation in this whole feature and one most karaoke software
simply does not do.

Results are written back as a sidecar `.lrc`, so the work is done once.

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

---

## 5. Where it fits

| Piece | Needs | Milestone |
|---|---|---|
| Band-limited centre cancellation | M1 (the crossover filters exist) | **K1** |
| Lyrics sources: tags, `.lrc`, LRCLIB | M3 (library) | **K1** |
| Karaoke screen, timed display, artwork | M3 | **K1** |
| Stem-based vocal removal and reduction | **M6** (stem engine) | **K2** |
| Transcription over the isolated vocal | M6, A2 (Whisper) | **K2** |
| Forced alignment of unsynced lyrics | M6 | **K2** |
| Beat-reactive and mic-reactive visuals | M2 (beat grid) | **K2** |
| Voice control and singer queue | A2 | **K2** |

**K1 is buildable after M3 and delivers a genuinely usable karaoke night** —
centre cancellation plus LRCLIB covers an enormous amount of real repertoire
with no models and no GPU. K2 is the quality tier that stem separation unlocks.
