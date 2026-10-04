# Streaming a night: §119 as decided

The owner decided §119's six open questions on 27 September (DIRECTIVE.md,
*Decided later (2026-09-27)*). This is how each is built, in the order it is
built, and what each can and cannot be shown to do from a container with no
camera, no phone and no OBS.

What already exists and stays: the room's reactions read, placed on the music
and kept (`dj_app::crowd`), the now-playing overlay OBS shows as a browser
source (§108, `dj_net::overlay`), and a stream's chat brought in as a log after
the night.

## 1. djmanzo drives OBS

*Start and stop the stream, switch scenes on moments, over obs-websocket.*

**The link.** OBS 28 and later carry obs-websocket 5 built in, listening on
`ws://localhost:4455` with a password the DJ copies from OBS's *Tools →
WebSocket Server Settings*. djmanzo is a client of it: a separate program
spoken to over a socket, so OBS's GPL does not reach djmanzo (ADR-0002;
RESEARCH.md has the row). The protocol is JSON over a WebSocket: OBS says
*Hello* with a challenge and a salt, djmanzo answers *Identify* with
`base64(sha256(base64(sha256(password + salt)) + challenge))`, and from then on
sends requests and reads their responses and OBS's events.

**What djmanzo asks of it** — only these, each a request the protocol names:
`GetStreamStatus` and `GetSceneList` to say what OBS is doing and which
scenes it has; `StartStream` and `StopStream`; `SetCurrentProgramScene`.
Nothing that changes OBS's own set-up: the scenes, sources and the stream
key are the DJ's, made in OBS.

**Where it runs.** On a thread of its own, like every other server here,
never near the audio thread. The interface asks for the link's state and
sends requests through commands; the thread owns the socket and reconnects
by itself when OBS is restarted, every few seconds, the way the singers'
inputs do (K3).

**Scenes on moments.** The DJ names, in djmanzo, a scene for each of a few
moments, and a scene to come back to:

| moment | known from |
|---|---|
| the drop of the record the room is hearing | the analysis's parts (§75), on the record `crowd::Hearing` says the room hears |
| a new singer at the microphone | the karaoke rotation (§107): *Sang* pressed with somebody called up after |
| the night ends | the event being played is ended (§118) |

Each switch holds for a number of seconds the DJ sets and then returns to
the base scene, and nothing switches for half a minute after the DJ has
switched by hand — in djmanzo or in OBS itself — so djmanzo does not argue
with a hand on OBS. The night's end is not undone: it stays on its scene. A
moment with no scene named does nothing, and the drop is not watched at all
until one is. The rules are `obs::Director` and `obs::DropWatch` in Rust,
tested against a clock without OBS. *(Built 4 October.)*

**What can be shown here.** The protocol, the authentication and every
request against a fake obs-websocket server in the tests, byte for byte; the
rules against a clock. Not OBS itself: there is none in this container.

## 2. The comments the rules cannot place go to the DJ's AI provider

*Only those, in batches, with the record and part that was playing.*

`crowd::read` leaves some reactions as *something else*. Those, and only
those, are gathered and sent to the provider the DJ chose in §120, a batch
at a time — every so many reactions or minutes, whichever comes first — each
with the record and part it was placed on, and the answer is read back into
the same two fields the rules fill (what it is about, which way it leans). A
reaction the provider cannot place stays *something else*. Off when no
provider is set, and the batch is the only thing that leaves the computer.

## 3. A saved night, replayed on one timeline and compared

*Both.*

**Replay**: one horizontal timeline of the night — the records as they were
heard (`crowd::Hearing`), their parts, the reactions placed on them, the
reactions per minute, the session log's actions — scrubbed with a cursor
that names the moment under it. **Compare**: two or more nights side by side
on the measures the goals already use (warmth, busiest minute, moments,
questions about the record, requests, cold reactions), and the records two
nights share with how each night met them.

## 4. YouTube's live chat, as it arrives

*Through a verified djmanzo Google app.*

The owner registers djmanzo in Google Cloud: a project, the YouTube Data API
v3 enabled, an OAuth consent screen with the scope
`https://www.googleapis.com/auth/youtube.readonly`, and an OAuth client of
type *Desktop app*; then takes the app through Google's verification, which
a sensitive scope needs before strangers can sign in. djmanzo carries a
client-id setting and nothing else of the owner's: the DJ signs in through
their browser (the loopback flow Google prescribes for desktop apps), the
token is kept on the computer, and djmanzo reads the live chat of the DJ's
own broadcast (`liveBroadcasts.list` for the active one, then
`liveChatMessages.list` at the interval YouTube's answer asks for) into the
crowd as it arrives. Until a client id is set, the YouTube row says what to
register and the log route stays.

## 5. The computer's webcam, inside djmanzo

A panel showing the camera, which OBS can capture as a window; the webview's
own camera access, asked for when the DJ opens the panel and never before.

## 6. A phone as a camera, from djmanzo's own page

A page on the local network, opened on a phone by its QR code, that sends
the phone's camera to a second page OBS shows as a browser source, the two
joined directly (WebRTC) with djmanzo carrying only their introduction.

**The catch, to be solved before it is built:** a phone's browser gives a
page its camera only on a secure origin — `https://`, or `localhost` — and a
page at `http://192.168.1.20:…` is neither. The ways round it are a
certificate the phone trusts (djmanzo's own, installed once on the phone, or
one for a name the DJ owns) or not using the browser (Camo, DroidCam, OBS's
own phone apps, which the first answer to *which camera* already covers).
This is the last of the six for that reason.

## Order

1, 2 and 3 first: each is whole without anything the owner still has to do.
4 waits on the owner's Google Cloud registration for its last step and is
built up to it. 5 and 6 last, 6 after its catch is settled.
