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

*Built 4 October* (`dj_assistant::crowd`, `crowd::Reading`): a batch at
twenty waiting or two minutes after the oldest, by a thread of its own, and
*Read them now* in the Crowd panel for any night. Each message goes with the
record, how far into it the room was and the part it was in (*1:02 in, just
after the drop*), and never with who said it. The model is asked for a row for every message, *other* where it
cannot read one, so an answer with no rows is a model not answering in rows:
said in the panel, rested five minutes, and asked again. There is always a
provider — djmanzo's own default is the local one — so it is a switch in the
Crowd panel instead, on unless the DJ turns it off, with the provider named
beside it. What came back is kept beside the night
(`crowd/read/<session>.jsonl`) and marked *AI* where it is drawn.

## 3. A saved night, replayed on one timeline and compared

*Both.*

**Replay**: one horizontal timeline of the night — the records as they were
heard (`crowd::Hearing`), their parts, the reactions placed on them, the
reactions per minute, the session log's actions — scrubbed with a cursor
that names the moment under it. **Compare**: two or more nights side by side
on the measures the goals already use (warmth, busiest minute, moments,
questions about the record, requests, cold reactions), and the records two
nights share with how each night met them.

*Built 4 October* (`crowd::timeline`, `crowd::compare`, in the Crowd panel).
The line is the records as the room heard them, measured where the night was
(`crowd::Hearing`), by arrival for a night kept before that; each stretch
ends at the record's own end when the collection knows its length. Drops,
breakdowns and the voice are drawn where the room heard them, the reactions
where what they were about was heard (less the stream's delay), and the
reactions per minute beneath. The cursor follows the pointer, or the arrows
(ten seconds) and Page Up/Down (a minute), and names the moment: the clock,
the record, how far in, the part just heard, how busy that minute was.
**The session log's actions are not on it:** the log lives in memory for
the run and is not kept per night, so a past night has none to show; that
waits for the log to be kept. Compare takes any two or more kept nights.

**The session log's actions, kept with the night — designed 12 October.**

- *Kept.* The bus's log is appended, from the pump, once a second and only
  when something new was logged, to `crowd/actions/<session>.djset`: a
  session file in `crate::session`'s own format, so a past night can be
  opened, replayed and diffed like any set the DJ saved by hand. A
  `# started <unix ms>` comment under the header ties the log's clock (time
  since the bus began) to the wall's, which is what the line is drawn in; the
  session reader already skips comments. A restart is a new session and a
  new file, as the reactions are.
- *Drawn.* Each event one of §14's gestures names (`signals::did`) — a
  record loaded or previewed, a deck cleared, a cue, a loop or its size, the
  stems, the EQ, the filter, an effect, the crossfader, the tempo, sync — is
  a mark on a row above the records, where it was done. A run of the same
  gesture from the same hand with less than three seconds between moves is
  one mark across its length, with its count — a crossfader worked through
  a blend is one stroke, not two hundred ticks. djmanzo's own moves (the
  log's `By::Machine`: automix, autopilot, break music) are marked as
  djmanzo's, as the log marks them. The cursor names the mark it is over,
  beside the record and the part. An event no gesture names is in the file
  and not on the line.
- *This night* is read from the bus itself, so the line is never a second
  behind; a past night from its file.
- *Not there:* a night kept before this has no actions, and its line says
  so rather than drawing an empty row as if nothing was done.

