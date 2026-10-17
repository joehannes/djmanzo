/** The shop's catalogue, fetched once from the Worker (the prices are the Worker's, never the page's). */

import type { Currency, Edition, EditionInfo, Module, Plan, Reference } from "../../shared/catalog";
import type { Fact } from "../../shared/facts";
import { api } from "./api";

type Bought = Exclude<Plan, "trial">;

export interface Catalog {
  editions: Record<Edition, EditionInfo>;
  modules: { id: Module; title: string; price: number; cheapest: Edition }[];
  foundation: number;
  plans: Record<Plan, { name: string; per: string; about: string }>;
  /** List prices in US cents, for every edition but Custom. */
  prices: Record<Exclude<Edition, "custom">, Record<Bought, number>>;
  reasons: Record<Edition, string>;
  seats: Record<Edition, number>;
  facts: Fact[];
  status: string;
  market: Reference[];
  proMonth: number;
  bands: { share: Record<string, number>; name: Record<string, string> };
  studentShare: number;
  trialDays: number;
  visitor: { country: string | null; band: number; currency: Currency };
  pay: { demo: boolean; stripe: { key: string } | null; paypal: { clientId: string } | null; transfer: boolean };
  major: number;
  downloads: string;
}

class CatalogState {
  data = $state<Catalog | null>(null);
  error = $state<string | null>(null);
  private loading: Promise<void> | null = null;

  load(): Promise<void> {
    if (!this.loading) {
      this.loading = api<Catalog>("/api/catalog")
        .then((c) => {
          this.data = c;
        })
        .catch((e) => {
          this.error = (e as Error).message;
          this.loading = null;
        });
    }
    return this.loading;
  }
}

export const catalog = new CatalogState();
