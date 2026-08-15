# ADR-0002 — Clean-room implementation under a permissive licence

- **Status**: accepted
- **Date**: 2026-08-14

## Context

Mixxx is the only mature open-source DJ application, and forking it would deliver an enormous
amount immediately: a working engine, DVS, a library, and over a hundred controller mappings.
It is licensed GPL-2.0-or-later, so anything derived from it must also be GPL.

Meanwhile, the product we are cloning is VirtualDJ — proprietary, and the reference for
features, appearance, workflow and handling.

Two questions had to be answered together: *what do we build on*, and *what licence do we ship
under*.

## Decision

**A fresh, clean-room codebase under a permissive licence (MIT OR Apache-2.0).**

We study open-source prior art for ideas, algorithms and protocol knowledge, and we write our
own implementation. Specifically:

| Source | How we may use it |
|---|---|
| Mixxx (GPL-2.0+) | Read the code and wiki to understand the *shape* of the problem — how a caching reader, a pull-based mixer or a sync engine is structured. Write our own. No copied source, no ported files. |
| xwax (GPL-2.0) | Learn timecode behaviour from the **published articles**, not the source. |
| Deep Symmetry, StagelinQ tooling | Use the **published protocol analyses** (prose documents) as the specification for our own Rust implementations. |
| VirtualDJ (proprietary) | Clone the *feature set, layout conventions and workflow*. Never the code, skin graphics, icons, fonts, sound assets or trademarks. No decompilation. No verbatim reuse of their mapping files. |

Dependencies must be licence-compatible with a permissive distribution. Permissive (MIT,
Apache-2.0, BSD) and file-level copyleft (MPL-2.0) are fine. **GPL and AGPL are not.**

## Alternatives considered

**Fork Mixxx and stay GPL.** Fastest route to something that works today. Rejected: it forces
the licence permanently, hands us roughly a million lines of C++/Qt we did not write and must
maintain, and — decisively — its UI and workflow are explicitly *not* what we want. We want
VirtualDJ's handling, which means rewriting the entire front end anyway. The inherited value
shrinks to the engine and the mappings, and the mappings are data we can re-create.

**Clean-room but GPL ourselves.** Would let us lift xwax's timecode decoder, Rubber Band R3,
`libKeyFinder` and `aubio` directly — technically the easiest path, and it would save real work
on beat and key detection. Rejected because it permanently forecloses a commercial release, and
that option is worth more than the saved effort.

## Consequences

**Good**

- Full freedom over architecture, stack and future licensing.
- No obligation to publish under a copyleft licence.
- A clean dependency tree we understand and can audit.

**Costs**

- **We implement beat and key detection ourselves.** Every mature library is copyleft:
  `aubio` (GPL), `libKeyFinder` (GPL), `Essentia` (AGPL), `BTrack` (GPL). The algorithms are
  published research and are implementable, but the quality bar is real work. Mitigated by a
  labelled regression set scored in CI from M2.
- Signalsmith Stretch (MIT) instead of Rubber Band (GPL/commercial) for keylock. Quality is
  reportedly comparable to Rubber Band's R3 engine, so this is a small loss at worst.
- Serato import cannot use `triseratops`, which is **AGPL-3.0-or-later**; it needs a clean-room
  implementation from published format documentation. (rekordbox is fine — `rekordcrate` is
  MPL-2.0, which is file-level copyleft and compatible.)
- Ableton Link is GPLv2+ or proprietary, so network tempo sync needs either a licence from
  Ableton or our own implementation of the documented protocol. Decision deferred to M7.
- Everything takes longer than forking would have.

## Standing rules

1. Every dependency's licence is recorded in [RESEARCH.md](../RESEARCH.md#3-dependency-shortlist)
   with a note on why it is compatible. A dependency without that entry does not get merged.
2. GPL/AGPL code is never linked, vendored or copied — including "just this one function".
3. When a GPL project is the best explanation of a technique, we read it to *understand*, then
   implement from the understanding. When prose documentation exists, prefer the prose.
4. All artwork, icons, fonts and sounds are original or permissively licensed, with provenance
   recorded.
5. djmanzo does not claim compatibility with, endorsement by, or affiliation with VirtualDJ,
   Pioneer DJ, Denon DJ, Serato or Native Instruments.