*Built 12 October* (`crowd::ActionKeeper`, `crowd::load_actions`,
`crowd::moves`, `ActionBus::log_since`, the *Crowd* panel's line), as
designed. Driven in WebKitGTK: the demo's night was kept as it was played —
`crowd/actions/session-1791767720.djset`, its `# started` within a second of
the session's own name, every load, EQ, cue, loop, the crossfader dragged
in six steps and a sync — and the line drew them above the records, the
cursor reading *Crossfader ×6* over the drag and *Sync* over the press; after
a restart the same night, chosen from the list, drew the same marks from its
file. Held by three Rust tests (the file appended, its header once, read back
as a session file at the wall's time; runs merged within the gap, the hands
apart, ungestured events left off; tonight's line from the bus, a past
night's from its file, an older night with none) and one on the bus, the
`crowd.json` golden carrying a night's moves, and two browser tests; nine
Rust and six interface mutants fail them. The pump's once-a-second call is
held only by the drive.

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

*Built 4 October* (`dj_app::youtube`, Settings → *YouTube live chat*). The
DJ's browser is sent to Google's sign-in with PKCE (`S256`) and a random
`state`; it comes back to a loopback address only djmanzo is listening on,
for at most five minutes; the code is traded with the verifier, and the
refresh token goes to the keychain (`SecretKind::YouTubeSignIn`). A thread
of its own finds the live broadcast (`liveBroadcasts.list`,
`broadcastStatus=active`) and reads its chat (`liveChatMessages.list`), page
after page with `pageToken`, at YouTube's `pollingIntervalMillis` but never
more often than every ten seconds. Text, Super Chats and Super Stickers
become reactions of the night with their author and YouTube's own
timestamp; the rest (new members, polls, deletions) is left; each message
is kept once, across pages and restarts. A chat that ends sends it looking
for the next broadcast; a sign-in Google no longer accepts is forgotten and
the settings say to sign in again; a spent quota rests ten minutes.
`liveChatMessages.streamList`, which `RESEARCH.md` names as the way round
the quota, is not used: its documentation does not give an HTTP form that
could be built and checked here without a Google account, and the polled
`list` is documented to the field.

### What the owner registers

1. **A Google Cloud project** for djmanzo, at console.cloud.google.com.
2. **APIs & Services → Library → YouTube Data API v3 → Enable.**
3. **OAuth consent screen** (*Google Auth Platform*): user type
   *External*; the app's name, a support e-mail, the home page and privacy
   policy addresses; the one scope
   `https://www.googleapis.com/auth/youtube.readonly`. While the app is in
   *Testing*, only the Google accounts listed as test users can sign in
   (up to 100), and their sign-ins lapse after seven days.
4. **Credentials → Create credentials → OAuth client ID → Desktop app.**
   Its client id goes in djmanzo's settings (and, if Google's token address
   asks for it, its secret, which Google says a desktop app cannot keep and
   so is not one). To ship it inside djmanzo instead of having each DJ paste
   it, it can be made the default in `youtube::Settings` — ask, and it will
   be.
5. **Verification.** `youtube.readonly` is a *sensitive* scope: before
   strangers can sign in without a warning screen, Google reviews the app —
   a privacy policy on the app's domain, the domain verified in Search
   Console, a short video of the sign-in and of what the scope is used for.
   It takes days to weeks.
6. **Quota.** A project has 10,000 units a day across everyone signing in
   with its client id. Finding the broadcast costs 1 and each page of chat
   costs 1, and djmanzo reads a page at most every ten seconds: a
   three-hour set is about 1,080 units, so about nine such sets a day per
   project. More needs YouTube's quota extension form (an audit of the
   app). A DJ can always paste a client of their own, with its own quota.

## 5. The computer's webcam, inside djmanzo

A panel showing the camera, which OBS can capture as a window; the webview's
own camera access, asked for when the DJ opens the panel and never before.

*Built 4 October* (the *Camera* panel, `Camera.svelte`; `monitors::Panel::Camera`).
Asked for with a press rather than on opening — a workspace that restores the
panel at launch must not turn a camera on by itself — and remembered for the
run by Rust (`camera_wanted`), so the panel popped out shows the camera in its
own window, *djmanzo - Camera*, without asking again; that title is what OBS's
*Window Capture* is pointed at. A camera picker once there are two, a mirror,
and *Hide the controls* for a picture with nothing of djmanzo's on it; a click
on the picture brings them back.

## 6. A phone as a camera, from djmanzo's own page

A page on the local network, opened on a phone by its QR code, that sends
the phone's camera to a second page OBS shows as a browser source, the two
joined directly (WebRTC) with djmanzo carrying only their introduction.

**The catch:** a phone's browser gives a page its camera only on a secure
origin — `https://`, or `localhost` — and a page at `http://192.168.1.20:…`
is neither. The ways round it are a certificate the phone trusts, or not
using the browser (Camo, DroidCam, OBS's own phone apps, which the first
answer to *which camera* already covers). This is the last of the six for
that reason.

### The design (4 October)

**One port, both kinds of connection.** djmanzo listens on one port (8443
unless it is taken) and looks at the first byte of each connection: a TLS
handshake is the phone, anything else is plain HTTP. Plain HTTP is answered
only from this computer — the page OBS loads, at
`http://127.0.0.1:8443/v/<key>/`, which is a secure origin because it is
local. From anywhere else, plain HTTP gets a page saying to use the code.

**HTTPS with djmanzo's own certificate authority.** Made once, the first
time the phone camera is switched on, and kept beside the settings:

- an authority whose name constraints confine it to private addresses
  (10/8, 172.16/12, 192.168/16, 169.254/16, 100.64/10, fc00::/7, fe80::/10)
  and to `.local` and `localhost`, so that even if its key were taken it
  could vouch for no real website;
- for each connection, a certificate for the address the phone actually
  reached — read from the socket, so a laptop that changes networks needs
  nothing redone — made on the spot and kept for the run.

The phone then has two ways in:

