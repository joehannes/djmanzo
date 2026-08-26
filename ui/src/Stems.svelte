<script lang="ts">
  import IconButton from "./controls/IconButton.svelte";
  import { dispatch, stemsStatus, type StemsStatus, type StemSwap } from "./api";
  import { onDestroy, onMount } from "svelte";

  let {
    deckNumber,
    muteState = [false, false, false, false],
    volumeState = [1.0, 1.0, 1.0, 1.0],
    eqState = [
      [1, 1, 1],
      [1, 1, 1],
      [1, 1, 1],
      [1, 1, 1],
    ],
    filterState = [0, 0, 0, 0],
    soloing = false,
    swap = null,
    deckCount = 2,
  }: {
    deckNumber: number;
    muteState?: boolean[];
    volumeState?: number[];
    /**
     * Per-stem EQ trim, low/mid/high, from the engine.
     *
     * The DJ's own setting rather than the effective coefficient: the deck's
     * EQ multiplies into it, and a knob showing the product would jump every
     * time the channel strip moved.
     */
    eqState?: number[][];
    /** Per-stem filter sweep, -1 low-pass .. 0 open .. +1 high-pass. */
    filterState?: number[];
    /** The stem swap in force anywhere, from the engine. */
    swap?: StemSwap | null;
    deckCount?: number;
    /**
     * Whether a stem solo is held on this deck.
     *
     * From the engine, not from a local flag, because a controller can hold a
     * solo too and a button that disagreed with the audio would be worse than
     * no button.
     */
    soloing?: boolean;
  } = $props();

  /**
   * Whether separation can run here.
   *
   * Asked once, on mount. It cannot change while the application is running:
   * the model is looked for at startup, so a DJ who installs one mid-set has
   * to restart -- and being told that is better than pads that quietly do
   * nothing.
   */
  let status = $state<StemsStatus>({ available: true, backend: null, reason: null });
  onMount(async () => {
    try {
      status = await stemsStatus();
    } catch (error) {
      status = {
        available: false,
        backend: null,
        reason: `could not ask about stems: ${error}`,
      };
    }
  });

  const STEM_LABELS = ["Vocals", "Drums", "Bass", "Other"];
  const STEM_KEYS = ["vocal", "drums", "bass", "other"];
  /**
   * One token per stem, not a hex value.
   *
   * These were four literal colours, which meant the stem pads were the only
   * controls in djmanzo that ignored the theme — the same colour on the light
   * palette, the industrial one and the cyber one. The tokens are defined in
   * the theme sheet beside every other accent.
   */
  const STEM_COLORS = [
    "var(--stem-vocal)",
    "var(--stem-drums)",
    "var(--stem-bass)",
    "var(--stem-other)",
  ];

  function toggleMute(index: number) {
    dispatch(`deck ${deckNumber} stem_mute ${STEM_KEYS[index]}`);
  }

  /** low, mid, high — matching `EqBand::ALL` on the engine side. */
  const BANDS = ["low", "mid", "high"];

  function changeEq(index: number, band: number, value: string) {
    const gain = parseFloat(value);
    dispatch(
      `deck ${deckNumber} stem_eq_${BANDS[band]} ${STEM_KEYS[index]}:${gain.toFixed(3)}`,
    );
  }

  function changeStemFilter(index: number, value: string) {
    const position = parseFloat(value);
    dispatch(
      `deck ${deckNumber} stem_filter ${STEM_KEYS[index]}:${position.toFixed(3)}`,
    );
  }

  /**
   * Put one stem's tone back to flat.
   *
   * Four knobs to return by hand is four chances to leave one slightly off,
   * and "slightly off" on a stem EQ is the kind of thing a DJ hears twenty
   * minutes later and cannot find.
   */
  function resetTone(index: number) {
    for (let band = 0; band < BANDS.length; band += 1) {
      dispatch(`deck ${deckNumber} stem_eq_${BANDS[band]} ${STEM_KEYS[index]}:1`);
    }
    dispatch(`deck ${deckNumber} stem_filter ${STEM_KEYS[index]}:0`);
  }

  /** True when this stem's tone is anywhere but flat, so the reset can say so. */
  function toneTouched(index: number): boolean {
    const eq = eqState?.[index] ?? [1, 1, 1];
    return (
      eq.some((gain) => Math.abs(gain - 1) > 0.001) ||
      Math.abs(filterState?.[index] ?? 0) > 0.001
    );
  }

  function changeVolume(index: number, value: string) {
    const vol = parseFloat(value);
    dispatch(`deck ${deckNumber} stem_volume ${STEM_KEYS[index]}:${vol.toFixed(3)}`);
  }

  /**
   * Acapella: hold the vocal alone, and let go again.
   *
   * **This latches.** The engine treats a solo as a held audition — it
   * snapshots the DJ's mutes on the way in and restores them on release, and
   * refuses every mute while one is held. Nothing in this panel ever sent the
   * release, so one click left the deck's whole stem section dead for the rest
   * of the set. A mouse cannot hold a button, so the second click is the
   * release.
   */
  function macroAcapella() {
    stopFade();
    dispatch(`deck ${deckNumber} stem_solo_${soloing ? "off" : "on"} vocal`);
  }

  /**
   * Instrumental: the vocal muted, whatever it was doing before.
   *
   * `stem_mute` is a toggle — right for a controller pad, wrong for a macro
   * that names an outcome, because it un-mutes a vocal that was already muted
   * and does the opposite of what the button says. `stem_mute_on` states it.
   */
  function macroInstrumental() {
    stopFade();
    // A held solo would refuse the mute outright, so it is released first.
    if (soloing) dispatch(`deck ${deckNumber} stem_solo_off vocal`);
    dispatch(`deck ${deckNumber} stem_mute_on vocal`);
  }

  /**
   * Which stem this deck is sending elsewhere, if any.
   *
   * There is one swap in the whole engine, so a panel shows it only when this
   * deck is the source — otherwise every deck would offer to cancel a swap it
   * has nothing to do with.
   */
  const sending = $derived(swap && swap.from === deckNumber ? swap : null);
  /** The deck a swap would go to. Anything but this one. */
  let target = $state<number | null>(null);
  $effect(() => {
    if (target === null || target === deckNumber) {
      target = deckNumber === 1 ? 2 : 1;
    }
  });

  function swapStem(index: number) {
    if (sending) {
      dispatch("stem_swap_off");
      return;
    }
    if (target === null) return;
    dispatch(`stem_swap ${STEM_KEYS[index]} ${deckNumber} ${target}`);
  }

  /** The running vocal fade, so a second click stops it rather than racing it. */
  let fade = $state<ReturnType<typeof setInterval> | null>(null);

  function stopFade() {
    if (fade !== null) {
      clearInterval(fade);
      fade = null;
    }
  }

  // Two fades writing the same parameter fight, and one left running after the
  // panel is gone keeps dispatching at a deck nobody is looking at.
  onDestroy(stopFade);

  const FADE_MS = 100;
  const FADE_STEP = 0.05;

  function macroVocalFadeOut() {
    if (fade !== null) {
      stopFade();
      return;
    }
    let vol = volumeState?.[0] ?? 1.0;
    fade = setInterval(() => {
      vol -= FADE_STEP;
      // `<= 0` alone leaves 2.8e-17 after twenty steps from 1.0, so the fade
      // runs one tick past the end before clamping. Round to the step.
      if (vol < FADE_STEP / 2) {
        vol = 0;
        stopFade();
      }
      dispatch(`deck ${deckNumber} stem_volume vocal:${vol.toFixed(3)}`);
    }, FADE_MS);
  }
