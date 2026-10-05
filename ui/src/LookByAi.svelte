<script lang="ts">
  /**
   * §122 step 6: let the DJ's AI provider pick each record's look. One switch
   * for every screen, kept in Rust; refused, and said why, with no provider
   * set up. See `docs/VISUALS.md`.
   */
  import { lookAi, type LookChosen } from "./api";

  let { on, onchange }: { on: boolean; onchange: (chosen: LookChosen) => void } = $props();
  let refused = $state<string | null>(null);

  async function set(box: HTMLInputElement) {
    try {
      refused = null;
      onchange(await lookAi(box.checked));
    } catch (e) {
      refused = String(e);
      box.checked = on;
    }
  }
</script>

<label class="by-ai">
  <input type="checkbox" checked={on} onchange={(e) => void set(e.currentTarget)} data-look-ai />
  Your AI picks the look for each record
</label>
{#if refused}
  <p class="hint" role="alert">{refused}</p>
{:else if on}
  <p class="hint" data-look-ai-note>
    Each record's title, artist, genre, tempo, key and energy go to your AI provider once, with the looks'
    names and what each is for. The look it picks is kept for that record; until it answers, and whenever it
    cannot, the look chosen here shows.
  </p>
{/if}

<style>
  .by-ai {
    display: flex;
    align-items: center;
    gap: 0.4em;
  }
</style>
