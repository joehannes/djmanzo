<script lang="ts">
  import { iconGlyph, normalizeIconName } from "./icons";

  let {
    name,
    size = "1.05rem",
  }: {
    /** Either `gear` or the `fa-solid fa-gear` form the call sites use. */
    name: string;
    size?: string;
  } = $props();

  let glyph = $derived(iconGlyph(name));
</script>

<!--
  `aria-hidden` throughout: an icon button carries its meaning in the `title`
  and `aria-label` its parent sets. Announcing the drawing as well would read
  the control out twice.
-->
{#if glyph}
  <svg
    class="icon"
    style="--icon-size: {size}"
    viewBox="0 0 24 24"
    fill={glyph.fill ? "currentColor" : "none"}
    stroke={glyph.fill ? "none" : "currentColor"}
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path d={glyph.d} />
  </svg>
{:else}
  <!--
    An unknown name shows the first letter rather than nothing. A blank square
    is what the CDN failure looked like, and it told nobody anything; a letter
    at least says which control is missing its drawing.
  -->
  <span class="fallback" style="--icon-size: {size}" aria-hidden="true">
    {normalizeIconName(name).charAt(0).toUpperCase() || "?"}
  </span>
{/if}

<style>
  .icon {
    width: var(--icon-size);
    height: var(--icon-size);
    display: block;
    flex: none;
  }
  .fallback {
    width: var(--icon-size);
    height: var(--icon-size);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: calc(var(--icon-size) * 0.8);
    font-weight: 700;
    opacity: 0.8;
  }
</style>
