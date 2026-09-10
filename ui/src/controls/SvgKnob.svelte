<script lang="ts">
  import type { KnobState } from "./grammar";
  import { theme as globalTheme } from "../theme.svelte";
  import { executeThemePipeline } from "./themes/engine";
  import SvgRenderer from "./SvgRenderer.svelte";

  interface Props {
    value: number;
    min: number;
    max: number;
    step?: number;
    label?: string;
    /** The value in words, shown under the label. A knob with no number
     *  beside it is a knob you cannot set to anything in particular. */
    readout?: string;
    oninput?: (value: number) => void;
    ondblclick?: () => void;
    /**
     * §29's level three: `[label, action]` per entry, from Rust.
     *
     * Right-click, or press and hold — the second because a booth has
     * trackpads and touchscreens, and a control reachable only by right-click
     * is a control half the room cannot reach.
     *
     * Absent means the control has no contextual options, and then neither
     * gesture does anything: an empty menu that opens is worse than no menu.
     */
    options?: [string, string][];
    onoption?: (action: string) => void;
    disabled?: boolean;
    size?: number;
    // Injectable theme, falls back to BaseTheme
    theme?: any; 
  }

  let {
    value,
    min,
    max,
    step = 0.01,
    label,
    readout,
    oninput,
    ondblclick,
    options,
    onoption,
    disabled = false,
    size = 48
  }: Props = $props();

  let dragging = $state(false);
  let startY = $state(0);
  let startVal = $state(0);

  /**
   * §29's level two: a shift-drag is a quarter of a drag.
   *
   * Read on every move rather than latched at the press, so a DJ can reach for
   * shift *during* a drag — which is what actually happens: you drag, you
   * overshoot, you hold shift and creep back.
   */
  const FINE = 0.25;

  /**
   * §29's level three, and the press-and-hold that opens it.
   *
   * Half a second: long enough that a drag never opens it by accident, short
   * enough to be a deliberate gesture rather than a wait.
   */
  const HOLD_MS = 500;

  let menu = $state(false);
  let menuEl = $state<HTMLElement | null>(null);
  let held: ReturnType<typeof setTimeout> | undefined;
  /** Whether the press has lasted long enough to count as a hold. */
  let longEnough = $state(false);

  function openMenu() {
    if (disabled || !options?.length) return;
    menu = true;
  }

  function cancelHold() {
    clearTimeout(held);
    held = undefined;
    longEnough = false;
  }

  let normalized = $derived((value - min) / (max - min));
  let angle = $derived(-135 + normalized * 270);
  
  let shape: KnobState = $derived({
    value, min, max, normalized, angle, dragging, disabled, size, label
  });

  let renderState = $derived(executeThemePipeline(globalTheme.activePackage, shape));

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
    cancelHold();
    // Marked here and opened on release rather than opened here.
    //
    // Opening mid-press puts the menu's scrim under the finger, so the release
    // that completed the hold lands on the scrim and closes what it just
    // opened — the menu appeared and vanished in the same gesture. Waiting for
    // the release is also how a long press behaves everywhere else.
    held = setTimeout(() => {
      longEnough = true;
    }, HOLD_MS);
  }

  function handlePointerMove(e: PointerEvent) {
    if (!dragging || disabled) return;
    const deltaY = startY - e.clientY;
    // Any movement is a drag, so it is not a hold.
    if (Math.abs(deltaY) > 2) cancelHold();
    const range = max - min;
    const deltaVal = (deltaY / 100) * range * (e.shiftKey ? FINE : 1);

    let nextVal = clamp(startVal + deltaVal);
    nextVal = Math.round(nextVal / step) * step;
    
    if (nextVal !== value && oninput) {
      oninput(nextVal);
    }
  }

  function handlePointerUp(e: PointerEvent) {
    dragging = false;
    const wasHeld = longEnough;
    cancelHold();
    // Opened on the next tick, not in this handler.
    //
    // Setting it here — during the pointerup that releases the pointer
    // capture — was reliably lost: the flag went true, the handler saw it
    // true, and the template never rendered the menu. Deferring by a tick puts
    // the state change outside that release and it holds. Found by driving it;
    // a type-check cannot see a state write that does not survive.
    if (wasHeld) setTimeout(openMenu, 0);
    const el = e.currentTarget as HTMLElement;
    if (el.hasPointerCapture(e.pointerId)) {
      el.releasePointerCapture(e.pointerId);
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (disabled) return;
    // Escape closes the menu before it does anything else, so a DJ who opened
    // one by accident is one key from where they were.
    if (e.key === "Escape" && menu) {
      menu = false;
      return;
    }
    // The keyboard reaches level three too (§33): a control a mouse can open
    // and a keyboard cannot is a control half the interface cannot use.
    if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
      openMenu();
      return;
    }
    let nextVal = value;
    // Shift is the fine step here too, for the same reason it is on the drag.
    const by = e.shiftKey ? step * FINE : step;
    if (e.key === "ArrowUp" || e.key === "ArrowRight") nextVal = clamp(value + by);
    if (e.key === "ArrowDown" || e.key === "ArrowLeft") nextVal = clamp(value - by);
    if (nextVal !== value && oninput) {
      oninput(nextVal);
    }
  }

  /**
   * Focus the menu when it opens.
   *
   * Otherwise Escape goes to the body and the menu a DJ opened by accident has
   * to be dismissed by aiming at something else — and a menu a keyboard cannot
   * reach is one half the interface cannot use (§33).
   */
  $effect(() => {
    if (menu) menuEl?.focus();
  });

  function pick(action: string) {
    menu = false;
    onoption?.(action);
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
  oncontextmenu={(e) => {
    if (!options?.length) return;
    e.preventDefault();
    openMenu();
  }}
  onkeydown={handleKeyDown}
  style="width: {size}px;"
>
  {#if label}
    <span class="label">{label}{#if readout}&nbsp;<em class="mono">{readout}</em>{/if}</span>
  {/if}
  
  <SvgRenderer {renderState} width={size} height={size} />

  <!--
    §29's level three. Drawn only when it is open and only when there is
    something in it — an empty menu that opens is worse than no menu, because a
    DJ learns the gesture and then learns it does nothing.

    Every entry sends an action, not a number: what the menu does is what a
    mapping does and what a MIDI CC does. See `dj_app::handle`.
  -->
  {#if menu && options?.length}
    <div
      class="options"
      role="menu"
      data-testid="handle-options"
      tabindex="-1"
      bind:this={menuEl}
      onkeydown={(e) => {
        if (e.key === "Escape") {
          menu = false;
          e.stopPropagation();
        }
      }}
    >
      {#each options as [label, action] (action)}
        <button role="menuitem" onclick={() => pick(action)}>{label}</button>
      {/each}
    </div>
    <!--
      Anything outside closes it. A menu a DJ has to aim at to dismiss is one
      more thing to aim at in the dark.
    -->
    <div
      class="scrim"
      role="presentation"
      onpointerdown={() => (menu = false)}
    ></div>
  {/if}
</div>

<style>
  .knob-container {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    user-select: none;
    cursor: ns-resize;
    outline: none;
  }
  /* §29's level three, over the control it belongs to rather than beside it:
     a menu that pushed the deck's layout around would move every other control
     out from under the DJ's hand. */
  .options {
    position: absolute;
    z-index: 40;
    top: 100%;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    min-width: 5.5rem;
    padding: 0.15rem;
    border: 1px solid var(--line);
    border-radius: 5px;
    background: var(--panel, var(--surface));
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.45);
  }

  .options button {
    font: inherit;
    font-size: 0.72rem;
    text-align: left;
    padding: 0.22rem 0.45rem;
    border: 0;
    border-radius: 3px;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .options button:hover,
  .options button:focus-visible {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .scrim {
    position: fixed;
    inset: 0;
    z-index: 39;
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
  .knob-container:focus-visible {
    /* On the container, not on `.renderer`: that class belongs to
       SvgRenderer, and Svelte scopes a component's styles to its own markup,
       so the rule never matched and these controls showed no focus at all. */
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 4px;
  }
</style>
