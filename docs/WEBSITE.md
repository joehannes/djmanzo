# The website: Isla Manzo

djmanzo's website presents the application, sells it, and is where a DJ's
account and licences live. It is one Cloudflare Worker with one KV namespace,
in [`website/`](../website/), and it is live at
**https://djmanzo.johannes-neugschwentner.workers.dev**.

## The idea

The site is an island, not a set of pages. It follows the application's own
metaphor ([VISUAL-LANGUAGE.md](VISUAL-LANGUAGE.md)): a watershed. Records wait
in the highlands (the record shop), two rivers meet at the booth the way two
decks meet at the mixer, and the river reaches the sea at the harbour (the box
office). Around the island float islets, one per genre, nearer for the music
the visitor names; past them the sea goes on in every direction, and every so
often a cay holds something worth knowing — a technique, a piece of the
culture, a karaoke host's trick.

Navigation uses a DJ's own controls: eight **hot-cue pads** jump to places
(the number keys too), a **crossfader** sets day or night (or follows the
visitor's clock or system), a **platter** is the minimap, and **Play the set**
is a guided tour in an order that depends on what the visitor said. Every
place opens as a record **sleeve** pulled from a crate. The **setlist** is a
plain list of every place, for anyone who would rather not drag a map, and
for screen readers.

**The MC** asks a few questions — name, DJ or karaoke host, genres, rooms,
gear, experience — and the island changes with each answer: the genre's
colours and display face, the loop the island plays, the islets that sail
closer, the order of the tour and the edition the box office opens on. The
genre knowledge (rooms, clothes, art, dances, how its DJs mix, the theme and
skin djmanzo ships for it) is in `website/src/lib/genres.ts`.

Everything audible is synthesised in the browser from each genre's pattern;
no recordings are used. Screenshots are of the real application.

## The places

| Place | What it is |
|---|---|
| El Muelle | What djmanzo is, honestly, with the real application on screen |
| La Cabina | A playable two-deck mixer: genre grooves, sync, EQ, crossfader, djmanzo's six effects |
| Tienda de Discos | The library's features as a crate to dig; a Camelot wheel |
| La Tarima | Karaoke: timed words, a guide voice, the voice and key knobs, a queue, emoji from the room |
| El Faro | three.js visuals that answer the music, three presets |
| La Taquilla | The shop |
| El Muro | The wishlist, with each wish's status |
| Radio Manzo | The blog, in many languages |
| La Arena | Comparisons with other DJ and karaoke software, including where they are stronger |
| El Tablero | Use cases: kinds of night, each walked through djmanzo's phases, with the edition it needs |
| La Plaza | Testimonials (moderated, none invented), comments, mixes, videos, sharing |
| Tu Pase | Sign-in, licences and machines, the code that links djmanzo, passport stamps, the island's prize |
| Backstage | The crew's: overview, promo codes and vouchers, wishes, moderation, orders, the blog desk, videos |

## Prices

The owner's rule: find what comparable software typically costs and charge
half. `website/shared/catalog.ts` holds the references (VirtualDJ, Serato,
rekordbox, Traktor, djay, KaraFun) with their sources; each base price is
*computed* as half the mean, rounded to a price ending .49 or .99, and a
yearly plan is capped at about ten months. The box office shows the
references beside the prices, and a test holds every price within a few
percent of half. Karaoke is compared with software-only karaoke, because
djmanzo has no song catalogue.

| | Monthly | Yearly | Buy once (version 1) |
|---|---|---|---|
| Hobbyist | $4.49 | $40.49 | $74.49 |
| Pro | $8.99 | $89.99 | $149.49 |
| Karaoke | $9.49 | $94.99 | $149.49 |
| Full | $14.99 | $149.99 | $249.49 |

**Regional prices** come in four bands (full, −20 %, −40 %, −60 %). The band
is *claimed* when the visitor picks a country and *proven* at payment: the
card's issuing country **and** the billing address must both be in the band,
and where they disagree the higher price applies (`shared/regions.ts`,
`proven`). When the proof does not back the claim, the checkout says what
price it does back and asks before charging it. An IP address only decides
what the island shows first.

## Paying

| Way | How the regional price is proven |
|---|---|
| Card (Stripe) | The Payment Element makes a payment method; the Worker reads its `card.country` and billing address, then charges or subscribes. 3-D Secure is handled. Renewals, cancellations and refunds arrive by webhook. |
| PayPal | The order is *authorised*, the payer's country read, and only then captured — or voided. Every plan is a prepaid term that does not renew. |
| Bank transfer | The order waits; the crew marks it paid in Backstage with the sending bank's country. |
| Demo | While no payment provider is configured: the same quote, proof and licence, with the countries typed in, and the licence marked demo. |

Promo codes are percent or fixed, general (any number of uses) or
individual (bound to one e-mail, single use); vouchers grant an edition and
plan outright. Backstage mints them one at a time, by the hundred, or one
per address in a list, and exports CSV. Walking the island (seven places)
earns a signed-in visitor an individual 15 % code.

## Accounts and licences

Sign-in is OAuth with PKCE through Google, GitHub, Discord, Facebook or
Microsoft — whichever have client ids set. Accounts link by *verified*
e-mail. With none configured the site offers a demo sign-in, labelled.

Licences are signed with **Ed25519**: `DJM1.<payload>.<signature>`. An
activation (`DJA1.…`) is one licence on one machine for thirty days. The
application verifies both offline with the public key it carries
(`dj_app::account::PUBLIC_KEYS`); one test vector pins the format on both
sides (`website/test/vector.test.ts`, `account::tests::the_website_s_tokens_verify`).

**Linking djmanzo**: Settings → Account → Sign in. djmanzo asks the site for
a pair of codes (the OAuth device flow, RFC 8628), shows the short one and
opens the browser at `/link`; the DJ signs in there, checks the code and
says yes; djmanzo, polling, receives a token kept in the system keychain.
It then fetches the account's licences, keeps those whose signatures hold,
and activates the best one. It renews once a day while running.

**Nothing is locked in the beta.** `account::ENFORCED` is `false`: the
account panel says what the licence covers, and every feature stays open.
Turning it on is the owner's decision; `Standing::covers` is where it would
be answered.

## Operating it

```
cd website
npm ci
npm test                      # the Worker's logic: prices, proof, promos, licences, flows
npx svelte-check
npm run build                 # read "✓ built"
npx wrangler deploy
```

`npx wrangler dev` serves it on `http://127.0.0.1:8787`; point djmanzo at it
with `DJMANZO_SITE=http://127.0.0.1:8787`. `node e2e/shoot.mjs URL out.png`
takes a screenshot (set `DJMANZO_CHROMIUM` where a browser is pre-installed).

### Secrets

Set with `npx wrangler secret put NAME`. None is committed.

| Secret | What it turns on |
|---|---|
| `ADMIN_TOKEN` | Backstage before any sign-in exists (set) |
| `ADMIN_EMAILS` | Comma-separated addresses whose sign-in opens Backstage (set) |
| `LICENCE_PRIVATE_KEY` | The Ed25519 signing key, PKCS#8 base64 (set). **Keep a copy**: a lost key means a new one, a new `PUBLIC_KEYS` entry in the application, and licences re-signed. |
| `GOOGLE_CLIENT_ID` / `_SECRET` (and `GITHUB_`, `DISCORD_`, `FACEBOOK_`, `MICROSOFT_`) | That sign-in. Redirect URI: `https://<site>/auth/callback/<provider>` |
| `STRIPE_SECRET_KEY`, `STRIPE_PUBLISHABLE_KEY`, `STRIPE_WEBHOOK_SECRET` | Card payments. Webhook: `https://<site>/api/webhooks/stripe` for `invoice.paid`, `customer.subscription.deleted`, `payment_intent.succeeded`, `charge.refunded` |
| `PAYPAL_CLIENT_ID`, `PAYPAL_CLIENT_SECRET`, `PAYPAL_ENV` (`live` or `sandbox`) | PayPal |
| `BANK_TRANSFER_DETAILS` | The bank transfer option, with these details shown |
| `ANTHROPIC_API_KEY` | Blog drafts by Claude; otherwise by Workers AI |

`DEMO_MODE` (a var) is `auto`: demo while no payment provider is set. With
the first real provider set, the demo checkout and demo sign-in switch off
by themselves.

### What KV holds

The key layout is documented at the top of `website/worker/store.ts`. KV is
eventually consistent: listings can lag a write by up to a minute (a new
wish may take that long to appear on the wall), and a promo code's use count
can run one over its limit across two places in the same second. Both were
accepted rather than paid for with a Durable Object.

## What has been proven, and what has not

**Proven here:** the Worker's logic (37 tests, the regional rule mutated
twice), the island in a browser (four Playwright journeys, one mutated), the
demo checkout end to end in production — including a refused regional claim
(a US card with a Dominican address) — and **the whole link from a running
djmanzo to the live site**: the application asked for a code, the code was
approved as a buyer, djmanzo fetched the licence, activated it, verified the
production signature offline and showed it; signing out freed the seat on
the site. The test accounts were deleted from production afterwards.

**Not proven:** any real payment or real provider sign-in — no keys exist
yet. Blog drafting could not be tried in production: the Cloudflare
account's Workers AI daily allocation was already spent by its other Workers,
and no `ANTHROPIC_API_KEY` is set. The desk reports that failure in words, as
it should.
