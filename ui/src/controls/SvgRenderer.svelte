<script lang="ts">
  import type { SvgRenderState } from "./themes/engine";

  let { renderState, size }: { renderState: SvgRenderState; size: number } = $props();
</script>

<div class="renderer" style="width: {size}px; height: {size}px; {renderState.containerStyle}">
  <svg
    width={size}
    height={size}
    viewBox="0 0 100 100"
    style="touch-action: none; width: 100%; height: 100%; display: block;"
  >
    {#each renderState.paths as path}
      <path 
        d={path.d} 
        fill={path.fill} 
        stroke={path.stroke} 
        stroke-width={path.strokeWidth}
        style={path.style}
        transform={path.transform}
      />
    {/each}
  </svg>
</div>
