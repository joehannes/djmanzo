<!--
  §130: what stands in a panel's place in an edition without it.

  docs/EDITIONS.md, "A look at what more there is": a quiet card saying what
  the part does and the cheapest edition that has it. Never a broken button,
  never a pop-up — it is drawn where the panel would be, and only there.
-->
<script lang="ts">
  import type { Missing } from "./edition.svelte";

  let { missing }: { missing: Missing } = $props();
</script>

<div class="quiet-card" data-quiet-card={missing.module}>
  <h4>{missing.title}</h4>
  <p>{missing.about}</p>
  <p class="where">In {missing.edition}, and in djmanzo Full.</p>
</div>

<style>
  .quiet-card {
    padding: 0.8rem 1rem;
    color: var(--text-dim, inherit);
    border: 1px dashed color-mix(in srgb, currentColor 30%, transparent);
    border-radius: 6px;
    margin: 0.5rem;
  }
  h4 {
    margin: 0 0 0.3rem;
    color: var(--text, inherit);
  }
  p {
    margin: 0.2rem 0;
    line-height: 1.4;
  }
  .where {
    opacity: 0.8;
  }
</style>
