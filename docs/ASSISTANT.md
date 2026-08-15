# The assistant

djmanzo's AI layer: a DJ you can talk to. It listens for a wake phrase, takes an
instruction in plain speech — Spanish or English — and turns it into actions on
the decks, the library and the session plan.

This document covers what it does, how it is wired, and — importantly — **what
the licensing reality permits**, because two of the requested integrations are
constrained in ways worth knowing before any code is written.

---

## 1. Why this fits the architecture rather than fighting it

[ADR-0003](adr/0003-action-bus-and-parameter-registry.md) put every user intent
on one typed bus with a **text form**: `deck 1 play`, `crossfader -0.5`,
`deck 2 eq_low 0`. That decision was made for controller mapping and
replayability. It turns out to be exactly what a language model needs.

```
  microphone ─→ wake word ─→ speech-to-text ─→ LLM ─→ action text ─→ Action bus
                                                          │              │
                                              "deck 2 eq_low 0"    the same bus
                                              "crossfader 0.4"     the UI uses
```

The assistant gets **no privileged access whatsoever**. It emits the same
strings a controller mapping emits, through the same parser, into the same
queue. Consequences worth stating plainly:

- Anything the assistant does, you can do — and undo — by hand.
- Everything it does lands in the session log, so an AI-assisted set replays and
  re-renders exactly like a hand-played one.
- A hallucinated command fails to parse and is rejected at the edge. It cannot
  reach the engine, because the engine only speaks the typed enum.
- Adding a new deck feature makes it automatically assistant-controllable.

The assistant is a *client* of djmanzo, not a component inside it.

---

## 2. Licensing reality

Three integrations were requested that touch other people's catalogues. The
honest position on each:

### Spotify — metadata only, never audio

Spotify's developer policy **explicitly prohibits** using their catalogue to
"segue, mix, re-mix, or overlap any Spotify Content with any other audio
content." That sentence describes DJing. It is not a grey area, and it is not a
matter of finding the right endpoint.

On top of that, extended Web API access was restricted in May 2025 and the Web
Playback SDK is now out of reach for most developers. The DJ applications that
*do* stream Spotify have individually negotiated licences, not public API keys.

**What djmanzo will do instead:** use a user-supplied Spotify token for
**discovery and planning only** — search, playlists, saved tracks, and whatever
track attributes the API still exposes — and then match those results against
the user's *own local library*. You can plan a set from a Spotify playlist and
play it from your own files. That is compliant, and it is most of the value.

**What djmanzo will never do:** route Spotify audio into a deck.

### YouTube — your call, off by default

YouTube's Terms forbid downloading without a provided button. The tooling
(`yt-dlp`) is itself legal and widely distributed; the *use* is what may or may
not be permitted, and that depends on the content and your jurisdiction. Plenty
of legitimate uses exist: your own uploads, Creative Commons material, public
domain, promos you have rights to.

**What djmanzo will do:** treat YouTube as a pluggable *source provider* for
search and metadata, with local acquisition as an **optional backend that is
disabled by default and requires you to enable it explicitly**. The application
ships no downloader and makes no acquisition decisions for you. It will tell you
once what the terms say, and then respect your judgement — it is your
jurisdiction, your content, and your call.

### The licensed route, for completeness

DJ-specific streaming licences exist and are what the commercial applications
use: **Beatport LINK**, **Beatsource LINK** (open-format — hip-hop, Latin,
dance, which is the relevant one here), **SoundCloud Go+**, and **TIDAL**. These
require partnership agreements with each service. Out of reach for now;
`dj-sources` is designed so a licensed provider drops in later without the
assistant or the engine noticing.

---

## 3. Providers, models and keys

### Settings

A single **Providers** panel. For each provider: a key field, a link to where
you get one, a free-tier note, and a model picker that reads the provider's live
model list rather than a list baked into a release.