</script>

<div class="stems-module" class:unavailable={!status.available}>
  {#if !status.available}
    <p class="stems-reason" role="status">
      {status.reason ?? "stem separation is unavailable"}
    </p>
  {:else if status.reason}
    <!--
      Separating, but with the fallback. Worth saying: the controls work, and
      a downloaded model would work better. Not an error, so it does not read
      as one.
    -->
    <p class="stems-reason" role="status">
      Using the {status.backend ?? "built-in"} separator — {status.reason}
    </p>
  {/if}
  <div class="stems-grid">
    {#each STEM_LABELS as name, i}
      <div class="stem-column">
        <button
          class="stem-pad"
          class:muted={muteState[i]}
          style="--stem-color: {STEM_COLORS[i]}"
          disabled={!status.available}
          onclick={() => toggleMute(i)}
          title="Toggle {name}"
        >
          <span class="label">{name}</span>
        </button>
        <div class="stem-slider-container" style="--stem-color: {STEM_COLORS[i]}">
          <div class="stem-meter" style="height: {volumeState[i] * 100}%"></div>
          <input 
            type="range" 
            min="0" 
            max="1" 
            step="0.01" 
            value={volumeState[i]} 
            disabled={!status.available}
            oninput={(e) => changeVolume(i, e.currentTarget.value)}
            class="stem-slider" 
          />
        </div>
        <!--
          This stem's own tone, on top of the deck's EQ rather than instead of
          it. Flat is 1.0 for the bands and 0.0 for the filter, so an untouched
          stem sits in the middle of every control and the deck's channel strip
          behaves exactly as it did before these existed.
        -->
        <div class="stem-tone" style="--stem-color: {STEM_COLORS[i]}">
          {#each ["Lo", "Md", "Hi"] as band, b (band)}
            <label class="tone-knob">
              <span>{band}</span>
              <input
                type="range"
                min="0"
                max="4"
                step="0.05"
                value={eqState?.[i]?.[b] ?? 1}
                disabled={!status.available}
                aria-label="{STEM_LABELS[i]} {['low', 'mid', 'high'][b]}"
                oninput={(e) => changeEq(i, b, e.currentTarget.value)}
              />
            </label>
          {/each}
          <label class="tone-knob">
            <span>Flt</span>
            <input
              type="range"
              min="-1"
              max="1"
              step="0.02"
              value={filterState?.[i] ?? 0}
              disabled={!status.available}
              aria-label="{STEM_LABELS[i]} filter"
              oninput={(e) => changeStemFilter(i, e.currentTarget.value)}
            />
          </label>
          <button
            class="tone-reset"
            class:active={toneTouched(i)}
            disabled={!status.available || !toneTouched(i)}
            title="Put {STEM_LABELS[i]} back to flat"
            onclick={() => resetTone(i)}
          >
            ⌀
          </button>
        </div>
      </div>
    {/each}
  </div>
  
  {#if status.available}
    <div class="swap-row">
      {#if sending}
        <span class="swap-note">
          {STEM_LABELS[sending.stem]} over deck {sending.to}
        </span>
        <IconButton
          icon="unlink"
          title="Put both decks back"
          active={true}
          onClick={() => swapStem(sending.stem)}
        />
      {:else}
        <span class="swap-note">Send a stem to deck</span>
        <select bind:value={target} aria-label="Which deck to send a stem to">
          {#each Array.from({ length: deckCount }, (_, i) => i + 1) as n (n)}
            {#if n !== deckNumber}
              <option value={n}>{n}</option>
            {/if}
          {/each}
        </select>
        {#each STEM_LABELS as name, i}
          <button
            class="swap-pick"
            style="--stem-color: {STEM_COLORS[i]}"
            title="Play this deck's {name.toLowerCase()} over deck {target}"
            onclick={() => swapStem(i)}
          >
            {name.slice(0, 2)}
          </button>
        {/each}
      {/if}
    </div>
  {/if}

  <div class="macros-row">
    <IconButton
      icon="fa-solid fa-microphone"
      title={soloing ? "Release the vocal solo" : "Solo Vocals (Acapella)"}
      active={soloing}
      disabled={!status.available}
      onClick={macroAcapella}
    />
    <IconButton
      icon="fa-solid fa-guitar"
      title="Mute Vocals (Instrumental)"
      disabled={!status.available}
      onClick={macroInstrumental}
    />
    <IconButton
      icon="fa-solid fa-hand"
      title={fade ? "Stop the fade" : "Gradually fade out vocals"}
      active={fade !== null}
      disabled={!status.available}
      onClick={macroVocalFadeOut}
    />
  </div>
</div>

<style>
  /* The tone row sits under each stem's fader, in that stem's colour, so a
     glance says which column a knob belongs to without reading a label. */
  .stem-tone {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    margin-top: 0.3rem;
  }

  .tone-knob {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.62rem;
    color: var(--text-dim);
  }

  .tone-knob span {
    width: 1.4em;
    flex: none;
    font-variant-numeric: tabular-nums;
  }

  .tone-knob input[type="range"] {
    accent-color: var(--stem-color);
    flex: 1;
    min-width: 0;
    height: 0.7rem;
  }

  .tone-reset {
    margin-top: 0.15rem;
    padding: 0.1rem 0;
    font-size: 0.7rem;
    line-height: 1;
  }

  /* Tokens, not white-on-black: `rgba(255,255,255,0.03)` is a panel on the
     dark theme and an invisible one on the light theme. */
  .stems-module {
    background: var(--panel-raised);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px;
    margin: 8px 0;
  }

  .stems-reason {
    margin: 0 0 8px;
    font-size: 0.78rem;
    line-height: 1.35;
    color: var(--text-dim);
  }

  .stems-module.unavailable .stems-grid {
    opacity: 0.45;
  }

  .stems-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }

  .stem-pad {
    position: relative;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    overflow: hidden;
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
    box-shadow: 0 4px 12px var(--scrim);
  }

  /* Active state.
     The glow was `calc(var(--stem-color) + '40')`, which is not CSS: `calc`
     cannot concatenate a colour with a string, so the whole declaration was
     invalid and the browser dropped it. The pads have never glowed.
     `color-mix` is how a colour is diluted; where it is unsupported the
     declaration is dropped exactly as before, so this cannot be worse. */
  .stem-pad:not(.muted) {
    background: var(--panel-hover);
    border-color: var(--stem-color);
    box-shadow:
      0 0 15px color-mix(in srgb, var(--stem-color) 25%, transparent),
      inset 0 0 10px color-mix(in srgb, var(--stem-color) 12%, transparent);
  }

  .stem-pad:not(.muted) .label {
    color: var(--text);
    text-shadow: 0 0 8px var(--stem-color);
  }

  /* Muted state */
  .stem-pad.muted {
    background: var(--panel);
    border-color: var(--border);
    box-shadow: none;
    opacity: 0.5;
  }

  .stem-pad.muted .label {
    color: var(--text-dim);
  }

  .label {
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    z-index: 1;
    transition: all 0.2s ease;
  }

  .stem-pad:hover {
    transform: translateY(-2px);
  }
  .stem-pad:active {
    transform: translateY(1px);
  }

  .stem-column {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .stem-slider-container {
    position: relative;
    height: 80px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 4px;
    overflow: hidden;
  }

  .stem-meter {
    position: absolute;
    bottom: 0;
    left: 0;
    width: 100%;
    background: var(--stem-color);
    opacity: 0.3;
    pointer-events: none;
    transition: height 0.1s ease-out;
  }

  .stem-slider {
    position: absolute;
    top: 0;
    left: 0;
    width: 80px;
    height: 100%;
    transform-origin: 40px 40px;
    transform: rotate(-90deg);
    appearance: none;
    background: transparent;
    cursor: pointer;
    margin: 0;
    outline: none;
  }

  .stem-slider::-webkit-slider-thumb {
    appearance: none;
    width: 16px;
    height: 16px;
    background: var(--text);
    border-radius: 50%;
    box-shadow: 0 0 4px var(--scrim);
    cursor: grab;
  }
  
  .stem-slider::-webkit-slider-thumb:active {
    cursor: grabbing;
  }

  .swap-row {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    margin-top: 6px;
    font-size: 0.68rem;
    color: var(--text-dim);
  }

  .swap-note {
    white-space: nowrap;
  }

  .swap-pick {
    border: 1px solid var(--stem-color);
    background: var(--panel);
    color: var(--text);
    border-radius: 4px;
    padding: 0.1rem 0.3rem;
    font-size: 0.65rem;
    cursor: pointer;
  }

  .swap-pick:hover {
    background: var(--panel-hover);
  }

  .macros-row {
    display: flex;
    justify-content: space-between;
    margin-top: 12px;
    gap: 8px;
  }
</style>
