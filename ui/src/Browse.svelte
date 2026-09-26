<script lang="ts">
  /**
   * Finding music: the collection, with every other configured source one
   * tick away in the same search.
   *
   * §124: *"there shall be no extra view for external sources, but the regular
   * search shall have a tickable toggle or something to include external
   * sources"*. This was two tabs, the collection and "Sources", each with its
   * own search box; now it is the collection, whose search has a tick that
   * brings the other sources' answers in below its own (`SourceResults`).
   */
  import Library from "./Library.svelte";

  let {
    enabled,
    deckCount = 2,
  }: { enabled: boolean; deckCount?: number } = $props();
</script>

<section class="browse">
  <Library {enabled} {deckCount} />
</section>

<style>
  .browse {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    flex: 1;
    min-height: 0;
  }
</style>
