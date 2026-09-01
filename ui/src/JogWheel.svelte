<script lang="ts">
  import { dispatch } from "./api";

  let {
    deckNumber,
    touched = false,
    mode = "vinyl",
    bend = 0,
    enabled = true,
  }: {
    deckNumber: number;
    touched?: boolean;
    mode?: string;
    bend?: number;
    enabled?: boolean;
  } = $props();

  let element = $state<HTMLDivElement | null>(null);
  let dragging = $state(false);
  let lastAngle = 0;

  /**
   * Where the pointer is, in radians about the centre of the wheel.
   *
   * Rotation rather than distance dragged: a round control should answer to
   * being turned, so the movement is the same whether a DJ works near the hub
   * or out at the rim -- which is how a real platter behaves, and what makes a
   * back-and-forth scratch land in the same place each time. A controller
   * sends revolutions directly and never comes through here.
   */
  function angleOf(event: PointerEvent): number | null {
    const box = element?.getBoundingClientRect();
    if (!box) return null;
    return Math.atan2(
      event.clientY - (box.top + box.height / 2),
      event.clientX - (box.left + box.width / 2),
    );
  }

  function grab(event: PointerEvent) {
    if (!enabled) return;
    const angle = angleOf(event);
    if (angle === null) return;
    lastAngle = angle;
    dragging = true;
    element?.setPointerCapture(event.pointerId);
    // The top of the platter is the record; the rim is not. Holding the rim
    // bends, holding the middle scratches -- the same as the hardware.
    if (onTop(event)) dispatch(`deck ${deckNumber} jog_touch`);
  }

  /** Whether the pointer is on the record rather than the rim. */
  function onTop(event: PointerEvent): boolean {
    const box = element?.getBoundingClientRect();
    if (!box) return false;
    const dx = event.clientX - (box.left + box.width / 2);
    const dy = event.clientY - (box.top + box.height / 2);
    return Math.hypot(dx, dy) < (box.width / 2) * 0.7;
  }

  function drag(event: PointerEvent) {
    if (!dragging) return;
    const angle = angleOf(event);
    if (angle === null) return;

    // Shortest way round, so crossing the top of the wheel is a small
    // movement rather than a full turn backwards.
    let delta = angle - lastAngle;
    if (delta > Math.PI) delta -= 2 * Math.PI;
    if (delta < -Math.PI) delta += 2 * Math.PI;
    lastAngle = angle;

    const turns = delta / (2 * Math.PI);
    if (turns !== 0) dispatch(`deck ${deckNumber} jog ${turns.toFixed(5)}`);
  }

  function release(event: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    element?.releasePointerCapture(event.pointerId);
    dispatch(`deck ${deckNumber} jog_release`);
  }

  function toggleMode() {
    const next = mode === "vinyl" ? "cdj" : "vinyl";
    dispatch(`deck ${deckNumber} jog_mode ${next}`);
  }

  // Turning the wheel with the keyboard, for anyone who cannot drag one.
  function nudge(event: KeyboardEvent) {
    const turns =
      event.key === "ArrowRight" ? 0.02 : event.key === "ArrowLeft" ? -0.02 : 0;
    if (turns === 0) return;
    event.preventDefault();
    dispatch(`deck ${deckNumber} jog ${turns}`);
  }

  let label = $derived(
    `Deck ${deckNumber} platter, ${mode} mode` +
      (touched ? ", touched" : "") +
      (bend !== 0 ? `, bending ${(bend * 100).toFixed(1)} percent` : ""),
  );
</script>

<div class="jog">
  <!--
    A div rather than a button: a button that is dragged in circles fights the
    browser's own click handling, and the keyboard path is explicit below.
  -->
  <div
    bind:this={element}
    class="platter"
    class:touched
    class:disabled={!enabled}
    style="--bend: {Math.max(-1, Math.min(1, bend * 5))}"
    role="slider"
    aria-label={label}
    aria-valuemin={-1}
    aria-valuemax={1}
    aria-valuenow={Number(bend.toFixed(3))}
    tabindex={enabled ? 0 : -1}
    onpointerdown={grab}
    onpointermove={drag}
    onpointerup={release}
    onpointercancel={release}
    onkeydown={nudge}
  >
    <div class="marker"></div>
    <div class="hub">{mode === "cdj" ? "CDJ" : "VINYL"}</div>
  </div>

  <button
    class="mode"
    type="button"
    disabled={!enabled}
    onclick={toggleMode}
    title={mode === "vinyl"
      ? "Vinyl: a hand on the record stops it"
      : "CDJ: the platter only bends the tempo"}
  >
    {mode === "vinyl" ? "Vinyl" : "CDJ"}
  </button>
</div>

<style>
  .jog {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.35rem;
  }

  .platter {
    /* Sized by whoever places it: a deck's channel strip wants a nudge
       target, and a detached window wants a platter. The default is the
       platter, because a component with no context should be the larger,
       more legible thing. */
    --size: var(--jog-size, 7.5rem);
    position: relative;
    width: var(--size);
    height: var(--size);
    border-radius: 50%;
    border: 1px solid var(--edge, rgba(255, 255, 255, 0.12));
    background:
      radial-gradient(
        circle at 50% 50%,
        var(--panel-raised, #1a1d1a) 0 32%,
        transparent 32%
      ),
      repeating-radial-gradient(
        circle at 50% 50%,
        rgba(255, 255, 255, 0.045) 0 2px,
        transparent 2px 4px
      ),
      var(--panel, #101210);
    cursor: grab;
    touch-action: none;
    user-select: none;
    /* The whole wheel leans the way the bend is going, which is the fastest
       way to see that a nudge is being applied without reading a number. */
    transform: rotate(calc(var(--bend, 0) * 6deg));
    transition: transform 90ms linear, box-shadow 140ms ease;
  }
  .platter:active {
    cursor: grabbing;
  }
  .platter:focus-visible {
    outline: 2px solid var(--accent, #4ade80);
    outline-offset: 3px;
  }
  .platter.touched {
    box-shadow: 0 0 0 2px var(--accent, #4ade80) inset;
  }
  .platter.disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .marker {
    position: absolute;
    left: 50%;
    top: 6%;
    width: 2px;
    height: 16%;
    margin-left: -1px;
    border-radius: 1px;
    background: var(--accent, #4ade80);
  }

  .hub {
    position: absolute;
    inset: 34%;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    font-size: 0.55rem;
    letter-spacing: 0.08em;
    color: var(--text-dim, rgba(255, 255, 255, 0.55));
    pointer-events: none;
  }

  .mode {
    font-size: 0.7rem;
    padding: 0.15rem 0.6rem;
    border-radius: var(--radius, 6px);
    border: 1px solid var(--edge, rgba(255, 255, 255, 0.12));
    background: var(--panel-raised, #1a1d1a);
    color: var(--text, #e6e6e6);
    cursor: pointer;
  }
  .mode:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