1. **Tap through the warning.** The phone's browser says the connection is
   not private; *Advanced → Proceed* (Chrome), *Show Details → visit this
   website* (Safari), *Accept the risk* (Firefox). djmanzo shows the
   authority's fingerprint so the DJ can check it is djmanzo's.
2. **Install djmanzo's authority once.** The phone page offers it as a
   download, with the steps for Android and iPhone; after that the page
   opens without a warning on any network.

**Whether a phone's browser gives a page reached by tapping through the
warning its camera is the one thing here this container cannot show.** The
reports are that Chrome and Firefox on Android do. Safari on an iPhone is
less clear, and the second way is for it. Both are recorded as unverified
until a phone has been pointed at it.

**Two keys, both random, both kept.** The phone's address carries one
(`/p/<key>`), the QR code is the only place it is shown, and *New phone
link* replaces it — for when someone in the room photographed the screen.
The OBS page carries another (`/v/<key>`), so a new phone link does not
break the browser source. A request without the right key is told nothing
exists; the OBS page's paths answer only this computer. With the key on the
OBS side too, no other website open on this computer can post an answer
that would send the phone's picture somewhere else.

**The introduction.** The phone offers, the OBS page answers, each in one
piece once its candidates are gathered (no trickle: on a local network they
are all there within a second, and one request each way is a protocol a
test can hold). Each side waits for the other with a held request of up to
twenty seconds. A new offer replaces the old one, so a phone that reloads,
or a second phone, takes over, and the OBS page follows. There are no STUN
or TURN servers. Both ends are on the same network, and nothing leaves it.

**The phone page:** asks for the camera on a press, the back camera first,
with a switch for the front. It shows its own picture, keeps the screen
awake while it sends, and sends up to 1280×720. It sends **video only**:
the phone's microphone in a room with a PA is an echo.

**The OBS page:** the picture filling the source, and nothing at all when
no phone is sending, so an idle source puts nothing on the stream.

**In djmanzo:** the Camera panel gains a second tab, *A phone*. It has the
switch, the QR code (shown on a press and hidden again once a phone is
sending), the address for OBS with a copy button, what the phone and OBS
are doing, *New phone link*, and the fingerprint.

**What is built on:** `rustls` 0.23 and `ring` 0.17, already in djmanzo for
its HTTPS requests; `httparse`, already here; `rcgen` for the certificates,
which is new (see RESEARCH.md). Not `tiny_http`'s TLS feature, which was
refused for pinning a 2021 `rustls` and an unmaintained `ring`. The request
parsing is `httparse`'s, and the limits are this server's own: one request
per connection, 16 KB of headers, 64 KB of body, no chunked bodies, at most
32 connections at once, ten seconds to send a request.

### Built (4 October)

`dj_net::authority`, `dj_net::secure`, `dj_net::phone` and its two pages
(`crates/dj-net/src/phone/`), `dj_app::phone_camera`, and the Camera
panel's tab *A phone* (`PhoneCamera.svelte`). `cargo run -p dj-net
--example phone_camera` serves it on its own, for trying with a phone
without the rest of djmanzo.

Two things the design did not foresee, found while building it:

- **A phone stopping says which offer it is stopping.** Otherwise a second
  phone that opened the page and pressed *Stop* ended the first one's
  picture.
- **A phone whose link to OBS went asks before offering again.** OBS lets go
  of one phone to show the next, and the first one used to take it back at
  once, so the two of them took turns. It now asks whether its offer is
  still the current one: if another phone's is, it stops and says so; if its
  link was replaced, it says that.

Driven in WebKitGTK with djmanzo's own server, against Chromium standing in
for both the phone (its fake camera, the certificate tapped past) and OBS:

- Switched on, the panel showed the code and `https://192.0.2.2:8443/p/…`,
  and OBS's address. The settings and the authority were kept, its key
  readable by its owner only.
- The phone page was a secure context and sent. OBS's page decoded it at
  480 × 270, rising. The panel said *A phone is sending its camera* and *OBS
  is showing the phone*, and had put the code away by itself.
- Restarted, djmanzo served again at the same address for OBS.
- *New phone link* while the phone was sending: OBS let go of the picture
  744 ms later. The cut-off phone said *This link was replaced* when it
  noticed, 12 s later. The new code came up, and OBS's address was the same.
- In the browser test, OBS's page reloaded (as OBS does when it restarts a
  source) had the phone back within about 16 s: the time the phone takes to
  notice the old page has gone.

**What is still not known:** whether a real phone's browser lends its camera
to a page reached by tapping past the warning. Chromium here was told to,
and no phone has been pointed at it. Nor has OBS's own browser source, which
is Chromium too.

## Order

1, 2 and 3 first: each is whole without anything the owner still has to do.
4 waits on the owner's Google Cloud registration for its last step and is
built up to it. 5 and 6 last, 6 after its catch is settled — as above.
