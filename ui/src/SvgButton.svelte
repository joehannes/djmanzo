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
>
  <svg viewBox="0 0 100 52" aria-hidden="true" focusable="false">
    <rect x="3" y="3" width="94" height="46" rx="10" fill={face} stroke={edge} stroke-width={stroke} />
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

  .pad {
    min-block-size: 3.1rem;
    font-size: 0.82em;
    font-weight: 700;
  }

  .tab {
    min-block-size: 1.65rem;
    font-size: 0.72em;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .control {
    min-block-size: 2rem;
  }

  .blank span {
    color: var(--text-dim);
  }
</style>
