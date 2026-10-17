# The marketplace

A place inside djmanzo, and on its website, where DJs find what other people
made for it: themes and skins, visuals, controller mappings, knowledge packs,
DJ techniques the autopilot can perform, lessons, and audio plugins. The
owner asked for it in [`DIRECTIVE.md`](DIRECTIVE.md) §131; the decision is
[ADR-0012](adr/0012-a-free-marketplace.md). **Planned; what exists today is
its foundation.**

## Everything in it is free

The owner asked that the marketplace need no business or commercial licence
of its own, and accepted everything being free if that is what it takes. It
is what it takes, so:

- **No prices, no payments, no commission, no payouts.** The owner lists and
  hosts what creators give away, and is party to no sale.
- **A creator may link to their own page** (a tip jar, sponsorship, their own
  shop) from their item. What happens there is between them and the DJ.
- **What still applies**, as to any site where people upload things: terms
  of use, a *Report* button on every item, taking down what infringes
  someone's rights, and saying why an item was removed. These are planned
  below and cost no licence.

Selling items, with the owner taking a share, would make it a marketplace in
law: payment services, payouts and their tax, checking each seller's
identity (the EU's Digital Services Act asks this where consumers buy from
traders), refunds. None of that is planned. This is not legal advice; the
owner should confirm it for their country. **Selling djmanzo itself is a
separate matter** and is a business in most countries whatever the
marketplace does.

## What it holds

| Kind | What it is | Data or code | In which edition |
|---|---|---|---|
| **Themes** | A palette over the fourteen colour roles every theme fills | Data | All |
| **Skins** | §129's light artwork layer over a theme | Data (SVG and images) | All |
| **Visuals** | Looks and MilkDrop presets, the `.djpack` that exists today | Data | Video and visuals |
| **Controller mappings** | A controller's table, with its lights | Data (Lua where a table cannot say it) | Foundation for a shipped controller; Controllers for the rest |
| **Knowledge packs** | A genre's or a night's knowledge for the assistant: its shape, its records' roles, its techniques | Data | Assistant |
| **DJ techniques** | A move, as a performance: what the hands do, when, and what it needs | Data | All to learn; Auto-pilot to have it performed |
| **Lessons** | A lesson in the teacher's format, with its exercise | Data | Teacher |
| **Karaoke screens** | The singers' screen's design and its break bar | Data | Karaoke |
| **Audio plugins** | CLAP effects and instruments | Native code | Plugins |

## DJ techniques the autopilot can perform

`dj_assistant::technique` is today a table djmanzo ships: each technique's
kind, what it needs, how hard it is, when it applies and the metaphor it is
taught with. A marketplace technique adds **the performance itself**: a short
script of moves in the action vocabulary, timed in beats and bars against
the two records' phrases (*at the outgoing record's last phrase, sweep the
low band out over eight bars; on the one, swap the basses*).

Because it is the action vocabulary, the one every controller, the keyboard
and the assistant already speak:

- **The autopilot performs it** through the same gates as anything else it
  does: a hand on a control wins at once, and the posture's matrix decides
  what it may touch.
- **The teacher teaches it**: it can play the move for the DJ, then watch
  them do it and say how close they came (the coach does this today for the
  shipped techniques).
- **It is checked before it is installed**: every parameter it names must
  exist, every value be in range, and its length be a bounded number of bars.
  A technique that names something this djmanzo does not have is refused
  with that reason.
- **It says what it needs**, a module included (a stem swap needs Stems), so
  an edition without it shows the technique as one to learn rather than one
  to perform.

## How it works

**Packages.** Every item is a signed `.djpack`, the format visuals already
use (`dj_app::packs`): a zip with a manifest naming its files and their
hashes, its licence, and its author's Ed25519 key, signed by the author and
checked whole before anything is installed. Today a pack holds looks and
presets; the marketplace adds a `kind` to the manifest and a checker per kind.

**The index.** `dj_app::packindex` already reads indexes of packs from
anywhere on the web, among them the community index in this repository
(`community/packs.json`). The marketplace is that index grown up: a public
list, submitted to by pull request, each entry reviewed before it is listed,
the files on Cloudflare R2 (no charge for downloads), and a page on the
website drawn from the same list.

**Inside djmanzo.** A *Marketplace* surface: browse by kind and genre, see an
item's pictures, its author, its licence and what it needs, install it in one
press, remove it in one press. Signing in is the licence's sign-in, used only
to remember what a DJ installed across machines; browsing needs none.

**Reviewing.** Data items are checked by the same code that installs them,
before they are listed. Plugins are native code and run inside djmanzo, so
they are listed only from creators who have been reviewed, marked as code,
and installed only after the DJ is told so; running plugins in a separate
process, so that one cannot crash a set, is planned with them.

**Licences.** Every item states a licence that lets it be given away (MIT,
Apache-2.0, a Creative Commons licence, CC0, or a freeware licence for a
plugin). Nothing from the marketplace ships inside djmanzo, so ADR-0002's
rule against GPL code is not affected by what a DJ installs.

**Not in it.** Music, samples and stems of other people's records: the
rights in those are rarely the uploader's to give.

## The order of work

1. A `kind` in the pack manifest, and a checker for themes and skins.
2. The *Marketplace* surface over the existing indexes.
3. Techniques as performances, the checker, the autopilot and the coach
   performing them.
4. Mappings, knowledge packs, lessons and karaoke screens as kinds.
5. The website's listing, the terms of use, *Report*, and the review queue.
6. Plugins, with the out-of-process host.
