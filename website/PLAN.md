# The djmanzo website — progress notes

Read [docs/WEBSITE.md](../docs/WEBSITE.md) first; it is the full account.

## Done (17 October 2026)
- Worker: catalogue, regional proof, promo codes and vouchers, OAuth,
  Ed25519 licences, device link, Stripe/PayPal/transfer/demo checkout,
  wishlist, comments, blog with AI drafts, Backstage. Deployed.
- Island: every place, the MC, the HUD, genre islets, endless cays.
- App: `dj_app::account` and Settings → Account; proven live end to end.

## Waiting on the owner
- Provider keys: OAuth clients, Stripe, PayPal (see WEBSITE.md, Secrets).
- `ANTHROPIC_API_KEY` for blog drafts (Workers AI's daily allocation on this
  Cloudflare account is used up by its other Workers).
- Whether and when `account::ENFORCED` turns on.
- A copy of the licence signing key kept somewhere safe.

## Ideas not built
- A Spanish interface for the island (the blog is already multilingual).
- A custom domain instead of workers.dev.
