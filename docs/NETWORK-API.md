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

### What it does not do

**There is no rate limiting.** The action bus is bounded, so a client that
outruns the engine gets `queue_full` and is expected to back off — that is
backpressure, not a rate limit, and a client that ignores it will keep getting
refused rather than being disconnected. Worth having; not yet built.

**A connection is dropped after five minutes of silence.** Send something, or
reconnect.
