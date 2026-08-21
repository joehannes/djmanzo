# Product parity audit — 2026-08-21

This is an implementation-status review, not a marketing claim. The existing
feature matrix is the source of truth for planned work; this note calls out the
remaining gaps that prevent full day-to-day VirtualDJ parity.

## Usable now

The core booth workflow is present: multi-deck transport, pitch/keylock, sync,
waveforms and overviews, hot cues/loops/pads, mixer EQ/filter/cueing, library
and common import formats, effects, sampler, recording, layouts, keyboard and
basic MIDI mappings. The UI also exposes a sound-device health state rather
than silently failing to play.

## Priority gaps

1. **M6 stems are not yet a performance feature.** The cache is now durable,
   but the separation worker still lacks chunk-boundary crossfades and engine
   integration. Per-stem EQ/effects, stem swapping and external outputs remain
   unimplemented.
2. **M7 interoperability remains a foundation.** `dj-net` has a safe shared
   action/control boundary and clock primitives, but it does not yet open a
   WebSocket/OSC socket, announce on Pro DJ Link, discover StagelinQ devices,
   or send Art-Net/DMX.
3. **Controller depth.** MIDI input/mappings are available, while mapping
   learn/editing, LED/display feedback, HID platters and controller-specific
   audio profiles are still absent.
4. **Output breadth.** WAV recording works; Icecast/Shoutcast broadcast and
   flexible per-stem/deck external routing do not.
5. **Professional display/workflow depth.** Phrase markers, video/VJ, lyrics,
   DVS and cloud-library sync are not currently equivalent to VirtualDJ.

## GUI review outcome

The current GUI already follows the correct performance-app pattern: transport
and mixer controls remain persistent while browser, sampler, assistant and
settings are mutually exclusive side panels. This pass fixed one concrete
workflow defect: the existing custom-logo backend was unreachable from the top
bar. The top-left mark now opens the native image picker directly and offers a
visible reset action after replacement.

## Next slice

Prioritize an authenticated loopback-only WebSocket adapter over `dj-net`, then
finish the stems data path before expanding visual surface area. Those two
items improve a live set more than new decorative controls would.
