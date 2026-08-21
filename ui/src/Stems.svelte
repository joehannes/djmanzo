<script lang="ts">
  import { dispatch } from "./api";

  let {
    deckNumber,
    muteState = [false, false, false, false],
    volumeState = [1.0, 1.0, 1.0, 1.0],
  }: {
    deckNumber: number;
    muteState?: boolean[];
    volumeState?: number[];
  } = $props();

  const STEM_LABELS = ["Vocals", "Drums", "Bass", "Other"];
  const STEM_KEYS = ["vocal", "drums", "bass", "other"];
  const STEM_COLORS = ["#0ea5e9", "#ef4444", "#a855f7", "#22c55e"]; // Blue, Red, Purple, Green

  function toggleMute(index: number) {
    dispatch(`deck ${deckNumber} stem_mute ${STEM_KEYS[index]}`);
  }

  function changeVolume(index: number, value: string) {
    const vol = parseFloat(value);
    dispatch(`deck ${deckNumber} stem_volume ${STEM_KEYS[index]}:${vol.toFixed(3)}`);
  }

  function macroAcapella() {
    dispatch(`deck ${deckNumber} stem_solo_on vocal`);
  }

  function macroInstrumental() {
    dispatch(`deck ${deckNumber} stem_mute vocal`); // Assuming unmuted before
  }

  function macroVocalFadeOut() {
    let vol = volumeState?.[0] ?? 1.0;
    const fadeInterval = setInterval(() => {
      vol -= 0.05;
      if (vol <= 0) {
        vol = 0;
        clearInterval(fadeInterval);
      }
      dispatch(`deck ${deckNumber} stem_volume vocal:${vol.toFixed(3)}`);
    }, 100);
  }
</script>

<div class="stems-module">
  <div class="stems-grid">
    {#each STEM_LABELS as name, i}
      <div class="stem-column">
        <button
          class="stem-pad"
          class:muted={muteState[i]}
          style="--stem-color: {STEM_COLORS[i]}"
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
            oninput={(e) => changeVolume(i, e.currentTarget.value)}
            class="stem-slider" 
          />
        </div>
      </div>
    {/each}
  </div>
  
  <div class="macros-row">
    <IconButton icon="fa-solid fa-microphone" title="Solo Vocals (Acapella)" onClick={macroAcapella} />
    <IconButton icon="fa-solid fa-guitar" title="Mute Vocals (Instrumental)" onClick={macroInstrumental} />
    <IconButton icon="fa-solid fa-hand" title="Gradually fade out vocals" onClick={macroVocalFadeOut} />
  </div>
</div>

<style>
  .stems-module {
    background: rgba(255, 255, 255, 0.03);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    padding: 8px;
    margin: 8px 0;
  }

  .stems-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }

  .stem-pad {
    position: relative;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    overflow: hidden;
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  }

  /* Active State (Glowing) */
  .stem-pad:not(.muted) {
    background: linear-gradient(145deg, rgba(255, 255, 255, 0.1), rgba(0, 0, 0, 0.2));
    border-color: var(--stem-color);
    box-shadow: 0 0 15px calc(var(--stem-color) + '40'), inset 0 0 10px calc(var(--stem-color) + '20');
  }

  .stem-pad:not(.muted) .label {
    color: #fff;
    text-shadow: 0 0 8px var(--stem-color);
  }

  /* Muted State (Dimmed) */
  .stem-pad.muted {
    background: rgba(0, 0, 0, 0.6);
    border-color: rgba(255, 255, 255, 0.05);
    box-shadow: none;
    opacity: 0.5;
  }

  .stem-pad.muted .label {
    color: rgba(255, 255, 255, 0.3);
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
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.05);
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
    background: #fff;
    border-radius: 50%;
    box-shadow: 0 0 4px rgba(0,0,0,0.5);
    cursor: grab;
  }
  
  .stem-slider::-webkit-slider-thumb:active {
    cursor: grabbing;
  }

  .macros-row {
    display: flex;
    justify-content: space-between;
    margin-top: 12px;
    gap: 8px;
  }

  .macro-btn {
    flex: 1;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.8);
    padding: 6px 0;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .macro-btn:hover {
    background: rgba(255, 255, 255, 0.15);
    color: #fff;
    transform: translateY(-1px);
  }

  .macro-btn:active {
    transform: translateY(1px);
  }
</style>
