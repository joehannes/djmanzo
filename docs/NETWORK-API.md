# Network control API (M7 foundation)

All network transports use `dj-net::ControlService`; they have no direct access
to the engine. This is the public action grammar already used by scripts,
controllers and the assistant, and preserves the single ordered action log.

## JSON messages

One JSON request receives one JSON response. A WebSocket adapter should use one
message per frame; an OSC adapter maps its address/arguments to the same shape.

```json
{"type":"action","action":"deck 1 play"}
{"type":"parameters"}
```

Successful action requests return `{"type":"accepted"}`. Parameter requests
return stable parameter names such as `deck.1.position` and
`master.crossfader`. Invalid JSON, invalid actions, and a full action queue
return a structured `error` response and never reach the audio engine.

## The server

`dj_net::ControlServer` is the transport djmanzo ships: **one JSON object per
line, `\n`-terminated, over TCP.** A WebSocket adapter is still the right thing
for a browser client and is still on the roadmap — but a WebSocket is a framing
layer over exactly this, and the capability is worth more than the framing.

Switch it on in **Settings → Remote control**. Three rules, and the third is
enforced in code rather than left to whoever writes the panel:

- **Off unless switched on.** Nobody reads a changelog before a set.
- **`127.0.0.1:7654` by default** — this machine only, which is what a script
  or a Stream Deck plugin on the same laptop needs.
- **A passphrase is required the moment the address is not loopback.**
  `ControlServer::start` refuses `0.0.0.0` without one. On loopback any local
  process can already drive the decks and a token buys little; the moment the
  socket faces a room it is the only thing between a set and the wifi.

When a passphrase is set, the first frame must be the greeting, and a wrong one
closes the connection rather than inviting another guess:

```json
{"type":"hello","token":"…"}
```

Driving it is four lines in any language:

```python
import socket, json
s = socket.create_connection(("127.0.0.1", 7654))
f = s.makefile("rw")
f.write(json.dumps({"type": "action", "action": "deck 1 play"}) + "\n"); f.flush()
print(json.loads(f.readline()))          # {"type": "accepted"}
```

### Limits

**Sixty requests a second, with a hundred and twenty in hand.** A DJ's hands
produce a few actions a second; a scene change fires a dozen at once and must
not be throttled for it; a runaway script hits the wall immediately. Past the
budget the answer is `too_fast` and **the connection stays open** — this is a
client to slow down, not one to throw out.

The bus is bounded as well, so a flood could never reach the audio thread
regardless; the limit is about the rest of the process not spending its evening
parsing frames instead of drawing waveforms.

**A connection is dropped after five minutes of silence.** Send something, or
reconnect. Frames are capped at 8 KB.

## OSC

The protocol TouchOSC, Lemur and QLab already speak. Settings → Remote control.

djmanzo invents no address space. **The action grammar is the address space**,
with slashes for spaces:

```text
  deck 1 play          ->  /deck/1/play
  deck 1 volume 0.4    ->  /deck/1/volume        , f 0.4
  crossfader -1        ->  /crossfader           , f -1.0
```

A float argument becomes the action's last word. That is the whole translation,
and it is what makes a TouchOSC layout readable next to a controller mapping.

**Loopback only, and that is not a default.** OSC is UDP: there is no handshake
to carry a passphrase, so there is nothing to authenticate *with*. A port facing
the network is refused outright rather than protected badly. Use the line
protocol for anything off this machine.

Three things are deliberately absent:

- **Bundles are refused**, not partly applied. A bundle exists to make several
  messages take effect together; quietly applying the first would be a scene
  change that half happened.
- **No pattern matching.** `/deck/*/play` would let one packet start every
  deck, which is a way to end a set rather than to run one.
- **No replies.** UDP has nobody to reply to. A surface that needs state should
  read it over the line protocol, which can answer.
