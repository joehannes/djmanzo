<script lang="ts">
  /**
   * §121: where next, from a board.
   *
   * > quick links to workflow like current activities as of different
   * > dashboards
   *
   * The activities on their digits, which take the DJ back to the decks
   * arranged for that job, and the other boards. What each is and does is
   * Rust's; this draws them and reports a press.
   */
  import type { Activity, Board } from "./api";
  import Icon from "./controls/Icon.svelte";

  let {
    activities,
    boards,
    current,
    onactivity,
    onboard,
  }: {
    activities: Activity[];
    boards: Board[];
    current: string | null;
    onactivity: (slug: string) => void;
    onboard: (slug: string) => void;
  } = $props();
</script>

<div class="links">
  <section aria-label="Activities">
    <h4>Back to the decks, for</h4>
    <ul>
      {#each activities.slice(0, 9) as activity, i (activity.slug)}
        <li>
          <button type="button" title={activity.doing} onclick={() => onactivity(activity.slug)}>
            <kbd>{i + 1}</kbd> {activity.title}
          </button>
        </li>
      {/each}
    </ul>
  </section>
  <section aria-label="Other dashboards">
    <h4>Other dashboards</h4>
    <ul>
      {#each boards.filter((b) => b.slug !== current) as board (board.slug)}
        <li>
          <button type="button" title={board.about} onclick={() => onboard(board.slug)}>
            <Icon name={board.glyph} size="0.9rem" /> {board.title}
          </button>
        </li>
      {/each}
    </ul>
  </section>
</div>

<style>
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    padding: 0.3rem;
  }

  section {
    flex: 1 1 16rem;
  }

  h4 {
    margin: 0 0 0.4rem;
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  button {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.85rem;
  }
</style>
