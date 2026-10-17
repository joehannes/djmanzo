# ADR-0011 — One download, unlocked by signing in

- **Status**: accepted — the application's half (`dj_app::licence`) and the service (`services/licence`) built and tested; not deployed, and nothing refused by it yet
- **Date**: 2026-10-17
- **Supersedes**: decisions 2 and 3 of [ADR-0010](0010-editions.md)

## Context

ADR-0010 built each edition as its own download, from the owner's
"Downloads: separate" (§130). The owner has since asked for the opposite
([`DIRECTIVE.md`](../DIRECTIVE.md) §131): one full version with the parts
unlocked per licence, the licence a Cloudflare KV entry written when the DJ
buys, secured by social OAuth, checked by the application's own social
sign-in, and a free 7-day trial of the Full edition or any other.

## Decision

1. **One download for everyone**, every module compiled in. The edition is
   decided at run time from the licence; `dj_core::edition::Modules` is built
   from it and `Modules::has` stays the one question every check asks.
2. **The licence is a signed statement, not a lookup.** A Cloudflare Worker
   reads the DJ's KV record and returns a licence (account, edition,
   modules, trial end, issued, expires) signed with Ed25519. The
   application checks it offline with a public key built in, and never
   talks to KV.
3. **Identity is a social sign-in** (Google, Apple, GitHub, Microsoft,
   Discord) through the system browser with a loopback redirect and PKCE
   (RFC 8252), brokered by the Worker so no client secret ships in the
   application. djmanzo keeps no passwords.
4. **The trial is a KV record**: seven days of any edition, once per
   account, wherever it was started.
5. **A gig is never interrupted.** Licences are renewed in the background
   every 30 days, and a purchase past its renewal keeps working; a licence
   that gives more applies at once, one that gives less at the next start,
   never while a deck plays; playing never needs a connection.

## Alternatives rejected

- **A download per edition** (ADR-0010). Seven builds, seven release rows,
  and an upgrade means a new download; and Custom needed a licence anyway.
- **The application reading KV directly.** It would need a Cloudflare token
  in the binary, which anyone could take; and KV would then be the thing
  trusted, rather than a signature.
- **A licence key typed in.** Keys are shared in forums; an account is not
  shared as lightly, and the DJ has nothing to keep.
- **Always online.** A venue's Wi-Fi is the least reliable thing at a gig.

## Consequences

- The download is as large as the Full edition's for every DJ.
- Refusing at the command boundary (ADR-0010's "refused, not left out")
  becomes the only mechanism, so each module's commands need a check and a
  test.
- A determined person can patch an offline binary. Accepted: the scheme
  is for honest DJs and casual copies, and adds nothing that could fail at a
  gig.
- The signature check needs no new dependency: `ring` already verifies the
  packs' Ed25519 signatures (`dj_app::packs`). Anything new goes in
  `docs/RESEARCH.md` with its licence (ADR-0002).
- The Worker and its KV namespace live in this repository and are deployed
  by the owner, who holds the Cloudflare account, the OAuth clients, the
  merchant of record and the private key.
