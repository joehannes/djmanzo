/**
 * Secrets the Worker reads, set with `wrangler secret put`. Every one is
 * optional: a provider whose keys are missing is simply not offered, and
 * with no payment provider at all the shop runs in demo mode and says so.
 */
export interface Secrets {
  /** Comma-separated e-mail addresses whose sign-in opens Backstage. */
  ADMIN_EMAILS: string;
  /** A bearer token that opens Backstage before any sign-in provider exists. */
  ADMIN_TOKEN: string;
  /** Ed25519 private key, PKCS#8, base64. Signs every licence. */
  LICENCE_PRIVATE_KEY: string;

  GOOGLE_CLIENT_ID: string;
  GOOGLE_CLIENT_SECRET: string;
  GITHUB_CLIENT_ID: string;
  GITHUB_CLIENT_SECRET: string;
  DISCORD_CLIENT_ID: string;
  DISCORD_CLIENT_SECRET: string;
  FACEBOOK_CLIENT_ID: string;
  FACEBOOK_CLIENT_SECRET: string;
  MICROSOFT_CLIENT_ID: string;
  MICROSOFT_CLIENT_SECRET: string;

  STRIPE_SECRET_KEY: string;
  STRIPE_PUBLISHABLE_KEY: string;
  STRIPE_WEBHOOK_SECRET: string;
  PAYPAL_CLIENT_ID: string;
  PAYPAL_CLIENT_SECRET: string;
  /** "live" or "sandbox" (the default). */
  PAYPAL_ENV: string;
  /** Bank details shown for a manual transfer; the transfer option is offered only when set. */
  BANK_TRANSFER_DETAILS: string;

  /** When set, blog drafts are written by Claude; otherwise by Workers AI. */
  ANTHROPIC_API_KEY: string;
}

export type AppEnv = Omit<Env, keyof Secrets> & Partial<Secrets>;
