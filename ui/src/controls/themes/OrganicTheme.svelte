<script module lang="ts">
  import type { KnobState, FaderState, PadState } from "../grammar";

  // Helper to generate a polygon (blob) based on vertices
  function generateBlobPath(cx: number, cy: number, r: number, points: number, angleOffset: number, energy: number) {
    let path = "";
    for (let i = 0; i < points; i++) {
      // Add some noise based on energy
      const noise = (Math.random() - 0.5) * (energy * 10);
      const radius = r + noise;
      const angle = (Math.PI * 2 * i) / points + (angleOffset * Math.PI) / 180;
      const x = cx + radius * Math.cos(angle);
      const y = cy + radius * Math.sin(angle);
      path += i === 0 ? `M ${x} ${y} ` : `L ${x} ${y} `;
    }
    return path + "Z";
  }
</script>

{#snippet knob(state: KnobState)}
  <!-- Organic Theme Knob: Shape shifts from smooth circle to jagged polygon based on energy -->
  {@const points = Math.max(3, Math.floor(30 - state.context.energy_level * 25))}
  {@const strokeColor = `hsl(${220 - state.context.energy_level * 180}, 80%, 60%)`}
  {@const blobRadius = 30 + (state.context.audio.momentary_loudness * 10)}
  {@const innerBlob = generateBlobPath(50, 50, blobRadius, points, state.angle, state.context.energy_level)}
  
  <svg
    width={state.size}
    height={state.size}
    viewBox="0 0 100 100"
    style="touch-action: none; width: 100%; height: 100%; display: block;"
  >
    <!-- Outer energy ring -->
    <circle 
      cx="50" 
      cy="50" 
      r="45" 
      fill="none" 
      stroke="var(--panel-raised)"
      stroke-width="2"
      stroke-dasharray="{state.normalized * 280}, 280"
      transform="rotate(-90 50 50)"
      style="transition: stroke-dasharray 0.2s ease-out;"
    />
    
    <!-- Morphing Inner Blob -->
    <path 
      d={innerBlob}
      fill="var(--panel)"
      stroke={strokeColor}
      stroke-width="3"
      style="transition: d 0.1s linear, stroke 0.3s;"
    />

    <!-- Indicator Line -->
    <line 
      x1="50" 
      y1="50" 
      x2={50 + 40 * Math.sin((state.angle * Math.PI) / 180)}
      y2={50 - 40 * Math.cos((state.angle * Math.PI) / 180)}
      stroke={strokeColor}
      stroke-width="4"
      stroke-linecap="round"
    />
  </svg>
{/snippet}

{#snippet fader(state: FaderState)}
  <!-- Organic Theme Fader: Looks like a fluid channel -->
  {@const strokeColor = `hsl(${220 - state.context.energy_level * 180}, 80%, 60%)`}
  {@const margin = 12}
  
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
    <!-- Fluid track -->
    <rect 
      x={state.orientation === "vertical" ? state.width / 2 - 10 : margin}
      y={state.orientation === "vertical" ? margin : state.height / 2 - 10}
      width={state.orientation === "vertical" ? 20 : state.width - margin * 2}
      height={state.orientation === "vertical" ? state.height - margin * 2 : 20}
      rx="10"
      fill="var(--panel-raised)"
    />

    <!-- The actual thumb is a water-like droplet -->
    <circle 
      cx={thumbX} 
      cy={thumbY} 
      r={14 + state.context.audio.momentary_loudness * 4} 
      fill="var(--panel)" 
      stroke={strokeColor} 
      stroke-width="3"
      style="transition: r 0.1s linear;"
    />
    <circle 
      cx={thumbX} 
      cy={thumbY} 
      r="4" 
      fill={strokeColor} 
    />
  </svg>
{/snippet}

{#snippet pad(state: PadState)}
  <!-- Organic Theme Pad: A soft, glowing, breathing membrane -->
  {@const strokeColor = state.active ? `hsl(${220 - state.context.energy_level * 180}, 80%, 60%)` : "var(--panel-raised)"}
  {@const loudnessScale = 1 + (state.context.audio.momentary_loudness * 0.15)}
  {@const scale = state.pressed ? 0.9 : loudnessScale}
  {@const rx = state.context.energy_level > 0.8 ? 5 : 20}
  
  <svg
    width={state.width}
    height={state.height}
    style="display: block;"
  >
    <g 
      style:transform="scale({scale})" 
      style:transform-origin="center"
      style:transition="transform 0.1s var(--ease)"
    >
      <!-- Fluid base -->
      <rect 
        x="4" 
        y="4" 
        width={state.width - 8} 
        height={state.height - 8} 
        fill={state.active ? "var(--panel)" : "var(--panel-raised)"} 
        {rx}
        stroke={strokeColor}
        stroke-width="3"
        style="transition: all 0.3s ease-out;"
      />
      {#if state.label}
        <text 
          x={state.width / 2} 
          y={state.height / 2 + 4} 
          text-anchor="middle" 
          fill={state.active ? strokeColor : "var(--text-dim)"}
          font-size="12px"
          font-family="sans-serif"
          font-weight="bold"
          style="pointer-events: none; transition: fill 0.3s;"
        >
          {state.label}
        </text>
      {/if}
    </g>
  </svg>
{/snippet}
