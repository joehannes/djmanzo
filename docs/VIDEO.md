# Video on a deck (§122)

The owner's decision (27 September): *play video files on a deck — the sound
on the deck like any record, the picture in a window of its own for a
projector, or hidden.*

## The sound is a record's

A video file's sound is decoded by `dj-decode` like any other file's, so
everything a record has, it has: the waveform, the grid, the key, stems,
cues, history. Symphonia reads the containers — MP4, M4V, MOV, MKV, WebM —
and the sound in them: AAC, ALAC and MP3 in MP4; Vorbis, FLAC, AAC and PCM
in Matroska.

**Opus too, since 10 October.** Symphonia 0.5 reads the containers Opus
comes in and has no decoder for it, so a WebM from YouTube — whose sound is
almost always Opus — and every `.opus` file were refused. `dj_decode::opus`
is that decoder, over `rusty-opus` (BSD-3-Clause, pure Rust), registered
beside Symphonia's own.

- **Chosen by measurement.** Two pure-Rust decoders were run against the
  twelve RFC 8251 conformance vectors, scored by Xiph's `opus_compare` as
  Xiph's `run_vectors.sh` scores them. Decoding to stereo, both passed all
  twelve; decoding to mono, each failed several — so djmanzo decodes to
  stereo only, a mono stream coming out in both channels. On the vectors
  coded with CELT the other, `opus-decoder`, took 4–11 s each and
  `rusty-opus` 0.03–0.2 s, and a record is decoded whole before it plays.
- **The encoder's look-ahead comes off the start** — the stream's
  *pre-skip*, 312 frames from libopus. Symphonia's Ogg reader records it
  and never trims by it, and Matroska does not say it, so the decoder takes
  it in both. The header's output gain is applied, as RFC 7845 asks.
- **Checked against ffmpeg**, decoding files libopus wrote: an Ogg Opus
  stereo file came out sample for sample the same, 140 dB below the signal;
  a VP8 + Opus WebM the same at offset zero, 132 dB below. It runs 13.5 ms
  longer: a WebM marks its last packet's padding with Matroska's
  `DiscardPadding`, which Symphonia 0.5 does not read, so that much
  near-silence stays at the record's end.
- **Driven:** the `.opus` and the WebM, on two decks in WebKitGTK, each
  with its waveform and key; the WebM's VP8 picture playing with its sound.
- **Not read:** surround Opus (more than two channels), refused with that
  reason.

One fix the containers needed: Matroska does not say how long a packet of
uncompressed sound is, and Symphonia's PCM decoder will not start without
being told, so a video with PCM sound was refused outright.
`dj_decode::decoder::with_packet_bound` gives it ten seconds.

## The picture is the webview's

djmanzo decodes no video. The picture is drawn by the webview's own
`<video>` element, muted, and kept to the deck:

- **Its clock follows the deck's.** Each frame of the snapshot, the
  element's rate is set to the deck's, and where the picture has drifted
  more than a few frames from the deck's position it is moved there.
  Paused with the deck; when the deck is scratched or played backwards, the
  picture follows by jumps, as a seek can.
- **It is served from the deck, not from the disk.** A small server on this
  computer, `http://127.0.0.1:<port>/<key>/<deck>/<record>`, answers only for
  the file on that deck, and only while that record is on it, in byte ranges
  so the picture can seek. Nothing else on the disk can be reached through
  it; it listens on `127.0.0.1` only, and the random key in its addresses
  means a website open in a browser here cannot find it.
- **Why a server and not one of djmanzo's own URI schemes**, as cover art
  is: WebKitGTK plays media through GStreamer, which fetches only from
  `http`, `https` and `blob` addresses. Given a scheme of djmanzo's own, a
  `<video>` found the type supported and then failed to load at once, before
  fetching a byte (seen driving it on Linux). Over HTTP from `127.0.0.1` it
  plays.
- **Which codecs play is the platform's.** macOS and Windows play H.264,
  their webviews' own. On Linux the webview is WebKitGTK, which plays
  through GStreamer: VP8, VP9 and AV1 with the usual plugins, and H.264 only
  with the distribution's `gstreamer1.0-libav`, which djmanzo cannot ship.
  When a picture cannot be played, the panel says so and names that package.

## Where it shows

A **Video** panel. Popped out, it is a window of its own, *djmanzo - Video*,
to drag onto the projector and fill. Not opened, the picture is hidden, and
nothing is decoded for it.

What it shows, by default, is **the mix**: each deck's picture in the share
of what is heard that the deck has (its fader, and the crossfader on its
side). The pictures crossfade as the sound does, and a picture fades to black
as a record without one takes over. A paused deck is not heard, so it is not
seen: a record being cued never reaches the projector. It can instead be
**pinned** to one deck, which shows that deck's picture whatever the faders
say, still at the deck's frame when it is paused.

As with the camera: *Hide the controls* leaves only the picture, on black,
for a projector or OBS's *Window Capture*.

**Instead of black, a look** (§122, [VISUALS.md](VISUALS.md)). The DJ can pick
one of djmanzo's layered looks for *With no picture*. It is drawn beneath the
pictures and seen as much as they are not:
- whole when no deck with a picture is heard
- gone when one is heard alone
- half and half when a record without a picture shares the mix

So the projector follows the music either way. The choice is kept with the
singers' screen's, in `looks.json`.

## Built (4 October)

`dj_app::video` (the pictures on the decks, and their server),
`Video.svelte` and `videoMix.ts` (the panel, and its arithmetic), and
`dj_decode::decoder::with_packet_bound`. A video file is accepted wherever a
record is: the library's scan, a folder of sources, the deck's *Load* dialog.

Driven in WebKitGTK with a 7-second VP8 WebM with PCM sound, as a browser's
own recorder writes it, with its length written in:

- It went onto a deck like any record: its waveform, grid and key.
- The Video panel docked showed its picture. With an audio-only record
  playing on the other deck and the crossfader in the middle, it was drawn
  at half strength; with the crossfader over to its side, at full.
- Popped out, *djmanzo - Video* opened at 1280 × 720. Five captures 0.67 s
  apart showed the picture 0.6–0.7 s apart. Cued back to the start, the
  picture was back at 0.2 s.
- Two faults were found and mended. The picture had no height docked: a box
  of layers has no size of its own, so it now has a projector's shape. And
  WebKitGTK would not play from a URI scheme, hence the server.

**H.264, driven on 10 October** with `gstreamer1.0-libav` 1.24.1 installed:
a 40-second MP4 of H.264 High (a keyframe every five seconds) and AAC sound,
its time written into each frame, as most music videos are made.

- The sound went onto the deck like any record's — its waveform, grid, key
  and stems, Symphonia's MP4 reader warning only that it skips two tables
  (`stss`, `ctts`) it does not need.
- The picture played in the Video panel. Five captures about 0.7 s apart
  read the deck at 0:14, 0:15, 0:15, 0:16, 0:17 and the picture at 14.57,
  14.87, 15.63, 16.37, 17.13.
- Jumped back by beats while playing, from 25.8 to 21.0 — across the
  keyframes at 25 and 20 seconds, so GStreamer had to decode forward from
  the earlier one — the picture followed within the second, the deck
  reading 0:21, and carried on: 22.2 a second later.

Nothing in djmanzo changed for it: the webview plays H.264 once the
distribution's plugin is there, and says which package is missing when it is
not. **Not known:** macOS's and Windows' webviews.

## Not now

- Effects on the picture, and transitions other than the crossfade.
- Karaoke CD+G.
- Sending the picture anywhere but a window: OBS captures it.
