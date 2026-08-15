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
  import { applyPreset, listPresets, presetFolder, type PresetItem, type PresetPack } from "./api";

  let { enabled, deckCount = 2 }: { enabled: boolean; deckCount?: number } = $props();

  let packs = $state<PresetPack[]>([]);
  let folder = $state("");
  let deck = $state(1);
  let applied = $state<string[]>([]);
  let appliedFrom = $state<string | null>(null);
  let error = $state<string | null>(null);
  let expanded = $state<Record<string, boolean>>({});

  $effect(() => {
    void listPresets().then((p) => (packs = p));
    void presetFolder().then((f) => (folder = f));
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
              <button
                class="peek"
                onclick={() => (expanded[preset.id] = !expanded[preset.id])}
                aria-label="Show what {preset.name} does"
              >
                {expanded[preset.id] ? "−" : "?"}
              </button>
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
    color: #0e0f14;
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
    box-shadow: 0 8px 24px #0008;
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
