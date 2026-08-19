<script module lang="ts">
  import type { KnobState, FaderState, PadState } from "../grammar";
</script>

{#snippet knob(state: KnobState)}
  {@const isPeak = state.context.phase === "peak" || state.context.phase === "heat"}
  {@const strokeWidth = isPeak ? 4 + state.context.energy_level * 3 : 6}
  {@const capStyle = isPeak ? "square" : "round"}
  {@const trackColor = "var(--panel-raised)"}
  {@const activeColor = isPeak ? "var(--danger)" : "var(--accent-2)"}
  
  <svg
    width={state.size}
    height={state.size}
    viewBox="0 0 100 100"
    style="touch-action: none; border-radius: 50%; width: 100%; height: 100%; display: block;"
  >
    <!-- Background Track -->
    <path
      d="M 20 80 A 45 45 0 1 1 80 80"
      fill="none"
      stroke={trackColor}
      stroke-width={strokeWidth}
      stroke-linecap={capStyle}
    />
    
    <!-- Active Value Track -->
    <path
      d="M 20 80 A 45 45 0 {state.angle > 45 ? 1 : 0} 1 {50 + 45 * Math.sin((state.angle * Math.PI) / 180)} {50 - 45 * Math.cos((state.angle * Math.PI) / 180)}"
      fill="none"
      stroke={activeColor}
      stroke-width={strokeWidth}
      stroke-linecap={capStyle}
      style="transition: stroke 0.2s var(--ease);"
    />

    <!-- Inner Dial Body -->
    <circle 
      cx="50" 
      cy="50" 
      r="32" 
      fill="var(--panel)" 
      stroke="var(--border)"
      stroke-width="2"
    />

    <!-- Indicator -->
    <g transform="rotate({state.angle} 50 50)">
      {#if isPeak}
        <polygon points="46,18 54,18 50,28" fill={activeColor} />
      {:else}
        <circle cx="50" cy="22" r="4" fill={activeColor} />
      {/if}
    </g>
  </svg>
{/snippet}

{#snippet fader(state: FaderState)}
  {@const isPeak = state.context.phase === "peak" || state.context.phase === "heat"}
  {@const trackWidth = isPeak ? 4 : 8}
  {@const thumbHeight = isPeak ? 4 : 12}
  {@const activeColor = isPeak ? "var(--danger)" : "var(--accent-2)"}
  {@const margin = 10}
  
  {@const thumbY = state.orientation === "vertical" 
    ? state.height - margin - (state.normalized * (state.height - margin * 2))
    : state.height / 2}
    
  {@const thumbX = state.orientation === "horizontal"
    ? margin + (state.normalized * (state.width - margin * 2))
    : state.width / 2}
    
  <svg
    width={state.width}
    height={state.height}
    style="touch-action: none; display: block;"
  >
    {#if state.orientation === "vertical"}
      <rect 
        x={state.width / 2 - trackWidth / 2} 
        y={margin} 
        width={trackWidth} 
        height={state.height - margin * 2} 
        fill="var(--panel-raised)" 
        rx={isPeak ? 0 : trackWidth / 2} 
      />
      <rect 
        x={state.width / 2 - trackWidth / 2} 
        y={thumbY} 
        width={trackWidth} 
        height={state.height - margin - thumbY} 
        fill={activeColor} 
        rx={isPeak ? 0 : trackWidth / 2} 
      />
    {:else}
      <rect 
        x={margin} 
        y={state.height / 2 - trackWidth / 2} 
        width={state.width - margin * 2} 
        height={trackWidth} 
        fill="var(--panel-raised)" 
        rx={isPeak ? 0 : trackWidth / 2} 
      />
      <line x1={state.width/2} y1={state.height/2 - 10} x2={state.width/2} y2={state.height/2 + 10} stroke="var(--text-dim)" stroke-width="1"/>
    {/if}

    {#if isPeak}
      <rect 
        x={thumbX - 12} 
        y={thumbY - thumbHeight / 2} 
        width="24" 
        height={thumbHeight} 
        fill="var(--text)" 
        stroke="var(--border)" 
        stroke-width="1"
      />
    {:else}
      <rect 
        x={thumbX - 10} 
        y={thumbY - thumbHeight / 2} 
        width="20" 
        height={thumbHeight} 
        fill="var(--text)" 
        rx={thumbHeight / 2}
      />
    {/if}
  </svg>
{/snippet}

{#snippet pad(state: PadState)}
  {@const isPeak = state.context.phase === "peak" || state.context.phase === "heat"}
  {@const rx = isPeak ? 2 : 12}
  {@const baseFill = state.active ? "var(--accent-2)" : "var(--panel)"}
  {@const pressScale = isPeak ? 0.95 : 0.98}
  {@const pressBrightness = isPeak ? 1.5 : 1.2}
  
  <svg
    width={state.width}
    height={state.height}
    style="display: block;"
  >
    <g 
      style:transform="scale({state.pressed ? pressScale : 1})" 
      style:transform-origin="center"
      style:filter={state.pressed ? `brightness(${pressBrightness})` : 'none'}
    >
      <rect 
        x="2" 
        y="2" 
        width={state.width - 4} 
        height={state.height - 4} 
        fill={baseFill} 
        {rx}
        stroke={state.active ? "var(--accent-2)" : "var(--border)"}
        stroke-width="2"
        style="transition: fill 0.2s var(--ease), rx 0.5s ease-out;"
      />
      {#if state.label}
        <text 
          x={state.width / 2} 
          y={state.height / 2 + 4} 
          text-anchor="middle" 
          fill={state.active ? "var(--on-accent)" : "var(--text)"}
          font-size="11px"
          font-family="monospace"
          font-weight="600"
          style="pointer-events: none;"
        >
          {state.label}
        </text>
      {/if}
    </g>
  </svg>
{/snippet}
