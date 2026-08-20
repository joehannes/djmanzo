<script lang="ts">
  import type { PadState } from "./grammar";
  import { theme as globalTheme } from "../theme.svelte";
  import { executeThemePipeline } from "./themes/engine";
  import SvgRenderer from "./SvgRenderer.svelte";

  interface Props {
    active?: boolean;
    disabled?: boolean;
    label?: string;
    width?: number;
    height?: number;
    onclick?: () => void;
    onpointerdown?: (e: PointerEvent) => void;
    onpointerup?: (e: PointerEvent) => void;
    onpointerleave?: (e: PointerEvent) => void;
  }

  let {
    active = false,
    disabled = false,
    label,
    width = 60,
    height = 40,
    onclick,
    onpointerdown,
    onpointerup,
    onpointerleave
  }: Props = $props();

  let pressed = $state(false);

  let shape: PadState = $derived({
    active, pressed, disabled, width, height, label
  });

  let renderState = $derived(executeThemePipeline(globalTheme.activePackage, shape));
  
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
  <SvgRenderer {renderState} {width} {height} fill />
  <!--
    The label is HTML over the SVG rather than an SVG `<text>`: it inherits the
    interface's font and colour tokens for free, and it stays legible when the
    pad is stretched, which `preserveAspectRatio="none"` would otherwise
    distort. Without this the transport pads rendered as four blank rectangles.
  -->
  {#if label}
    <span class="label">{label}</span>
  {/if}
</div>

<style>
  .pad-container {
    position: relative;
    display: block;
    width: 100%;
    user-select: none;
    cursor: pointer;
    outline: none;
  }
  .label {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.75em;
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--text);
    pointer-events: none;
  }
  .disabled {
    opacity: 0.5;
    pointer-events: none;
  }
  .pad-container:focus-visible {
    /* See SvgKnob: a child component's class cannot be styled from here. */
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 4px;
  }
</style>
