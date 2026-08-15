# ADR-0006 — Where tracks may come from

- **Status**: accepted
- **Date**: 2026-08-15

## Context

djmanzo needs to find music: search a local pool, plan sets from playlists that
live elsewhere, and integrate with the services a working DJ already pays for.
Spotify and YouTube were both requested specifically.

Both have constraints that are not negotiable by writing better code, and the
project is better served by naming them once, here, than by discovering them
during implementation.

## The constraints

### Spotify prohibits exactly what a DJ application does

Spotify's developer policy forbids using their catalogue to "segue, mix,
re-mix, or overlap any Spotify Content with any other audio content, including
other Spotify content." That is a description of DJing.

This is not an access-tier problem. Extended Web API access was additionally
restricted in May 2025 and the Web Playback SDK is now unavailable to most
developers — but even with full access, mixing the audio would breach the
policy. The DJ applications that stream Spotify hold individually negotiated
licences.

### YouTube's terms forbid downloading, but the use case is not uniformly illegitimate

YouTube's Terms prohibit downloading without a provided button. The tooling
itself is legal and widely distributed; whether a *particular* download is
permitted depends on the content and the jurisdiction. Your own uploads,
Creative Commons material, public domain recordings and promos you have rights
to are all legitimate.

### Licensed DJ streaming exists, behind partnership agreements

Beatport LINK, Beatsource LINK, SoundCloud Go+ and TIDAL all run DJ-specific
licensing programmes, which is how the commercial applications offer streaming
legally. Beatsource is the open-format one — hip-hop, Latin, dance — and is the
relevant one for this repertoire. All require a partnership.

## Decision

**A pluggable `SourceProvider` abstraction, with each provider's capabilities
declared explicitly rather than assumed.**

```rust
trait SourceProvider {
    fn capabilities(&self) -> Capabilities;  // search? metadata? playable audio?
    fn search(&self, query: &Query) -> Result<Vec<TrackRef>>;
    fn resolve(&self, track: &TrackRef) -> Result<Playable>;
}
```

`Capabilities` is the load-bearing part. A provider that cannot supply playable
audio says so, and the engine will not ask. This makes the policy a property of
the type system rather than a rule someone has to remember.

| Provider | Search | Metadata | Playable audio |
|---|---|---|---|
| Local library | yes | yes | **yes** |
| Spotify | yes | yes | **no — never** |
| YouTube | yes | yes | optional backend, off by default |
| Beatsource / Beatport / TIDAL / SoundCloud | *if a partnership is obtained* | yes | yes |
| Generated (HeartMuLa) | yes | yes | **yes** |

**Spotify** is wired for discovery and planning only: search, playlists, saved
tracks, and whatever attributes the API exposes. Results are matched against the
user's own local files, so a set can be *planned* from a Spotify playlist and
*played* from files the user owns. Its `Capabilities` will never report playable
audio, so no future change can accidentally route it into a deck.

**YouTube** is a search and metadata provider. Local acquisition is a separate,
optional backend that is **disabled by default** and requires explicit
enablement. The application ships no downloader binary and makes no acquisition
decisions on the user's behalf. It surfaces the terms once, clearly, and then
respects the user's judgement — it is their content and their jurisdiction, and
a tool that lectures its owner on every use is a tool people work around.

**Licensed streaming** gets a provider slot from the start, so obtaining a
partnership later is an implementation rather than an architectural change.

## Alternatives considered

**Ship a YouTube downloader and say nothing.** Rejected: users deserve to know
what the terms say before they rely on a workflow, and quietly building
ToS-violating behaviour into the default path exposes them without their
knowledge.

**Refuse YouTube entirely.** Rejected as paternalistic and wrong on the facts —
there are plenty of legitimate uses, and blanket refusal would mostly push
people to worse tools that give them less information.

**Attempt Spotify audio anyway.** Rejected. It is an explicit policy violation,
it would get user accounts banned, and it would not survive the first review.

## Consequences

- The user gets Spotify's discovery value without a false promise about playback,
  and the reason is stated in the UI rather than left as a mystery.
- The type system prevents a whole class of licensing accident.
- Adding a licensed streaming partner later is a new `SourceProvider`, nothing
  more.
- Users who want YouTube acquisition can have it, with the terms stated plainly,
  once, and then out of their way.
