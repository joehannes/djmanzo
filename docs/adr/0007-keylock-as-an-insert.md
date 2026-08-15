# ADR-0007 — Keylock is an insert, not a transport change

- **Status**: accepted
- **Date**: 2026-08-15

## Context

Keylock lets a DJ pull the pitch fader for tempo without the track going sharp.
[ADR-0002](0002-clean-room-permissive-licensing.md) already settled *what* does
the work — Signalsmith Stretch, MIT, rather than Rubber Band, GPL-or-pay. This
ADR settles *where it sits*, which turns out to matter more.

There are two ways to build it, and they differ in what else they change.

## The two designs

### Time-stretch instead of resampling

Read the source at its natural rate and ask the stretcher for a different number
of output frames than input frames. This is what the library is named for, and
it is the theoretically cleaner signal path: one pass over the spectrum instead
of a resample followed by a transpose.

It also means the deck's read cursor and its playhead advance at different
rates, and the relationship between them depends on whether keylock is on. Every
piece of code that maps between them — the waveform, the beat grid, seeking,
cue points, position reporting, loop boundaries, sync — acquires a conditional.

### Pitch-shift after resampling

Read the source exactly as before, at `step` frames per output frame, then
transpose the result back down by `1/tempo` to undo the pitch change the speed
introduced.

The signal path is marginally longer. Nothing else changes at all.

## Decision

**Keylock is an insert in the channel strip, immediately after the source read
and before the EQ.** The transport is untouched.

`position` advances by exactly `step` per output frame whether keylock is on or
off. The waveform, the beat grid, seeks, cue points and every future feature
that reasons about time see one behaviour, not two. `Deck::process` branches
once, at the top, into two loops that share the whole gain chain below them.

The transposition is the reciprocal of the **musical** speed only —
`rate × (1 + pitch)`. Sample-rate conversion is also part of `step`, but a
44.1 kHz track played on a 48 kHz device is not sharp; undoing that factor would
put the track *out* of key rather than into it.

## Latency, and why it is the real design problem

A phase vocoder has group delay. Ours is about 40 ms round trip — a 40 ms
analysis window with a 10 ms hop, which resolves down to roughly 25 Hz (below a
kick drum's fundamental) while staying short enough that a pitch-fader move
feels immediate.

If keylock simply inserted that delay, pressing it would shove a beatmatched
track 40 ms out of time. At 128 BPM a semiquaver is 117 ms, so that is a third
of one — not subtle, and it would read as the DJ's mistake rather than the
application's.

So the deck **reads ahead** by exactly the shifter's group delay, and primes the
shifter's history from before that point so the first block out is already
correct rather than a fade-in from silence. Re-priming happens on load, seek and
cue — the discontinuous jumps. Jog-wheel scrubbing goes through `set_rate`,
which is continuous and needs none.

This is asserted, not assumed. `engaging_keylock_does_not_move_the_music_in_time`
renders the same track with keylock on and off, correlates the two energy
envelopes and requires the best-fit lag to be within 2 windows (~10 ms).
Deleting the compensation makes it fail by 37–43 ms, which is the group delay
appearing exactly where the theory says it should.

## Consequences

**Good.**

- Keylock cannot break beat matching, waveform alignment or seeking, because it
  does not participate in any of them.
- Turning it on and off mid-track is safe: same playhead, same timing.
- Proven allocation-free on the audio thread by `rt_safety.rs` — two keylocked
  decks, 5,000 blocks, zero allocations, including the re-prime burst. That
  mattered: the library is C++ we did not write, and "designed for realtime" is
  a claim, not a measurement.

**Costs, accepted.**

- **A C++ toolchain and libclang are now build requirements**, because the crate
  compiles the upstream C++ with `cc` and generates bindings with `bindgen`.
  CI installs `libclang-dev`; the README says so; the failure mode
  ("Unable to find libclang") is named there so nobody has to guess.
- Resample-then-transpose is theoretically a shade worse than a true
  time-stretch. At the ±8% a DJ actually uses, this is not the axis on which the
  result will be judged.
- Keylock defaults **off**. At unity pitch there is nothing to correct, and a
  shifter in the path costs CPU and latency for no audible gain.

**Unverified.** Nobody has heard it. There is no audio device in CI. The tests
prove the pitch is right, the timing is right, the level survives and nothing
allocates — they cannot prove it sounds good. That needs a real machine.

## Alternatives rejected

**Rubber Band.** Better known, comparable quality, GPL or a commercial licence.
[ADR-0002](0002-clean-room-permissive-licensing.md) rules out copyleft in the
core; nothing here reopens that.

**Write our own phase vocoder.** Removes the C++ toolchain requirement, which is
a real cost. Rejected because keylock quality is a research problem with a long
tail — transient handling, phase locking, formant behaviour — and DJs are
unusually sensitive to it. Spending that effort to avoid an apt package is a bad
trade.

**Bypass the shifter at unity tempo.** Saves CPU in the common case, and adds a
discontinuity every time the pitch fader crosses zero. A click at centre pitch
is worse than the CPU it saves.
