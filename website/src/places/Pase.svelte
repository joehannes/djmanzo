<script lang="ts">
  import type { PlaceId } from "../lib/places";
  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();
</script>

<p>Coming together.</p>
<button class="btn" onclick={() => go("muelle")}>Back to the dock</button>
