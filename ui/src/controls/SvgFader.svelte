<script lang="ts">
  import type { SessionContext } from "../api";
  import type { FaderState } from "./grammar";
  import * as BaseTheme from "./themes/BaseTheme.svelte";

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
    width?: number;
    height?: number;
    orientation?: "vertical" | "horizontal";
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
    width = 30,
    height = 120,
    orientation = "vertical",
    theme = BaseTheme
  }: Props = $props();

  let dragging = $state(false);
  let startMouse = $state(0);
  let startVal = $state(0);
  let container: HTMLElement;

  let normalized = $derived((value - min) / (max - min));
  
  let state: FaderState = $derived({
    value, min, max, normalized, dragging, disabled, width, height, orientation, label, context
  });

  function clamp(v: number) {
    return Math.max(min, Math.min(max, v));
  }

  function handlePointerDown(e: PointerEvent) {
    if (disabled) return;
    dragging = true;
    startMouse = orientation === "vertical" ? e.clientY : e.clientX;
    startVal = value;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
  }

  function handlePointerMove(e: PointerEvent) {
    if (!dragging || disabled || !container) return;
    
    const currentMouse = orientation === "vertical" ? e.clientY : e.clientX;
    const deltaMouse = currentMouse - startMouse;
    
    const travel = orientation === "vertical" ? height : width;
    const direction = orientation === "vertical" ? -1 : 1;
    
    const deltaVal = (deltaMouse / travel) * (max - min) * direction;
    
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
    const increment = step * 10;
    if (e.key === "ArrowUp" || e.key === "ArrowRight") nextVal = clamp(value + increment);
    if (e.key === "ArrowDown" || e.key === "ArrowLeft") nextVal = clamp(value - increment);
    if (nextVal !== value && oninput) {
      oninput(nextVal);
    }
  }
</script>

<div 
  class="fader-container" 
  class:disabled
  bind:this={container}
  role="slider"
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={value}
  aria-label={label || "fader"}
  tabindex={disabled ? -1 : 0}
  onpointerdown={handlePointerDown}
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
  onpointercancel={handlePointerUp}
  ondblclick={ondblclick}
  onkeydown={handleKeyDown}
>
  {#if label && orientation === "vertical"}
    <span class="label">{label}</span>
  {/if}
  
  <div class="renderer" style="width: {width}px; height: {height}px;">
    {@render theme.fader(state)}
  </div>

  {#if label && orientation === "horizontal"}
    <span class="label">{label}</span>
  {/if}
</div>

<style>
  .fader-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    user-select: none;
    cursor: pointer;
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
  .fader-container:focus-visible .renderer {
    box-shadow: 0 0 0 2px var(--accent);
  }
</style>
