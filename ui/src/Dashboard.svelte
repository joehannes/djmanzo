<script lang="ts">
  /**
   * §117: the dashboard — every destination the toolbars held, on one screen
   * called up with `0` and dismissed the same way.
   *
   * > instead of toolbards let there be an central dashboard view with easy
   * > nav buttons for packages/presets/activities/overview ... even different
   * > dashboards per purpose/activity ... and auto-arrangeable dashboards
   *
   * What is on it, in which order, and what each tile runs are Rust's
   * (`dj_app::dashboard`): the activity's own section first, the activities
   * on their digits, then panels, presets, themes and workspaces arranged by
   * how often the DJ reaches for them. This draws it and hands a pressed tile
   * back to be run the way its key would run it.
   */
  import type { Dashboard, DashboardTile } from "./api";

  let {
    board,
    onrun,
    onclose,
  }: {
    board: Dashboard;
    onrun: (tile: DashboardTile) => void;
    onclose: () => void;
  } = $props();
</script>

<div class="scrim" aria-hidden="true" onclick={onclose}></div>
<div class="dashboard" role="dialog" aria-label="Dashboard" data-dashboard>
  <header>
    <h2>Dashboard</h2>
    <span class="how"><kbd>0</kbd> or <kbd>Esc</kbd> to close · <kbd>Space</kbd> for keys</span>
  </header>
  <div class="sections">
    {#each board.sections as section, s (section.title)}
      <section class:first={s === 0} aria-label={section.title}>
        <h3>{section.title}</h3>
        <ul>
          {#each section.tiles as tile (tile.id)}
            <li>
              <button type="button" data-tile={tile.id} title={tile.about} onclick={() => onrun(tile)}>
                <span class="label">{tile.label}</span>
                {#if tile.key}<kbd aria-label={`key ${tile.key}`}>{tile.key}</kbd>{/if}
                {#if tile.about}<span class="about">{tile.about}</span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: var(--scrim, rgb(0 0 0 / 0.45));
  }

  .dashboard {
    position: fixed;
    inset: 3.5rem 1rem 1rem;
    z-index: 61;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0.8rem 1rem 1rem;
    overflow: auto;
    border: 1px solid var(--border-strong, var(--border));
    border-radius: var(--radius, 0.6rem);
    background:
      linear-gradient(var(--panel-raised), var(--panel-raised)),
      var(--bg);
    box-shadow: 0 1rem 3rem rgb(0 0 0 / 0.5);
    color: var(--text);
  }

  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
  }

  h2 {
    margin: 0;
    font-size: 1.05rem;
  }

  .how {
    color: var(--text-dim);
    font-size: 0.8rem;
  }

  .sections {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(19rem, 1fr));
    gap: 1rem 1.25rem;
    align-items: start;
  }

  /* The activity's own section, and the activities, read first and wide. */
  section.first,
  section:nth-child(2) {
    grid-column: 1 / -1;
  }

  h3 {
    margin: 0 0 0.4rem;
    font-size: 0.72rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  ul {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(12.5rem, 1fr));
    gap: 0.4rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  button {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0.15rem 0.5rem;
    width: 100%;
    min-height: 3.1rem;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-s, 0.35rem);
    background: var(--panel);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  button:hover,
  button:focus-visible {
    border-color: var(--selected);
    background: var(--panel-hover);
  }

  .label {
    font-weight: 600;
    font-size: 0.88rem;
  }

  .about {
    grid-column: 1 / -1;
    overflow: hidden;
    color: var(--text-dim);
    font-size: 0.72rem;
    line-height: 1.3;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  kbd {
    align-self: start;
    min-width: 1.4em;
    padding: 0 0.3em;
    border: 1px solid var(--border-strong, var(--border));
    border-radius: 0.25rem;
    background: var(--panel-raised);
    font-family: var(--mono, monospace);
    font-size: 0.75rem;
    text-align: center;
  }
</style>
