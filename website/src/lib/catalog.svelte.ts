/** The shop's catalogue, fetched once from the Worker (the prices are the Worker's, never the page's). */

import type { Currency, Edition, EditionInfo, Feature, Plan, Reference } from "../../shared/catalog";
import { api } from "./api";

export interface Catalog {
  editions: Record<Edition, EditionInfo>;
  plans: Record<Plan, { name: string; per: string; about: string }>;
  base: Record<Edition, Record<Plan, number>>;
  seats: Record<Edition, number>;
  features: Feature[];
  market: Record<Edition, Reference[]>;
  typical: Record<Edition, Record<Plan, number | null>>;
  bands: { share: Record<string, number>; name: Record<string, string> };
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
