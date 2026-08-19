<script lang="ts">
  import type { SessionContext } from "../api";
  import type { PadState } from "./grammar";
  import * as BaseTheme from "./themes/BaseTheme.svelte";

  interface Props {
    context: SessionContext;
    active?: boolean;
    disabled?: boolean;
    label?: string;
    width?: number;
    height?: number;
    onclick?: () => void;
    onpointerdown?: (e: PointerEvent) => void;
    onpointerup?: (e: PointerEvent) => void;
    onpointerleave?: (e: PointerEvent) => void;
    theme?: any;
  }

  let {
    context,
    active = false,
    disabled = false,
    label,
    width = 60,
    height = 40,
    onclick,
    onpointerdown,
    onpointerup,
    onpointerleave,
    theme = BaseTheme
  }: Props = $props();

  let pressed = $state(false);

  let state: PadState = $derived({
    active, pressed, disabled, width, height, label, context
  });
  
  function handlePointerDown(e: PointerEvent) {
    if (disabled) return;
    pressed = true;
    if (onpointerdown) onpointerdown(e);
  }

  function handlePointerUp(e: PointerEvent) {
    if (disabled) return;
    pressed = false;
    if (onpointerup) onpointerup(e);
  }

  function handlePointerLeave(e: PointerEvent) {
    if (disabled) return;
    pressed = false;
    if (onpointerleave) onpointerleave(e);
  }
  
  function handleKeyDown(e: KeyboardEvent) {
    if (disabled) return;
    if (e.key === "Enter" || e.key === " ") {
      pressed = true;
      if (onclick) onclick();
      setTimeout(() => pressed = false, 100);
    }
  }

</script>

<div 
  class="pad-container" 
  class:disabled
  role="button"
  aria-pressed={active}
  aria-label={label || "pad"}
  tabindex={disabled ? -1 : 0}
  {onclick}
  onpointerdown={handlePointerDown}
  onpointerup={handlePointerUp}
  onpointerleave={handlePointerLeave}
  onpointercancel={handlePointerLeave}
  onkeydown={handleKeyDown}
>
  <div class="renderer" style="width: {width}px; height: {height}px;">
    {@render theme.pad(state)}
  </div>
</div>

<style>
  .pad-container {
    display: inline-flex;
    user-select: none;
    cursor: pointer;
    outline: none;
  }
  .disabled {
    opacity: 0.5;
    pointer-events: none;
  }
  .pad-container:focus-visible .renderer {
    box-shadow: 0 0 0 2px var(--accent);
    border-radius: 4px;
  }
</style>
