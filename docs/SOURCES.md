# Where music comes from

The question every DJ application dodges: **which of these services can I
actually mix from?**

The answers differ per service, none of the differences are technical, and
almost all of them are counter-intuitive. Paying for a service is no guarantee
you may play it; having an API key is no guarantee either. So they are written
down here once, and encoded in `dj-sources` so the code obeys the same thing you
just read.

The architectural decision is [ADR-0006](adr/0006-music-sources-and-licensing.md).
This document is the practical version: what to sign up for, what it costs, and
what you get.

---

## The short version

| Source | Search | Playlists | Mixable audio | Costs |
|---|:---:|:---:|:---:|---|
| **Your own files** | ● | ● | **●** | nothing |
| **Internet Archive** | ● | ○ | **●** | nothing, no signup |
| **Jamendo** | ● | ● | **●** | free key |
| Spotify | ● | ● | **✕ never** | free key |
| YouTube | ● | ● | your own files only | free quota |
| YouTube Music | ● | ● | **✕ none exists** | same key |
| Beatsource Streaming | ◐ | ◐ | ◐ | subscription **+ partnership** |
| Beatport Streaming | ◐ | ◐ | ◐ | subscription **+ partnership** |
| TIDAL | ◐ | ◐ | ◐ | subscription **+ agreement** |
| SoundCloud | ◐ | ◐ | ◐ | **registration closed since 2019** |

● works · ○ not offered · ◐ ready, waiting on an agreement · ✕ never

**If you want to start playing in the next five minutes:** add a music folder.
That is the whole setup. Everything else on this list is a way of deciding what
to put in that folder.

---

## The three that just work

### Your own files

No account, no network, no terms of service, and nothing that stops working when
a venue's wifi does. Add folders in Settings; they are scanned on the spot.

Filenames of the form `Artist - Title.mp3` are split into artist and title.
Only the *first* separator splits, so `Aventura - Obsesion - Bachata Remix.mp3`
keeps the remix credit with the title rather than losing it. Real tag reading
arrives with the library in M3.

### Internet Archive

Free, open, and needs no key at all. Deep in public-domain and freely-licensed
recordings, including a lot of early Caribbean and Latin material that is
genuinely hard to find elsewhere — merengue and son recordings from the 78rpm
era, live sets, radio transcriptions.

Audio quality varies enormously, from clean transfers to something taped off a
radio in 1974. Check a track before you rely on it in a set.

### Jamendo

Creative Commons releases from independent artists. Free API key, direct MP3
URLs, explicitly licensed for reuse — one of the very few *online* sources you
may mix without asking anyone.

You will not find a chart bachata here. What you will find is a real catalogue,
tempo metadata that is actually populated, and a way to prove the whole path
works — search, resolve, fetch, load, play — before spending money on a service
that may turn out to be partner-gated.

