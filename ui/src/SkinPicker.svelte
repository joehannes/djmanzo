<script lang="ts">
  /**
   * Karaoke beta B6: choose a skin for the theme being worn, or none.
   * Offered only the skins that suit the theme; Rust is not involved — a
   * skin is drawing, kept in this window's storage like the theme itself.
   */
  import { theme } from "./theme.svelte";
</script>

<section class="skin-picker" aria-label="Skin over the theme">
  <h4>Skin over the theme</h4>
  {#if theme.skins.length === 0}
    <p class="hint">No skin is made for {theme.chosen.name}.</p>
  {:else}
    <div class="choices" role="radiogroup" aria-label="Skin">
      <label>
        <input type="radio" name="skin" checked={theme.skin === null} onchange={() => theme.setSkin(null)} />
        None
      </label>
      {#each theme.skins as skin (skin.id)}
        <label title={skin.about}>
          <input
            type="radio"
            name="skin"
            data-skin-choice={skin.id}
            checked={theme.skin === skin.id}
            onchange={() => theme.setSkin(skin.id)}
          />
          {skin.name}
        </label>
      {/each}
    </div>
    <p class="hint">Faint artwork over the whole interface; it never takes a press, and the singers' screen never shows it.</p>
  {/if}
</section>

<style>
  .skin-picker h4 {
    margin: 0 0 0.3rem;
    font-size: 0.8rem;
  }

  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem 0.8rem;
    font-size: 0.78rem;
  }

  .hint {
    margin: 0.3rem 0 0;
    font-size: 0.7rem;
    color: var(--text-dim);
  }
</style>
