<script lang="ts">
  import { dispatch, formatTime, loadTrack, type DeckState } from "./api";
  import Waveform from "./Waveform.svelte";
  import { open } from "@tauri-apps/plugin-dialog";

  let {
    deck,
    enabled,
    cueAvailable = false,
  }: { deck: DeckState; enabled: boolean; cueAvailable?: boolean } = $props();

  let title = $state<string>("");
  let artist = $state<string>("");
  let error = $state<string | null>(null);
  let loading = $state(false);

  const progress = $derived(
    deck.length_frames > 0 ? deck.position_frames / deck.length_frames : 0,
  );

  async function pickTrack() {
    error = null;
    const path = await open({
      multiple: false,
      filters: [
        {
          name: "Audio",
          extensions: ["mp3", "flac", "wav", "aiff", "aif", "ogg", "m4a", "aac", "opus"],
        },
      ],
    });
    if (typeof path !== "string") return;

    loading = true;
    try {
      const track = await loadTrack(deck.number, path);
      title = track.title;
      artist = track.artist ?? "";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  const send = async (action: string) => {
    try {
      await dispatch(action);
      error = null;
    } catch (e) {
      error = String(e);
    }
  };
</script>

<section class="deck" class:playing={deck.playing}>
  <header>
    <span class="number">{deck.number}</span>
    <div class="meta">
      <div class="title" title={title}>{title || "— no track —"}</div>
      <div class="artist">{artist || (deck.loaded ? "" : "load a file to begin")}</div>
    </div>
    <button onclick={pickTrack} disabled={!enabled || loading}>
      {loading ? "Loading…" : "Load"}
    </button>
  </header>

  <!--
    Tiles come from the Rust renderer and are scrolled by a CSS transform;
    nothing here draws. See docs/adr/0004-waveform-rendering-strategy.md.
  -->
  <Waveform {deck} height={96} />

  <div class="progress" role="progressbar" aria-valuenow={progress * 100}>
    <div class="fill" style:width="{Math.min(progress, 1) * 100}%"></div>
  </div>

  <div class="times mono">
    <span>{formatTime(deck.position_seconds)}</span>
    <span class="remaining">
      -{formatTime(Math.max(0, deck.length_seconds - deck.position_seconds))}
    </span>
  </div>

  <div class="transport">
    <button onclick={() => send(`deck ${deck.number} cue`)} disabled={!enabled || !deck.loaded}>
      Cue
    </button>
    <button
      class:active={deck.playing}
      onclick={() => send(`deck ${deck.number} play_pause`)}
      disabled={!enabled || !deck.loaded}
    >
      {deck.playing ? "Pause" : "Play"}
    </button>
    <button onclick={() => send(`deck ${deck.number} eject`)} disabled={!enabled || !deck.loaded}>
      Eject
    </button>
  </div>

  <!--
    Pre-fader listen. Deliberately explains itself when unavailable rather than
    just sitting greyed out: a 2-channel laptop output has nowhere to send a
    cue, and "why is this dead" is a bad thing to wonder mid-set.
  -->
  <button
    class="cue"
    class:on={deck.cue_enabled}
    disabled={!enabled || !cueAvailable}
    onclick={() => send(`deck ${deck.number} cue_toggle`)}
    title={cueAvailable
      ? "Pre-fader listen — hear this deck in the headphones"
      : "Needs a 4-channel output device"}
  >
    <span>CUE</span>
    <span class="pfl-meter" aria-hidden="true">
      <span class="pfl-fill" style:width="{Math.min(deck.pre_fader_level, 1) * 100}%"></span>
    </span>
  </button>

  <!--
    Isolator EQ: each knob runs from a true kill at 0 to +12 dB. Double-click
    resets to unity, because reaching for exactly 1.00 with a mouse mid-mix is
    not a thing anyone can do.
  -->
  <div class="eq">
    {#each [{ id: "eq_high", label: "HI", value: deck.eq_high }, { id: "eq_mid", label: "MID", value: deck.eq_mid }, { id: "eq_low", label: "LOW", value: deck.eq_low }] as band (band.id)}
      <label class="band" class:killed={band.value < 0.001}>
        <span>{band.label}</span>
        <input
          type="range"
          min="0"
          max="4"
          step="0.01"
          value={band.value}
          disabled={!enabled}
          oninput={(e) => send(`deck ${deck.number} ${band.id} ${e.currentTarget.value}`)}
          ondblclick={() => send(`deck ${deck.number} ${band.id} 1`)}
        />
        <button
          class="kill"
          class:on={band.value < 0.001}
          disabled={!enabled}
          onclick={() => send(`deck ${deck.number} ${band.id} ${band.value < 0.001 ? 1 : 0}`)}
          title="Kill {band.label}"
          aria-label="Kill {band.label}"
        ></button>
      </label>
    {/each}
  </div>

  <label class="control">
    <span>
      Filter
      <em class="mono">
        {#if Math.abs(deck.filter) <= 0.02}off{:else if deck.filter < 0}LP {Math.round(
            -deck.filter * 100,
          )}%{:else}HP {Math.round(deck.filter * 100)}%{/if}
      </em>
    </span>
    <input
      type="range"
      min="-1"
      max="1"
      step="0.01"
      value={deck.filter}
      disabled={!enabled}
      oninput={(e) => send(`deck ${deck.number} filter ${e.currentTarget.value}`)}
      ondblclick={() => send(`deck ${deck.number} filter 0`)}
    />
  </label>

  <label class="control">
    <span>Volume <em class="mono">{deck.volume.toFixed(2)}</em></span>
    <input
      type="range"
      min="0"
      max="1"
      step="0.01"
      value={deck.volume}
      disabled={!enabled}
      oninput={(e) => send(`deck ${deck.number} volume ${e.currentTarget.value}`)}
    />
  </label>

  <label class="control">
    <span>Pitch <em class="mono">{(deck.pitch * 100).toFixed(1)}%</em></span>
    <input
      type="range"
      min="-0.16"
      max="0.16"
      step="0.001"
      value={deck.pitch}
      disabled={!enabled}
      oninput={(e) => send(`deck ${deck.number} pitch ${e.currentTarget.value}`)}
    />
  </label>

  <div class="meter" aria-label="deck level">
    <div class="meter-fill" style:width="{Math.min(deck.peak, 1) * 100}%"></div>
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}
</section>

<style>
  .deck {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.9rem;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
    min-width: 0;
  }

  .deck.playing {
    border-color: color-mix(in srgb, var(--accent-2) 45%, var(--border));
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    min-width: 0;
  }

  .number {
    font-size: 1.5rem;
    font-weight: 700;
    color: var(--accent);
    width: 1.4rem;
    text-align: center;
    flex: none;
  }

  .meta {
    flex: 1;
    min-width: 0;
  }

  .title,
  .artist {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title {
    font-weight: 600;
  }

  .artist {
    color: var(--text-dim);
    font-size: 0.85em;
  }

  .progress {
    height: 8px;
    background: var(--panel-raised);
    border-radius: 4px;
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
  }

  .times {
    display: flex;
    justify-content: space-between;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .remaining {
    color: var(--warn);
  }

  .transport {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.4rem;
  }

  .control {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .control em {
    font-style: normal;
    color: var(--text);
  }

  .eq {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .band {
    display: grid;
    grid-template-columns: 2.2rem 1fr auto;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75em;
    color: var(--text-dim);
    letter-spacing: 0.05em;
  }

  .band.killed span {
    color: var(--danger);
  }

  .kill {
    width: 14px;
    height: 14px;
    padding: 0;
    border-radius: 3px;
    background: var(--panel-raised);
  }

  .kill.on {
    background: var(--danger);
    border-color: var(--danger);
  }

  .cue {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75em;
    letter-spacing: 0.08em;
    font-weight: 600;
  }

  .cue.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #0e0f14;
  }

  .pfl-meter {
    flex: 1;
    height: 4px;
    background: #0e0f1466;
    border-radius: 2px;
    overflow: hidden;
  }

  .pfl-fill {
    display: block;
    height: 100%;
    background: var(--accent-2);
  }

  .meter {
    height: 4px;
    background: var(--panel-raised);
    border-radius: 2px;
    overflow: hidden;
  }

  .meter-fill {
    height: 100%;
    background: var(--accent-2);
    /* No CSS transition: a level meter that lags is a lying level meter. */
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 0.8em;
  }
</style>
