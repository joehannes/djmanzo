<script lang="ts">
  import type { SessionContext } from "../api";
  import type { KnobState } from "./grammar";
  import { theme as globalTheme } from "../theme.svelte";
  import { executeThemePipeline } from "./themes/engine";
  import SvgRenderer from "./SvgRenderer.svelte";

  interface Props {
    context: SessionContext;
    value: number;
    min: number;
    max: number;
    step?: number;
    label?: string;
    oninput?: (value: number) => void;
    ondblclick?: () => void;
    disabled?: boolean;
    size?: number;
    // Injectable theme, falls back to BaseTheme
    theme?: any; 
  }

  let {
    context,
    value,
    min,
    max,
    step = 0.01,
    label,
    oninput,
    ondblclick,
    disabled = false,
    size = 48
  }: Props = $props();

  let dragging = $state(false);
  let startY = $state(0);
  let startVal = $state(0);

  let normalized = $derived((value - min) / (max - min));
  let angle = $derived(-135 + normalized * 270);
  
  let state: KnobState = $derived({
    value, min, max, normalized, angle, dragging, disabled, size, label, context
  });

  let renderState = $derived(executeThemePipeline(globalTheme.activePackage, state));

  function clamp(v: number) {
    return Math.max(min, Math.min(max, v));
  }

  function handlePointerDown(e: PointerEvent) {
    if (disabled) return;
    dragging = true;
    startY = e.clientY;
    startVal = value;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
  }

  function handlePointerMove(e: PointerEvent) {
    if (!dragging || disabled) return;
    const deltaY = startY - e.clientY;
    const range = max - min;
    const deltaVal = (deltaY / 100) * range;
    
    let nextVal = clamp(startVal + deltaVal);
    nextVal = Math.round(nextVal / step) * step;
    
    if (nextVal !== value && oninput) {
      oninput(nextVal);
    }
  }

  function handlePointerUp(e: PointerEvent) {
    dragging = false;
    const el = e.currentTarget as HTMLElement;
    if (el.hasPointerCapture(e.pointerId)) {
      el.releasePointerCapture(e.pointerId);
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (disabled) return;
    let nextVal = value;
    if (e.key === "ArrowUp" || e.key === "ArrowRight") nextVal = clamp(value + step);
    if (e.key === "ArrowDown" || e.key === "ArrowLeft") nextVal = clamp(value - step);
    if (nextVal !== value && oninput) {
      oninput(nextVal);
    }
  }
</script>

<div 
  class="knob-container" 
  class:disabled
  role="slider"
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={value}
  aria-label={label || "knob"}
  tabindex={disabled ? -1 : 0}
  onpointerdown={handlePointerDown}
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
  onpointercancel={handlePointerUp}
  ondblclick={ondblclick}
  onkeydown={handleKeyDown}
  style="width: {size}px;"
>
  {#if label}
    <span class="label">{label}</span>
  {/if}
  
  <SvgRenderer {renderState} {size} />
</div>

<style>
  .knob-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    user-select: none;
    cursor: ns-resize;
    outline: none;
  }
  .disabled {
    opacity: 0.5;
    pointer-events: none;
  }
  .label {
    font-size: 0.75em;
    color: var(--text-dim);
    letter-spacing: 0.05em;
  }
  .knob-container:focus-visible .renderer {
    box-shadow: 0 0 0 2px var(--accent);
    border-radius: 50%;
  }
</style>
