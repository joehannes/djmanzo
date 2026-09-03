<script lang="ts">
  import type { SvgRenderState } from "./themes/engine";

  /**
   * Paints the paths a theme produced.
   *
   * Width and height are separate rather than one `size`: a pad is 60×40 and
   * squeezing it through a single dimension drew it in a square, which is why
   * the transport pads came out the wrong shape. The viewBox stays 0–100 in
   * both axes, so geometry is written in one coordinate space and stretched to
   * whatever the caller asked for.
   *
   * **Both are multiplied by `--density`**, which is the one number a layout's
   * density moves. Without that these were a fixed count of device pixels and
   * the setting simply did not reach them -- and since the faders and knobs are
   * the second largest block on a deck, driving density to its floor of 0.8
   * moved a deck from 878 px to 810. Sixty-eight against the two hundred and
   * eighty that would put the crossfader back on screen, which is why the
   * interface "getting denser" never bought the room it looked like it should.
   * Measured in `docs/GUI-OVERHAUL.md` section 20a.
   */
  let {
    renderState,
    width,
    height,
    /**
     * Take the parent's whole width instead of `width` pixels.
     *
     * A pad in a four-column grid is stretched to its cell, and a fixed-width
     * drawing inside it leaves the label centred over the cell rather than over
     * the pad — which is how "CUE" ended up sitting next to its button.
     */
    fill = false,
  }: {
    renderState: SvgRenderState;
    width: number;
    height: number;
    fill?: boolean;
  } = $props();
</script>

<div
  class="renderer"
  style="width: {fill
    ? '100%'
    : `calc(${width}px * var(--density, 1))`}; height: calc({height}px * var(--density, 1)); {renderState.containerStyle}"
>
  <svg
    viewBox="0 0 100 100"
    preserveAspectRatio="none"
    style="touch-action: none; width: 100%; height: 100%; display: block;"
  >
    {#each renderState.paths as path, index (index)}
      <path
        d={path.d}
        fill={path.fill}
        stroke={path.stroke}
        stroke-width={path.strokeWidth}
        stroke-linecap="round"
        vector-effect="non-scaling-stroke"
        style="--sw: {path.strokeWidth}px; {path.style ?? ''}"
        transform={path.transform}
      />
    {/each}
  </svg>
</div>
