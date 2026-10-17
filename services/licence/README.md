# The licence service

djmanzo is one download; what a DJ may use comes from a licence their sign-in
fetches from here ([`docs/EDITIONS.md`](../../docs/EDITIONS.md),
[ADR-0011](../../docs/adr/0011-licence-by-sign-in.md)). This is a Cloudflare
Worker over one KV namespace, with no dependencies.

**Built and tested; not deployed.** Deploying needs the owner's accounts.

## What it does

| Route | Who calls it | What happens |
|---|---|---|
| `GET /login/:provider` | djmanzo, in the system browser | Keeps djmanzo's PKCE challenge for ten minutes, sends the DJ to Google, Apple, GitHub, Microsoft or Discord |
| `GET /callback/:provider` | the provider | Learns the DJ's id there (never their e-mail), hands djmanzo a two-minute one-time grant on `127.0.0.1` |
| `POST /licence` | djmanzo | Trades the grant and the PKCE verifier for a signed licence and a renewal token; can start the 7-day trial |
| `POST /renew` | djmanzo, in the background | A fresh licence for a renewal token |
| `POST /signout` | djmanzo | Forgets a renewal token |
| `POST /webhook/lemonsqueezy` | the merchant of record | Checks the HMAC signature; records a purchase or a refund on the account the checkout carried |

The licence format is the application's (`crates/dj-app/src/licence.rs`), and
the two halves are held together by tests on both sides: `test/vector.txt`
is written here and read there, and `src/editions.json` is compared with
`dj_core::edition` by a Rust test.

## Test

```sh
npm test          # node --test, no install needed
UPDATE=1 npm test # after a deliberate change to the licence format
```

## Deploy (the owner)

1. `npx wrangler login`, then `npx wrangler kv namespace create LICENCES` and
   put the id in `wrangler.toml`.
2. Make the store's signing key, and keep the private half only as a secret:
   ```sh
   node -e "const c=require('crypto');const k=c.generateKeyPairSync('ed25519');console.log('private',k.privateKey.export({type:'pkcs8',format:'der'}).toString('base64'));console.log('public',k.publicKey.export({type:'spki',format:'der'}).subarray(12).toString('base64'))"
   npx wrangler secret put SIGNING_KEY      # the private line
   ```
   Release builds of djmanzo are then built with `DJMANZO_LICENCE_KEY=<the public line>`.
3. Register an OAuth application with each provider, with the redirect
   `https://<your worker>/callback/<provider>`, and put each id and secret.
4. Create the products at the merchant of record, put the webhook
   `https://<your worker>/webhook/lemonsqueezy`, its secret, and the
   `VARIANTS` table.
5. `npx wrangler deploy`.

Never deploy with `test/dev-key.json`: its private half is public.
