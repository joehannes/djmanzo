# The djmanzo website — progress notes

Isla Manzo: a Svelte 5 island on one Cloudflare Worker (`djmanzo`) with one KV
namespace (`DJMANZO_KV`, bound as `KV`). Read this first when resuming.

## Done
- Worker (`worker/`): catalogue, regional proof, promo codes and vouchers,
  OAuth sign-in, Ed25519 licences, device link, Stripe/PayPal/transfer/demo
  checkout, wishlist, comments, blog with AI drafts, Backstage API. 34 tests.
- Island: map, MC, HUD, booth, dock, genre islets, real app screenshots.

## Next, in order
1. Places still stubs: Taquilla (shop), Tarima, Tienda, Faro (three.js),
   Muro, Radio, Arena, Gigs, Plaza, Pase (+ /link), Backstage.
2. Deploy: `npm run build` (read "✓ built"), `npx wrangler deploy`;
   secrets ADMIN_TOKEN, ADMIN_EMAILS, LICENCE_PRIVATE_KEY.
3. App side: `dj_app::account` — device link + licence verification + panel.
4. Docs: README section, RESEARCH.md licences (three, motion, svelte, zod,
   @anthropic-ai/sdk, wrangler: all MIT/Apache).

## Checks
`npm test` · `npx svelte-check` · `npm run build` · `node e2e/shoot.mjs URL out.png`
(with `DJMANZO_CHROMIUM=/opt/pw-browsers/chromium-1194/chrome-linux/chrome`).
