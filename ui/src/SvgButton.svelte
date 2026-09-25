<script lang="ts">
  /**
   * SVG-faced button.
   *
   * The button element remains for focus, keyboard activation and disabled
   * semantics; the visible control is an SVG plate. This is the compromise that
   * keeps the GUI SVG-driven without rebuilding browser accessibility badly.
   */
  let {
    label,
    active = false,
    lit = false,
    held = false,
    blank = false,
    disabled = false,
    title,
    kind = "control",
    onclick,
    onpointerdown,
    onpointerup,
    onpointercancel,
    oncontextmenu,
  }: {
    label: string;
    active?: boolean;
    lit?: boolean;
    held?: boolean;
    blank?: boolean;
    disabled?: boolean;
    title?: string;
    kind?: "control" | "pad" | "tab";
    onclick?: (event: MouseEvent) => void;
    onpointerdown?: (event: PointerEvent) => void;
    onpointerup?: (event: PointerEvent) => void;
    onpointercancel?: (event: PointerEvent) => void;
    oncontextmenu?: (event: MouseEvent) => void;
  } = $props();

  const hot = $derived(active || lit || held);
  const face = $derived(
    blank
      ? "transparent"
      : hot
        ? "var(--accent)"
        : kind === "tab"
          ? "var(--panel)"
          : "var(--panel-raised, var(--panel))",
  );
  /**
   * A tab's plate is drawn at the tab's own size.
   *
   * Every kind used to draw a 100x52 plate and let the SVG's aspect ratio
   * decide the height, so a tab was as tall as its width made it: 57 px on a
   * deck with five pages, taller than the pads under it, and 20 on one with
   * eight (§120, *"the rest of the faders/knobs/controls seem to take a lot of
   * space"*). A tab is a label and is as tall as one now; its plate is
   * measured rather than stretched, so the corners stay round. A pad keeps
   * its proportions -- its height comes from the pad grid.
   */
  let width = $state(0);
  let height = $state(0);
  const plate = $derived(
    kind === "tab" && width > 0 && height > 0 ? { w: width, h: height, r: 6 } : { w: 100, h: 52, r: 10 },
  );
  const edge = $derived(hot ? "var(--accent)" : "var(--edge)");
  const text = $derived(hot ? "var(--on-accent)" : "var(--text)");
  const stroke = $derived(held ? 3 : 1.5);
</script>

<button
  class="svg-button {kind}"
  class:active={active}
  class:lit={lit}
  class:held={held}
  class:blank={blank}
  {disabled}
  {title}
  style={`color: ${text}`}
  aria-pressed={active || lit || held ? "true" : undefined}
  {onclick}
  {onpointerdown}
  {onpointerup}
  {onpointercancel}
  {oncontextmenu}
  bind:clientWidth={width}
  bind:clientHeight={height}
>
  <svg viewBox="0 0 {plate.w} {plate.h}" aria-hidden="true" focusable="false">
    <rect x="3" y="3" width={plate.w - 6} height={plate.h - 6} rx={plate.r} fill={face} stroke={edge} stroke-width={stroke} />
    {#if kind === "pad" && !blank}
      <path d="M 14 38 C 31 28, 45 45, 62 35 S 84 29, 90 37" fill="none" stroke="currentColor" stroke-opacity="0.3" stroke-width="4" stroke-linecap="round" />
    {/if}
  </svg>
  <span>{label}</span>
</button>

<style>
  .svg-button {
    position: relative;
    display: grid;
    place-items: center;
    min-inline-size: 0;
    border: 0;
    padding: 0;
    background: transparent;
    font: inherit;
    cursor: pointer;
  }

  .svg-button:disabled {
    cursor: default;
    opacity: 0.55;
  }

  .svg-button:focus-visible {
    outline: 2px solid var(--accent-2, var(--accent));
    outline-offset: 2px;
  }

  svg,
  span {
    grid-area: 1 / 1;
  }

  svg {
    inline-size: 100%;
    block-size: 100%;
    color: currentColor;
    filter: drop-shadow(0 1px 0 rgba(255, 255, 255, 0.04));
  }

  span {
    z-index: 1;
    padding: 0 0.35rem;
    text-align: center;
    line-height: 1.05;
    pointer-events: none;
  }

  /*
    The pad fills the cell it is given.

    `min-block-size: 3.1rem` was here, and it was a floor the pad never reached
    -- its height came from its own 100x52 aspect ratio stretched to a grid cell
    as wide as the deck, which made a two-row pad zone 155 px on a two-deck
    layout and less on a four-deck one. The grid sets the row height now (see
    `Pads.svelte`), so this stops competing with it.
  */
  .pad {
    block-size: 100%;
    min-block-size: 2.2rem;
    font-size: 0.82em;
    font-weight: 700;
  }

  .pad svg {
    block-size: 100%;
    inline-size: 100%;
  }

  .tab {
    block-size: 1.8rem;
    font-size: 0.72em;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  /* Laid over the tab rather than in it, so the plate follows the tab's
     size and never sets it: a plate drawn from a measured size that was also
     its intrinsic size would keep whatever width it first happened to get. */
  .tab svg {
    position: absolute;
    inset: 0;
  }

  .control {
    min-block-size: 2rem;
  }

  .blank span {
    color: var(--text-dim);
  }
</style>
