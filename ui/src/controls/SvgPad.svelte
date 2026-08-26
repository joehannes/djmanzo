<script lang="ts">
  import type { PadState } from "./grammar";
  import { theme as globalTheme } from "../theme.svelte";
  import { executeThemePipeline } from "./themes/engine";
  import SvgRenderer from "./SvgRenderer.svelte";
  import { HOLD_MS, holdProgress } from "../care";

  interface Props {
    active?: boolean;
    disabled?: boolean;
    /**
     * Require a deliberate hold before firing.
     *
     * For the controls that cannot be undone by pressing them again -- ejecting
     * a playing deck, loading over one. Set from the occasion: at peak time a
     * mis-click is heard by everyone, and alone at home it costs nothing, so
     * the same pad is a press in one setting and a hold in the other.
     *
     * The pad says "hold" when it is on. A control that needed holding and did
     * not say so would read as broken the first time it was pressed, and the
     * first time is in front of people.
     */
    hold?: boolean;
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
    hold = false,
    label,
    width = 60,
    height = 40,
    onclick,
    onpointerdown,
    onpointerup,
    onpointerleave
  }: Props = $props();

  let pressed = $state(false);
  /** How far through a hold, 0..=1. Zero whenever nothing is being held. */
  let holdFill = $state(0);
  let holdTimer: ReturnType<typeof setTimeout> | null = null;
  let holdFrame = 0;

  function endHold() {
    holdFill = 0;
    if (holdTimer) clearTimeout(holdTimer);
    holdTimer = null;
    cancelAnimationFrame(holdFrame);
  }

  function beginHold() {
    const started = performance.now();
    // The bar is driven by the same clock as the timer, so what is seen and
    // what fires cannot disagree. The arithmetic itself lives in `care`, where
    // it is tested -- including the clamps, which matter here because a DJ's
    // laptop is suspended between every gig and `performance.now` is not
    // guaranteed monotonic across that.
    const step = () => {
      holdFill = holdProgress(started, performance.now());
      if (holdTimer) holdFrame = requestAnimationFrame(step);
    };
    holdTimer = setTimeout(() => {
      endHold();
      pressed = false;
      if (onclick) onclick();
    }, HOLD_MS);
    holdFrame = requestAnimationFrame(step);
  }

  /*
    Cancel an unfinished hold if the pad goes away underneath it.

    Pointer-up and pointer-leave cover a hand letting go, but not the panel
    being closed, the deck count being reduced or a layout being swapped
    mid-press. Without this the timer would still fire and eject a deck whose
    pad no longer exists -- which is the one outcome this whole mechanism is
    here to prevent.
  */
  $effect(() => endHold);

  let shape: PadState = $derived({
    active, pressed, disabled, width, height, label
  });

  let renderState = $derived(executeThemePipeline(globalTheme.activePackage, shape));
  
  function handlePointerDown(e: PointerEvent) {
    if (disabled) return;
    pressed = true;
    if (hold) beginHold();
    if (onpointerdown) onpointerdown(e);
  }

  function handlePointerUp(e: PointerEvent) {
    if (disabled) return;
    pressed = false;
    // Letting go early cancels. The `onclick` the container carries is
    // suppressed for a holding pad -- see the markup -- so a short press does
    // nothing at all rather than firing anyway.
    if (hold) endHold();
    if (onpointerup) onpointerup(e);
  }

  function handlePointerLeave(e: PointerEvent) {
    if (disabled) return;
    pressed = false;
    if (hold) endHold();
    if (onpointerleave) onpointerleave(e);
  }
  
  function handleKeyDown(e: KeyboardEvent) {
    if (disabled) return;
    if (e.key === "Enter" || e.key === " ") {
      pressed = true;
      // A keyboard press is as deliberate as a hold: reaching for a key is not
      // something a sleeve does. Requiring a held key as well would make the
      // keyboard worse than the mouse at exactly the moment it should be
      // better -- one hand, no aiming.
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
  onclick={hold ? undefined : onclick}
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
  {#if holdFill > 0}
    <span class="hold-fill" style="width: {holdFill * 100}%"></span>
  {/if}
  {#if label}
    <span class="label">{label}{#if hold}<em class="hold-hint">hold</em>{/if}</span>
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

  /*
    Fills left to right as a hold completes. Behind the label, so the text
    stays readable the whole way across -- a DJ watching this needs to know
    both that it is filling and what it is about to do.
  */
  .hold-fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--warn, #d97706);
    opacity: 0.3;
    pointer-events: none;
    border-radius: inherit;
  }

  /* Beside the label rather than replacing it. */
  .hold-hint {
    font-style: normal;
    opacity: 0.6;
    font-size: 0.72em;
    margin-left: 0.28em;
  }
</style>
