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
  style="width: {fill ? '100%' : `${width}px`}; height: {height}px; {renderState.containerStyle}"
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
