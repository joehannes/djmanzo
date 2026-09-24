<script lang="ts">
  /**
   * §111: a song djmanzo does not have, and where to buy it.
   *
   * Links to the stores' own searches, opened in the DJ's browser — the
   * address built in Rust from the store's table, never here. The purchase is
   * the store's business and the file lands in the downloads folder, where
   * djmanzo files it. The one sentence under the links says what buying
   * covers, because a DJ asking "can I play this" deserves the answer where
   * they are looking.
   */
  import { openStore, storeLinks, type StoreLinks } from "./api";

  let {
    artist = "",
    title,
    karaoke = false,
  }: { artist?: string; title: string; karaoke?: boolean } = $props();

  let links = $state<StoreLinks | null>(null);
  let error = $state("");

  $effect(() => {
    void storeLinks(karaoke)
      .then((found) => (links = found))
      .catch(() => (links = null));
  });

  function open(slug: string) {
    error = "";
    void openStore(slug, artist, title).catch((e) => (error = String(e)));
  }
</script>

{#if links && links.stores.length > 0}
  <div class="buy" data-buy-links>
    <span class="lead">Find it to buy:</span>
    <ul aria-label="Stores">
      {#each links.stores as store (store.slug)}
        <li>
          <button
            type="button"
            data-store={store.slug}
            title="{store.sells}{store.searches ? '' : ' — opens the store; search there'}"
            onclick={() => open(store.slug)}>{store.name}{store.searches ? "" : " ↗"}</button
          >
        </li>
      {/each}
    </ul>
    <p class="covers">{links.covers}</p>
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
  </div>
{/if}

<style>
  .buy {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.3rem;
    margin-top: 0.4rem;
  }

  .lead {
    color: var(--text-dim);
    font-size: 0.85em;
  }

  ul {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  button {
    font-size: 0.82em;
    padding: 0.1rem 0.45rem;
  }

  .covers {
    flex-basis: 100%;
    margin: 0.15rem 0 0;
    font-size: 0.75em;
    color: var(--text-dim);
  }

  .error {
    flex-basis: 100%;
    margin: 0;
    color: var(--danger);
  }
</style>
