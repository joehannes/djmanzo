# Editions

djmanzo sold at different levels, from one codebase: the owner's word is
[`DIRECTIVE.md`](DIRECTIVE.md) §130, the decision is
[ADR-0010](adr/0010-editions.md), and the parts list is `dj_core::edition`.

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

## Separate downloads

The owner's word: **each edition is its own download**, built with only what
it carries.

1. **The build names its edition.** `DJMANZO_EDITION=karaoke` (any slug of
   `dj_core::edition::Edition`) when the application is built; the
   application knows its edition from then on.
2. **The heavy parts are left out of builds that don't carry them**, by
   cargo features: speech recognition (`whisper`, karaoke's words in time),
   stem separation, plugin hosting and timecode vinyl. Smaller downloads, and
   nothing to switch back on.
3. **Everything else is there and refused**: a command a module owns answers
   *"that is in djmanzo Pro DJ"*, and the interface draws the quiet card above
   instead of the panel.
4. **Each edition gets its own name, identifier and icon** (a Tauri config
   per edition, as `tauri.karaoke.conf.json` already is) and its own row in
   the release build.

**Custom is the one exception.** Fifteen modules make 32,768 possible
choices, too many to build one download for each. So Custom is a single
download carrying everything, unlocked module by module by a licence file
the store signs and the application checks offline, with no server.

**The old-Mac build is a platform, not an edition.** Today's
*djmanzo Karaoke* download is the whole application for Macs back to 10.13,
opening on karaoke. Once editions are built it becomes the Karaoke edition for
those Macs. Any edition whose modules run there can be built for them.

## What is built, and what is left

| Edition | Built | Left |
|---|---|---|
| Auto-Pilot | Automix, the autopilot's choices, the takeover, suggestions from the collection, the knowledge packs | A surface of its own: style the night (genres, energy over time, tempo range, era), fill the playlist from the collection and the online sources, start it, take over |
| Hobbyist | The foundation | The quiet cards for what is not there |
| Apprentice | Guides, the welcome, the practice lab and practice deck, the coach, the quiz, the event workflow | A curriculum in order (beatmatching, phrasing, EQ, each genre's basics from its pack, crowd dynamics, a DJ's life, first gigs, preparing them, the road ahead), with exercises the teacher checks and a record of progress |
| Karaoke | All of it | Nothing for the edition itself |
| Pro DJ, Full | All of it | Nothing for the edition itself |
| Custom | — | The signed licence file, and the store |

## The order of work

1. **The parts list** (`dj_core::edition`): done.
2. **The build switch**: the edition at build time, commands refusing what is
   not there, the interface hiding it, the heavy parts left out, a Tauri
   config and a release row per edition. Next: it touches files Session 2
   works in, and Session 2's round 2 is merged (v0.46.0).
3. **The quiet cards.**
4. **The Auto-Pilot surface.**
5. **The Apprentice curriculum**: the largest piece, and most of it is
   teaching content the owner's knowledge decides.
6. **Custom's licence file**, then the store.
