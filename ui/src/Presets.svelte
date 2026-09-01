<script lang="ts">
  /**
   * Preset packs.
   *
   * Every preset shows exactly what it will run before you press it, and says
   * what it did afterwards. A preset that silently changes eight things is the
   * kind of feature people stop trusting — and since these are just actions on
   * the same bus, anything one sets can be changed back with the control that
   * owns it.
   */
  import IconButton from "./controls/IconButton.svelte";
  import { applyPreset, listPresets, presetFolder, type PresetItem, type PresetPack } from "./api";

  let { enabled, deckCount = 2 }: { enabled: boolean; deckCount?: number } = $props();

  let packs = $state<PresetPack[]>([]);
  let folder = $state("");
  let deck = $state(1);
  let applied = $state<string[]>([]);
  let appliedFrom = $state<string | null>(null);
  let error = $state<string | null>(null);
  let expanded = $state<Record<string, boolean>>({});

  /**
   * Whether the first ask has come back yet.
   *
   * Without it there is no way to tell "no packs" from "not asked yet", and
   * the panel showed the same nothing for both.
   */
  let asked = $state(false);

  $effect(() => {
    void listPresets()
      .then((p) => {
        packs = p;
      })
      // A bare `.then` was swallowing this. The panel then showed a header and
      // an empty space with no packs, no message and nothing to press -- the
      // exact dead end the polish pass exists to remove, and unreadable as a
      // failure because it looks identical to having no packs.
      .catch((problem) => {
        error = `could not read the presets: ${problem}`;
      })
      .finally(() => {
        asked = true;
      });
    void presetFolder()
      .then((f) => (folder = f))
      .catch(() => {
        // Where user packs go is a nicety; failing to learn it is not worth a
        // second error line under the first one.
      });
  });

  async function run(preset: PresetItem) {
    error = null;
    try {
      applied = await applyPreset(preset.id, preset.per_deck ? deck : undefined);
      appliedFrom = preset.name;
    } catch (e) {
      error = String(e);
      applied = [];
      appliedFrom = null;
    }
  }
</script>

<section class="presets">
  <header>
    <span class="title">Presets</span>
    <label class="deck-pick">
      Deck
      <select bind:value={deck}>
        {#each Array.from({ length: deckCount }, (_, i) => i + 1) as n (n)}
          <option value={n}>{n}</option>
        {/each}
      </select>
    </label>
    {#if appliedFrom}
      <span class="applied mono">{appliedFrom}: {applied.join(" · ")}</span>
    {/if}
  </header>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <div class="packs">
    {#each packs as pack (pack.id)}
      <article class="pack">
        <h4>
          {pack.name}
          {#if pack.user}<span class="badge">yours</span>{/if}
        </h4>
        <p class="description">{pack.description}</p>

        <div class="grid">
          {#each pack.presets as preset (preset.id)}
            <div class="preset">
              <button
                class="apply {preset.category}"
                disabled={!enabled}
                onclick={() => run(preset)}
                title={preset.description}
              >
                {preset.name}
                {#if preset.per_deck}<em>{deck}</em>{/if}
              </button>
              <IconButton
                icon={expanded[preset.id] ? 'fa-solid fa-eye-slash' : 'fa-solid fa-eye'}
                title={expanded[preset.id] ? `Hide details for ${preset.name}` : `Show details for ${preset.name}`}
                onClick={() => (expanded[preset.id] = !expanded[preset.id])}
              />
              {#if expanded[preset.id]}
                <div class="detail">
                  <p>{preset.description}</p>
                  <!-- Exactly what it will run, before you press it. -->
                  <ul class="mono">
                    {#each preset.actions as action (action)}
                      <li>{action}</li>
                    {/each}
                  </ul>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      </article>
    {/each}
    {#if asked && packs.length === 0 && !error}
      <!--
        Reachable in principle: every built-in pack is compiled in, so this
        only shows if a future build ships none. It says so rather than
        showing an empty panel, because an empty panel reads as broken.
      -->
      <p class="hint">No preset packs. Even the built-in ones are missing, which is a bug rather than a setting.</p>
    {/if}
  </div>

  {#if folder}
    <p class="hint">
      Your own packs go in <span class="mono">{folder}</span> as JSON, and are
      checked when the app starts — a pack containing an action that does not
      parse is refused there rather than when you press it mid-set.
    </p>
  {/if}
</section>

<style>
  .presets {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    flex: 1;
    min-height: 0;
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.7rem;
  }

  .title {
    font-weight: 600;
  }

  .deck-pick {
    font-size: 0.78em;
    color: var(--text-dim);
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .applied {
    font-size: 0.72em;
    color: var(--accent-2);
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .packs {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }

  .pack {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.7rem 0.9rem;
  }

  h4 {
    margin: 0 0 0.2rem;
    font-size: 0.88rem;
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .badge {
    font-size: 0.62em;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
    background: var(--accent-2);
    color: var(--on-accent);
  }

  .description,
  .hint {
    margin: 0 0 0.5rem;
    font-size: 0.76em;
    line-height: 1.5;
    color: var(--text-dim);
  }

  .grid {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .preset {
    display: flex;
    align-items: flex-start;
    gap: 0.15rem;
    position: relative;
  }

  .apply {
    font-size: 0.78em;
    padding: 0.3rem 0.6rem;
  }

  .apply em {
    font-style: normal;
    opacity: 0.6;
    margin-left: 0.3rem;
  }

  /* Category colour, so a phase and an EQ move do not look alike. */
  .apply.phase {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
  }

  .apply.eq {
    border-color: color-mix(in srgb, var(--accent-2) 40%, var(--border));
  }

  .apply.move {
    border-color: color-mix(in srgb, var(--warn) 35%, var(--border));
  }

  .peek {
    padding: 0.3rem 0.35rem;
    font-size: 0.7em;
    opacity: 0.65;
  }

  .detail {
    position: absolute;
    top: 100%;
    left: 0;
    z-index: 5;
    width: 22rem;
    max-width: 70vw;
    background: var(--panel-raised);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.6rem 0.7rem;
    margin-top: 0.2rem;
    box-shadow: 0 8px 24px var(--scrim);
  }

  .detail p {
    margin: 0 0 0.4rem;
    font-size: 0.76em;
    line-height: 1.5;
    color: var(--text-dim);
  }

  .detail ul {
    margin: 0;
    padding-left: 1rem;
    font-size: 0.74em;
    color: var(--accent);
    user-select: text;
    -webkit-user-select: text;
  }

  .error {
    margin: 0;
    padding: 0.5rem 0.8rem;
    background: color-mix(in srgb, var(--danger) 12%, var(--panel));
    border: 1px solid var(--danger);
    border-radius: 8px;
    color: var(--danger);
    font-size: 0.82em;
  }
</style>
