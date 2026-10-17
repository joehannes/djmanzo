# ADR-0012 — A marketplace where everything is free

- **Status**: accepted — planned, nothing built
- **Date**: 2026-10-17

## Context

The owner wants a marketplace of plugins, themes and DJ techniques, the
techniques ideally usable by the autopilot, and does not want it to need a
business or commercial licence of its own: "if necessary to make all
available for free in the marketplace, so be it" ([`DIRECTIVE.md`](../DIRECTIVE.md) §131).

## Decision

1. **Everything in the marketplace is free.** No prices, no payments, no
   commission, no payouts. The owner hosts and lists what others give away,
   and is party to no sale between a creator and a DJ.
2. **A creator may link to their own page** (a tip jar, a sponsorship page,
   their own shop) from their item. Whatever happens there is between them
   and the DJ.
3. **Data before code.** Themes, skins, mappings, knowledge packs and
   techniques are data the application validates before using, as it does a
   stranger's mapping today. Audio plugins are native code and carry a
   warning, and come only from creators the marketplace has reviewed.
4. **Techniques are performances in the action vocabulary**, so the
   autopilot performs them through the same gates as a person: a hand on a
   control wins, and the posture's matrix decides what it may touch.
5. **Every item states a licence that lets it be given away**, and nothing
   from the marketplace ships inside djmanzo (ADR-0002 holds).

## Why this answers the owner's question

Selling other people's work is what turns a website into a marketplace in
law: payment services, payouts and the tax on them, checking each seller's
identity (the EU's Digital Services Act asks this of platforms where
consumers buy from traders), and refunds. With nothing sold, none of that
applies. What remains is what any site hosting things people upload has to
do: terms of use, a way to report an item, and taking down what infringes.
That is planned in [MARKETPLACE.md](../MARKETPLACE.md).

This is not legal advice. It is the shape that avoids running a shop for
others; the owner should confirm it for their own country with someone
qualified. **Separately, selling djmanzo itself is a business in most
countries**: a merchant of record handles sales tax and VAT, but registering
the owner's own income is the owner's to check, marketplace or not.

## Alternatives rejected

- **Paid items with a commission.** Needs everything above, and a payments
  provider's marketplace product (identity checks and payouts per creator).
- **No marketplace, files shared by hand.** Already possible, and what the
  owner asked to go beyond.
