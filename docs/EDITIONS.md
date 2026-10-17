# Editions

djmanzo sold at different levels, from one codebase and one download: the owner's word is
[`DIRECTIVE.md`](DIRECTIVE.md) §130 and §131, the decisions are
[ADR-0010](adr/0010-editions.md) and [ADR-0011](adr/0011-licence-by-sign-in.md), and the parts list is `dj_core::edition`.

## The editions, as the owner named them

| Edition | Who it is for | What it carries |
|---|---|---|
| **Auto-Pilot** | Parties, bars, anyone who wants the music to run itself | Two decks the application mixes: the DJ styles and fills the playlist, sets the autopilot up, and takes over by hand whenever they like |
| **Hobbyist** | A DJ playing for fun | Everything basic a DJ needs (the *foundation*, below), with a look at what more there is |
| **Apprentice** | Someone learning to DJ | The Hobbyist's, with an assistant and a teacher: how to DJ, the music behind it (above all for their genres), mixing techniques, crowd dynamics, a DJ's life, the first gigs, preparing them, and the road ahead |
| **Karaoke** | Karaoke hosts, bars | The Hobbyist's, with karaoke |
| **Pro DJ** | The working DJ | Everything except karaoke and the teacher |
| **Full** | Everyone who wants it all | Everything |
| **Custom** | A DJ who knows what they want | The Hobbyist foundation, with the modules they pick, each a few dollars |

## The foundation and the modules

**The foundation** is in every edition and is the Hobbyist edition: two
decks, the mixer, the library of the DJ's own files, cues, loops and pads,
the built-in effects, the sampler, recording, the keyboard, a controller
through its shipped mapping, waveforms and themes.

**The modules** are everything else. An edition is nothing more than the
modules it carries, so a new edition is a new row, not new code.

| Module | What it is | Auto-Pilot | Hobbyist | Apprentice | Karaoke | Pro DJ | Full |
|---|---|:-:|:-:|:-:|:-:|:-:|:-:|
| Four decks | Decks three and four | | | | | ✓ | ✓ |
| Stems | Vocals, drums, bass and the rest, apart, live | | | | | ✓ | ✓ |
| Auto-pilot | Automix, the autopilot's choices, a styled playlist, taking over | ✓ | | | | ✓ | ✓ |
| Assistant | What to play next, the whisper, the guides, knowledge packs, talking to it | | | ✓ | | ✓ | ✓ |
| Teacher | Lessons, the coach, practice lab and practice deck, the quiz, the road to the first gig | | | ✓ | | | ✓ |
| Karaoke | Singers' screen, queue, vocals out, words in time, sign-up page, guest book, microphones | | | | ✓ | | ✓ |
| Reading the room | Camera, the floor by microphone, the crowd's reactions, what a mix did | | | | | ✓ | ✓ |
| Controllers | Mapping editor, HID devices, motorised platters, an interface that reads what is plugged in | | | | | ✓ | ✓ |
| Timecode vinyl | DVS | | | | | ✓ | ✓ |
| Video and visuals | Video mixing, visuals, the visualiser | | | | | ✓ | ✓ |
| Going live | Broadcasting, OBS, sharing a set | | | | | ✓ | ✓ |
| Plugins | Hosting audio plugins | | | | | ✓ | ✓ |
| Online sources | Searching, streaming and buying beyond the DJ's own files | | | | | ✓ | ✓ |
| Events | Events kept step by step, the night's plan, the press kit | | | | | ✓ | ✓ |
| Replay | Hearing a set back, rendering it again offline | | | | | ✓ | ✓ |

The table is `dj_core::edition`'s, and a test holds the code to it.

## A look at what more there is

In every edition but Full, a module that is not there is shown in its place
as a quiet card: what it does, and the cheapest edition that has it
(`Edition::cheapest_with`). Never as a broken button, never as a pop-up, and
never while the DJ is mixing: the same restraint as §17's warm-up. That is
the Hobbyist's "lookout to the Pro version".

## Prices

The owner's rule: **the usual price, at half of what the professional
competitors charge on average**, with regional and student discounts.

### What the competitors charge (one-time list prices, US dollars, October 2026)

| Product | Price | Note |
|---|---|---|
| Serato DJ Pro | $299 | or $11.99 a month |
| Serato DJ Suite | $499 | or $14.99 a month |
| VirtualDJ Pro Infinity | $299 | lifetime; carries karaoke and video; or $19 a month |
| Traktor Pro 4 | $149 | one-time only |
| PCDJ DEX 4 Pro | $199 | |
| rekordbox | about $10, $15 or $30 a month | Core, Creative, Professional; subscription only |
| djay Pro | $49.99 a year | carries its AI automix |
| PCDJ Karaoki (karaoke host) | $99 | |
| PCDJ LYRX (karaoke) | $79.95 | |
| KaraFun Pro (public venues) | about $49 a month | |