| Provider | Get a key | Free tier |
|---|---|---|
| **OpenRouter** | [openrouter.ai/keys](https://openrouter.ai/keys) | Yes — a rotating set of `:free` models. **Recommended starting point:** one key, hundreds of models, free and paid side by side. |
| **Anthropic** | [console.anthropic.com](https://console.anthropic.com/settings/keys) | Trial credit |
| **OpenAI** | [platform.openai.com/api-keys](https://platform.openai.com/api-keys) | Trial credit |
| **Google AI Studio** | [aistudio.google.com/apikey](https://aistudio.google.com/apikey) | Yes, generous |
| **Groq** | [console.groq.com/keys](https://console.groq.com/keys) | Yes — very fast, which matters for voice |
| **Local (Ollama / llama.cpp)** | no key | Free, private, works with no internet |

Models are fetched from each provider's list endpoint and cached, tagged
**free** / **paid** with pricing where the provider reports it. A model that
disappears does not break the app; it falls back and says so.

### Where keys live

Keys go in the OS keychain — Keychain on macOS, Secret Service on Linux — never
in a config file, never in the session log, never in a crash report. The
settings UI shows only the last four characters once a key is saved.

A local model needs no key at all, which is the right default for anyone who
would rather not send their track list to a third party.

### Cost control

Voice-driven DJing can issue a lot of requests. The assistant therefore:

- routes trivial commands ("play", "cue deck 2") through a **local intent
  matcher** that never calls a model at all;
- uses the model only for genuine language understanding and planning;
- shows a running session cost when the provider reports usage;
- lets you set a hard per-session spend cap.

---

## 4. Voice

```
  mic ─→ ring buffer ─→ wake word ("hello DJ" / "oye DJ")  ─┐
                                                            ├─→ capture until silence
  push-to-talk shortcut (configurable, default F1) ─────────┘         │
                                                                       ▼
                                                            speech-to-text (local)
                                                                       │
                                        ┌──────────────────────────────┤
                                        ▼                              ▼
                              local intent matcher            LLM planner
                              (fast path, no network)      (everything else)
                                        └──────────────┬───────────────┘
                                                       ▼
                                                  action text
```

- **Wake word** — [openWakeWord](https://github.com/dscripka/openWakeWord)
  (Apache-2.0), which runs many models at once on very modest hardware. Custom
  phrases can be trained, so "hello DJ" is configurable rather than fixed.
- **Speech-to-text** — whisper.cpp (MIT) via `whisper-rs`, running **locally**.
  A booth is loud and often has no reliable internet; sending audio to a cloud
  STT service would add latency exactly when it is least affordable. Whisper
  also handles Spanish and code-switched Spanish/English natively, which matters
  a great deal for the intended user.
- **Push-to-talk** — a configurable global shortcut, for when the room is too
  loud for a wake word to be reliable. This is the honest fallback: wake words
  fail in a club, and the assistant should not pretend otherwise.
- **The fast path matters.** "Play", "cue two", "pon bachata" resolve locally in
  milliseconds. Only real planning language reaches a model.

Audio for the assistant is captured on its own input stream and never touches
the realtime mixing path.

---

## 5. Musical intelligence

### The Dominican / Caribbean pack

Generic "AI DJ" features are useless without knowing what the music actually
does. djmanzo ships a domain pack for the repertoire this is being built for.

| Genre | Typical BPM | Notes for mixing |
|---|---|---|
| **Bachata** (moderna) | 120–140 | Requinto lead, bongó, güira. Dominant floor-filler; sits close to merengue in half-time feel. |
| **Bachata** (clásica) | 80–120 | Slower, more room to breathe; good for cooling a room without stopping it. |
| **Merengue** | 120–160 | Tambora and güira drive it. Energy climbs fast; hard to follow with anything slower. |
| **Merengue típico** | 160–180+ | Accordion-driven, Santiago style. Peak-energy, short bursts — a whole hour of it exhausts a floor. |
| **Dembow** (dominicano) | 110–125 | Heavy bassline, insistent riddim. The current peak-time engine. |
| **Reggaetón** | 88–100 | Half-time feel pairs naturally with dembow at double time. |
| **Salsa** | 150–200 (≈75–100 felt) | Different dance crowd; treat as its own block rather than interleaving. |
| **Cumbia** | 90–110 | Good bridge between reggaetón and bachata. |

Encoded as **relationships**, not just numbers: which genres bridge into which,
where a double/half-time transition works, which pairs need a hard cut instead
of a blend, and which combinations empty a floor. A bachata→merengue move is a
large tempo jump that works because of the felt-time relationship, not in spite
of it — the planner needs to know that rather than rejecting it on BPM distance.

### Session planning

Both **absolute** and **relative** instructions, because that is how DJs
actually think:

> "Half an hour of warm-up bachata, then an hour building through merengue into
> dembow, then bring it down for the last twenty minutes."

> "Más caliente." · "Make it dancier." · "Take it to a chill-out ending." ·
> "Add something the crowd here will know."

A plan is a **timeline of blocks**, each with a duration, an energy target, a
genre mix and optional anchor tracks. The planner fills blocks from the library
using BPM, key, energy and phrase structure — and every choice it makes is
**visible and editable**, because a plan you cannot override is worse than no
plan. Plans save as **templates**: "Saturday wedding", "beach sunset",
"after-hours".

Live steering adjusts the *remaining* plan rather than regenerating it — you
should never lose the next three tracks because you asked for more energy.

### Suggestions that explain themselves

Every suggestion carries its reasoning: "8A → 9A, +4 BPM, energy up one step,
you played this two weeks ago." An opaque score is not usable under pressure.

---

## 6. Generated music: HeartMuLa on Kaggle

[HeartMuLa](https://github.com/HeartMuLa/heartlib) is an open-source music
foundation model — an LLM-based song generator conditioned on lyrics and tags,
multilingual, with a companion codec and audio-text alignment model. Good enough
to make a real, personalised track.

It needs a GPU. Kaggle gives every account free GPU notebook hours, so:

```
  user enters Kaggle credentials  ─→  djmanzo pushes a notebook that installs
                                      the current heartlib release
                                              │
  "make a bachata for María's birthday" ──────┤
                                              ▼
                                      job queued on Kaggle
                                              │
                                    poll ─────┤
                                              ▼
                                   audio fetched into the
                                   Generated container
                                              │
                                    "María's track is ready"
```

- Requests are spoken naturally: *sounds like X, in the style of Y, about Z,
  with or without lyrics, mentioning these names.*
- Generation runs entirely in the background. A stuck or slow job never blocks
  the set.
- Results land in a dedicated **Generated** container in the browser, marked as
  synthetic, analysed like any other track so they can be beatmatched and mixed.
- Kaggle credentials live in the keychain with everything else.

**Stated honestly:** free Kaggle GPU quota is finite and shared. Generation is
minutes, not seconds. This is a feature for "make something for the birthday
girl during the next three tracks", not for on-demand playback.

### Sharing

Finished tracks export to a normal file and hand off to WhatsApp — Web or the
desktop app — with a prefilled message. djmanzo does not automate anyone's
WhatsApp account or send on your behalf: it prepares the share and you press
send. That keeps it within WhatsApp's terms and keeps you in control of what
goes out under your name.

---

## 7. Crate additions

```
crates/
  dj-assistant   provider abstraction, model catalogue, planning, tool schema
  dj-voice       mic capture, wake word, whisper.cpp STT, local intent matcher
  dj-sources     pluggable providers: local, Spotify (metadata), YouTube
                 (optional), room for a licensed streaming partner later
  dj-generate    Kaggle deployment, HeartMuLa job lifecycle, result ingest
  dj-secrets     OS keychain wrapper
```

All sit *above* the engine and depend on `dj-core` for the action vocabulary.
None of them can reach the audio thread except by putting text on the bus.

---

## 8. What this depends on

Worth being clear, because it sets the order of work: the assistant is only as
good as the library underneath it. Suggesting a track requires knowing the
tracks — their BPM, key, energy and structure. So:

- **Assistant foundation** (providers, keys, chat-to-action) needs only M0, and
  is buildable now.
- **Voice** needs only M0.
- **Session planning and suggestions** genuinely need **M2** (beatgrid, key,
  loudness) and **M3** (library, play history).
- **Generation** is independent and can land any time.

Building the planner before the analyser would mean a planner with nothing to
reason about. The roadmap sequences accordingly — see
[ROADMAP.md](ROADMAP.md).
