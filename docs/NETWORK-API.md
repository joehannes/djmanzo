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

The process host owns authentication, binding address and rate limiting. It
must default to loopback-only and require explicit opt-in before accepting
LAN control; a DJ computer must not expose transport control on a club network
by accident.
