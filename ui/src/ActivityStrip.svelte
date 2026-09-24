<script lang="ts">
  /**
   * §109: the activity strip.
   *
   * > create a different more interactive GUI mode ... where the current
   * > screen basically show the necessary (as of current activity)
   * > controls/widgets only ... then you can change activies
   *
   * It stands where the row of panel buttons stands in the full cockpit, and
   * in that row's place rather than beside it: entering activity mode swaps one
   * row for another of the same height, so the decks below never move — the
   * one thing a DJ mid-set must be able to count on.
   *
   * The strip decides nothing. Which activities exist, their keys, and what the
   * moment seems to call for are `dj_app::activity`'s; this draws them and
   * reports a press.
   */
  import type { Activity, ActivitySuggestion } from "./api";
  import Icon from "./controls/Icon.svelte";

  let {
    activities,
    current,
    suggestion,
    error = "",
    onchoose,
    onkeep,
    onforget,
    onleave,
  }: {
    activities: Activity[];
    current: string;
    /** What the assistant would suggest, if anything. Marked, never followed. */
    suggestion: ActivitySuggestion | null;
    /** Why the last activity was not kept, if it was not. */
    error?: string;
    onchoose: (slug: string) => void;
    onkeep: (title: string) => void;
    onforget: (slug: string) => void;
    onleave: () => void;
  } = $props();

  let naming = $state(false);
  let name = $state("");

  function keep() {
    const title = name.trim();
    if (!title) return;
    onkeep(title);
    naming = false;
    name = "";
  }

  /** A key as it is printed on the keyboard: `Digit3` is `3`. */
  function keyName(code: string): string {
    return code.startsWith("Digit") ? code.slice(5) : code;
  }

  /** The line a tab's tooltip carries: what it is for, its key, and why it is suggested. */
  function titleOf(activity: Activity): string {
    const key = activity.key ? ` (${keyName(activity.key)})` : "";
    const why =
      suggestion?.activity === activity.slug && activity.slug !== current
        ? `\nSuggested: ${suggestion.because}`
        : "";
    return `${activity.doing}${key}${why}`;
  }
</script>

<nav class="strip" aria-label="Activities" data-activity-strip>
  {#each activities as activity (activity.slug)}
    {@const suggested = suggestion?.activity === activity.slug && activity.slug !== current}
    <span class="tab-wrap">
      <button
        class="tab"
        class:current={activity.slug === current}
        class:suggested
        aria-pressed={activity.slug === current}
        data-activity={activity.slug}
        title={titleOf(activity)}
        onclick={() => onchoose(activity.slug)}
      >
        <Icon name={activity.icon} size="1.15rem" />
        <span class="name">{activity.title}</span>
        {#if activity.key}
          <kbd class="key" aria-hidden="true">{keyName(activity.key)}</kbd>
        {/if}
        {#if suggested}
          <!--
            §115: the assistant's suggestion, as a mark and nothing more. Not
            motion — §18's budget decides what may move, and a suggestion is
            never worth interrupting a mix for. The reason is in the tooltip
            and read out to a screen reader.
          -->
          <span class="hint" role="img" aria-label={`Suggested: ${suggestion?.because}`}>●</span>
        {/if}
      </button>
      {#if !activity.shipped}
        <button
          class="forget"
          aria-label={`Forget ${activity.title}`}
          title={`Forget ${activity.title}`}
          onclick={() => onforget(activity.slug)}
        >
          <Icon name="xmark" size="0.75rem" />
        </button>
      {/if}
    </span>
  {/each}

  {#if naming}
    <form
      class="naming"
      onsubmit={(event) => {
        event.preventDefault();
        keep();
      }}
    >
      <!-- svelte-ignore a11y_autofocus -->
      <input
        bind:value={name}
        placeholder="Name this activity"
        aria-label="Name for the new activity"
        autofocus
        onkeydown={(event) => {
          if (event.key === "Escape") {
            naming = false;
            name = "";
          }
        }}
      />
      <button type="submit" disabled={!name.trim()}>Keep</button>
    </form>
  {:else}
    <button
      class="add"
      title="Keep what is on screen now as an activity of your own"
      onclick={() => (naming = true)}
    >
      <Icon name="plus" size="0.95rem" />
      <span class="name">Keep as activity</span>
    </button>
  {/if}

  {#if error}
    <span class="refused" role="alert">{error}</span>
  {/if}

  <button
    class="leave"
    title="Back to the full cockpit, with every panel button"
    onclick={onleave}
  >
    <Icon name="expand" size="0.95rem" />
    <span class="name">Full cockpit</span>
  </button>
</nav>

<style>
  .strip {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem;
    min-width: 0;
  }

  .tab-wrap {
    position: relative;
    display: inline-flex;
  }

  /*
    Big enough to hit without looking, which is what a DJ switching mid-set is
    doing: the targets are wider than the panel buttons they replace, and the
    key is on the button so it can be learned from it.
  */
  .tab,
  .add,
  .leave {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    min-height: 2.25rem;
    padding: 0.3rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }

  .tab:hover,
  .add:hover,
  .leave:hover {
    background: var(--panel-hover);
  }

  /*
    The current one is filled, and bordered as well: §33 says colour alone
    must never carry state, and a DJ looking through haze reads the outline
    before the hue.
  */
  .tab.current {
    background: var(--selected);
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
    font-weight: 600;
  }

  .name {
    white-space: nowrap;
  }

  .key {
    font: inherit;
    font-size: 0.7em;
    padding: 0 0.3rem;
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--text-dim);
  }

  .hint {
    color: var(--accent);
    font-size: 0.7em;
    line-height: 1;
  }

  .tab.suggested {
    border-style: dashed;
    border-color: var(--accent);
  }

  .forget {
    position: absolute;
    top: -0.4rem;
    right: -0.4rem;
    width: 1.1rem;
    height: 1.1rem;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--bg);
    color: var(--text-dim);
    cursor: pointer;
  }

  .add,
  .leave {
    color: var(--text-dim);
  }

  .naming {
    display: inline-flex;
    gap: 0.3rem;
  }

  .naming input {
    min-width: 10rem;
  }

  .refused {
    color: var(--danger);
    font-size: 0.85em;
  }
</style>
