<script lang="ts">
  import type { KnobState } from "./grammar";
  import { theme as globalTheme } from "../theme.svelte";
  import { executeThemePipeline } from "./themes/engine";
  import SvgRenderer from "./SvgRenderer.svelte";

  interface Props {
    value: number; // For playhead mapping (e.g. 0 to 1 loop, or infinite accumulating angle)
    size?: number;
    disabled?: boolean;
    ondrag?: (delta: number) => void;
  }

  let {
    value,
    size = 150,
    disabled = false,
    ondrag
  }: Props = $props();

  let dragging = $state(false);
  let startY = $state(0);
  
  // A fake state wrapper, since the Engine primarily expects KnobState for rotaries.
  let shape: KnobState = $derived({
    value, min: 0, max: 1, normalized: value % 1, angle: (value % 1) * 360, dragging, disabled, size, label: "JOG"
  });

  let renderState = $derived(executeThemePipeline(globalTheme.activePackage, shape));

  function handlePointerDown(e: PointerEvent) {
    if (disabled) return;
    dragging = true;
    startY = e.clientY;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
  }

  function handlePointerMove(e: PointerEvent) {
    if (!dragging || disabled) return;
    // Simple drag delta logic. Could be refined to proper radial drag.
    const deltaY = startY - e.clientY;
    if (ondrag) ondrag(deltaY);
    startY = e.clientY;
  }

  function handlePointerUp(e: PointerEvent) {
    if (disabled) return;
    dragging = false;
    const el = e.currentTarget as HTMLElement;
    if (el.hasPointerCapture(e.pointerId)) {
      el.releasePointerCapture(e.pointerId);
    }
  }

</script>

<div 
  class="jog-container"
  class:disabled
  role="slider"
  aria-valuemin={0}
  aria-valuemax={1}
  aria-valuenow={value}
  aria-label="Jog Wheel"
  tabindex={disabled ? -1 : 0}
  onpointerdown={handlePointerDown}
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
  onpointercancel={handlePointerUp}
  style="width: {size}px; height: {size}px;"
>
  <SvgRenderer {renderState} width={size} height={size} />
  <div class="center-cap"></div>
</div>

<style>
  .jog-container {
    position: relative;
    border-radius: 50%;
    cursor: grab;
    user-select: none;
    outline: none;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .jog-container:active {
    cursor: grabbing;
  }
  .disabled {
    opacity: 0.5;
    pointer-events: none;
  }
  .center-cap {
    position: absolute;
    width: 25%;
    height: 25%;
    border-radius: 50%;
    background: var(--panel-raised);
    box-shadow: inset 0 2px 4px rgba(0,0,0,0.5);
    pointer-events: none;
  }
  .jog-container:focus-visible .center-cap {
    box-shadow: 0 0 0 2px var(--accent), inset 0 2px 4px rgba(0,0,0,0.5);
  }
</style>
