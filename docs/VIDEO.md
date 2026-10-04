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

**Not Opus.** Symphonia 0.5 has no Opus decoder, so a WebM from YouTube,
whose sound is almost always Opus, cannot be played; djmanzo says why rather
than failing quietly. (The same is true of an `.opus` file today, though the
library lists them.) Pure-Rust Opus decoders exist and are candidates; none has
been checked against the reference vectors yet, so none is used.

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

**Not known:** H.264 (most music videos) could not be tried, because this
container has no `gstreamer1.0-libav`. Nor could macOS's or Windows'
webviews.

## Not now

- Effects on the picture, and transitions other than the crossfade.
- Karaoke CD+G.
- Sending the picture anywhere but a window: OBS captures it.
