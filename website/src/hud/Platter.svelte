<!--
  The minimap is a platter: the island drawn on a spinning record, the
  window you are looking through as the stylus. Tap anywhere on it to go
  there; the buttons zoom, and the centre label takes you home.
-->
<script lang="ts">
  import { camera } from "../map/camera.svelte";
  import { PLACES } from "../lib/places";
  import { profile } from "../lib/profile.svelte";
  import { GENRES } from "../lib/genres";
  import { isletAt } from "../map/islets";

  const R = 3600; // world units across the platter's radius
  const size = 132;
  const k = size / 2 / R;

  function go(e: MouseEvent) {
    const el = e.currentTarget as HTMLElement;
    const rect = el.getBoundingClientRect();
    const x = (e.clientX - rect.left - rect.width / 2) / k;
    const y = (e.clientY - rect.top - rect.height / 2) / k;
    void camera.flyTo(x, y, camera.z, { token: false });
  }
  let innerWidth = $state(1200);
  let innerHeight = $state(800);
  const view = $derived({
    w: Math.min(size, (innerWidth / camera.z) * k),
    h: Math.min(size, (innerHeight / camera.z) * k),
    x: size / 2 + camera.x * k,
    y: size / 2 + camera.y * k,
  });
</script>

<svelte:window bind:innerWidth bind:innerHeight />

<div class="platter" aria-label="Minimap">
  <button class="disc" onclick={go} aria-label="Go to a point on the minimap">
    <svg viewBox="0 0 {size} {size}" width={size} height={size} aria-hidden="true">
      <circle cx={size / 2} cy={size / 2} r={size / 2 - 1} fill="#141416" />
      {#each [0.92, 0.8, 0.68, 0.56] as r (r)}
        <circle cx={size / 2} cy={size / 2} r={(size / 2) * r} fill="none" stroke="#2a2a2e" stroke-width="1" />
      {/each}
      <ellipse cx={size / 2 - 20 * k} cy={size / 2 + 40 * k} rx={1260 * k} ry={900 * k} fill="var(--primary)" opacity="0.8" />
      {#each GENRES as g (g.id)}
        {@const at = isletAt(g, profile.p.genres)}
        <circle cx={size / 2 + at.x * k} cy={size / 2 + at.y * k} r={at.mine ? 4 : 2.5} fill={g.palette.primary} />
      {/each}
      {#each PLACES.filter((p) => !p.hidden) as p (p.id)}
        <circle cx={size / 2 + p.x * k} cy={size / 2 + p.y * k} r="1.6" fill="#111" />
      {/each}
      <rect x={view.x - view.w / 2} y={view.y - view.h / 2} width={view.w} height={view.h} fill="none" stroke="var(--ink)" stroke-width="1.5" rx="2" />
    </svg>
  </button>
  <button class="label" onclick={() => camera.flyTo(0, 160, 0.55)} aria-label="Back to the middle of the island">⌂</button>
  <div class="zoom">
    <button onclick={() => (camera.z = camera.clampZ(camera.z * 1.3))} aria-label="Zoom in">+</button>
    <button onclick={() => (camera.z = camera.clampZ(camera.z / 1.3))} aria-label="Zoom out">−</button>
  </div>
</div>

<style>
  .platter {
    position: fixed;
    right: 14px;
    bottom: 14px;
    width: 132px;
    height: 132px;
    z-index: 14;
  }
  .disc {
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: none;
    cursor: crosshair;
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.5), 0 0 0 4px #2b2c31;
  }
  .label {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 30px;
    height: 30px;
    border-radius: 50%;
    border: 0;
    background: var(--secondary);
    color: var(--on-secondary);
    font-weight: 800;
    cursor: pointer;
  }
  .zoom {
    position: absolute;
    left: -46px;
    bottom: 4px;
    display: grid;
    gap: 6px;
  }
  .zoom button {
    width: 38px;
    height: 38px;
    border-radius: 10px;
    border: 2px solid var(--line);
    background: color-mix(in srgb, var(--surface) 90%, transparent);
    font-size: 20px;
    font-weight: 800;
    cursor: pointer;
  }
  @media (max-width: 640px) {
    .platter {
      transform: scale(0.72);
      transform-origin: 100% 100%;
      bottom: 82px;
      right: 8px;
    }
  }
</style>