Get a key at **[devportal.jamendo.com](https://devportal.jamendo.com/)**.

---

## Spotify — planning, never playing

Spotify's developer policy forbids using their content to *"segue, mix, re-mix,
or overlap any Spotify Content with any other audio content, including other
Spotify content."*

That sentence is a description of DJing. It is not an access-tier problem and
not something a better integration works around; the DJ applications that do
stream Spotify hold individually negotiated licences. Attempting it anyway would
get user accounts closed.

**So what is it good for?** Quite a lot, actually. Search, your playlists, your
saved tracks. djmanzo matches every Spotify result against your own library, so
you can plan a set from a Spotify playlist and play it from music you own — and
the browser tells you which tracks in the playlist you already have and which
you are missing.

A Spotify result never reports itself as playable, and the code that would load
one refuses. That is enforced by the type system rather than by anyone
remembering it.

Free key at **[developer.spotify.com/dashboard](https://developer.spotify.com/dashboard)**.

---

## YouTube — search yes, audio no

The Data API gives search and metadata on a free quota, and it is genuinely
useful: edits, extended versions, live cuts and regional releases that exist
nowhere else.

It does not give audio a mixer can use. YouTube licenses playback only through
its own embedded player, which cannot be routed into a deck. If you already hold
a local copy of something — your own upload, a Creative Commons release, a promo
you were sent — djmanzo matches the search result to your file.

**djmanzo ships no downloader and makes no acquisition decisions on your
behalf.** Whether a particular download is permitted depends on the content and
your jurisdiction, and that is your call to make, not the application's.

Free key at **[console.cloud.google.com/apis/credentials](https://console.cloud.google.com/apis/credentials)**
(enable "YouTube Data API v3").

### YouTube Music

This is the one people most often hope for, so here is the flat answer:

> **There is no public API for YouTube Music, and no route by which a
> third-party application may stream its audio into a mixer — not with Premium,
> not with a paid key, not at any subscription tier.**

The unofficial clients that circulate work by impersonating the web player.
That breaks the terms and gets accounts closed, and djmanzo will not do it.

What *does* work, on the same YouTube Data API key: finding music and importing
playlists, so a set can be planned from your YouTube Music library and played
from files you own. If Google ever sanctions an API, this provider is where it
lands, and nothing above it will need to change.

---

## The licensed DJ services

These are the only legal way to stream a commercial catalogue into a mixer, and
they are what the commercial applications use. Each requires a commercial
agreement between the service and **the application** — not just a subscription
held by you.

djmanzo has the slots ready. They appear in Settings, they accept and store your
credentials, and they say plainly that credentials alone will not be enough.
That is better than pretending they work or leaving them out: you need to know
djmanzo is ready for them, and you need not to buy a subscription expecting it
to work today.

### Beatsource Streaming — the one for this repertoire

The open-format arm of Beatport: hip-hop, R&B, dancehall, **reggaetón, bachata,
merengue, dembow** — the repertoire a working party DJ actually needs, licensed
specifically for DJ use.

If you play Dominican and Caribbean music, this is the service worth wanting.
[beatsource.com/link](https://www.beatsource.com/link)

### Beatport Streaming

Same arrangement, aimed at electronic music — house, techno, drum and bass, and
the remix pools around them.
[api.beatport.com](https://api.beatport.com/v4/docs/)

### TIDAL

Large, well-mastered catalogue, which is why several DJ applications support it.
Developer access is open and gets you metadata; the right to mix the audio is a
separate commercial agreement.
[developer.tidal.com](https://developer.tidal.com/)

### SoundCloud

Where a great deal of the interesting material lives — edits, bootlegs, DJ
tools, local scenes that never reach a distributor. Also the hardest to reach:
**new API application registrations have been closed since 2019** with no
reopening announced, and Go+ for DJs is a partner programme.

If you hold a client ID from before the shutdown, djmanzo will use it.
Otherwise this stays a slot.

---

## Your keys

Credentials go in the **operating system's keychain** — Keychain on macOS,
Secret Service on Linux — never in a config file. A config directory gets copied
between machines, synced to cloud storage, and pasted into forum posts when
something breaks; a key sitting in one is a key that will eventually leak.

There is no way to read a key back out. Settings shows the last four characters,
which is enough to tell two keys apart and useless to anyone reading over your
shoulder.

On a machine with no working keychain — a minimal Linux install with no Secret
Service, say — keys are held in memory and **will be gone after a restart**.
Settings says so at the point you type them, rather than letting you find out
before a gig.

---

## Streamed tracks are fetched, not streamed

When a track comes from Jamendo or the Internet Archive, djmanzo downloads it to
a cache before it reaches a deck rather than decoding from the network.

A DJ set is the wrong place to discover that a track stalls because the venue's
wifi dropped. Once a track is on disk it stays playable whatever the network
does, and the second load is instant. The cache is content-addressed by URL, so
the same track fetched twice is fetched once.