These are taken from third-party price guides and resellers, which disagree
in places; check them against the makers' own pages before a price is
published. Sources: [dj.studio price guide](https://dj.studio/blog/professional-dj-software-price-guide),
[dj.studio subscriptions](https://dj.studio/blog/dj-software-subscriptions-buyers-guide),
[clubdjsoftware.com](https://www.clubdjsoftware.com/blog/how-much-does-dj-software-cost),
[Sweetwater (Serato)](https://www.sweetwater.com/store/detail/SeratoDJ--serato-dj-pro-software),
[Native Instruments (Traktor)](https://www.native-instruments.com/products/traktor-pro),
[PCDJ Karaoki](https://pcdj.com/karaoke-software/karaoki/),
[PCDJ karaoke software](https://pcdj.com/karaoke-software/),
[Algoriddim](https://www.algoriddim.com/news).

### Proposed prices

| Edition | Price | How it was reached |
|---|---|---|
| Auto-Pilot | **$49** | below the Custom equivalent ($59) |
| Hobbyist | **$39** | half of the first year of a paid entry tier (djay Pro $50, rekordbox Core about $120: average $85) |
| Apprentice | **$69** | the Hobbyist's, with the two largest modules, below their Custom price ($79) |
| Karaoke | **$55** | below the Custom equivalent ($59); a karaoke host alone sells at $80–99 |
| Pro DJ | **$119** | half of the professional average: Serato DJ Pro $299, VirtualDJ Pro Infinity $299, Traktor Pro 4 $149, DEX 4 Pro $199 → $236.50 |
| Full | **$149** | half of VirtualDJ Pro Infinity, the all-in-one that also carries karaoke and video; and Pro DJ plus $30 |
| Custom | **$39 + modules** | the Hobbyist foundation, then: |

Custom's modules, at the owner's "5–10 USD each or similar":

- **$5**: four decks, timecode vinyl, plugins, events, replay
- **$10**: controllers, online sources, video and visuals, going live, reading the room
- **$20**: stems, assistant, teacher, karaoke, auto-pilot: the five that each
  carry an edition, and that would undercut those editions at $10

**Every edition costs less than the same modules bought one by one**, so
nobody is better off building an edition out of Custom, and the checkout says
so when a Custom choice reaches an edition's price. Every module at once
would be $214 against Full's $149.

**Upgrades** cost the difference between the two editions.

### Discounts

- **By region**, by purchasing power, using the World Bank's income groups:
  high income at the list price, upper-middle income at 60 %, lower-middle
  at 40 %, low income at 30 %.
- **Students and teachers**: 50 %, on proof (a school address, or the payment
  provider's own check).
- Discounts **do not stack**: the larger one applies.

A *merchant of record* (Paddle, FastSpring or Lemon Squeezy among them)
handles tax in every country and regional prices, and is the owner's choice
when selling starts.

## One download, unlocked by signing in

The owner first asked for a download per edition (§130), then for **one full
download with the parts unlocked by the licence** (§131). That is what is
planned; [ADR-0011](adr/0011-licence-by-sign-in.md) is the decision.

- **Everyone downloads the same application**, with every module in it.
  What the DJ may use is decided by their licence, not by what was compiled.
- **The licence is fetched by signing in**, with an account the DJ already
  has: Google, Apple, GitHub, Microsoft or Discord (social OAuth). There is
  no djmanzo password to forget or leak.
- **A purchase writes a record in Cloudflare KV**, under that account. The
  application never reads KV itself; it asks a small Cloudflare Worker,
  which answers with a **licence signed with the store's key**. The
  application checks the signature with a public key built into it.
- **Custom is no longer the exception.** Its licence lists its modules, like
  every other edition's.

### How it works, end to end

1. **Buying.** On the shop page the DJ signs in, picks an edition (or
   Custom's modules), and pays through the *merchant of record*. The
   checkout carries their account's identifier.
2. **The record.** The merchant calls the Worker's webhook when the payment
   clears. The Worker checks the webhook's signature and writes
   `account:<provider>:<subject>` in KV: the edition, Custom's modules,
   the purchase, any trial.
3. **Signing in from the application.** *Sign in* opens the system browser
   (never a web view inside djmanzo, so the DJ types their Google password
   into Google and nowhere else) on the Worker's login page, which hands over
   to the provider. The answer comes back to djmanzo on a port on
   `127.0.0.1` only djmanzo is listening on, with a PKCE check, as RFC 8252
   asks of native applications.
4. **The licence.** The Worker reads the KV record and signs a short licence:
   the account, the edition, its modules, the trial's end if there is one,
   when it was issued and when it expires (30 days). djmanzo keeps it on
   disk and checks its Ed25519 signature offline; `dj_core::edition::Modules`
   comes from it, and every check asks `Modules::has` as before.
5. **Renewing.** While online, djmanzo renews the licence in the background
   every few days. Offline, it works until the licence expires: 30 days
   without a connection is a long tour.

### Rules that keep a gig safe

- **A licence never runs out mid-set.** Expiry, a lapsed trial or a refund
  takes effect the next time the application starts, never while a deck is
  playing.
- **No connection is ever needed to play.** Signing in and renewing need
  one; playing does not.
- **Nothing the DJ made is held hostage.** The collection, cues, playlists,
  recordings, sets and settings belong to the DJ in every state of the
  licence.

### The 7-day trial

- **Any edition, Full included**, for seven days, once per account. The DJ
  picks which when they start it, and may buy any edition during or after it.
- **The trial is a KV record too**, so it starts once whichever machine it is
  started on, and reinstalling does not restart it.
- **The application says how many days are left**, in the status strip only
  (never as a pop-up, never while mixing), and the last day says what buying
  keeps.
- **When it ends without a purchase,** the application opens on the shop.
  What the DJ made is kept, and a purchase unlocks it where it was. Whether a
  free tier exists after the trial (a player with two decks and nothing else,
  for instance) is the owner's decision, not made here.

### What the owner sets up, which this container cannot

- A Cloudflare account: the Worker, one KV namespace, the shop page.
- An OAuth client with each provider (Google, Apple, GitHub, Microsoft,
  Discord); each is free, Apple's needs the $99-a-year developer programme
  the macOS build already wants for signing.
- The merchant of record, and its webhook secret.
- The signing key pair: the private key a Worker secret, the public key in
  the application.

Cloudflare's free plan covers 100,000 Worker requests and 100,000 KV reads a
day, and 1,000 KV writes; a renewal is one read, a purchase one write. The
paid plan ($5 a month) is there when sales outgrow it.

### What it does not try to be

An offline application can always be patched by someone determined, and no
licence scheme changes that. This one is built so that an honest DJ is never
locked out and a casual copy does not work, and stops there: no device
fingerprints, no always-online check, nothing that could fail at a gig.

**The old-Mac build is a platform, not an edition.** Today's *djmanzo
Karaoke* download is the whole application for Macs back to 10.13, opening on
karaoke. With licences it becomes the one application for those Macs, signed
in like any other.

## Pictures for the shop

`docs/screenshots/` holds the application's screens in its themes, skins and
states, named for what they show, for the shop pages (see its README).

## What is built, and what is left

| Edition | Built | Left |
|---|---|---|
| Auto-Pilot | Automix, the autopilot's choices, the takeover, suggestions from the collection, the knowledge packs | A surface of its own: style the night (genres, energy over time, tempo range, era), fill the playlist from the collection and the online sources, start it, take over |
| Hobbyist | The foundation | The quiet cards for what is not there |
| Apprentice | Guides, the welcome, the practice lab and practice deck, the coach, the quiz, the event workflow | A curriculum in order (beatmatching, phrasing, EQ, each genre's basics from its pack, crowd dynamics, a DJ's life, first gigs, preparing them, the road ahead), with exercises the teacher checks and a record of progress |
| Karaoke | All of it | Nothing for the edition itself |
| Pro DJ, Full | All of it | Nothing for the edition itself |
| Custom | — | Nothing of its own: the licence and the shop below |
| All of them | The parts list | The licence: the Worker, sign-in in the application, the trial, the shop |

## How the switch works

- **One licence per run** (`crate::licence::Licence`), held on the
  application's state. Until signed licences exist it is a development
  licence: Full, or the edition `DJMANZO_EDITION` names (`hobbyist`, `pro`,
  `karaoke`, …), with `DJMANZO_MODULES` adding modules as a Custom licence
  lists them.
- **What each thing needs** is `dj_core::edition::Needs`: the foundation, or
  any one of a few modules (the singers' screen's looks are karaoke's *or*
  the visuals'). An action says it itself (`Needs::of_action`); commands,
  panels, activities, mission-bar readings, waveform layers and windows of
  their own are placed in `crate::licence`.
- **Every command is placed.** `licence::MODULE_COMMANDS` lists those a
  module owns and `FOUNDATION_COMMANDS` the rest; a test reads `lib.rs`'s
  handler list and fails on a command in neither, or in both. **Adding a
  command means deciding where it belongs.** A command that only reads how
  something stands is the foundation's: the interface may always ask, and a
  panel that is not drawn never does.
- **Refused at the door.** Every command passes `licence::gate` before it
  runs; every action passes `Needs::of_action` on the path every pad, key,
  button, script and the autopilot share; a preset is refused whole; a
  third deck's load is refused where every load arrives. The sentence names
  what is missing and the cheapest edition with it: *Stems is not in this
  edition of djmanzo: it comes with djmanzo Pro DJ*.
- **Hidden from the screen.** The lists the interface is drawn from leave out
  what the run lacks, at their source in Rust, so the interface never offers
  a button that would be refused; the few controls drawn without such a list
  ask `has()` (`ui/src/edition.svelte.ts`). An arrangement from a larger
  edition opens as the most of it this one has, with a note per correction.

## The order of work

1. **The parts list** (`dj_core::edition`): done.
2. **The switch at run time**: done (17 October), as `crate::licence` —
   see *How the switch works* below.
3. **The licence in the application**: reading and checking a signed
   licence, the trial's days in the status strip, *Sign in*.
4. **The licence service**: the Worker, KV, the webhook, the trial (in this
   repository, deployed by the owner).
5. **The quiet cards.**
6. **The Auto-Pilot surface.**
7. **The Apprentice curriculum**: the largest piece, and most of it is
   teaching content the owner's knowledge decides.
8. **The shop page**, with the pictures from `docs/screenshots/`.
