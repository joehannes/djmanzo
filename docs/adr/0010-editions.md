# ADR-0010 — Editions from one codebase

- **Status**: accepted — the parts list built, the build switch next
- **Date**: 2026-10-17

## Context

The owner wants djmanzo sold at different levels and specialised later
([`DIRECTIVE.md`](../DIRECTIVE.md) §130): Auto-Pilot, Hobbyist, Apprentice,
Karaoke, Pro DJ, Full, and a Custom edition built from modules. Each edition
is a separate download. Prices are half of what the professional competitors
charge on average.

## Decision

1. **One codebase, never a fork per edition.** An edition is a set of
   *modules* on top of a *foundation* every edition has
   (`dj_core::edition`). Adding an edition adds a row.
2. **The edition is chosen when the application is built.** Heavy parts an
   edition does not carry are not compiled in (cargo features); the rest are
   compiled in and refused at the boundary, in Rust, where a command a module
   owns answers with the edition that has it. The interface hides what is
   refused. It never decides on its own.
3. **Custom is one download unlocked by a signed licence file**, checked
   offline. It is the only edition that is not its own build, because fifteen
   modules make 32,768 choices.
4. **The platform is not the edition.** The macOS 10.13 build is a target any
   edition can be built for.

## Consequences

- "Is this here?" has one answer, `Modules::has`, asked in one place per
  command, so an edition cannot carry half a module.
- A module that is refused rather than left out is in the binary. For
  everything but the heavy parts, that is the accepted cost of one codebase.
- The licence file adds a signature check. Its library's licence goes in
  `docs/RESEARCH.md` like every dependency's (ADR-0002).
- The tests run the Full edition. Each edition's build is checked in CI by
  building it, and its refusals by a test per module boundary.
