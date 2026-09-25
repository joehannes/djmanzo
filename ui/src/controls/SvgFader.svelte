<script lang="ts">
  import type { FaderState } from "./grammar";
  import { theme as globalTheme } from "../theme.svelte";
  import { executeThemePipeline } from "./themes/engine";
  import SvgRenderer from "./SvgRenderer.svelte";

  interface Props {
    value: number;
    min: number;
    max: number;
    step?: number;
    label?: string;
    /**
     * What a screen reader and a test call it, when that is longer than the
     * label drawn beside it: four knobs labelled "Lo" in four stem columns
     * are "Vocals low", "Drums low" and so on to anything that cannot see
     * the column.
     */
    name?: string;
    /** The value in words, shown under the label. A knob with no number
     *  beside it is a knob you cannot set to anything in particular. */
    readout?: string;
    oninput?: (value: number) => void;
    ondblclick?: () => void;
    /**
     * §29's *AI hover = suggestion*, from `dj_app::handle::suggested`.
     *
     * The same prop the knob takes, because a fader is a control with the same
     * gestures on it — §29's example is a knob and its list is about controls.
     * Absent means the plan says nothing about this fader, and then nothing is
     * drawn: a mark that is always there says nothing.
     */
    suggestion?: { to: number | null; action: string; because: string } | null;
    /** Send the suggestion's action, when a DJ takes it. */
    onoption?: (action: string) => void;
    disabled?: boolean;
    width?: number;
    height?: number;
    orientation?: "vertical" | "horizontal";
    /** §114: the value the fader rests at, which it fills from. Absent is `min`. */
    origin?: number;
  }

  let {
    value,
    min,
    max,
    step = 0.01,
    label,
    name,
    readout,
    oninput,
    ondblclick,
    suggestion = null,
    onoption,
    disabled = false,
    width = 30,
    height = 120,
    orientation = "vertical",
    origin
  }: Props = $props();

  /**
   * A shift-drag is a quarter of a drag, as on the knob (§29's level two).
   *
   * It matters more here than there. A fader's drag is its drawn height, so
   * the pitch fader's ±16% over a hundred pixels is a third of a percent a
   * pixel -- too coarse to hold two records together. A quarter of that is
   * the precision the taller fader it replaced never had either.
   */
  const FINE = 0.25;

  let dragging = $state(false);
  /** Where the pointer was at the last move, not at the press. */
  let lastMouse = 0;
  /** The value the drag has reached, before it is rounded to a step. */
  let reached = 0;
  let container: HTMLElement;

  /** Whether the suggestion is showing. Hover and focus, as on the knob. */
  let showing = $state(false);

  let normalized = $derived((value - min) / (max - min));
  
  let shape: FaderState = $derived({
    value, min, max, normalized, dragging, disabled, width, height, orientation, label,
    origin: origin === undefined ? undefined : (origin - min) / (max - min)
  });

  let renderState = $derived(executeThemePipeline(globalTheme.activePackage, shape));

  function clamp(v: number) {
    return Math.max(min, Math.min(max, v));
  }

  function handlePointerDown(e: PointerEvent) {
    if (disabled) return;
    // A press on one of the control's own buttons -- a menu entry, the
    // suggestion's "Do it" -- is that button's, not the start of a drag.
    // Capturing the pointer here sent the button's click to the control
    // instead, so no menu entry ever ran; the kill entry only seemed to,
    // because the EQ band's label passed every click on to its kill button.
    if ((e.target as Element | null)?.closest?.("button, [role='menu']")) return;
    dragging = true;
    lastMouse = orientation === "vertical" ? e.clientY : e.clientX;
    reached = value;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
  }

  function handlePointerMove(e: PointerEvent) {
    if (!dragging || disabled || !container) return;
    
    const currentMouse = orientation === "vertical" ? e.clientY : e.clientX;
    const deltaMouse = currentMouse - lastMouse;
    lastMouse = currentMouse;

    const travel = orientation === "vertical" ? height : width;
    const direction = orientation === "vertical" ? -1 : 1;

    // Each move at its own rate, added to where the drag had got to, so
    // shift pressed half way through slows what follows rather than
    // rescaling what has already happened -- which would throw the value
    // back towards where the drag began.
    const deltaVal = (deltaMouse / travel) * (max - min) * direction * (e.shiftKey ? FINE : 1);
    reached = clamp(reached + deltaVal);

    const nextVal = Math.round(reached / step) * step;
    
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
    const increment = step * 10 * (e.shiftKey ? FINE : 1);
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
  aria-label={name || label || "fader"}
  tabindex={disabled ? -1 : 0}
  onpointerdown={handlePointerDown}
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
  onpointercancel={handlePointerUp}
  ondblclick={ondblclick}
  onkeydown={handleKeyDown}
  onpointerenter={() => (showing = true)}
  onpointerleave={() => (showing = false)}
  onfocusin={() => (showing = true)}
  onfocusout={() => (showing = false)}
>
  {#if label && orientation === "vertical"}
    <span class="label">{label}{#if readout}&nbsp;<em class="mono">{readout}</em>{/if}</span>
  {/if}
  
  <SvgRenderer {renderState} width={width} height={height} />

  {#if label && orientation === "horizontal"}
    <span class="label">{label}{#if readout}&nbsp;<em class="mono">{readout}</em>{/if}</span>
  {/if}

  <!-- §29's AI hover. See `SvgKnob` for why the mark is drawn at all. -->
  {#if suggestion}
    <span class="proposed" data-testid="fader-suggested" aria-hidden="true"></span>
    {#if showing}
      <div class="suggestion" role="note" data-testid="fader-suggestion">
        <p>{suggestion.because}</p>
        <button
          type="button"
          disabled={disabled}
          onclick={() => onoption?.(suggestion.action)}
        >
          Do it{#if suggestion.to !== null}&nbsp;<em class="mono"
              >{suggestion.to.toFixed(2)}</em
            >{/if}
        </button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .fader-container {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    user-select: none;
    cursor: pointer;
    outline: none;
  }

  /* The same two pieces the knob draws, for the same reasons. */
  .proposed {
    position: absolute;
    top: 0;
    right: 0;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--assistant);
    pointer-events: none;
  }

  .suggestion {
    position: absolute;
    z-index: 45;
    top: 100%;
    left: 50%;
    transform: translateX(-50%);
    width: max-content;
    max-width: 13rem;
    padding: 0.3rem 0.4rem;
    border: 1px solid var(--assistant);
    border-radius: 5px;
    background: var(--panel, var(--surface));
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.45);
    text-align: left;
    cursor: default;
  }

  .suggestion p {
    margin: 0 0 0.25rem;
    font-size: 0.68rem;
    line-height: 1.35;
    color: var(--text);
  }

  .suggestion button {
    font: inherit;
    font-size: 0.68rem;
    padding: 0.15rem 0.4rem;
    border: 1px solid var(--line);
    border-radius: 3px;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .suggestion button:hover:not(:disabled) {
    border-color: var(--assistant);
    color: var(--assistant);
  }

  .suggestion button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .disabled {
    opacity: 0.5;
    pointer-events: none;
  }
  .label .mono {
    font-style: normal;
    color: var(--text);
  }
  .label {
    font-size: 0.75em;
    color: var(--text-dim);
    letter-spacing: 0.05em;
    /* One line: "Filter LP 60%" broken across two lines reads as two
       different facts, and the control below it moves when the text rewraps. */
    white-space: nowrap;
  }
  .fader-container:focus-visible {
    /* See SvgKnob: a child component's class cannot be styled from here. */
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 4px;
  }
</style>
